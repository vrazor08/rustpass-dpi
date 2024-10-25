// use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_uring::net::{TcpStream, TcpListener};

use bytes::BytesMut;
use anyhow;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;

const PROXY_ADDR: &str = "127.0.0.2:8082";
const MSG_BUF_SIZE: usize = 8192;
const SOCKS_PROXY_LEN: usize = 8;

fn is_connect_req(input: &[u8]) -> Result<SocketAddrV4, anyhow::Error> {
  if input.len() < SOCKS_PROXY_LEN {
    anyhow::bail!("it isn't fisrt socks packet because len: {}", input.len());
  }
  if (input[0] != 4u8) && (input[1] != 1 || input[1] != 2) {
    anyhow::bail!("it isn't fisrt socks packet or unknown socks version: {}", input[0]);
  }
  let port = u16::from_be_bytes([input[2], input[3]]);
  let ip = Ipv4Addr::new(input[4], input[5], input[6], input[7]);
  let dst = SocketAddrV4::new(ip, port);
  Ok(dst)
}

async fn socks_proxy(mut input: Vec<u8>, mut proxy_stream: TcpStream, mut client_stream: TcpStream, id: i32) -> Result<(), anyhow::Error> {
  // let (mut proxy_read, mut proxy_write) = proxy_stream.into_split();
  // let (mut client_read, mut client_write) = client_stream.into_split();
  let (write_size, input) = proxy_stream.write(input).submit().await;
  let write_size = write_size.unwrap();
  println!("{id}: sending req of size: {write_size} to proxy stream");
  // let output = BytesMut::zeroed(MSG_BUF_SIZE);
  let output = vec![0, MSG_BUF_SIZE];

  // let mut proxy_read_buf = output.clone();
  // let mut last_proxy_read_size = 0;
  // let proxy_write_buf = output.clone();

  // let mut client_read_buf = input.clone();
  // let client_write_buf = input.clone();
  // let mut last_client_read_size = 0;

  loop {
    // let n = proxy_read.read(&mut input).await?;
    // if n == 0 { break; }
    // tokio::select! {
    //   client_read_result = client_read.read(&mut client_read_buf) => {
    //     let size = client_read_result?;
    //     if size == 0 { println!("client read = 0"); break }
    //     last_client_read_size = size;
    //   }
    //   proxy_read_result = proxy_read.read(&mut proxy_read_buf) => {
    //     let size = proxy_read_result?;
    //     if size == 0 { println!("proxy read = 0"); break }
    //     last_proxy_read_size = size;
    //   }
    //   client_write_result = client_write.write(&proxy_write_buf[0..last_proxy_read_size]) => {
    //     client_write_result?;
    //   }
    //   proxy_write_result = proxy_write.write_all(&client_write_buf[0..last_client_read_size]) => {
    //     proxy_write_result?;
    //   }
    // }
  }
  Ok(())
}

async fn handle_connection(mut stream: TcpStream, id: i32) {
  let mut proxy_stream: Option<TcpStream> = None;
  // let mut proxy_dst: Option<SocketAddrV4> = None;
  // let mut first_input = BytesMut::zeroed(MSG_BUF_SIZE);
  let first_input = vec![0; MSG_BUF_SIZE];
  let (result, first_input) = stream.read(first_input).await;
  result.unwrap();
  match is_connect_req(&first_input) {
    Ok(dst) => {
      println!("{id}: proxy request to dst: {:?}", dst);
      match TcpStream::connect(SocketAddr::from(dst)).await {
        Ok(pr_stream) => {
          let (result, _) = stream.write(vec![0, 90, first_input[2], first_input[3], first_input[4], first_input[5], first_input[6], first_input[7]])
            .submit()
            .await;
          result.unwrap();
          proxy_stream = Some(pr_stream);
          // proxy_dst = Some(dst);
        }
        Err(e) => {
          eprintln!("{id}: Error: {}", e);
          let (result, _) = stream.write(vec![0, 91, first_input[2], first_input[3], first_input[4], first_input[5], first_input[6], first_input[7]])
            .submit()
            .await;
          result.unwrap();
          return;
        }
      }
    } Err(e) => {
      println!("proxing... because {e:?}");
      socks_proxy(first_input, proxy_stream.unwrap(), stream, id).await;
    }
  }
}

fn main() {
  tokio_uring::start(async {
    let listener = TcpListener::bind(SocketAddr::from_str(PROXY_ADDR).unwrap()).unwrap();
    println!("Listening on: {}", PROXY_ADDR);
    let mut id = 1;
    loop {
      let (mut stream, peer) = listener.accept().await.unwrap();
      println!("New connection from: {peer}");
      tokio_uring::spawn(async move { handle_connection(stream, id) } );
      id+=1;
    }
  });
}
