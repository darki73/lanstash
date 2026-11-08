//! HTTPS pass-through against a fake origin on localhost.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lanstash_adapter_dns::resolver::Resolve;
use lanstash_adapter_http::tls_passthrough::{TlsConfig, TlsCounters, TlsPassthrough};
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::Clock;
use lanstash_ports::events::{Event, Events};

struct Localhost;
impl Resolve for Localhost {
    fn resolve(&self, _host: &str) -> Result<Vec<Ipv4Addr>, String> {
        Ok(vec![Ipv4Addr::LOCALHOST])
    }
}

struct WallClock;
impl Clock for WallClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap()
    }
}

#[derive(Default)]
struct Recorded(Mutex<Vec<String>>);
impl Events for Recorded {
    fn emit(&self, e: &Event<'_>) {
        self.0.lock().unwrap().push(format!("{e:?}"));
    }
}

/// A minimal ClientHello naming `host`.
fn hello(host: &str) -> Vec<u8> {
    let mut sni = vec![0];
    sni.extend_from_slice(&(host.len() as u16).to_be_bytes());
    sni.extend_from_slice(host.as_bytes());
    let mut ext = vec![0, 0];
    ext.extend_from_slice(&((sni.len() + 2) as u16).to_be_bytes());
    ext.extend_from_slice(&(sni.len() as u16).to_be_bytes());
    ext.extend(sni);
    let mut body = vec![0x03, 0x03];
    body.extend_from_slice(&[7u8; 32]);
    body.extend_from_slice(&[0, 0, 2, 0x13, 0x01, 1, 0]);
    body.extend_from_slice(&(ext.len() as u16).to_be_bytes());
    body.extend(ext);
    let mut hs = vec![0x01];
    hs.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
    hs.extend(body);
    let mut rec = vec![0x16, 0x03, 0x01];
    rec.extend_from_slice(&(hs.len() as u16).to_be_bytes());
    rec.extend(hs);
    rec
}

/// Starts the pass-through in front of an origin listening on `origin_port`.
fn start(origin_port: u16, idle: Duration) -> (SocketAddr, Arc<TlsCounters>, Arc<Recorded>) {
    start_on("127.0.0.1:0".parse().unwrap(), origin_port, idle)
}

fn start_on(listen: SocketAddr, origin_port: u16, idle: Duration) -> (SocketAddr, Arc<TlsCounters>, Arc<Recorded>) {
    let mut hosts = HostRules::default();
    hosts.add_host("blizzard", "blizzard.com");
    let mut cfg = TlsConfig::new(listen);
    cfg.upstream_port = origin_port;
    cfg.hello_timeout = Duration::from_millis(300);
    cfg.idle_timeout = idle;
    let events = Arc::new(Recorded::default());
    let p = TlsPassthrough::bind(cfg, hosts, Arc::new(Localhost), events.clone(), Arc::new(WallClock)).unwrap();
    let (addr, counters) = (p.local_addr().unwrap(), Arc::clone(&p.counters));
    std::thread::spawn(move || p.run());
    (addr, counters, events)
}

fn read_all(s: &mut TcpStream) -> Vec<u8> {
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    out
}

#[test]
fn relays_hello_and_both_directions() {
    let origin = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = origin.local_addr().unwrap().port();
    let h = hello("eu.cdn.blizzard.com");
    let expected_hello = h.clone();
    let server = std::thread::spawn(move || {
        let (mut s, _) = origin.accept().unwrap();
        let mut got = vec![0u8; expected_hello.len() + 5];
        s.read_exact(&mut got).unwrap();
        assert_eq!(&got[..expected_hello.len()], &expected_hello[..], "the ClientHello arrives unchanged");
        assert_eq!(&got[expected_hello.len()..], b"after");
        s.write_all(&vec![42u8; 300_000]).unwrap();
        s.shutdown(Shutdown::Write).unwrap();
    });
    let (addr, counters, events) = start(port, Duration::from_secs(5));
    let mut c = TcpStream::connect(addr).unwrap();
    // The hello in two pieces, then more client bytes.
    c.write_all(&h[..10]).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    c.write_all(&h[10..]).unwrap();
    c.write_all(b"after").unwrap();
    let down = read_all(&mut c);
    server.join().unwrap();
    assert_eq!(down.len(), 300_000);
    assert_eq!(counters.relayed.load(Ordering::Relaxed), 1);
    assert_eq!(counters.bytes_down.load(Ordering::Relaxed), 300_000);
    assert!(events.0.lock().unwrap().is_empty(), "{:?}", events.0.lock().unwrap());
}

