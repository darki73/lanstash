//! HTTPS pass-through by host name.
//!
//! The local DNS points CDN names at the cache, so HTTPS for those names
//! arrives here too. Nothing is decrypted: the server name is read from the
//! TLS ClientHello, checked against the cached services' host lists (so this
//! is no open relay), resolved through the upstream DNS, and the connection is
//! then relayed byte for byte, starting with the ClientHello itself.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use lanstash_ports::clock::Clock;

use lanstash_domain::service::hosts::HostRules;
use lanstash_domain::tls::{MAX_HELLO, Sni, parse_client_hello};
use lanstash_ports::events::{Event, Events};

use lanstash_adapter_dns::resolver::Resolve;

/// Settings (passed in from the composition root).
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub listen: SocketAddr,
    pub max_connections: usize,
    /// The ClientHello must arrive within this time.
    pub hello_timeout: Duration,
    pub connect_timeout: Duration,
    /// A connection with no bytes in either direction for this long is closed.
    pub idle_timeout: Duration,
    /// Origin port (443; tests use others).
    pub upstream_port: u16,
    pub thread_stack_bytes: usize,
}

impl TlsConfig {
    #[must_use]
    pub fn new(listen: SocketAddr) -> Self {
        Self {
            listen,
            max_connections: 1024,
            hello_timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(300),
            upstream_port: 443,
            thread_stack_bytes: 128 * 1024,
        }
    }
}

/// Running totals.
#[derive(Debug, Default)]
pub struct TlsCounters {
    pub connections: AtomicU64,
    pub relayed: AtomicU64,
    /// A name not on the host lists.
    pub refused: AtomicU64,
    /// Not TLS, no server name, or no ClientHello in time.
    pub bad_hello: AtomicU64,
    pub connect_failed: AtomicU64,
    pub bytes_up: AtomicU64,
    pub bytes_down: AtomicU64,
    pub active: AtomicUsize,
}

impl TlsCounters {
    /// Every counter by name.
    #[must_use]
    pub fn values(&self) -> Vec<(&'static str, u64)> {
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        vec![
            ("connections", g(&self.connections)),
            ("active", self.active.load(Ordering::Relaxed) as u64),
            ("relayed", g(&self.relayed)),
            ("refused", g(&self.refused)),
            ("bad_hello", g(&self.bad_hello)),
            ("connect_failed", g(&self.connect_failed)),
            ("bytes_up", g(&self.bytes_up)),
            ("bytes_down", g(&self.bytes_down)),
        ]
    }
}

pub struct TlsPassthrough {
    listener: TcpListener,
    cfg: TlsConfig,
    hosts: HostRules,
    resolver: Arc<dyn Resolve>,
    events: Arc<dyn Events>,
    clock: Arc<dyn Clock>,
    pub counters: Arc<TlsCounters>,
}

impl std::fmt::Debug for TlsPassthrough {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsPassthrough")
            .field("cfg", &self.cfg)
            .field("counters", &self.counters)
            .finish_non_exhaustive()
    }
}

/// What a connection needs, shared with its thread.
struct Ctx {
    cfg: TlsConfig,
    hosts: HostRules,
    resolver: Arc<dyn Resolve>,
    events: Arc<dyn Events>,
    clock: Arc<dyn Clock>,
    counters: Arc<TlsCounters>,
}

impl TlsPassthrough {
    /// Binds the listening socket.
    ///
    /// # Errors
    /// Address in use or not permitted.
    pub fn bind(
        cfg: TlsConfig,
        hosts: HostRules,
        resolver: Arc<dyn Resolve>,
        events: Arc<dyn Events>,
        clock: Arc<dyn Clock>,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind(cfg.listen)?;
        Ok(Self { listener, cfg, hosts, resolver, events, clock, counters: Arc::default() })
    }

