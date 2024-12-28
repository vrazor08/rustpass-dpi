fn main() {
  cc::Build::new()
    .file("./src/c_nfqueue/c-udp-bypass.c")
    .include("./src/c_nfqueue")
    .opt_level(2)
    .compile("c-udp-bypass");
  println!("cargo:rustc-link-lib=mnl");
  println!("cargo:rustc-link-lib=netfilter_queue");
  println!("cargo:rerun-if-changed=./src/c_nfqueue/c-udp-bypass.c");
  println!("cargo:rerun-if-changed=./src/c_nfqueue/c-udp-bypass.h");
}
