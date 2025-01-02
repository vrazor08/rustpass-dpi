use std::{io, os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd}, rc::Rc};
use std::time::Duration;
use std::ffi::CString;

use anyhow::bail;
use tokio_uring::net::TcpStream;
use tokio_uring::buf::BoundedBuf;
use socket2::{self, Socket};
use log::{debug, info};
use libc;

const DEFAULT_TTL: u32 = 64;

static FAKE_TLS: [u8; 517] = [
  22, 3, 1, 2, 0, 1, 0, 1, 252, 3, 3, 3, 95, 111, 44, 237, 19, 34, 248, 220, 178, 242, 96, 72, 45, 114, 102, 111, 87,
  221, 19, 157, 27, 55, 220, 250, 54, 46, 186, 249, 146, 153, 58, 32, 249, 223, 12, 46, 138, 85, 137, 130, 49, 99, 26,
  239, 168, 190, 8, 88, 167, 163, 90, 24, 211, 150, 95, 4, 92, 180, 98, 175, 137, 215, 15, 139, 0, 62, 19, 2, 19, 3, 19,
  1, 192, 44, 192, 48, 0, 159, 204, 169, 204, 168, 204, 170, 192, 43, 192, 47, 0, 158, 192, 36, 192, 40, 0, 107, 192, 35,
  192, 39, 0, 103, 192, 10, 192, 20, 0, 57, 192, 9, 192, 19, 0, 51, 0, 157, 0, 156, 0, 61, 0, 60, 0, 53, 0, 47, 0, 255,
  1, 0, 1, 117, 0, 0, 0, 22, 0, 20, 0, 0, 17, 119, 119, 119, 46, 119, 105, 107, 105, 112, 101, 100, 105, 97, 46, 111,
  114, 103, 0, 11, 0, 4, 3, 0, 1, 2, 0, 10, 0, 22, 0, 20, 0, 29, 0, 23, 0, 30, 0, 25, 0, 24, 1, 0, 1, 1, 1, 2, 1, 3, 1,
  4, 0, 16, 0, 14, 0, 12, 2, 104, 50, 8, 104, 116, 116, 112, 47, 49, 46, 49, 0, 22, 0, 0, 0, 23, 0, 0, 0, 49, 0, 0, 0,
  13, 0, 42, 0, 40, 4, 3, 5, 3, 6, 3, 8, 7, 8, 8, 8, 9, 8, 10, 8, 11, 8, 4, 8, 5, 8, 6, 4, 1, 5, 1, 6, 1, 3, 3, 3, 1,
  3, 2, 4, 2, 5, 2, 6, 2, 0, 43, 0, 9, 8, 3, 4, 3, 3, 3, 2, 3, 1, 0, 45, 0, 2, 1, 1, 0, 51, 0, 38, 0, 36, 0, 29, 0, 32,
  17, 140, 184, 140, 232, 138, 8, 144, 30, 238, 25, 217, 221, 232, 212, 6, 177, 209, 226, 171, 224, 22, 99, 214, 220,
  218, 132, 164, 184, 75, 251, 14, 0, 21, 0, 172, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];


#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum DesyncType {
  Split,
  Disorder,
  Splitoob,
  Disoob,
  Fake
}

#[derive(Clone, Debug)]
pub struct SplitPosition {
  pub pos: i32,
  pub desync_type: DesyncType
}

#[derive(Clone, Debug)]
pub struct BypassOptions {
  split_positions: Vec<SplitPosition>,
  fake_ttl: u32,
  pub oob_data: u8,
  pub timeout: Option<Duration>,
}

impl BypassOptions {
  pub fn new(fake_ttl: u32) -> Self {
    Self{split_positions: Vec::new(), fake_ttl, oob_data: 97, timeout: None}
  }