    /// # Errors
    /// Socket query failed.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts connections forever.
    pub fn run(self) {
        let ctx = Arc::new(Ctx {
            cfg: self.cfg,
            hosts: self.hosts,
            resolver: self.resolver,
            events: self.events,
            clock: self.clock,
            counters: self.counters,
        });
        for stream in self.listener.incoming() {
            let Ok(stream) = stream else { continue };
            let c = &ctx.counters;
            c.connections.fetch_add(1, Ordering::Relaxed);
            if c.active.load(Ordering::Relaxed) >= ctx.cfg.max_connections {
                continue; // dropped: closes the connection
            }
            c.active.fetch_add(1, Ordering::Relaxed);
            let ctx2 = Arc::clone(&ctx);
            let spawned = std::thread::Builder::new()
                .name("lanstash-tls".into())
                .stack_size(ctx.cfg.thread_stack_bytes)
                .spawn(move || {
                    ctx2.connection(stream);
                    ctx2.counters.active.fetch_sub(1, Ordering::Relaxed);
                });
            if spawned.is_err() {
                c.active.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }
}

impl Ctx {
    fn fail(&self, counter: &AtomicU64, host: &str, error: &str) {
        counter.fetch_add(1, Ordering::Relaxed);
        self.events.emit(&Event::TlsFailed { host, error });
    }

    fn connection(&self, mut client: TcpStream) {
        let c = &self.counters;
        let (hello, host) = match read_hello(&mut client, self.cfg.hello_timeout, &*self.clock) {
            Ok(x) => x,
            Err(e) => return self.fail(&c.bad_hello, "-", &e),
        };
        if self.hosts.classify(&host).is_none() {
            return self.fail(&c.refused, &host, "host not on the cached services' lists");
        }
        // The address and port the client reached lanstash on: never relayed to (lanstash again).
        let own = match client.local_addr() {
            Ok(a) => match a.ip().to_canonical() {
                std::net::IpAddr::V4(ip) => Some(SocketAddr::V4(SocketAddrV4::new(ip, a.port()))),
                std::net::IpAddr::V6(_) => None,
            },
            Err(_) => None,
        };
        let upstream = match self.connect(&host, own) {
            Ok(s) => s,
            Err(e) => return self.fail(&c.connect_failed, &host, &e),
        };
        c.relayed.fetch_add(1, Ordering::Relaxed);
        // Closing an idle connection is normal, not an event.
        if let Err(e) = relay(client, upstream, &hello, self.cfg.idle_timeout, &*self.clock, c)
            && e.kind() != io::ErrorKind::TimedOut
        {
            self.events.emit(&Event::TlsFailed { host: &host, error: &e.to_string() });
        }
    }

    fn connect(&self, host: &str, own: Option<SocketAddr>) -> Result<TcpStream, String> {
        let addrs = self.resolver.resolve(host)?;
        let mut last = format!("{host}: no addresses");
        for ip in addrs.iter().take(3) {
            let addr = SocketAddr::V4(SocketAddrV4::new(*ip, self.cfg.upstream_port));
            // lanstash itself (upstream DNS answering with the rewrites): relaying there loops.
            if Some(addr) == own {
                last = format!("{host} resolves to lanstash itself: the upstream DNS has its rewrites");
                continue;
            }
            if ip.is_unspecified() || *ip == Ipv4Addr::BROADCAST {
                continue;
            }
            match TcpStream::connect_timeout(&addr, self.cfg.connect_timeout) {
                Ok(s) => return Ok(s),
                Err(e) => last = format!("connect {addr}: {e}"),
            }
        }
        Err(last)
    }
}

/// The bytes of the whole TLS records at the start of `buf` (each a 5-byte header and its length).
fn whole_records(buf: &[u8]) -> usize {
    let mut at = 0;
    while let Some(h) = buf.get(at..at + 5) {
        let end = at + 5 + usize::from(u16::from_be_bytes([h[3], h[4]]));
        if end > buf.len() {
            break;
        }
        at = end;
    }
    at
}

/// Reads until the ClientHello is complete, within `timeout` in all (however slowly it comes);
/// returns the bytes read and the host. It is looked at only when a record is whole.
fn read_hello(client: &mut TcpStream, timeout: Duration, clock: &dyn Clock) -> Result<(Vec<u8>, String), String> {
    let deadline = clock.now_ms().saturating_add(u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX));
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 4096];
    let mut parsed = 0;
    loop {
        let left = deadline.saturating_sub(clock.now_ms());
        if left == 0 {
            return Err("no ClientHello in time".into());
        }
        client.set_read_timeout(Some(Duration::from_millis(left))).map_err(|e| e.to_string())?;
        let n = match client.read(&mut chunk) {
            Ok(0) => return Err("closed before the ClientHello was complete".into()),
            Ok(n) => n,
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                return Err("no ClientHello in time".into());
            }
            Err(e) => return Err(e.to_string()),
        };
        buf.extend_from_slice(&chunk[..n]);
        let whole = whole_records(&buf);
        if whole == parsed && buf.len() < 5 + MAX_HELLO + 1024 && buf.get(..1).is_none_or(|b| b[0] == 0x16) {
            continue; // nothing new to look at yet (and what came is still a handshake record)
        }
        parsed = whole;
        match parse_client_hello(&buf) {
            Sni::Host(h) => return Ok((buf, h)),
            Sni::NoName => return Err("ClientHello without a server name".into()),
            Sni::NotTls => return Err("not a TLS ClientHello".into()),
            Sni::NeedMore if buf.len() > MAX_HELLO + 1024 => return Err("ClientHello too large".into()),
            Sni::NeedMore => {}
        }
    }
}

