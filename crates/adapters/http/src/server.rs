//! HTTP/1.1 server adapter: one OS thread per connection, blocking std I/O.
//! Parsing is done by `lanstash_domain::http`; requests go to a [`RequestHandler`].

use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use lanstash_ports::clock::Clock;

use lanstash_adapter_zerocopy::sendfile::{Sent, send_file};
use lanstash_app::serve::{ConnState, RequestHandler, ResponseHead, ResponseSink, SinkError};
use lanstash_domain::http::{
    HttpError, Limits, RequestHead, parse_request_head, reason_phrase, response_head, stream_response_head,
};
use lanstash_ports::storage::FileRead;

use crate::link::{self, Link};

/// One finished request, for access logging.
#[derive(Debug)]
pub struct AccessEntry<'a> {
    pub peer: Option<SocketAddr>,
    pub req: &'a RequestHead,
    /// 0 when no response head was sent.
    pub status: u16,
    /// The `X-LanStash` response header (HIT, MISS, PASS, …), if any.
    pub cache: Option<&'a str>,
    /// Body bytes written to the socket.
    pub bytes: u64,
    /// False when the response was cut off (connection closed).
    pub complete: bool,
}

/// Receives an entry for every request (e.g. a log file).
pub trait AccessLog: Send + Sync {
    fn record(&self, entry: &AccessEntry<'_>);
}

/// Server settings (passed in from the composition root).
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub listen: SocketAddr,
    pub max_connections: usize,
    /// Close a connection after this long without a complete request.
    pub idle_timeout: Duration,
    /// Close a connection whose request head is not whole this long after its first byte, however
    /// it trickles in (a byte now and then keeps no connection open for ever).
    pub head_timeout: Duration,
    /// Connections one client address may hold at once (the rest stay for the others).
    pub max_per_client: usize,
    pub write_timeout: Duration,
    pub limits: Limits,
    pub thread_stack_bytes: usize,
    /// Stored bodies go from the file to the socket by the kernel where it can (`sendfile`),
    /// without being copied through lanstash.
    pub zero_copy: bool,
}

/// A bound server, ready to run.
pub struct HttpServer {
    listener: TcpListener,
    config: ServerConfig,
    active: Arc<AtomicUsize>,
    /// Connections open now, by client address.
    per_client: Arc<Mutex<HashMap<IpAddr, usize>>>,
    access_log: Option<Arc<dyn AccessLog>>,
    /// Times a request head's deadline ([`ServerConfig::head_timeout`]); without one only the
    /// idle timeout applies.
    clock: Option<Arc<dyn Clock>>,
    /// Every connection is TLS under this configuration (HTTPS); none: plain HTTP.
    tls: Option<Arc<rustls::ServerConfig>>,
}

impl std::fmt::Debug for HttpServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpServer").field("config", &self.config).finish_non_exhaustive()
    }
}

impl HttpServer {
    /// Binds the listening socket.
    ///
    /// # Errors
    /// Address in use or not permitted.
    pub fn bind(config: ServerConfig) -> io::Result<Self> {
        let listener = TcpListener::bind(config.listen)?;
        let (active, per_client) = (Arc::new(AtomicUsize::new(0)), Arc::default());
        Ok(Self { listener, config, active, per_client, access_log: None, clock: None, tls: None })
    }

    /// Serves HTTPS: every connection is TLS under `tls` (files are then copied, not sent whole).
    #[must_use]
    pub fn with_tls(mut self, tls: Arc<rustls::ServerConfig>) -> Self {
        self.tls = Some(tls);
        self
    }

    /// Gives every request head [`ServerConfig::head_timeout`], timed by `clock`.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Records every request to `log`.
    #[must_use]
    pub fn with_access_log(mut self, log: Arc<dyn AccessLog>) -> Self {
        self.access_log = Some(log);
        self
    }

