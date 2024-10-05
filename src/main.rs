use std::net::{Ipv4Addr, Shutdown, SocketAddrV4, TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

use log::{trace, debug, info, warn, error};

use bytes::BytesMut;
use anyhow;

const PROXY_ADDR: &str = "127.0.0.2:8082";
const MSG_BUF_SIZE: usize = 65536;
const READ_TIMEOUT: Option<Duration> = Some(Duration::new(2, 0));

fn is_tls_chello(input: &[u8]) -> bool {
  return input.len() > 5 && u16::from_be_bytes([input[0], input[1]]) == 0x1603 && input[5] == 1
}

/// https://www.openssh.com/txt/socks4.protocol
fn is_connect_req(input: &[u8]) -> Result<SocketAddrV4, anyhow::Error> {
  if input.len() < 8 {
    anyhow::bail!("it isn't fisrt socks packet because len: {}", input.len());
  }
  if (input[0] != 4u8) && (input[1] != 1 || input[1] != 2) {
    anyhow::bail!("it isn't fisrt socks packet or unknown socks version: {}", input[0]);
  }
  let port = u16::from_be_bytes([input[2], input[3]]);
  let ip = Ipv4Addr::new(input[4], input[5], input[6], input[7]);
  let dst = SocketAddrV4::new(ip, port);
  // let ip = u32::from_be_bytes([input[4], input[5], input[6], input[7]]);
  // println!("ip of given socks req: {ip:?}, port: {port}");
  Ok(dst)
}

// TODO: use io-uring
fn socks_proxy(mut proxy_stream: TcpStream, mut client_stream: TcpStream, id: i32) -> Result<(), anyhow::Error> {
  let mut client_buf = BytesMut::zeroed(MSG_BUF_SIZE);
  let mut proxy_buf = BytesMut::zeroed(MSG_BUF_SIZE);
  let mut proxy_size;
  let mut client_size;
  client_stream.set_nonblocking(true).unwrap();
  loop {
    match client_stream.read(&mut client_buf) {
      Ok(size) => {
        if size == 0 { break; }
        client_size = size;
        debug!("{id}: recieving req of size: {client_size} from client stream");
        if is_tls_chello(&client_buf[0..client_size]) {
          debug!("set ttl = 1");
          proxy_stream.set_ttl(1).unwrap();
          proxy_stream.write(&client_buf[0..1])?;
          proxy_stream.set_ttl(64).unwrap();
          let write_size = proxy_stream.write(&client_buf[1..client_size])?;
          debug!("{id}: sending req of size: {write_size} to proxy stream");
        } else {
          let write_size = proxy_stream.write(&client_buf[0..client_size])?;
          debug!("{id}: sending req of size: {write_size} to proxy stream");
        }

        // proxy_flag = 0;
      }
      Err(e) => {
        trace!("{id}: client read err: {e}");
        // client_flag += 1;
        // break
      }
    }

    match proxy_stream.read(&mut proxy_buf) {
      Ok(size) => {
        if size == 0 { break; }
        proxy_size = size;
        debug!("{id}: recieving req of size: {proxy_size} from proxy stream");
        let write_size = client_stream.write(&proxy_buf[0..proxy_size])?;
        debug!("{id}: sending req of size: {write_size} to client stream");
        // client_flag = 0;
      }
      Err(e) => {
        trace!("{id}: proxy read err: {e}");
        // proxy_flag += 1;
        // continue;
      }
    }
    // if proxy_flag >= 2 && client_flag >= 2 {
    //   println!("end");
    //   break;
    // }

  }
  Ok(())
}

fn handle_client(mut stream: TcpStream, id: i32) {
  // let mut proxy_stream: Option<TcpStream> = None;
  let mut first_input = BytesMut::zeroed(MSG_BUF_SIZE);
  match stream.read(&mut first_input) {
    Ok(size) => {
      // echo everything!
      debug!("{id}: first_input len: {size}");
      if size == 0 {
        stream.shutdown(Shutdown::Both).unwrap_or(());
        println!("shutdown");
        return;
      }
      match is_connect_req(&first_input[0..size]) {
        Ok(dst) => {
          println!("{id}: proxy request to dst: {:?}", dst);
          match TcpStream::connect(dst) {
            Ok(pr_stream) => {
              stream.write(&[0, 90, first_input[2], first_input[3], first_input[4], first_input[5], first_input[6], first_input[7]]).unwrap();
              // proxy_stream = Some(pr_stream);
              // proxy_stream.as_ref().unwrap().set_read_timeout(READ_TIMEOUT).unwrap();
              pr_stream.set_nonblocking(true).unwrap();
              pr_stream.set_read_timeout(READ_TIMEOUT).unwrap();
              match socks_proxy(pr_stream, stream, id) {
                Ok(()) => debug!("{id}: exiting from proxy function"),
                Err(e) => warn!("{id}: Error: {}", e)
              }
            }
            Err(e) => {
              warn!("{id}: Error: {}", e);
              stream.write(&[0, 91, first_input[2], first_input[3], first_input[4], first_input[5], first_input[6], first_input[7]]).unwrap();
            }
          }
        }
        Err(e) => {
          error!("{id}: Error: proxy_stream is None, but it must be Some, {e}");
          stream.shutdown(Shutdown::Both).unwrap_or(());
          return;
        }
      }
    },
    Err(e) => {
      error!("{id}: An error: {:#?} occurred in reading, terminating", e);
      stream.shutdown(Shutdown::Both).unwrap_or(());
      return;
      }
  }
}

fn main() {
  env_logger::init();
  let listener = TcpListener::bind(PROXY_ADDR).unwrap();
  // accept connections and process them, spawning a new thread for each one
  info!("Server listening on {}", PROXY_ADDR);
  let mut id = 1;
  for stream in listener.incoming() {
    match stream {
      Ok(stream) => {
        debug!("New connection: {}", stream.peer_addr().unwrap());
        stream.set_read_timeout(READ_TIMEOUT).unwrap();
        thread::spawn(move|| {
          handle_client(stream, id);
        });
        id += 1;
      }
      Err(e) => error!("Error: {}", e)
    }
  }
}