  pub async fn desync(&self, fd: RawFd, stream: Rc<TcpStream>, mut buf: Vec<u8>, size: usize) -> Result<Vec<u8>, anyhow::Error> {
    let mut prev_pos: i32 = 0;
    let mut current_pos: usize = 0;
    for i in 0..self.split_positions.len() {
      // current_pos = self.split_positions[i].pos as usize;
      if self.split_positions[i].pos < 0 { current_pos = size - (self.split_positions[i].pos.unsigned_abs() as usize); }
      else { current_pos = self.split_positions[i].pos as usize; }
      if prev_pos < 0 { prev_pos += size as i32 }
      info!("prev_pos = {prev_pos}, current_pos = {current_pos}");
      if current_pos >= size {
        info!("split_pos >= size");
        let (res, slice) = stream.write(buf.slice(prev_pos as usize..size)).submit().await; res?;
        buf = slice.into_inner();
        return Ok(buf);
      }
      match self.split_positions[i].desync_type {
        DesyncType::Split => {
          let (res, slice) = stream.write(buf.slice(prev_pos as usize..current_pos)).submit().await; res?;
          buf = slice.into_inner();
          prev_pos = current_pos as i32;
        }
        DesyncType::Disorder => {
          BypassOptions::set_ttl(fd, 1)?;
          let (res, slice) = stream.write(buf.slice(prev_pos as usize..current_pos)).submit().await; res?;
          buf = slice.into_inner();
          BypassOptions::set_ttl(fd, DEFAULT_TTL)?;
          prev_pos = current_pos as i32;
        }
        DesyncType::Splitoob => {
          self.write_oob(stream.as_raw_fd(), &buf[prev_pos as usize..current_pos])?;
          prev_pos = current_pos as i32;
        }
        DesyncType::Fake => {
          BypassOptions::set_ttl(fd, self.fake_ttl)?;
          BypassOptions::send_fake(fd, current_pos - (prev_pos as usize), Vec::from(&buf[prev_pos as usize..current_pos]))?;
          BypassOptions::set_ttl(fd, DEFAULT_TTL)?;
          prev_pos = current_pos as i32;
        }
        _ => bail!("unimplemented type")
      }
    }
    if current_pos != size {
      // BypassOptions::set_ttl(fd, DEFAULT_TTL)?;
      let (res, slice) = stream.write(buf.slice(current_pos..size)).submit().await; res?;
      buf = slice.into_inner();
    }
    Ok(buf)
  }

  pub fn at_least_one_option(&self) -> bool { !self.split_positions.is_empty()}

  pub fn append_options(&mut self, mut options: Vec<SplitPosition>) {
    self.split_positions.append(options.as_mut());
    self.split_positions.sort_by(|a,b| {
      if (a.pos < 0 || b.pos < 0) && !(a.pos < 0 && b.pos < 0) { return a.pos.cmp(&b.pos).reverse(); }
      a.pos.cmp(&b.pos)
    });
  }

  // TODO: use io-uring
  pub fn write_oob(&self, fd: RawFd, buf: &[u8]) -> io::Result<usize> {
    let mut buf = buf.to_vec();
    buf.push(self.oob_data);
    let sock = unsafe { Socket::from_raw_fd(fd) };
    let ret = sock.send_out_of_band(&buf);
    sock.into_raw_fd();
    ret
  }

  // TODO: use io-uring zero copy sending
  pub fn send_fake(fd: RawFd, current_pos: usize, buf: Vec<u8>) -> Result<(), anyhow::Error> {
    let mut w_bytes;
    debug!("current_pos = {current_pos}");
    let name = CString::new("name").unwrap();
    let ffd = unsafe { libc::memfd_create(name.as_ptr(), 0)};
    if ffd < 0 { fd.into_raw_fd(); bail!("ffd < 0"); }
    unsafe {
      w_bytes = libc::write(ffd, FAKE_TLS.as_ptr() as _, current_pos);
      debug!("fake bytes write: {w_bytes}", );
      libc::lseek(ffd, 0, libc::SEEK_SET);
      if libc::sendfile(fd, ffd, 0 as _, current_pos) < 0 { fd.into_raw_fd(); bail!("sendfile < 0"); }
      libc::lseek(ffd, 0, libc::SEEK_SET);
      w_bytes = libc::write(ffd, buf.as_ptr() as _, current_pos);
      debug!("good bytes write: {w_bytes}");
      fd.into_raw_fd();
    }
    Ok(())
  }

  pub fn set_ttl(fd: RawFd, ttl: u32) -> io::Result<()> {
    let sock = unsafe { Socket::from_raw_fd(fd) };
    let ret = sock.set_ttl(ttl);
    sock.into_raw_fd();
    ret
  }
}
