#![allow(non_camel_case_types,dead_code)]

use std::io;
use std::os::unix::io::RawFd;
use std::time::Duration;

use nix;
use nix::poll::{EventFlags, PollFd};
#[cfg(target_os = "linux")]
use nix::sys::signal::SigSet;
#[cfg(target_os = "linux")]
use nix::sys::time::{TimeSpec, TimeValLike};

pub fn wait_read_fd(fd: RawFd, timeout: Duration) -> io::Result<()> {
    wait_fd(fd, EventFlags::POLLIN, timeout)
}

pub fn wait_write_fd(fd: RawFd, timeout: Duration) -> io::Result<()> {
    wait_fd(fd, EventFlags::POLLOUT, timeout)
}

fn wait_fd(fd: RawFd, events: EventFlags, timeout: Duration) -> io::Result<()> {
    use nix::errno::Errno::{EIO, EPIPE};

    let mut fds = vec![PollFd::new(fd, events)];

    let milliseconds =
        timeout.as_secs() as i64 * 1000 + i64::from(timeout.subsec_nanos()) / 1_000_000;
    #[cfg(target_os = "linux")]
    let wait_res = {
        let timespec = TimeSpec::milliseconds(milliseconds);
        nix::poll::ppoll(fds.as_mut_slice(), timespec, SigSet::empty())
    };
    #[cfg(not(target_os = "linux"))]
    let wait_res = nix::poll::poll(fds.as_mut_slice(), milliseconds as nix::libc::c_int);

    let wait = match wait_res {
        Ok(r) => r,
        Err(e) => return Err(io::Error::from(::Error::from(e))),
    };
    // All errors generated by poll or ppoll are already caught by the nix wrapper around libc, so
    // here we only need to check if there's at least 1 event
    if wait != 1 {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Operation timed out",
        ));
    }

    // Check the result of ppoll() by looking at the revents field
    match fds[0].revents() {
        Some(e) if e == events => return Ok(()),
        // If there was a hangout or invalid request
        Some(e) if e.contains(EventFlags::POLLHUP) || e.contains(EventFlags::POLLNVAL) => {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, EPIPE.desc()));
        }
        Some(_) | None => (),
    }

    Err(io::Error::new(io::ErrorKind::Other, EIO.desc()))
}
