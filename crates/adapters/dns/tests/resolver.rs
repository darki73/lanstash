//! Tests for `lanstash_adapter_dns::resolver`.

use lanstash_adapter_dns::resolver::*;
use lanstash_ports::clock::Clock;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Fake DNS server answering every A query with 10.0.0.<n> (n = queries so far), TTL 60.
fn fake_dns() -> (SocketAddr, Arc<AtomicU64>) {
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = sock.local_addr().unwrap();
    let count = Arc::new(AtomicU64::new(0));
    let c = Arc::clone(&count);
    std::thread::spawn(move || {
        let mut buf = [0u8; 512];
        while let Ok((n, from)) = sock.recv_from(&mut buf) {
            let k = c.fetch_add(1, Ordering::SeqCst) + 1;
            let mut r = buf[..n].to_vec();
            r[2] |= 0x80; // QR
            r[7] = 1; // ANCOUNT
            r.extend_from_slice(&[0xC0, 12, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 10, 0, 0, k as u8]);
            let _ = sock.send_to(&r, from);
        }
    });
    (addr, count)
}

#[test]
fn resolves_caches_and_expires() {
    let (server, count) = fake_dns();
    let clock = Arc::new(TestClock(AtomicU64::new(1_000_000)));
    let cfg = ResolverConfig { servers: vec![server], ..ResolverConfig::default() };
    let r = UdpResolver::new(cfg, clock.clone());
    assert_eq!(r.resolve("Lancache.SteamContent.com").unwrap(), vec![Ipv4Addr::new(10, 0, 0, 1)]);
    assert_eq!(r.resolve("lancache.steamcontent.com.").unwrap(), vec![Ipv4Addr::new(10, 0, 0, 1)], "cached");
    assert_eq!(count.load(Ordering::SeqCst), 1);
    clock.0.fetch_add(61_000, Ordering::SeqCst);
    assert_eq!(r.resolve("lancache.steamcontent.com").unwrap(), vec![Ipv4Addr::new(10, 0, 0, 2)], "expired");
    assert_eq!(r.resolve("192.0.2.7").unwrap(), vec![Ipv4Addr::new(192, 0, 2, 7)], "IP literal, no query");
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn unreachable_server_times_out() {
    // A bound socket that never answers.
    let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
    let cfg = ResolverConfig {
        servers: vec![silent.local_addr().unwrap()],
        timeout: Duration::from_millis(200),
        ..ResolverConfig::default()
    };
    let r = UdpResolver::new(cfg, Arc::new(TestClock(AtomicU64::new(0))));
    assert!(r.resolve("example.com").is_err());
}

/// A server that answers like [`fake_dns`] until `answering` is cleared, then stays silent.
fn switchable_dns() -> (SocketAddr, Arc<AtomicU64>, Arc<std::sync::atomic::AtomicBool>) {
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = sock.local_addr().unwrap();
    let count = Arc::new(AtomicU64::new(0));
    let answering = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let (c, on) = (Arc::clone(&count), Arc::clone(&answering));
    std::thread::spawn(move || {
        let mut buf = [0u8; 512];
        while let Ok((n, from)) = sock.recv_from(&mut buf) {
            let k = c.fetch_add(1, Ordering::SeqCst) + 1;
            if !on.load(Ordering::SeqCst) {
                continue;
            }
            std::thread::sleep(Duration::from_millis(20)); // slow enough for queries to overlap
            let mut r = buf[..n].to_vec();
            r[2] |= 0x80;
            r[7] = 1;
            r.extend_from_slice(&[0xC0, 12, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4, 10, 0, 0, k as u8]);
            let _ = sock.send_to(&r, from);
        }
    });
    (addr, count, answering)
}

#[test]
fn many_at_once_ask_once() {
    let (server, count, _) = switchable_dns();
    let cfg = ResolverConfig { servers: vec![server], ..ResolverConfig::default() };
    let r = Arc::new(UdpResolver::new(cfg, Arc::new(TestClock(AtomicU64::new(1_000_000)))));
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let r = Arc::clone(&r);
            std::thread::spawn(move || r.resolve("level3.blizzard.com").unwrap())
        })
        .collect();
    for t in threads {
        assert_eq!(t.join().unwrap(), vec![Ipv4Addr::new(10, 0, 0, 1)]);
    }
    assert_eq!(count.load(Ordering::SeqCst), 1, "one query for sixteen downloads");
}

#[test]
fn a_silent_server_is_asked_again_then_the_last_answer_is_used() {
    let (server, count, answering) = switchable_dns();
    let clock = Arc::new(TestClock(AtomicU64::new(1_000_000)));
    let cfg = ResolverConfig {
        servers: vec![server],
        timeout: Duration::from_millis(100),
        stale_secs: 600,
        ..ResolverConfig::default()
    };
    let r = UdpResolver::new(cfg, clock.clone());
    assert_eq!(r.resolve("eu.cdn.blizzard.com").unwrap(), vec![Ipv4Addr::new(10, 0, 0, 1)]);
    answering.store(false, Ordering::SeqCst);
    clock.0.fetch_add(61_000, Ordering::SeqCst);
    assert_eq!(r.resolve("eu.cdn.blizzard.com").unwrap(), vec![Ipv4Addr::new(10, 0, 0, 1)], "stale");
    assert_eq!(count.load(Ordering::SeqCst), 3, "the first query, then two attempts");
    clock.0.fetch_add(700_000, Ordering::SeqCst);
    assert!(r.resolve("eu.cdn.blizzard.com").is_err(), "too old to use");
    assert!(r.resolve("never.example").is_err());
}
