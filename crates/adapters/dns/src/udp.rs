//! One DNS exchange over UDP: a query sent to a server, and its answer.

use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// Sends `query` to `server` and reads datagrams from it until `answer` takes one: it gives
/// `None` for a datagram that is not the answer to this query (it is ignored), else the result.
///
/// # Errors
/// The socket failed, nothing came within `timeout`, or `answer`'s error.
pub fn exchange<T>(
    server: SocketAddr,
    query: &[u8],
    timeout: Duration,
    answer: impl Fn(&[u8]) -> Option<Result<T, String>>,
) -> Result<T, String> {
    let bind: SocketAddr =
        if server.is_ipv4() { ([0, 0, 0, 0], 0).into() } else { "[::]:0".parse().map_err(|_| "bind")? };
    let sock = UdpSocket::bind(bind).map_err(|e| format!("bind: {e}"))?;
    sock.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    sock.send_to(query, server).map_err(|e| format!("send to {server}: {e}"))?;
    let mut buf = [0u8; 1500];
    loop {
        let (n, from) = sock.recv_from(&mut buf).map_err(|e| match e.kind() {
            // A read timeout says `WouldBlock` on Linux ("Resource temporarily unavailable").
            ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                format!("{server}: no answer within {} ms", timeout.as_millis())
            }
            _ => format!("{server}: {e}"),
        })?;
        if from != server {
            continue; // not from the server asked: ignore
        }
        if let Some(r) = answer(&buf[..n]) {
            return r;
        }
    }
}
