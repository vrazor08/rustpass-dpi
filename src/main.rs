mod bypass;
mod proxy_server;
mod socks;
mod udp;

use std::net::SocketAddr;
use std::str::FromStr;

use log::info;

use bypass::{SplitPosition, DesyncType};
use proxy_server::ProxyServer;
use udp::UdpBypassHelpData;
// const PROXY_ADDR: &str = "127.0.0.1:8085";
const PROXY_ADDR: &str = "127.0.0.2:8082";
const UDP_RECV_BUF_SIZE: usize = 2048;

fn main() {
  env_logger::init();
  let mut server = ProxyServer::new(SocketAddr::from_str(PROXY_ADDR).unwrap(), 4u8);
  info!("Server listening on {}", PROXY_ADDR);
  server.set_msg_buf_size(662);
  // let desync_options = vec![
  //   SplitPosition{pos: -2, desync_type: DesyncType::Disorder},
  //   SplitPosition{pos: 2, desync_type: DesyncType::Split},
  // ];
  // let desync_options = vec![
  //   SplitPosition{pos: 2, desync_type: DesyncType::Splitoob},
  //   SplitPosition{pos: 25, desync_type: DesyncType::Splitoob},
  // ];
  let desync_options = vec![
    SplitPosition{pos: 1, desync_type: DesyncType::Split},
    SplitPosition{pos: -1, desync_type: DesyncType::Fake},
  ];

  let mut udp_options = UdpBypassHelpData::new::<UDP_RECV_BUF_SIZE>(12345, 0);
  // let desync_options = vec![SplitPosition{pos: 1, desync_type: DesyncType::Disorder}];
  server.bypass_options.append_options(desync_options);

  // server.bypass_options.timeout = Some(Duration::new(3, 0));
  info!("Desync options:\n{:#?}", server.bypass_options);
  info!("Udp desync options:\n{:#?}", udp_options);
  udp_options.init_queue().unwrap();
  udp_options.desync_udp().unwrap();
  server.start_server();
}
