//! HTTP/1.1 client for origin servers, with keep-alive connection reuse.
//!
//! A connection goes back to the per-host idle pool only after its response
//! body was read completely and the server agreed to keep it open. A request
//! on a reused connection that fails before any response byte arrived (the
//! server closed the idle connection) is retried once on a fresh connection.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use lanstash_domain::http::chunked::ChunkedDecoder;
use lanstash_domain::http::{Limits, ResponseHeadIn, parse_response_head, request_head};
use lanstash_ports::upstream::{Upstream, UpstreamError, UpstreamRequest, UpstreamResponse};

use lanstash_adapter_dns::resolver::Resolve;

/// Client settings.
#[derive(Debug, Clone, Copy)]
pub struct UpstreamConfig {
    pub port: u16,
    pub connect_timeout: Duration,
    /// Per read/write; a stalled origin fails after this long.
    pub io_timeout: Duration,
    pub max_idle_per_host: usize,
    pub limits: Limits,
}

impl Default for UpstreamConfig {
    fn default() -> Self {
        Self {
            port: 80,
            connect_timeout: Duration::from_secs(5),
            io_timeout: Duration::from_secs(30),
            max_idle_per_host: 32,
            limits: Limits::default(),
        }
    }
}

type IdlePool = Arc<Mutex<HashMap<String, Vec<TcpStream>>>>;

/// The client.
pub struct HttpUpstream {
    resolver: Arc<dyn Resolve>,
    cfg: UpstreamConfig,
    idle: IdlePool,
}

impl std::fmt::Debug for HttpUpstream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpUpstream").field("cfg", &self.cfg).finish_non_exhaustive()
    }
}

fn err(e: impl std::fmt::Display) -> UpstreamError {
    UpstreamError::new(e.to_string())
}

/// A socket's failure as `what: e`: a timeout when it timed out (a socket with a read timeout says
/// `WouldBlock` on some systems: "Resource temporarily unavailable").
fn io_err(what: &str, e: &std::io::Error) -> UpstreamError {
    if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) {
        UpstreamError::timeout(format!("{what}: timed out"))
    } else {
        UpstreamError::new(format!("{what}: {e}"))
    }
}

/// A failure, and whether a fresh connection might succeed.
struct Attempt {
    error: UpstreamError,
    retry: bool,
}

impl HttpUpstream {
    #[must_use]
    pub fn new(resolver: Arc<dyn Resolve>, cfg: UpstreamConfig) -> Self {
        Self { resolver, cfg, idle: Arc::new(Mutex::new(HashMap::new())) }
    }

    fn take_idle(&self, host: &str) -> Option<TcpStream> {
        self.idle.lock().unwrap_or_else(PoisonError::into_inner).get_mut(host)?.pop()
    }

    fn connect(&self, host: &str, port: u16) -> Result<TcpStream, UpstreamError> {
        let addrs = self.resolver.resolve(host).map_err(|e| err(format!("resolve {host}: {e}")))?;
        let mut last = err(format!("{host}: no addresses"));
        for ip in addrs {
            let addr = SocketAddr::from((ip, port));
            match TcpStream::connect_timeout(&addr, self.cfg.connect_timeout) {
                Ok(s) => {
                    s.set_read_timeout(Some(self.cfg.io_timeout)).map_err(err)?;
                    s.set_write_timeout(Some(self.cfg.io_timeout)).map_err(err)?;
                    s.set_nodelay(true).map_err(err)?;
                    return Ok(s);
                }
                Err(e) => last = io_err(&format!("connect {addr} ({host})"), &e),
            }
        }
        Err(last)
    }

    fn exchange(&self, mut stream: TcpStream, req: &UpstreamRequest<'_>, reused: bool) -> Result<Body, Attempt> {
        let mut headers: Vec<(&str, &str)> = Vec::with_capacity(req.headers.len() + 2);
        let host = pool_key(req);
        headers.push(("Host", &host));
        headers.extend(req.headers.iter().filter(|(n, _)| !n.eq_ignore_ascii_case("host")));
        headers.push(("Connection", "keep-alive"));
        let bytes = request_head(req.method, req.target, &headers);
        let fail = |error: UpstreamError, got_any: bool| Attempt { error, retry: reused && !got_any };
        if let Err(e) = stream.write_all(&bytes) {
            return Err(fail(io_err("send", &e), false));
        }
        let mut buf = Vec::with_capacity(8192);
        let mut tmp = [0u8; 8192];
        loop {
            match parse_response_head(&buf, self.cfg.limits) {
                Ok(Some((head, used))) if (100..200).contains(&head.status) => {
                    buf.drain(..used); // interim response (100 Continue): wait for the real one
                }
                Ok(Some((head, used))) => {
                    buf.drain(..used);
                    let framing = if req.method == "HEAD" || head.bodyless() {
                        Framing::Length(0)
                    } else if head.is_chunked() {
                        Framing::Chunked(ChunkedDecoder::default())
                    } else if let Some(n) = head.content_length() {
                        Framing::Length(n)
                    } else {
                        Framing::UntilClose
                    };
                    let reusable = head.keep_alive() && !matches!(framing, Framing::UntilClose);
                    return Ok(Body {
                        head,
                        stream: Some(stream),
                        pending: buf,
                        decoded: Vec::new(),
                        decoded_pos: 0,
                        framing,
                        reusable,
                        host: pool_key(req),
                        idle: Arc::clone(&self.idle),
                        max_idle: self.cfg.max_idle_per_host,
                    });
                }
                Ok(None) => {}
                Err(e) => return Err(fail(err(format!("bad response head: {e}")), true)),
            }
            match stream.read(&mut tmp) {
                Ok(0) => return Err(fail(err("connection closed before the response"), !buf.is_empty())),
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(fail(io_err("receive", &e), !buf.is_empty())),
            }
        }
    }
}