#[test]
fn a_quiet_client_does_not_end_a_busy_download() {
    let origin = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = origin.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut s, _) = origin.accept().unwrap();
        let mut b = [0u8; 512];
        let _ = s.read(&mut b).unwrap();
        // 1 s of data in small steps; the client sends nothing meanwhile.
        for _ in 0..10 {
            s.write_all(&[1u8; 1000]).unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
        s.shutdown(Shutdown::Write).unwrap();
    });
    let (addr, _, _) = start(port, Duration::from_millis(300));
    let mut c = TcpStream::connect(addr).unwrap();
    c.write_all(&hello("level3.blizzard.com")).unwrap();
    assert_eq!(read_all(&mut c).len(), 10_000);
    server.join().unwrap();
}

#[test]
fn refuses_other_hosts_and_non_tls() {
    let origin = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = origin.local_addr().unwrap().port();
    let (addr, counters, events) = start(port, Duration::from_secs(5));

    let mut c = TcpStream::connect(addr).unwrap();
    c.write_all(&hello("example.org")).unwrap();
    assert!(read_all(&mut c).is_empty());
    let mut c = TcpStream::connect(addr).unwrap();
    c.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();
    assert!(read_all(&mut c).is_empty());
    let mut c = TcpStream::connect(addr).unwrap(); // says nothing: hello timeout
    assert!(read_all(&mut c).is_empty());

    assert_eq!(counters.refused.load(Ordering::Relaxed), 1);
    assert_eq!(counters.bad_hello.load(Ordering::Relaxed), 2);
    assert_eq!(counters.relayed.load(Ordering::Relaxed), 0);
    let ev = events.0.lock().unwrap();
    assert!(ev.iter().any(|e| e.contains("example.org")), "{ev:?}");
    origin.set_nonblocking(true).unwrap();
    assert!(origin.accept().is_err(), "nothing may reach the origin");
}

#[test]
fn unreachable_origin_is_an_event() {
    // A port nobody listens on.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let (addr, counters, events) = start(port, Duration::from_secs(5));
    let mut c = TcpStream::connect(addr).unwrap();
    c.write_all(&hello("eu.cdn.blizzard.com")).unwrap();
    assert!(read_all(&mut c).is_empty());
    assert_eq!(counters.connect_failed.load(Ordering::Relaxed), 1);
    assert!(events.0.lock().unwrap()[0].contains("connect"));
}

#[test]
fn a_name_that_resolves_to_lanstash_itself_is_not_relayed_in_a_loop() {
    // The origin's address and port are the pass-through's own.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let listen = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let (addr, counters, events) = start_on(listen, port, Duration::from_secs(5));
    let mut c = TcpStream::connect(addr).unwrap();
    c.write_all(&hello("eu.cdn.blizzard.com")).unwrap();
    assert!(read_all(&mut c).is_empty());
    assert_eq!((counters.connect_failed.load(Ordering::Relaxed), counters.relayed.load(Ordering::Relaxed)), (1, 0));
    assert!(events.0.lock().unwrap()[0].contains("lanstash itself"));
}

#[test]
fn a_hello_that_trickles_in_is_given_up_at_its_deadline() {
    let (addr, counters, _) = start(1, Duration::from_secs(5));
    let mut c = TcpStream::connect(addr).unwrap();
    let started = std::time::Instant::now();
    // The hello deadline is 300 ms; a byte every 50 ms would take far longer.
    for b in hello("eu.cdn.blizzard.com") {
        if c.write_all(&[b]).is_err() || started.elapsed() > Duration::from_secs(3) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = read_all(&mut c);
    assert!(started.elapsed() < Duration::from_secs(4), "{:?}", started.elapsed());
    assert_eq!(counters.bad_hello.load(Ordering::Relaxed), 1);
}
