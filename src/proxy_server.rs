use std::os::fd::AsRawFd;
use std::net::{Shutdown, SocketAddr};
use std::rc::Rc;
use std::time::Duration;

use tokio_uring::{self, buf::BoundedBuf};
use tokio::time::timeout;
use log::{trace, debug, info, error};

use crate::socks::{Socks4, Socks4Phase, SOCKS4_VERSION};
use crate::bypass::BypassOptions;

const BUF_SIZE: usize = 16384;
pub const BUF_SIZE_STR: &str = concat!(16384);

#[derive(Clone, Debug)]
pub struct ProxyServer {
  socks_version: u8,
  pub server_addr: SocketAddr,
  msg_buf_size: usize,
  pub bypass_options: BypassOptions
}

fn is_tls_chello(input: &[u8]) -> bool {
  input.len() > 5 && u16::from_be_bytes([input[0], input[1]]) == 0x1603 && input[5] == 1
}

impl ProxyServer {
  pub fn new(addr: SocketAddr) -> Self {
    Self{
      socks_version: SOCKS4_VERSION,
      server_addr: addr,
      msg_buf_size: BUF_SIZE,
      bypass_options: BypassOptions::new()
    }
  }

  pub fn set_msg_buf_size(&mut self, size: usize) {
    self.msg_buf_size = size;
  }

  pub async fn proxy_one_side(read_stream: Rc<tokio_uring::net::TcpStream>, write_stream: Rc<tokio_uring::net::TcpStream>,
                              mut proxy_buf: Vec<u8>, read_timeout: Option<Duration>) -> Result<(), anyhow::Error> {
    let mut first_pkt = true;
    let mut result;
    let mut npbuf;
    loop {
      if let Some(r_t) = read_timeout.and_then(|r_t| first_pkt.then_some(r_t)) {
        if let Ok(ret) = timeout(r_t, read_stream.read(proxy_buf)).await {
          (result, npbuf) = ret;
          first_pkt = false;
        } else {
          debug!("timeout");
          break;
        }
      } else { (result, npbuf) = read_stream.read(proxy_buf).await; }
      let proxy_size = result?;
      if proxy_size == 0 { break; }
      proxy_buf = npbuf;
      let (res, proxy_slice) = write_stream.write(proxy_buf.slice(..proxy_size)).submit().await; res?;
      proxy_buf = proxy_slice.into_inner();
    }
    read_stream.shutdown(Shutdown::Both)?;
    debug!("shutdown with client");
    Ok(())
  }

  pub async fn socks_proxy(self, socks4: Socks4) -> Result<(), anyhow::Error> {
    let mut client_buf = vec![0u8; self.msg_buf_size];
    let proxy_buf = vec![0u8; self.msg_buf_size];
    let mut client_size;
    let proxy_stream = socks4.proxy_stream.unwrap();
    let proxy_stream_rc = Rc::new(proxy_stream);
    let client_stream_rc = Rc::new(socks4.client_stream);
    let proxy_stream_rc1 = proxy_stream_rc.clone();
    let client_stream_rc1 = client_stream_rc.clone();
    let res = tokio_uring::spawn(async move {
      ProxyServer::proxy_one_side(proxy_stream_rc1, client_stream_rc1, proxy_buf, self.bypass_options.timeout).await
    });
    loop {
      let (result, nbuf) = client_stream_rc.read(client_buf).await;
      client_size = result?;
      if client_size == 0 { break; }
      client_buf = nbuf;
      let proxy_fd = proxy_stream_rc.as_raw_fd();
      if is_tls_chello(&client_buf[..client_size]) {
        client_buf = self.bypass_options.desync(proxy_fd, proxy_stream_rc.clone(), client_buf, client_size).await?;
      } else {
        let (res, slice) = proxy_stream_rc.write(client_buf.slice(..client_size)).submit().await; res?;
        client_buf = slice.into_inner();
      }
    }
    proxy_stream_rc.shutdown(Shutdown::Both)?;
    trace!("shutdown with proxy");
    res.abort();
    Ok(())
  }

  pub async fn handle_client(self, stream: tokio_uring::net::TcpStream) -> Result<(), anyhow::Error> {
    let first_input = vec![0u8; self.msg_buf_size];
    let (result, first_input) = stream.read(first_input).await;
    let n = result?;
    if n == 0 {
      debug!("exiting because n=0");
      return Ok(());
    }
    let mut socks4 = Socks4::is_connect_req(&first_input[..n], stream)?;
    socks4.connect_to_dst(&first_input[..n]).await?;
    if let Some(pr_stream) = socks4.proxy_stream.as_ref() { pr_stream.set_nodelay(true)?; }
    socks4.phase = Socks4Phase::Proxing;
    self.socks_proxy(socks4).await?;
    Ok(())
  }

  pub fn start_server(self) {
    assert_eq!(self.socks_version, 4);
    tokio_uring::start(async {
      let listener = tokio_uring::net::TcpListener::bind(self.server_addr).unwrap();
      loop {
        let (stream, socket_addr) = listener.accept().await.unwrap();
        let proxy_server = self.clone();
        info!("Accepted connection from: {socket_addr}");
        tokio_uring::spawn(async move {
          let _ = proxy_server.handle_client(stream).await.inspect_err(|e| error!("{e:?}"));
        });
      }
    });
  }
}
