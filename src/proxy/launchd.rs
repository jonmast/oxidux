// Socket activation integration with launchd
// Copied from https://github.com/LinkTed/doh-client/blob/3920321902ab270a0c68f67717036d405fbdcd87/src/dns.rs

use libc::{c_void, free, size_t};
use std::{
    ffi::CString,
    io::{Error, ErrorKind::Other},
    net::{TcpListener, UdpSocket},
    os::{
        raw::{c_char, c_int},
        unix::io::{FromRawFd, RawFd},
    },
    ptr::null_mut,
};

type FdType = RawFd;

extern "C" {
    fn launch_activate_socket(name: *const c_char, fds: *mut *mut c_int, cnt: *mut size_t)
        -> c_int;
}

pub(crate) fn get_udp_socket(name: &str) -> Result<UdpSocket, Error> {
    // Safety: launchd should always return valid fds, and won't return duplicates
    get_fd(name).map(|fd| unsafe { UdpSocket::from_raw_fd(fd) })
}

pub(crate) fn get_tcp_socket(name: &str) -> Result<TcpListener, Error> {
    // Safety: launchd should always return valid fds, and won't return duplicates
    get_fd(name).map(|fd| unsafe { TcpListener::from_raw_fd(fd) })
}

fn get_fd(name: &str) -> Result<RawFd, Error> {
    let fds = get_fds(name).unwrap_or_default();

    match fds.get(0) {
        Some(fd) => Ok(*fd),
        None => Err(Error::new(
            Other,
            "Couln't find file descriptor from socket activation",
        )),
    }
}

/// Get raw file descriptors for lanchd socket of a given name
fn get_fds(name: &str) -> Option<Vec<FdType>> {
    // Launchd takes a name, array pointer, and count pointer and fills the array with file
    // descriptors for the named socket.  We are responsible for freeing the memory, so copy the
    // array into a new vector and free it immediately so we don't need to set up a custom
    // desctructor.
    unsafe {
        let mut fds: *mut c_int = null_mut();
        let mut cnt: size_t = 0;

        let name = CString::new(name).expect("CString::new failed");

        if launch_activate_socket(name.as_ptr(), &mut fds, &mut cnt) == 0 {
            assert!(!fds.is_null());
            let result = std::slice::from_raw_parts(fds, cnt).to_vec();
            free(fds as *mut c_void);

            Some(result)
        } else {
            None
        }
    }
}
