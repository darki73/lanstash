//! `sendfile(2)`: `len` bytes of a file from `offset` to a socket, by the kernel. The offset
//! is passed with each call, so the file's own position is never used or moved (many
//! connections share one open file). Where the system or the file system cannot do it, it
//! says so before anything was sent, and the caller sends the bytes another way.

use std::fs::File;
use std::io;
use std::net::TcpStream;

/// What [`send_file`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sent {
    /// All of it went to the socket.
    All,
    /// Nothing was sent: this system or file system cannot (send it another way).
    Unsupported,
}

/// Sends `len` bytes of `file` from `offset` to `socket`.
///
/// # Errors
/// The socket failed or timed out (its write timeout), or the file is shorter than
/// `offset + len`; some bytes may have been sent: the response is broken.
#[cfg(target_os = "linux")]
pub fn send_file(socket: &TcpStream, file: &File, offset: u64, len: u64) -> io::Result<Sent> {
    use std::os::fd::AsRawFd;
    // At most this much per call (what Linux moves at most, rounded down to a page).
    const MAX: u64 = 0x7fff_f000;
    let mut sent = 0u64;
    while sent < len {
        let mut at = libc::off_t::try_from(offset + sent).map_err(|_| io::ErrorKind::InvalidInput)?;
        let count = usize::try_from((len - sent).min(MAX)).unwrap_or(usize::MAX);
        // SAFETY: both descriptors are valid while `socket` and `file` are borrowed, the
        // kernel writes only `at` (a local it may update), and nothing is read from or
        // written to memory of ours.
        #[allow(unsafe_code)]
        let n = unsafe { libc::sendfile(socket.as_raw_fd(), file.as_raw_fd(), &raw mut at, count) };
        if n < 0 {
            let e = io::Error::last_os_error();
            match e.raw_os_error() {
                Some(libc::EINTR) => continue,
                // The socket's write timeout ran out.
                Some(libc::EAGAIN) => return Err(io::Error::new(io::ErrorKind::TimedOut, e)),
                // This file system (or socket) cannot: nothing went, send it another way.
                Some(libc::EINVAL | libc::ENOSYS | libc::EOPNOTSUPP) if sent == 0 => return Ok(Sent::Unsupported),
                _ => return Err(e),
            }
        }
        let n = u64::try_from(n).unwrap_or(0);
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the file ended before the bytes to send"));
        }
        sent += n;
    }
    Ok(Sent::All)
}

/// Elsewhere it never can: the bytes are sent another way.
///
/// # Errors
/// None.
#[cfg(not(target_os = "linux"))]
pub fn send_file(_socket: &TcpStream, _file: &File, _offset: u64, _len: u64) -> io::Result<Sent> {
    Ok(Sent::Unsupported)
}