/// The `Host` header and idle-pool key: the host, with the port when one is asked for.
fn pool_key(req: &UpstreamRequest<'_>) -> String {
    match req.port {
        Some(p) => format!("{}:{p}", req.host),
        None => req.host.to_owned(),
    }
}

impl Upstream for HttpUpstream {
    fn fetch(&self, req: &UpstreamRequest<'_>) -> Result<Box<dyn UpstreamResponse>, UpstreamError> {
        if let Some(s) = self.take_idle(&pool_key(req)) {
            match self.exchange(s, req, true) {
                Ok(b) => return Ok(Box::new(b)),
                Err(a) if a.retry => {}
                Err(a) => return Err(a.error),
            }
        }
        let s = self.connect(req.host, req.port.unwrap_or(self.cfg.port))?;
        self.exchange(s, req, false).map(|b| Box::new(b) as Box<dyn UpstreamResponse>).map_err(|a| a.error)
    }
}

enum Framing {
    /// Bytes of body still to come.
    Length(u64),
    Chunked(ChunkedDecoder),
    UntilClose,
}

struct Body {
    head: ResponseHeadIn,
    /// `None` once the body is complete (returned to the pool or dropped).
    stream: Option<TcpStream>,
    /// Raw bytes received but not yet consumed.
    pending: Vec<u8>,
    /// Decoded bytes not yet handed out (chunked only).
    decoded: Vec<u8>,
    decoded_pos: usize,
    framing: Framing,
    reusable: bool,
    host: String,
    idle: IdlePool,
    max_idle: usize,
}

impl Body {
    /// Raw bytes: buffered first, then from the socket. `Ok(0)` = peer closed.
    fn raw(&mut self, buf: &mut [u8]) -> Result<usize, UpstreamError> {
        if !self.pending.is_empty() {
            let n = buf.len().min(self.pending.len());
            buf[..n].copy_from_slice(&self.pending[..n]);
            self.pending.drain(..n);
            return Ok(n);
        }
        let Some(s) = self.stream.as_mut() else { return Ok(0) };
        loop {
            match s.read(buf) {
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(io_err("receive body", &e)),
            }
        }
    }

    /// The body is complete: keep the connection if it can serve another request.
    fn complete(&mut self) {
        let Some(s) = self.stream.take() else { return };
        if self.reusable && self.pending.is_empty() {
            let mut idle = self.idle.lock().unwrap_or_else(PoisonError::into_inner);
            let list = idle.entry(std::mem::take(&mut self.host)).or_default();
            if list.len() < self.max_idle {
                list.push(s);
            }
        }
    }
}

impl UpstreamResponse for Body {
    fn head(&self) -> &ResponseHeadIn {
        &self.head
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, UpstreamError> {
        if buf.is_empty() {
            return Ok(0);
        }
        match &mut self.framing {
            Framing::Length(0) => {
                self.complete();
                Ok(0)
            }
            Framing::Length(left) => {
                let want = usize::try_from(*left).unwrap_or(usize::MAX).min(buf.len());
                let n = self.raw(&mut buf[..want])?;
                if n == 0 {
                    self.stream = None;
                    return Err(err("connection closed mid-body"));
                }
                if let Framing::Length(left) = &mut self.framing {
                    *left -= n as u64;
                    if *left == 0 {
                        self.complete();
                    }
                }
                Ok(n)
            }
            Framing::UntilClose => {
                let n = self.raw(buf)?;
                if n == 0 {
                    self.stream = None;
                }
                Ok(n)
            }
            Framing::Chunked(_) => loop {
                if self.decoded_pos < self.decoded.len() {
                    let n = buf.len().min(self.decoded.len() - self.decoded_pos);
                    buf[..n].copy_from_slice(&self.decoded[self.decoded_pos..self.decoded_pos + n]);
                    self.decoded_pos += n;
                    return Ok(n);
                }
                self.decoded.clear();
                self.decoded_pos = 0;
                if let Framing::Chunked(d) = &self.framing
                    && d.is_done()
                {
                    self.complete();
                    return Ok(0);
                }
                let mut raw = [0u8; 16 * 1024];
                let n = self.raw(&mut raw)?;
                if n == 0 {
                    self.stream = None;
                    return Err(err("connection closed mid-body"));
                }
                let Framing::Chunked(d) = &mut self.framing else { unreachable!() };
                let used = d.decode(&raw[..n], &mut self.decoded).map_err(|e| err(format!("chunked body: {e}")))?;
                if used < n {
                    // Bytes after the final chunk: keep them so the connection is not reused.
                    self.pending.extend_from_slice(&raw[used..n]);
                }
            },
        }
    }
}
