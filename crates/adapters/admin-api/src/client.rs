//! Talks to a running lanstash's admin port (`admin_listen`) from this
//! machine: asks it to stop, checks whether it is up, reads its status.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::Duration;

use lanstash_domain::json::parse;

/// The address to reach `admin_listen` on from this machine (a wildcard
/// listen address is reached through loopback).
#[must_use]
pub fn local_addr(admin_listen: SocketAddr) -> SocketAddr {
    let ip = match admin_listen.ip() {
        IpAddr::V4(v) if v.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(v) if v.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    SocketAddr::new(ip, admin_listen.port())
}

/// Whether something accepts connections at `addr`.
#[must_use]
pub fn is_up(addr: SocketAddr, timeout: Duration) -> bool {
    TcpStream::connect_timeout(&addr, timeout).is_ok()
}

/// Sends one request and returns the status code and body.
///
/// # Errors
/// Connection or protocol failure.
pub fn request(addr: SocketAddr, method: &str, path: &str, timeout: Duration) -> Result<(u16, String), String> {
    let mut s = TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("{addr}: {e}"))?;
    s.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    let head = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    s.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    s.read_to_end(&mut out).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out);
    let status = text
        .split_ascii_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| format!("{addr}: not an HTTP answer"))?;
    let body = text.split_once("\r\n\r\n").map_or("", |(_, b)| b).to_owned();
    Ok((status, body))
}

/// Sends a request to the API path, and to the path from before the API
/// when the answer is 404 or 405 (a lanstash older than the API is running, as
/// during an upgrade).
///
/// # Errors
/// Connection or protocol failure.
pub fn request_api(
    addr: SocketAddr,
    method: &str,
    api_path: &str,
    old_path: &str,
    timeout: Duration,
) -> Result<(u16, String), String> {
    let answer = request(addr, method, api_path, timeout)?;
    if matches!(answer.0, 404 | 405) { request(addr, method, old_path, timeout) } else { Ok(answer) }
}

/// `GET /api/v1/status` (or `/status.json` on an older lanstash): status and body.
///
/// # Errors
/// Connection or protocol failure.
pub fn status(addr: SocketAddr, timeout: Duration) -> Result<(u16, String), String> {
    request_api(addr, "GET", "/api/v1/status", "/status.json", timeout)
}

/// What a status answer says of the store: its game directories and bytes, and the problems
/// its background check (`verify`) found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreFigures {
    pub games: u64,
    pub bytes: u64,
    pub problems: u64,
}

/// The store's figures in a status answer's body; `None` for a body that is not one.
#[must_use]
pub fn store_figures(body: &str) -> Option<StoreFigures> {
    let v = parse(body).ok()?;
    let usage = v.get("usage")?.as_array();
    let bytes = usage.iter().filter_map(|r| r.get("bytes")?.as_f64()).sum::<f64>() as u64;
    let problems = v.get("counters").and_then(|c| c.get("verify")?.get("problems")?.as_f64()).unwrap_or(0.0) as u64;
    Some(StoreFigures { games: usage.len() as u64, bytes, problems })
}

/// Asks lanstash to stop and waits until its admin port closes.
///
/// # Errors
/// Not running, refused, or still up after `wait`.
pub fn stop(admin_listen: SocketAddr, wait: Duration) -> Result<(), String> {
    let addr = local_addr(admin_listen);
    let step = Duration::from_millis(200);
    let (status, body) = request_api(addr, "POST", "/api/v1/shutdown", "/shutdown", Duration::from_secs(5))?;
    if status != 202 {
        return Err(format!("{addr}: {status} {}", body.trim()));
    }
    let mut waited = Duration::ZERO;
    while is_up(addr, step) {
        if waited >= wait {
            return Err(format!("{addr}: still running after {wait:?}"));
        }
        std::thread::sleep(step);
        waited += step;
    }
    Ok(())
}
