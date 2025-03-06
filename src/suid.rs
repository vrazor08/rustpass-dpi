pub static mut RUSTPASS_UID: libc::uid_t = 0;
pub static mut RUSTPASS_GID: libc::gid_t = 0;

#[inline(always)]
pub fn set_user_uid() {
  assert_ne!(unsafe { libc::seteuid(RUSTPASS_UID) }, -1);
  assert_ne!(unsafe { libc::setegid(RUSTPASS_GID) }, -1);
}

#[inline(always)]
pub fn set_root_uid() {
  unsafe {
    assert_ne!(RUSTPASS_UID, 0);
    assert_ne!(RUSTPASS_GID, 0);
    #[allow(unused)] {
      let mut rv = libc::seteuid(0);
      rv |= libc::setegid(0);
    }
  }
}
