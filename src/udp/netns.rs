use std::ffi::CString;

use anyhow::bail;
use log::debug;

pub fn netns(netns_name: &str) -> Result<(), anyhow::Error> {
  let control_file_name = CString::new(format!("/var/run/netns/{netns_name}"))?;
  unsafe {
    let ns_fd = libc::open(control_file_name.as_ptr(), libc::O_RDONLY|libc::O_CLOEXEC);
    if ns_fd < 0 { bail!("cannot open {control_file_name:?}"); }
    if libc::setns(ns_fd, libc::CLONE_NEWNET) < 0 { bail!("setns failed"); }
    libc::close(ns_fd);
  }
  debug!("rustpass-dpi was moved in {control_file_name:?} network namespace");
  Ok(())
}
