//! A WebSocket over TLS (`wss://`), client side: TLS by rustls with Mozilla's
//! root certificates, the opening handshake (RFC 6455 section 4), then binary
//! messages both ways ([`crate::frame`]); pings are answered, a close ends it.
//! Also a plain HTTPS GET, for the list of Steam's servers.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use lanstash_domain::checksum::sha1::{base64, sha1};
use lanstash_ports::random::Random;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

use crate::frame::{self, BINARY, CLOSE, CONTINUATION, Frame, PING, PONG, TEXT};

/// The GUID RFC 6455 appends to the client's key.
const ACCEPT_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Largest message accepted.
const MAX_MESSAGE: usize = 64 << 20;

type Tls = StreamOwned<ClientConnection, TcpStream>;

fn random<const N: usize>() -> Result<[u8; N], String> {
    let mut b = [0u8; N];
    crate::random::RingRandom.fill(&mut b)?;
    Ok(b)
}

/// A TLS connection to `host:port`, the server's certificate checked against Mozilla's roots.
fn tls(host: &str, port: u16, timeout: Duration) -> Result<Tls, String> {
    let addr = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("{host}: {e}"))?
        .next()
        .ok_or(format!("{host}: no address"))?;
    let tcp = TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("{host}: {e}"))?;
    tcp.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    tcp.set_write_timeout(Some(timeout)).map_err(|e| e.to_string())?;
    let roots = RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    let config = ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = ServerName::try_from(host.to_owned()).map_err(|e| format!("{host}: {e}"))?;
    let conn = ClientConnection::new(Arc::new(config), name).map_err(|e| e.to_string())?;
    Ok(StreamOwned::new(conn, tcp))
}

/// Reads an HTTP response head (to the blank line), and what came after it.
fn read_head(s: &mut Tls) -> Result<(String, Vec<u8>), String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..end]).into_owned();
            return Ok((head, buf[end + 4..].to_vec()));
        }
        if buf.len() > 64 << 10 {
            return Err("an HTTP head over 64 KiB".into());
        }
        let n = s.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("closed before the HTTP head ended".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// `GET https://host/path`: the body of a 200 (the server closes after it).
///
/// # Errors
/// Connection, TLS, or a status other than 200.
pub fn https_get(host: &str, path: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    let mut s = tls(host, 443, timeout)?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: lanstash\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let (head, mut body) = read_head(&mut s)?;
    if !head.starts_with("HTTP/1.1 200") {
        return Err(format!("{host}{path}: {}", head.lines().next().unwrap_or("")));
    }
    // A peer that closes without close_notify still gave the body.
    let _ = s.read_to_end(&mut body);
    Ok(body)
}

/// An open WebSocket.
pub struct Wss {
    s: Tls,
    /// Bytes read past the last frame.
    pending: Vec<u8>,
    /// A fragmented message so far.
    partial: Vec<u8>,
}

impl std::fmt::Debug for Wss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wss").finish_non_exhaustive()
    }
}

impl Wss {
    /// Opens `wss://host:port/path`; reads wait at most `timeout`.
    ///
    /// # Errors
    /// Connection, TLS, or a server that does not accept the upgrade.
    pub fn connect(host: &str, port: u16, path: &str, timeout: Duration) -> Result<Self, String> {
        let mut s = tls(host, port, timeout)?;
        let key = base64(&random::<16>()?);
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nUser-Agent: lanstash\r\n\r\n"
        );
        s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
        let (head, pending) = read_head(&mut s)?;
        if !head.starts_with("HTTP/1.1 101") {
            return Err(format!("upgrade refused: {}", head.lines().next().unwrap_or("")));
        }
        let accept = base64(&sha1(format!("{key}{ACCEPT_GUID}").as_bytes()));
        let ok = head.lines().any(|l| {
            l.split_once(':').is_some_and(|(n, v)| n.eq_ignore_ascii_case("sec-websocket-accept") && v.trim() == accept)
        });
        if !ok {
            return Err("the server's Sec-WebSocket-Accept does not match".into());
        }
        Ok(Self { s, pending, partial: Vec::new() })
    }

    fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> Result<(), String> {
        let f = frame::encode(opcode, payload, random::<4>()?);
        self.s.write_all(&f).map_err(|e| e.to_string())
    }

    /// Sends one binary message.
    ///
    /// # Errors
    /// The connection failed.
    pub fn send(&mut self, message: &[u8]) -> Result<(), String> {
        self.send_frame(BINARY, message)
    }

    /// The next frame; `None` when the read timed out (what was read is kept).
    fn frame(&mut self) -> Result<Option<Frame>, String> {
        let mut chunk = [0u8; 16 << 10];
        loop {
            if let Some((f, used)) = frame::decode(&self.pending, MAX_MESSAGE)? {
                self.pending.drain(..used);
                return Ok(Some(f));
            }
            let n = match self.s.read(&mut chunk) {
                Ok(n) => n,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => return Ok(None),
                Err(e) => return Err(e.to_string()),
            };
            if n == 0 {
                return Err("the server closed the connection".into());
            }
            self.pending.extend_from_slice(&chunk[..n]);
        }
    }

    /// The next message (binary or text), pings answered on the way; `None`
    /// when nothing whole came within the timeout.
    ///
    /// # Errors
    /// The connection failed or the server closed it.
    pub fn recv(&mut self) -> Result<Option<Vec<u8>>, String> {
        loop {
            let Some(f) = self.frame()? else { return Ok(None) };
            match f.opcode {
                PING => self.send_frame(PONG, &f.payload)?,
                PONG => {}
                CLOSE => return Err("the server closed the WebSocket".into()),
                BINARY | TEXT | CONTINUATION => {
                    self.partial.extend_from_slice(&f.payload);
                    if self.partial.len() > MAX_MESSAGE {
                        return Err("a message over the limit".into());
                    }
                    if f.fin {
                        return Ok(Some(std::mem::take(&mut self.partial)));
                    }
                }
                other => return Err(format!("an unknown frame opcode {other}")),
            }
        }
    }
}
