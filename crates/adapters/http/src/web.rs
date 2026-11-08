//! HTTP/1.1 client for web APIs on the network ([`Web`]): one connection per
//! request (`Connection: close`), over plain TCP or TLS ([`crate::web_tls`]),
//! the body sent with its length, the whole response read (by its length,
//! chunked, or until the server closes) up to a limit. Names are resolved the
//! way the machine resolves them, so a name on the local network works (the
//! upstream resolver is for the CDNs). A certificate that is not trusted fails
//! the request, saying which certificate it was, so it can be pinned.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use lanstash_domain::http::chunked::ChunkedDecoder;
use lanstash_domain::http::{Limits, parse_response_head, request_head};
use lanstash_ports::web::{Web, WebError, WebRequest, WebResponse};
use rustls::{ClientConnection, StreamOwned};

use crate::web_tls::{self, Seen};

/// The client.
#[derive(Debug, Clone, Copy)]
pub struct HttpWeb {
    pub connect_timeout: Duration,
    /// Per read or write.
    pub io_timeout: Duration,
    /// The largest response body read.
    pub max_body: usize,
}

impl Default for HttpWeb {
    fn default() -> Self {
        Self { connect_timeout: Duration::from_secs(5), io_timeout: Duration::from_secs(15), max_body: 16 << 20 }
    }
}

/// A connection, plain or over TLS.
trait Stream: Read + Write {}
impl Stream for TcpStream {}
impl Stream for StreamOwned<ClientConnection, TcpStream> {}

impl HttpWeb {
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream, String> {
        let addrs = (host, port).to_socket_addrs().map_err(|e| format!("resolve {host}: {e}"))?;
        let mut last = format!("{host}: no addresses");
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, self.connect_timeout) {
                Ok(s) => {
                    s.set_read_timeout(Some(self.io_timeout)).map_err(|e| e.to_string())?;
                    s.set_write_timeout(Some(self.io_timeout)).map_err(|e| e.to_string())?;
                    return Ok(s);
                }
                Err(e) => last = format!("connect {addr} ({host}): {e}"),
            }
        }
        Err(last)
    }
}

/// Reads more of the stream into `buf`; false at its end.
fn more(stream: &mut dyn Stream, buf: &mut Vec<u8>) -> std::io::Result<bool> {
    let mut tmp = [0u8; 8192];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => return Ok(false),
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                return Ok(true);
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            // A TLS server that closes without its close notice: the end, as plain TCP has it.
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(e),
        }
    }
}

/// Whether an I/O failure is TLS refusing the server's certificate.
fn refused_certificate(e: &std::io::Error) -> bool {
    e.get_ref()
        .and_then(|i| i.downcast_ref::<rustls::Error>())
        .is_some_and(|r| matches!(r, rustls::Error::InvalidCertificate(_) | rustls::Error::General(_)))
}

impl HttpWeb {
    fn exchange(&self, stream: &mut dyn Stream, req: &WebRequest<'_>) -> Result<WebResponse, std::io::Error> {
        let other = |m: String| std::io::Error::other(m);
        let host = format!("{}:{}", req.host, req.port);
        let length = req.body.len().to_string();
        let mut headers: Vec<(&str, &str)> = vec![("Host", &host)];
        headers.extend(
            req.headers
                .iter()
                .filter(|(n, _)| !["host", "content-length", "connection"].iter().any(|x| n.eq_ignore_ascii_case(x))),
        );
        headers.push(("Content-Length", &length));
        headers.push(("Connection", "close"));
        let mut bytes = request_head(req.method, req.target, &headers);
        bytes.extend_from_slice(req.body);
        stream.write_all(&bytes)?;
        stream.flush()?;
        let mut buf = Vec::with_capacity(8192);
        let (head, used) = loop {
            match parse_response_head(&buf, Limits::default()).map_err(|e| other(format!("bad response head: {e}")))? {
                Some((h, used)) if (100..200).contains(&h.status) => {
                    buf.drain(..used);
                }
                Some(found) => break found,
                None => {
                    if !more(stream, &mut buf)? {
                        return Err(other("connection closed before the response".into()));
                    }
                }
            }
        };
        buf.drain(..used);
        let too_large = || other(format!("response larger than {} bytes", self.max_body));
        let cut = || other("connection closed inside the body".into());
        let body = if req.method == "HEAD" || head.bodyless() {
            Vec::new()
        } else if head.is_chunked() {
            let mut d = ChunkedDecoder::default();
            let mut out = Vec::new();
            loop {
                let n = d.decode(&buf, &mut out).map_err(|e| other(format!("chunked body: {e}")))?;
                buf.drain(..n);
                if d.is_done() {
                    break out;
                }
                if out.len() > self.max_body {
                    return Err(too_large());
                }
                if !more(stream, &mut buf)? {
                    return Err(cut());
                }
            }
        } else if let Some(n) = head.content_length() {
            let n = usize::try_from(n).map_err(|_| too_large())?;
            if n > self.max_body {
                return Err(too_large());
            }
            while buf.len() < n {
                if !more(stream, &mut buf)? {
                    return Err(cut());
                }
            }
            buf.truncate(n);
            buf
        } else {
            while more(stream, &mut buf)? {
                if buf.len() > self.max_body {
                    return Err(too_large());
                }
            }
            buf
        };
        Ok(WebResponse { status: head.status, body })
    }
}

impl Web for HttpWeb {
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        let tcp = self.connect(req.host, req.port)?;
        let (mut stream, seen): (Box<dyn Stream>, Option<Seen>) = match req.tls {
            None => (Box::new(tcp), None),
            Some(tls) => {
                let (conn, seen) = web_tls::client(req.host, tls.pin, tls.unchecked)?;
                (Box::new(StreamOwned::new(conn, tcp)), Some(seen))
            }
        };
        self.exchange(stream.as_mut(), req).map_err(|e| {
            let untrusted = if refused_certificate(&e) {
                seen.and_then(|s| s.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone())
            } else {
                None
            };
            let what = if untrusted.is_some() { "its certificate is not trusted" } else { "" };
            let message =
                if what.is_empty() { format!("{}: {e}", req.host) } else { format!("{}: {what}: {e}", req.host) };
            WebError { message, untrusted }
        })
    }
}
