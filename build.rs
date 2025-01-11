#[cfg(target_os = "linux")]
fn main() {
  #[cfg(feature = "udp-desync")] {
    cc::Build::new()
      .file("./src/c_nfqueue/c-udp-bypass.c")
      .include("./src/c_nfqueue")
      .opt_level(2)
      .compile("c-udp-bypass");
    println!("cargo:rustc-link-lib=netfilter_queue");
    println!("cargo:rerun-if-changed=./src/c_nfqueue/c-udp-bypass.c");
    println!("cargo:rerun-if-changed=./src/c_nfqueue/c-udp-bypass.h");
  }
}

#[cfg(not(target_os = "linux"))]
fn main() {
  compile_error!("Only linux supported");
}
