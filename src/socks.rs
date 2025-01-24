use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use anyhow::bail;
use tokio_uring::net::TcpStream;

pub const SOCKS4_VERSION: u8 = 4u8;
pub const SOCKS4_CONNECT_COMMAND: u8 = 1u8;
pub const SOCKS4_BIND_COMMAND: u8 = 2u8;

#[derive(Debug)]
pub enum Socks4Phase {
  ConnectReq,
  ConnectRep,
  Proxing
}

pub struct Socks4 {
  pub phase: Socks4Phase,
  pub proxy_addr: SocketAddr,
  pub proxy_stream: Option<TcpStream>,
  pub client_stream: TcpStream
}

impl Socks4 {
  pub fn is_connect_req(input: &[u8], client_stream: TcpStream) -> Result<Self, anyhow::Error> {
    if input.len() < 8 || input.len() > 30 {
      bail!("it isn't fisrt socks packet because len: {}", input.len());
    }
    match input[0] {
      SOCKS4_VERSION => {
        if input[1] != SOCKS4_CONNECT_COMMAND && input[1] != SOCKS4_BIND_COMMAND {
          bail!("it isn't fisrt socks{} packet", input[0]);
        }
        let port = u16::from_be_bytes([input[2], input[3]]);
        let ip = Ipv4Addr::new(input[4], input[5], input[6], input[7]);
        let dst = SocketAddrV4::new(ip, port);
        Ok(Self{
          phase: Socks4Phase::ConnectReq,
          proxy_addr: SocketAddr::from(dst),
          proxy_stream: None,
          client_stream
        })
      }
      ver => bail!("unsupported socks version: {ver}")
    }
  }

  pub async fn connect_to_dst(&mut self, input: &[u8]) -> Result<(), anyhow::Error> {
    assert!(input.len() >= 8, "input len must be >= 8 given: {}", input.len());
    match &self.phase {
      Socks4Phase::ConnectReq => {
        match TcpStream::connect(self.proxy_addr).await {
          Ok(pr_stream) => {
            let (res, _) = self.client_stream.write(vec![0u8, 90, input[2], input[3], input[4], input[5], input[6], input[7]])
              .submit()
              .await; res?;
            self.proxy_stream = Some(pr_stream);
            self.phase = Socks4Phase::ConnectRep;
          }
          Err(e) => {
            let (res, _) = self.client_stream.write(vec![0u8, 91, input[2], input[3], input[4], input[5], input[6], input[7]])
                .submit()
                .await; res?;
            return Err(anyhow::Error::new(e));
          }
        }
      }
      socks_phase => bail!("Socks4 Phase for connect must be {:?}, but current: {:?}", Socks4Phase::ConnectReq, socks_phase)
    }
    Ok(())
  }
}
