use std::{net::SocketAddr, time::Duration, str::FromStr};

use anyhow::bail;
use structopt::StructOpt;

use crate::proxy_server::{ProxyServer, BUF_SIZE_STR};
use crate::bypass::{DesyncType, SplitPosition, SplitPositions};

#[cfg(feature = "udp-desync")]
use crate::udp::{self, UdpBypassHelpData, UDP_RECV_BUF_SIZE};

macro_rules! gen_subcommand {
  ($enum_name:ident, $udp_name:ident, $tcp_name:ident) => {
    gen_subcommand!((#[cfg(target_os = "linux")]), (#[cfg(target_os = "linux")]), $enum_name, $udp_name, $tcp_name);
  };
  (tcp_udp, $enum_name:ident) => {
    gen_subcommand!((#[cfg(target_os = "windows")]), (#[cfg(target_os = "linux")]), $enum_name, __none, __none);
  };
  (udp_tcp, $enum_name:ident) => {
    gen_subcommand!((#[cfg(target_os = "linux")]), (#[cfg(target_os = "windows")]), $enum_name, __none, __none);
  };
  (($(#[$attr_tcp:meta])*), ($(#[$attr_udp:meta])*), $enum_name:ident, $udp_name:ident, $tcp_name:ident) => {
    #[derive(Clone, Debug, StructOpt)]
    #[structopt(rename_all = "kebab-case")]
    pub enum $enum_name {
      $(#[$attr_tcp])*
      #[allow(unused)]
      #[structopt(name = "tcp", about = "Use to specify tcp desync options")]
      /// Use to specify tcp desync options
      ///
      /// Warning: If you use options that expect a list of args, such as: --split,
      /// you need to put a dot at the end if the next arg is a udp subcommand, for example:
      /// --split 2 -1 10 . udp -N ns1
      Tcp {
        /// listen addr in ip:port format
        #[structopt()]
        proxy_addr: String,

        /// TTL for fake packets.
        ///
        /// If you get something like this when connecting:
        /// Secure Connection Failed
        /// Error code: SSL_ERROR_PROTOCOL_VERSION_ALERT
        /// decreasing fake-ttl may help
        #[structopt(short="F", long, default_value="6")]
        fake_ttl: u8,

        /// TCP buf size
        #[structopt(default_value=BUF_SIZE_STR, short, long)]
        buf_size: usize,

        /// TCP timeout in secs
        #[structopt(short, long, default_value, hide_default_value=true)]
        timeout: f32,

        /// disorder position
        #[structopt(short, long, default_value, hide_default_value=true)]
        disorder: i32,

        /// Split positions.
        /// Can be single number or list of numbers separated by space: -s 2 -1 10 or many --split arguments: -s 2 -s -1 -s 10
        #[structopt(short, long, value_terminator("."))]
        split: Vec<i32>,

        /// Disorder with oob data positions.
        /// Can be single number or list of numbers separated by space: -D 2 -1 10 or many --disoob arguments: -D 2 -D -1 -D 10
        #[structopt(long, short="D", value_terminator("."))]
        disoob: Vec<i32>,

        /// Split with oob data positions.
        /// Can be single number or list of numbers separated by space: -S 2 -1 10 or many --splitoob arguments: -S 2 -S -1 -S 10
        #[structopt(long, short="S", value_terminator("."))]
        splitoob: Vec<i32>,

        /// Split with send fake packets.
        /// Can be single number or list of numbers separated by space: -f 2 -1 10 or many --fake arguments: -f 2 -f -1 -f 10
        #[structopt(short, long, value_terminator("."))]
        fake: Vec<i32>,

        /// Byte sent outside the main stream
        #[structopt(short, long, default_value="97")]
        oob_data: u8,


        $(#[$attr_udp])*
        /// Udp command
        #[structopt(subcommand)]
        udp: Option<$udp_name>,
      },
      $(#[$attr_udp])*
      #[allow(unused)]
      #[structopt(name = "udp", about = "Use to specify udp desync options and network namespace")]
      /// Use to specify udp desync options and network namespace
      ///
      /// Warning: for all of these options you need to be a root
      Udp {
        /// TTL for udp fake packets.
        #[structopt(short="F", long, default_value="6")]
        fake_ttl: u8,

        /// Mark for outgoing udp fake packets.
        /// Must be the same as in ./udp-bypass-helper.sh BYPASS_MARK env if use
        #[structopt(short, long)]
        mark: i32,

        /// Nfqueue num for sending udp fake packets
        /// Must be the same as in ./udp-bypass-helper.sh QUEUE_NUM env if use
        #[structopt(short, long)]
        nfqueue_num: u16,

        /// Experimental. Run rustpass-dpi in a named, persistent network namespace.
        #[structopt(short="N", long, default_value, hide_default_value=true)]
        netns: String,

        $(#[$attr_tcp])*
        /// TCP command
        #[structopt(subcommand)]
        tcp: Option<$tcp_name>,
      }
    }
  };
}

macro_rules! gen_subcommands {
  ($main_enum_name:ident, $udp_sb_enum_name:ident, $tcp_sb_enum_name:ident) => {
    gen_subcommand!($main_enum_name, $udp_sb_enum_name, $tcp_sb_enum_name);
    gen_subcommand!(tcp_udp, $udp_sb_enum_name);
    gen_subcommand!(udp_tcp, $tcp_sb_enum_name);
  };
}

gen_subcommands!(Subcommands, TcpSubcommand, UdpSubcommand);

#[derive(Clone, Debug, StructOpt)]
#[structopt(name = "rustpass-dpi")]
#[structopt(global_setting = structopt::clap::AppSettings::AllowNegativeNumbers)]
#[allow(dead_code)]
/// Bypass dpi written in rust inspired by byedpi and zapret.
///
/// Rustpass-dpi supports bypassing tls using socks4 proxy and udp using nfqueue and network namespace(if need)
pub struct Cmd {
  #[structopt(subcommand)]
  pub cmd: Subcommands,

  /// Experimental. Run app with rustpass-dpi. It makes sense only with --netns option.
  /// To use this option you need to set suid bit.
  /// If you use this option you don't to run rustpass-dpi with sudo
  #[structopt(short, long)]
  pub run_app: Option<String>,
}

impl TryInto<ProxyServer> for Subcommands {
  type Error = anyhow::Error;

  fn try_into(self) -> Result<ProxyServer, Self::Error> {
    struct DesyncVecs {
      disorder: i32,
      split: Vec<i32>,
      disoob: Vec<i32>,
      splitoob: Vec<i32>,
      fake: Vec<i32>
    }

    trait PushPositions {
      fn push_split_pos(&mut self, desync_vecs: DesyncVecs);
    }

    impl PushPositions for SplitPositions {
      #[inline]
      fn push_split_pos(&mut self, desync_vecs: DesyncVecs) {
        if desync_vecs.disorder != 0 { self.push(SplitPosition{ pos: desync_vecs.disorder, desync_type: DesyncType::Disorder }) }
        desync_vecs.split.iter().for_each(|x| self.push(SplitPosition{ pos: *x, desync_type: DesyncType::Split }));
        desync_vecs.disoob.iter().for_each(|x| self.push(SplitPosition{ pos: *x, desync_type: DesyncType::Disoob }));
        desync_vecs.splitoob.iter().for_each(|x| self.push(SplitPosition{ pos: *x, desync_type: DesyncType::Splitoob }));
        desync_vecs.fake.iter().for_each(|x| self.push(SplitPosition{ pos: *x, desync_type: DesyncType::Fake }));
      }
    }

    fn create_server(proxy_addr: String, fake_ttl: u8, buf_size: usize, timeout: f32, oob_data: u8, desync_ves: DesyncVecs) -> ProxyServer {
      let mut server = ProxyServer::new(SocketAddr::from_str(proxy_addr.as_str()).unwrap());
      let mut desync_options = SplitPositions::new();
      server.set_msg_buf_size(buf_size);
      desync_options.push_split_pos(desync_ves);
      server.bypass_options.append_options(desync_options);
      server.bypass_options.fake_ttl = fake_ttl as u32;
      server.bypass_options.oob_data = oob_data;
      if timeout > 0.0 { server.bypass_options.timeout = Some(Duration::from_secs_f32(timeout)); }
      assert!(server.bypass_options.at_least_one_option(), "You need to specify at least one option");
      server
    }

    match self {
      Self::Tcp { proxy_addr, fake_ttl, buf_size, timeout, disorder, split, disoob, splitoob, fake, oob_data, ..} => {
        #[allow(clippy::redundant_field_names)]
        let server = create_server(proxy_addr, fake_ttl, buf_size, timeout, oob_data, DesyncVecs {
          disorder: disorder, split: split, disoob: disoob, splitoob: splitoob, fake: fake
        });
        Ok(server)
      }
      Self::Udp { tcp, .. } => {
        if let Some(tcp_opts) = tcp {
          match tcp_opts {
            UdpSubcommand::Tcp { proxy_addr, fake_ttl, buf_size, timeout, disorder, split, disoob, splitoob, fake, oob_data, } => {
              #[allow(clippy::redundant_field_names)]
              let server = create_server(proxy_addr, fake_ttl, buf_size, timeout, oob_data, DesyncVecs {
                disorder: disorder, split: split, disoob: disoob, splitoob: splitoob, fake: fake
              });
              Ok(server)
            }
          }
        } else { bail!("tcp subcommand not found"); }
      }
    }
  }
}

#[cfg(feature = "udp-desync")]
impl TryInto<UdpBypassHelpData> for Subcommands {
  type Error = anyhow::Error;

  fn try_into(self) -> Result<UdpBypassHelpData, Self::Error> {

    //concat!(stringify!(UDP_RECV_BUF_SIZE));
    match self {
      Self::Tcp { udp, .. } => {
        if let Some(udp_opts) = udp {
          match udp_opts {
            TcpSubcommand::Udp { fake_ttl, mark, nfqueue_num, netns} => {
              if !netns.is_empty() { udp::netns(netns.as_str()).unwrap(); }
              Ok(UdpBypassHelpData::new::<UDP_RECV_BUF_SIZE>(mark, nfqueue_num, fake_ttl))
            }
          }
        } else { bail!("udp subcommand not found"); }
      }
      Self::Udp { fake_ttl, mark, nfqueue_num, netns, .. } => {
        if !netns.is_empty() { udp::netns(netns.as_str()).unwrap(); }
        Ok(UdpBypassHelpData::new::<UDP_RECV_BUF_SIZE>(mark, nfqueue_num, fake_ttl))
      }
    }
  }
}

#[cfg(not(feature = "udp-desync"))]
#[inline]
pub fn is_udp_opts(cmd: &Subcommands) -> bool {
  match cmd {
    Subcommands::Tcp { udp, .. } => udp.is_some(),
    Subcommands::Udp { .. } => true
  }
}
