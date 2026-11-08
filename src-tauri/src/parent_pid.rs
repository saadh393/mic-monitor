use std::mem::{size_of, zeroed};

pub fn of(pid: i32) -> Option<i32> {
    let mut info: libc::proc_bsdinfo = unsafe { zeroed() };
    let size = size_of::<libc::proc_bsdinfo>() as i32;
    let read = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, &mut info as *mut _ as *mut _, size) };
    (read == size).then_some(info.pbi_ppid as i32)
}
