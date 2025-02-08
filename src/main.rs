mod bypass;
mod cmd;
mod proxy_server;
mod socks;

use env_logger::Env;
use log::info;
use structopt::StructOpt;
use cfg_block::cfg_block;

use cmd::Cmd;
use proxy_server::ProxyServer;

cfg_block! {
  #[cfg(feature = "udp-desync")] {
    use std::thread;

    mod udp;
    use udp::UdpBypassHelpData;

    fn run_bypassing(server: Option<ProxyServer>, udp_options: Option<UdpBypassHelpData>) {
      thread::scope(|s| {
        thread::Builder::new().name("tcp-desync".into()).spawn_scoped(s, || {
          if let Some(tcp_opts) = server {
            info!("Desync options:\n{:#?}", tcp_opts);
            tcp_opts.start_server();
          }
        }).expect("failed to spawn thread");

        thread::Builder::new().name("udp-desync".into()).spawn_scoped(s, || {
          if let Some(mut udp_opts) = udp_options {
            assert!(unsafe { libc::getuid() == 0 }, "You need to be a root");
            info!("Udp desync options:\n{:#?}", udp_opts);
            udp_opts.init_queue().unwrap();
            udp_opts.run_nfq_loop();
          }
        }).expect("failed to spawn thread");
      });
    }
  }
}

fn main() {
  env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
  let opt = Cmd::from_args();
  #[cfg(debug_assertions)] { log::trace!("opt: {:#?}", opt); }
  let server: Option<ProxyServer> = opt.clone().cmd.try_into().ok();
  #[cfg(feature = "udp-desync")] {
    let udp_options: Option<UdpBypassHelpData> = opt.cmd.try_into().ok();
    run_bypassing(server, udp_options);
  }
  #[cfg(not(feature = "udp-desync"))] {
    use cmd::is_udp_opts;
    assert!(
      !is_udp_opts(&opt.cmd),
      "For udp_desync or netns you need to compile rustpass-dpi with --features udp-desync or with default features"
    );
    info!("Desync options:\n{:#?}", server.as_ref().unwrap());
    server.unwrap().start_server();
  }
}
