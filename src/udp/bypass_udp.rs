use core::marker::{PhantomData, PhantomPinned};

use std::fmt::{self, Debug};
use std::ptr::null_mut;
use std::os::raw::c_char;

use anyhow::bail;
use libc::__errno_location;

pub const UDP_RECV_BUF_SIZE: usize = 2048;
const FAKE_PKT_LEN: usize = 64;
static FAKE_UDP_PKT: [u8; FAKE_PKT_LEN] = [0; FAKE_PKT_LEN];

#[repr(C)]
struct NfqHandle {
  _data: [u8; 0],
  _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

#[repr(C)]
struct NfqQHandle {
  _data: [u8; 0],
  _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

#[repr(C)]
struct BypassData {
  mark: i32,
  queue_num: u16,
  fake_ttl: u8,
  fake_pkt_payload: *const u8,
  fake_pkt_payload_len: usize,
  log_level: i32
}

extern "C" {
  #[allow(improper_ctypes)]
  fn init_nfq(bypass_data: *const BypassData, h: *mut *mut NfqHandle, qh: *mut *mut NfqQHandle) -> i32;

  #[allow(improper_ctypes)]
  fn run_nfq(h: *mut NfqHandle, buf: *mut c_char, buf_size: usize);

  #[allow(improper_ctypes)]
  fn destroy_nfq(h: *mut NfqHandle, qh: *mut NfqQHandle);
}

pub struct UdpBypassHelpData {
  bypass_data: BypassData,
  h: *mut NfqHandle,
  qh: *mut NfqQHandle,
  buf: Box<[u8]>
}

impl UdpBypassHelpData {
  pub fn new<const BUF_SIZE: usize>(mark: i32, queue_num: u16, fake_ttl: u8) -> Self {
    Self {
      bypass_data: BypassData {
        mark,
        queue_num,
        fake_ttl,
        fake_pkt_payload: &FAKE_UDP_PKT as *const u8,
        fake_pkt_payload_len: FAKE_PKT_LEN,
        log_level: log::max_level() as i32
      },
      h: null_mut(),
      qh: null_mut(),
      buf: Box::new([0u8; BUF_SIZE])
    }
  }

  pub fn init_queue(&mut self) -> Result<(), anyhow::Error> {
    unsafe {
      if init_nfq(&(self.bypass_data), &mut (self.h), &mut (self.qh)) != 0 {
        let errno = *__errno_location();
        bail!("init_nfq error: errno={}", errno);
      };
    }
    if self.h.is_null() { bail!("NfqHandle is null"); }
    if self.qh.is_null() { bail!("NfqQHandle is null"); }
    Ok(())
  }

  pub fn run_nfq_loop(mut self) {
    unsafe {
      run_nfq(self.h, self.buf.as_mut_ptr() as *mut c_char, self.buf.len());
    }
  }
}

impl Drop for UdpBypassHelpData {
  fn drop(&mut self) {
    log::trace!("droping UdpBypassHelpData");
    unsafe {
      destroy_nfq(self.h, self.qh);
    }
  }
}

unsafe impl Send for UdpBypassHelpData {}

impl Debug for BypassData {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut payload;
    if self.fake_pkt_payload.is_null() { payload = "NULL".into(); }
    else {
      payload = String::with_capacity(self.fake_pkt_payload_len);
      for i in 0..self.fake_pkt_payload_len as isize {
        payload.push_str(unsafe { (*self.fake_pkt_payload.offset(i)).to_string().as_str() });
      }
    }
    f.debug_struct("BypassData")
      .field("mark", &self.mark)
      .field("queue_num", &self.queue_num)
      .field("fake_ttl", &self.fake_ttl)
      .field("fake_pkt_payload_len", &self.fake_pkt_payload_len)
      .field("fake_pkt_payload", &payload)
      .finish()
  }
}

impl Debug for UdpBypassHelpData {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let h_fmt: &dyn Debug = if self.h.is_null() { &"NULL" }
      else { &self.h };
    let qh_fmt: &dyn Debug = if self.qh.is_null() { &"NULL" }
      else { &self.qh };
    f.debug_struct("UdpBypassHelpData")
      .field("bypass_data", &self.bypass_data)
      .field("h", h_fmt)
      .field("qh", qh_fmt)
      .field("recv_buf", &format!("Box<[u8; {}]>", self.buf.len()))
      .finish()
  }
}