    /// The actual bound address (useful with port 0).
    ///
    /// # Errors
    /// Socket query failed.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts connections forever.
    pub fn run(&self, handler: &Arc<dyn RequestHandler>) {
        for stream in self.listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let client = stream.peer_addr().map(|p| p.ip().to_canonical()).ok();
            if self.active.load(Ordering::Relaxed) >= self.config.max_connections || !self.take(client) {
                let _ = stream.write_all(&response_head(503, "Service Unavailable", &[], 0, false));
                continue;
            }
            self.active.fetch_add(1, Ordering::Relaxed);
            let (active, per_client) = (Arc::clone(&self.active), Arc::clone(&self.per_client));
            let handler = Arc::clone(handler);
            let config = self.config.clone();
            let (log, clock, tls) = (self.access_log.clone(), self.clock.clone(), self.tls.clone());
            let spawned = std::thread::Builder::new()
                .name("lanstash-conn".into())
                .stack_size(config.thread_stack_bytes)
                .spawn(move || {
                    let (handler, log, clock) = (&*handler, log.as_deref(), clock.as_deref());
                    let peer = stream.peer_addr().ok();
                    let _ = prepare(&stream, &config).and_then(|()| {
                        let mut tls = match &tls {
                            Some(t) => link::tls(t, stream)?,
                            None => return serve_connection(&mut stream, peer, false, handler, &config, log, clock),
                        };
                        let r = serve_connection(&mut tls, peer, true, handler, &config, log, clock);
                        tls.close();
                        r
                    });
                    active.fetch_sub(1, Ordering::Relaxed);
                    give_back(&per_client, client);
                });
            if spawned.is_err() {
                self.active.fetch_sub(1, Ordering::Relaxed);
                give_back(&self.per_client, client);
            }
        }
    }
}

impl HttpServer {
    /// Counts one more connection of `client`, unless it holds [`ServerConfig::max_per_client`].
    fn take(&self, client: Option<IpAddr>) -> bool {
        let Some(ip) = client else { return true };
        let mut m = self.per_client.lock().unwrap_or_else(PoisonError::into_inner);
        let n = m.entry(ip).or_insert(0);
        if *n >= self.config.max_per_client {
            return false;
        }
        *n += 1;
        true
    }
}

fn give_back(per_client: &Mutex<HashMap<IpAddr, usize>>, client: Option<IpAddr>) {
    let Some(ip) = client else { return };
    let mut m = per_client.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(n) = m.get_mut(&ip) {
        *n -= 1;
        if *n == 0 {
            m.remove(&ip);
        }
    }
}

/// Writes responses to the connection; drops bodies for HEAD.
struct TcpSink<'a, S: Link> {
    stream: &'a mut S,
    head_only: bool,
    keep_alive: bool,
    /// A response without a length was started: close afterwards.
    streaming: bool,
    status: u16,
    cache: Option<String>,
    bytes: u64,
    zero_copy: bool,
}

impl<S: Link> ResponseSink for TcpSink<'_, S> {
    fn send_head(&mut self, head: &ResponseHead<'_>) -> Result<(), SinkError> {
        self.status = head.status;
        self.cache =
            head.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case("x-lanstash")).map(|(_, v)| (*v).to_owned());
        let bytes = response_head(head.status, head.reason, head.headers, head.content_length, self.keep_alive);
        self.stream.write_all(&bytes).map_err(|e| SinkError(e.to_string()))
    }

    fn send_stream_head(&mut self, status: u16, headers: &[(&str, &str)]) -> Result<(), SinkError> {
        self.status = status;
        self.streaming = true;
        let bytes = stream_response_head(status, reason_phrase(status), headers);
        self.stream.write_all(&bytes).map_err(|e| SinkError(e.to_string()))?;
        self.stream.flush().map_err(|e| SinkError(e.to_string()))
    }

    fn send_body(&mut self, data: &[u8]) -> Result<(), SinkError> {
        if self.head_only {
            return Ok(());
        }
        self.stream.write_all(data).map_err(|e| SinkError(e.to_string()))?;
        self.bytes += data.len() as u64;
        Ok(())
    }

    fn send_file(&mut self, file: &dyn FileRead, offset: u64, len: u64) -> Result<bool, SinkError> {
        if self.head_only {
            return Ok(true); // nothing of a HEAD answer's body is sent
        }
        let os = file.os_file().and_then(|f| f.downcast_ref::<std::fs::File>());
        let Some(os) = os.filter(|_| self.zero_copy) else { return Ok(false) };
        let Some(socket) = self.stream.socket() else { return Ok(false) };
        match send_file(socket, os, offset, len) {
            Ok(Sent::All) => {
                self.bytes += len;
                Ok(true)
            }
            Ok(Sent::Unsupported) => Ok(false),
            Err(e) => Err(SinkError(e.to_string())),
        }
    }
}

