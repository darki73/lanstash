//! What the network calls an address: a reverse (PTR) query to the LAN's own DNS
//! servers. The machine's resolver (`/etc/resolv.conf`) is the one that knows the
//! LAN's names (a router or a DNS server handing out addresses); the upstream
//! servers lanstash resolves CDN names through are public and do not.

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;

use lanstash_domain::dns::{DnsError, TYPE_PTR, build_query, parse_ptr_response, reverse_name};
use lanstash_ports::clients::ReverseNames;

use crate::udp::exchange;

/// The `nameserver` lines of a `resolv.conf`, as servers on port 53, in order.
#[must_use]
pub fn nameservers(resolv_conf: &str) -> Vec<SocketAddr> {
    resolv_conf
        .lines()
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            (words.next() == Some("nameserver")).then(|| words.next()).flatten()
        })
        // `fe80::1%eth0`: the zone is left out; such a server is rarely the LAN's.
        .filter_map(|a| a.split('%').next()?.parse::<IpAddr>().ok())
        .map(|ip| SocketAddr::new(ip, 53))
        .collect()
}

/// Asks `servers` (in order, each once) what an address is called.
#[derive(Debug)]
pub struct PtrResolver {
    servers: Vec<SocketAddr>,
    timeout: Duration,
    next_id: AtomicU16,
}

impl PtrResolver {
    #[must_use]
    pub fn new(servers: Vec<SocketAddr>, timeout: Duration) -> Self {
        Self { servers, timeout, next_id: AtomicU16::new(0x2F1D) }
    }

    fn ask(&self, server: SocketAddr, name: &str) -> Result<(Vec<String>, u32), String> {
        let id = self.next_id.fetch_add(0x9E37, Ordering::Relaxed);
        let q = build_query(id, name, TYPE_PTR).map_err(|e| e.to_string())?;
        exchange(server, &q, self.timeout, |b| match parse_ptr_response(b, id) {
            Err(DnsError::NotOurResponse) => None,
            r => Some(r.map_err(|e| format!("{name}: {e}"))),
        })
    }
}

impl ReverseNames for PtrResolver {
    fn names_of(&self, ip: IpAddr) -> Result<(Vec<String>, u32), String> {
        let name = reverse_name(ip.to_canonical());
        let mut last = String::from("no DNS servers to ask");
        for &server in &self.servers {
            match self.ask(server, &name) {
                Ok(found) => return Ok(found),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
}