/// Copies both directions until both have ended or nothing moved for `idle`.
fn relay(
    client: TcpStream,
    upstream: TcpStream,
    hello: &[u8],
    idle: Duration,
    clock: &dyn Clock,
    c: &TlsCounters,
) -> io::Result<()> {
    let last = AtomicU64::new(clock.now_ms());
    // Short read timeouts so each direction can notice the other is still busy.
    let tick = idle.min(Duration::from_secs(5)).max(Duration::from_millis(10));
    for s in [&client, &upstream] {
        s.set_read_timeout(Some(tick))?;
        s.set_write_timeout(Some(idle))?;
        let _ = s.set_nodelay(true);
    }
    (&upstream).write_all(hello)?;
    c.bytes_up.fetch_add(hello.len() as u64, Ordering::Relaxed);
    let idle_ms = u64::try_from(idle.as_millis()).unwrap_or(u64::MAX);
    std::thread::scope(|s| {
        let up = s.spawn(|| pump(&client, &upstream, &last, idle_ms, clock, &c.bytes_up));
        let down = pump(&upstream, &client, &last, idle_ms, clock, &c.bytes_down);
        let up = up.join().unwrap_or_else(|_| Err(io::Error::other("relay thread panicked")));
        down.and(up)
    })
}

/// One direction. At the end of `from`, half-closes `to` so the peer sees it.
fn pump(
    from: &TcpStream,
    to: &TcpStream,
    last: &AtomicU64,
    idle_ms: u64,
    clock: &dyn Clock,
    bytes: &AtomicU64,
) -> io::Result<()> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut r = from;
    let mut w = to;
    let result = loop {
        match r.read(&mut buf) {
            Ok(0) => break Ok(()),
            Ok(n) => {
                if let Err(e) = w.write_all(&buf[..n]) {
                    break Err(e);
                }
                bytes.fetch_add(n as u64, Ordering::Relaxed);
                last.store(clock.now_ms(), Ordering::Relaxed);
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                if clock.now_ms().saturating_sub(last.load(Ordering::Relaxed)) >= idle_ms {
                    break Err(io::Error::new(io::ErrorKind::TimedOut, "idle"));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => break Err(e),
        }
    };
    // Either way this direction is over; on errors, end the other one as well.
    let _ = to.shutdown(Shutdown::Write);
    if result.is_err() {
        let _ = from.shutdown(Shutdown::Both);
        let _ = to.shutdown(Shutdown::Both);
    }
    result
}