/// The socket's options for serving: no delay, the idle and write timeouts.
fn prepare(stream: &TcpStream, config: &ServerConfig) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(config.idle_timeout))?;
    stream.set_write_timeout(Some(config.write_timeout))
}

#[allow(clippy::too_many_arguments)]
fn serve_connection<S: Link>(
    stream: &mut S,
    peer: Option<SocketAddr>,
    secure: bool,
    handler: &dyn RequestHandler,
    config: &ServerConfig,
    log: Option<&dyn AccessLog>,
    clock: Option<&dyn Clock>,
) -> io::Result<()> {
    let mut inbuf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut chunk = [0u8; 8 * 1024];
    // An IPv4 client of a dual-stack socket shows as `::ffff:a.b.c.d`.
    let mut conn = ConnState::new(peer.is_some_and(|p| p.ip().to_canonical().is_loopback()));
    if let Some(p) = peer {
        conn = conn.with_client(p.ip().to_canonical());
    }
    if secure {
        conn = conn.secured();
    }
    // When (ms) the first byte of the request head being read came.
    let now = || clock.map(Clock::now_ms);
    let head_ms = u64::try_from(config.head_timeout.as_millis()).unwrap_or(u64::MAX);
    let mut head_since: Option<u64> = None;
    loop {
        // Parse as many complete requests as are buffered (pipelining), then read more.
        let parsed = parse_request_head(&inbuf, config.limits);
        let (req, used) = match parsed {
            Ok(Some(r)) => r,
            Ok(None) => {
                // Between requests the idle timeout applies; once a head started, its deadline.
                let wait = match (head_since, now()) {
                    (Some(t), Some(n)) => match head_ms.saturating_sub(n.saturating_sub(t)) {
                        0 => return Ok(()),
                        left => Duration::from_millis(left).min(config.idle_timeout),
                    },
                    _ => config.idle_timeout,
                };
                stream.set_read_timeout(Some(wait))?;
                let n = stream.read(&mut chunk)?;
                if n == 0 {
                    return Ok(());
                }
                if head_since.is_none() {
                    head_since = now();
                }
                inbuf.extend_from_slice(&chunk[..n]);
                continue;
            }
            Err(e) => {
                let status = if e == HttpError::HeadTooLarge { 431 } else { 400 };
                let reason = if status == 431 { "Request Header Fields Too Large" } else { "Bad Request" };
                stream.write_all(&response_head(status, reason, &[], 0, false))?;
                return Ok(());
            }
        };
        inbuf.drain(..used);
        // A pipelined head already buffered started arriving with this one.
        head_since = if inbuf.is_empty() { None } else { now() };
        if req.has_body() {
            // We never accept request bodies; don't try to skip them.
            stream.write_all(&response_head(400, "Bad Request", &[], 0, false))?;
            return Ok(());
        }
        let keep_alive = req.keep_alive();
        let mut sink = TcpSink {
            stream: &mut *stream,
            head_only: req.method == "HEAD",
            keep_alive,
            streaming: false,
            status: 0,
            cache: None,
            bytes: 0,
            zero_copy: config.zero_copy,
        };
        let result = handler.handle(&req, &mut conn, &mut sink);
        if let Some(log) = log {
            log.record(&AccessEntry {
                peer,
                req: &req,
                status: sink.status,
                cache: sink.cache.as_deref(),
                bytes: sink.bytes,
                complete: result.is_ok(),
            });
        }
        if result.is_err() {
            return Ok(()); // response incomplete: close the connection
        }
        if !keep_alive || sink.streaming {
            return Ok(());
        }
    }
}
