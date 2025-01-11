mod bypass;
mod proxy_server;
mod socks;

use std::{net::SocketAddr, time::Duration};
use std::str::FromStr;

use env_logger::Env;
use log::info;
use structopt::StructOpt;
use cfg_block::cfg_block;

use bypass::{SplitPosition, SplitPositions, DesyncType};
use proxy_server::{ProxyServer, BUF_SIZE_STR};

cfg_block! {
  #[cfg(feature = "udp-desync")] {
    mod udp;
    use udp::UdpBypassHelpData;
    const UDP_RECV_BUF_SIZE: usize = 2048;

    fn run_bypassing(server: ProxyServer, udp_options: Option<UdpBypassHelpData>) {
      std::thread::scope(|s| {
        s.spawn(|| {
          info!("Desync options:\n{:#?}", server.bypass_options);
          server.start_server();
        });
        s.spawn(|| {
          if let Some(mut udp_opts) = udp_options {
            if unsafe { libc::getuid() } != 0 { panic!("You need to be a root"); }
            info!("Udp desync options:\n{:#?}", udp_opts);
            udp_opts.init_queue().unwrap();
            udp_opts.run_nfq_loop();
          }
        });
      });
    }
  }
}

#[allow(dead_code)]
#[derive(Debug, StructOpt)]
#[structopt(name = "rustpass-dpi")]
#[structopt(global_setting = structopt::clap::AppSettings::AllowNegativeNumbers)]
/// Bypass dpi written in rust inspired by byedpi and zapret.
///
/// Rustpass-dpi supports bypassing tls using socks4 proxy and udp using nfqueue and network namespace(if need)
struct Cmd {
  /// listen addr in ip:port format
  #[structopt()]
  proxy_addr: String,

  /// TCP buf size
  #[structopt(default_value=BUF_SIZE_STR, short, long)]
  buf_size: usize,

  /// TCP timeout in secs
  #[structopt(short, long, default_value)]
  timeout: f32,

  /// disorder position
  #[structopt(short, long, default_value)]
  disorder: i32,

  /// Split positions.
  /// Can be single number or list of numbers separated by space: -s 2 -1 10 or many --split arguments: -s 2 -s -1 -s 10
  #[structopt(short, long)]
  split: Vec<i32>,

  /// Disorder with oob data positions.
  /// Can be single number or list of numbers separated by space: -D 2 -1 10 or many --disoob arguments: -D 2 -D -1 -D 10
  #[structopt(long, short="D")]
  disoob: Vec<i32>,

  /// Split with oob data positions.
  /// Can be single number or list of numbers separated by space: -S 2 -1 10 or many --splitoob arguments: -S 2 -S -1 -S 10
  #[structopt(long, short="S")]
  splitoob: Vec<i32>,

  /// Split with send fake packets.
  /// Can be single number or list of numbers separated by space: -f 2 -1 10 or many --fake arguments: -f 2 -f -1 -f 10
  #[structopt(short, long)]
  fake: Vec<i32>,

  /// TTL for fake packets.
  ///
  /// If you get something like this when connecting:
  /// Secure Connection Failed
  /// Error code: SSL_ERROR_PROTOCOL_VERSION_ALERT
  /// decreasing fake-ttl may help
  #[structopt(short="F", long, default_value="6")]
  fake_ttl: u8,

  /// Byte sent outside the main stream
  #[structopt(short, long, default_value="97")]
  oob_data: u8,

  /// Use udp desync. Warning for it you need to run rustpass-dpi as root.
  /// You can also run udp-bypass-helper.sh for creating new network namespace.
  /// It can be useful when you want to desync udp trafic only for some apps.
  #[structopt(short="U", long)]
  udp_desync: bool,

  /// Mark for outgoing udp fake packets.
  /// Must be the same as in ./udp-bypass-helper.sh BYPASS_MARK env if use
  #[structopt(short, long, default_value="12345")]
  mark: i32,

  /// Nfqueue num for sending udp fake packets
  /// Must be the same as in ./udp-bypass-helper.sh QUEUE_NUM env if use
  #[structopt(short, long, default_value="0")]
  nfqueue_num: u16
}

trait PushPositions {
  fn push_split_pos(&mut self, opt: &mut Cmd);
}

impl PushPositions for SplitPositions {
  fn push_split_pos(&mut self, opt: &mut Cmd) {
    if opt.disorder != 0 { self.push(SplitPosition{ pos: opt.disorder, desync_type: DesyncType::Disorder }) }
    opt.split.iter().for_each(|x| self.push(SplitPosition{ pos: *x, desync_type: DesyncType::Split }));
    opt.disoob.iter().for_each(|x| self.push(SplitPosition{ pos: *x, desync_type: DesyncType::Disoob }));
    opt.splitoob.iter().for_each(|x| self.push(SplitPosition{ pos: *x, desync_type: DesyncType::Splitoob }));
    opt.fake.iter().for_each(|x| self.push(SplitPosition{ pos: *x, desync_type: DesyncType::Fake }));
  }
}

#[cfg(target_os = "linux")]
fn main() {
  env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
  let mut opt = Cmd::from_args();
  let mut server = ProxyServer::new(SocketAddr::from_str(opt.proxy_addr.as_str()).unwrap());
  let mut desync_options = SplitPositions::new();
  info!("Server listening on {}", opt.proxy_addr);
  server.set_msg_buf_size(opt.buf_size);
  desync_options.push_split_pos(&mut opt);
  server.bypass_options.append_options(desync_options);
  server.bypass_options.fake_ttl = opt.fake_ttl as u32;
  server.bypass_options.oob_data = opt.oob_data;
  if opt.timeout > 0.0 { server.bypass_options.timeout = Some(Duration::from_secs_f32(opt.timeout)); }
  #[cfg(feature = "udp-desync")] {
    let mut udp_options = None;
    if opt.udp_desync { udp_options = Some(UdpBypassHelpData::new::<UDP_RECV_BUF_SIZE>(opt.mark, opt.nfqueue_num, opt.fake_ttl)); }
    run_bypassing(server, udp_options);
  }
  #[cfg(not(feature = "udp-desync"))] {
    if opt.udp_desync {
      panic!("For udp_desync you need to compile rustpass-dpi with --features udp-desync or with default features");
    }
    server.start_server();
  }
}
