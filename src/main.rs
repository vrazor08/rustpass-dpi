use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;

use log::{trace, debug, info, warn, error};

use bytes::BytesMut;
use anyhow;

const PROXY_ADDR: &str = "127.0.0.2:8082";
const MSG_BUF_SIZE: usize = 65536;
const READ_TIMEOUT: Option<Duration> = Some(Duration::new(2, 0));

const S5CONNECT: u8 = 0x01;
const S5BIND: u8 = 0x02;
const S5UDP_ASSOCIATE: u8 = 0x03;

fn is_tls_chello(input: &[u8]) -> bool {
  return input.len() > 5 && u16::from_be_bytes([input[0], input[1]]) == 0x1603 && input[5] == 1
}

#[derive(Debug)]
enum SupportedProtos {
  Socks4(SocketAddrV4),
  Socks5(u8) // method
}



/// https://www.openssh.com/txt/socks4.protocol
fn is_connect_req(input: &[u8]) -> Result<SupportedProtos, anyhow::Error> {
  if input.len() < 3 {
    anyhow::bail!("it isn't fisrt socks packet because len: {}", input.len());
  }
  match input[0] {
    4u8 => {
      if input[1] != 1 && input[1] != 2 {
        anyhow::bail!("it isn't fisrt socks{} packet", input[0]);
      }
      let port = u16::from_be_bytes([input[2], input[3]]);
      let ip = Ipv4Addr::new(input[4], input[5], input[6], input[7]);
      let dst = SocketAddrV4::new(ip, port);
      // let ip = u32::from_be_bytes([input[4], input[5], input[6], input[7]]);
      // println!("ip of given socks req: {ip:?}, port: {port}");
      return Ok(SupportedProtos::Socks4(dst));
    }
    5u8 => {
      if input[2..].iter().find(|&&x| x == 0).is_some() { // bad
        return Ok(SupportedProtos::Socks5(0));
      }
      anyhow::bail!("not supported socks5 auth methods was founded");
    }
    ver => {
      anyhow::bail!("unsupported socks version: {ver}");
    }
  }
}

fn socks5_handshake(method: u8, stream: &mut TcpStream, mut input: BytesMut) -> Result<TcpStream, anyhow::Error> {
  stream.write([5u8, method].as_ref())?;
  stream.set_read_timeout(READ_TIMEOUT)?;
  let n = stream.read(&mut input)?;
  if n == 0 { anyhow::bail!("socks5 handshake failed"); }
  let dst;
  let atyp;
  if n > 9 && input[0] == 5u8 {
    match input[3] {
      1 => {
        atyp = 1u8;
        let port = u16::from_be_bytes([input[8], input[9]]);
        dst = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(input[4], input[5], input[6], input[7])), port)
      }
      4u8 => {
        atyp = 4u8;
        if n < 21 { anyhow::bail!("n too low: {n}"); }
        let port = u16::from_be_bytes([input[20], input[21]]);
        let ipv6_bytes: [u8; 16] = (&input[4..20]).try_into()?;
        dst = SocketAddr::new(IpAddr::V6(Ipv6Addr::from(ipv6_bytes)), port)
      }
      atyp => anyhow::bail!("unknow atyp: {atyp}")
    }
    match input[1] {
      S5CONNECT => {
        let pr_stream = TcpStream::connect(dst)?;
        // let mut reply: Vec<u8> = vec![5, 0, 0, atyp, 127, 0, 0, 2, 0, 0];
        // match dst.ip() {
        //   IpAddr::V4(ip) => reply.append(&mut Vec::from(ip.octets())),
        //   IpAddr::V6(ip) => reply.append(&mut Vec::from(ip.octets()))
        // }
        // stream.write(reply.as_slice())?;
        stream.write(&[5u8, 0, 0, atyp, 127, 0, 0, 2, 0, 0])?;
        return Ok(pr_stream);
      }
      S5UDP_ASSOCIATE => {
        error!("UDP ASSOCIATE cmd!");
        anyhow::bail!("unknown cmd: {S5UDP_ASSOCIATE}")
      }
      cmd => anyhow::bail!("unknown cmd: {cmd}")
    }
  } else { anyhow::bail!("n too low or it isn't sock5");  }
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
      }
    }

    match proxy_stream.read(&mut proxy_buf) {
      Ok(size) => {
        if size == 0 { break; }
        proxy_size = size;
        debug!("{id}: recieving req of size: {proxy_size} from proxy stream");
        let write_size = client_stream.write(&proxy_buf[0..proxy_size])?;
        debug!("{id}: sending req of size: {write_size} to client stream");
      }
      Err(e) => {
        trace!("{id}: proxy read err: {e}");
      }
    }
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
          info!("{id}: proxy request to dst: {:?}", dst);
          match dst {
            SupportedProtos::Socks4(dst) => {
              match TcpStream::connect(dst) {
                Ok(pr_stream) => {
                  stream.write(&[0, 90, first_input[2], first_input[3], first_input[4], first_input[5], first_input[6], first_input[7]]).unwrap();
                  pr_stream.set_nonblocking(true).unwrap();
                  pr_stream.set_read_timeout(READ_TIMEOUT).unwrap();
                  socks_proxy(pr_stream, stream, id).unwrap_or_else(|e| warn!("{id}: Error: {}", e));
                  debug!("{id}: exiting from proxy function")
                }
                Err(e) => {
                  warn!("{id}: {}", e);
                  stream.write(&[0, 91, first_input[2], first_input[3], first_input[4], first_input[5], first_input[6], first_input[7]]).unwrap();
                }
              }
            }
            SupportedProtos::Socks5(method) => {
              match socks5_handshake(method, &mut stream, first_input) {
                Ok(proxy_stream) => {
                  proxy_stream.set_nonblocking(true).unwrap();
                  proxy_stream.set_read_timeout(READ_TIMEOUT).unwrap();
                  socks_proxy(proxy_stream, stream, id).unwrap_or_else(|e| warn!("{id}: Error: {}", e));
                  debug!("{id}: exiting from proxy function")
                }
                Err(e) => {
                  warn!("{id}: {}", e);
                  stream.write(&[5u8, 1, 0]).unwrap();
                }
              }
              // error!("Socks 5 not supported yet");
              return;
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
