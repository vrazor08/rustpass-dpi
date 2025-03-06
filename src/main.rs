mod bypass;
mod cmd;
mod proxy_server;
mod socks;

use env_logger::Env;
#[allow(unused_imports)]
use log::{debug, info};
use structopt::StructOpt;
use cfg_block::cfg_block;

use cmd::Cmd;
use proxy_server::ProxyServer;

macro_rules! root_block {
  ($bl: expr) => {
    #[cfg(feature = "suid")]
    set_root_uid();
    $bl;
    #[cfg(feature = "suid")]
    set_user_uid();
  };
}

cfg_block! {
  #[cfg(feature = "udp-desync")] {
    use std::thread;

    mod udp;
    use udp::UdpBypassHelpData;
    #[allow(unused_variables)]
    fn run_bypassing(server: Option<ProxyServer>, udp_options: Option<UdpBypassHelpData>, app: String) {
      #[cfg(feature = "suid")] {
        Command::new("bash")
          .arg("-c")
          .arg(app)
          .stdout(Stdio::null())
          .stderr(Stdio::null())
          .spawn()
          .expect("Failed to run app");
      }
      thread::scope(|s| {
        thread::Builder::new().name("tcp-desync".into()).spawn_scoped(s, || {
          if let Some(tcp_opts) = server {
            info!("Desync options:\n{:#?}", tcp_opts);
            tcp_opts.start_server();
          }
        }).expect("failed to spawn thread");

        thread::Builder::new().name("udp-desync".into()).spawn_scoped(s, || {
          if let Some(mut udp_opts) = udp_options {
            #[cfg(not(feature = "suid"))]
            assert_eq!(unsafe { libc::getuid() }, 0, "You need to be a root");

            info!("Udp desync options:\n{:#?}", udp_opts);
            root_block!({
              assert_eq!(unsafe { libc::geteuid() }, 0, "euid must be 0, maybe you don't have suid bit");
              udp_opts.init_queue().unwrap();
              udp_opts.run_nfq_loop();
            });
          }
        }).expect("failed to spawn thread");
      });
    }
  }
  #[cfg(feature = "suid")] {
    use std::process::{Command, Stdio};
    mod suid;
    use suid::{RUSTPASS_GID, RUSTPASS_UID, set_root_uid, set_user_uid};
  }
}

fn main() {
  env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
  #[cfg(feature = "suid")] {
    unsafe {
      RUSTPASS_UID = libc::getuid();
      RUSTPASS_GID = libc::getgid();
      debug!("[{}] current uid is: {}, gid is: {}", line!(), libc::getuid(), libc::getgid());
    };
    set_user_uid();
  }
  let opt = Cmd::from_args();
  #[cfg(debug_assertions)] { log::trace!("opt: {:#?}", opt); }
  let app = opt.clone().run_app.unwrap_or(String::new());
  if !app.is_empty() && !cfg!(feature = "suid") {
    panic!("To use --run-app option you need to compile rustpass-dpi with --features suid");
  }
  let server: Option<ProxyServer> = opt.clone().cmd.try_into().ok();
  #[cfg(feature = "udp-desync")] {
    let udp_options: Option<UdpBypassHelpData>;
    root_block!(udp_options = opt.cmd.try_into().ok());
    run_bypassing(server, udp_options, app);
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
