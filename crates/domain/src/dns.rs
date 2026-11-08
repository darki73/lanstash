//! Minimal DNS (RFC 1035) for resolving CDN host names over UDP: build an
//! A query, parse the response (A records, following CNAME chains, with name
//! compression); and for naming a client by its address (a PTR query). Pure
//! byte handling.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::{self, Write as _};
use core::net::{IpAddr, Ipv4Addr};

pub const TYPE_A: u16 = 1;
pub const TYPE_CNAME: u16 = 5;
pub const TYPE_PTR: u16 = 12;
const CLASS_IN: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsError {
    BadName,
    Truncated,
    /// Response id/flags don't match our query.
    NotOurResponse,
    /// Server returned an error code (RCODE).
    ServerError(u8),
    /// The response was truncated (TC bit); it is not retried over TCP.
    TruncatedResponse,
    Malformed(&'static str),
}

impl fmt::Display for DnsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadName => write!(f, "invalid host name"),
            Self::Truncated => write!(f, "DNS message truncated"),
            Self::NotOurResponse => write!(f, "DNS response does not match the query"),
            Self::ServerError(c) => write!(f, "DNS server error (rcode {c})"),
            Self::TruncatedResponse => write!(f, "DNS response has TC bit set"),
            Self::Malformed(w) => write!(f, "malformed DNS response: {w}"),
        }
    }
}

impl core::error::Error for DnsError {}

/// An IPv4 address with its TTL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ARecord {
    pub addr: Ipv4Addr,
    pub ttl_secs: u32,
}

/// Builds a recursive query for `name`.
///
/// # Errors
/// Empty name, empty label or label longer than 63 bytes.
pub fn build_query(id: u16, name: &str, qtype: u16) -> Result<Vec<u8>, DnsError> {
    let name = name.trim_end_matches('.');
    if name.is_empty() || name.len() > 253 {
        return Err(DnsError::BadName);
    }
    let mut q = Vec::with_capacity(18 + name.len());
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&0x0100u16.to_be_bytes()); // RD
    q.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    q.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // AN, NS, AR
    for label in name.split('.') {
        let len = u8::try_from(label.len()).map_err(|_| DnsError::BadName)?;
        if len == 0 || len > 63 || !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            return Err(DnsError::BadName);
        }
        q.push(len);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&qtype.to_be_bytes());
    q.extend_from_slice(&CLASS_IN.to_be_bytes());
    Ok(q)
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, DnsError> {
    b.get(at..at + 2).map(|s| u16::from_be_bytes([s[0], s[1]])).ok_or(DnsError::Truncated)
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, DnsError> {
    b.get(at..at + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]])).ok_or(DnsError::Truncated)
}

/// Reads a (possibly compressed) name at `at`; returns it lower-cased and the
/// offset just after the name in the original position.
fn read_name(b: &[u8], at: usize) -> Result<(String, usize), DnsError> {
    name_at(b, at, true)
}

/// As [`read_name`], lower-cased or as the message has it.
fn name_at(b: &[u8], at: usize, lower: bool) -> Result<(String, usize), DnsError> {
    let mut name = String::new();
    let mut pos = at;
    let mut end_after: Option<usize> = None;
    for _ in 0..128 {
        let len = *b.get(pos).ok_or(DnsError::Truncated)?;
        match len {
            0 => {
                return Ok((name, end_after.unwrap_or(pos + 1)));
            }
            l if l & 0xC0 == 0xC0 => {
                let ptr = usize::from(u16_at(b, pos)? & 0x3FFF);
                end_after.get_or_insert(pos + 2);
                if ptr >= b.len() {
                    return Err(DnsError::Malformed("compression pointer out of range"));
                }
                pos = ptr;
            }
            l if l & 0xC0 == 0 => {
                let l = usize::from(l);
                let label = b.get(pos + 1..pos + 1 + l).ok_or(DnsError::Truncated)?;
                if !name.is_empty() {
                    name.push('.');
                }
                for &c in label {
                    name.push(char::from(if lower { c.to_ascii_lowercase() } else { c }));
                }
                pos += 1 + l;
            }
            _ => return Err(DnsError::Malformed("label type")),
        }
    }
    Err(DnsError::Malformed("name too long or pointer loop"))
}

/// One record of a response's answer section: its owner (lower-cased), type, TTL, and where
/// its data is in the message and how long.
struct Answer {
    owner: String,
    rtype: u16,
    ttl: u32,
    rdata: usize,
    rdlen: usize,
}

/// The answer section of a response to a query with `id`.
fn answers(b: &[u8], id: u16) -> Result<Vec<Answer>, DnsError> {
    if b.len() < 12 {
        return Err(DnsError::Truncated);
    }
    let flags = u16_at(b, 2)?;
    if u16_at(b, 0)? != id || flags & 0x8000 == 0 {
        return Err(DnsError::NotOurResponse);
    }
    if flags & 0x0200 != 0 {
        return Err(DnsError::TruncatedResponse);
    }
    let rcode = (flags & 0x000F) as u8;
    if rcode != 0 {
        return Err(DnsError::ServerError(rcode));
    }
    let qd = u16_at(b, 4)?;
    let an = u16_at(b, 6)?;
    let mut pos = 12;
    for _ in 0..qd {
        let (_, after) = read_name(b, pos)?;
        pos = after + 4;
    }
    let mut out = Vec::new();
    for _ in 0..an {
        let (owner, after) = read_name(b, pos)?;
        let rtype = u16_at(b, after)?;
        let ttl = u32_at(b, after + 4)?;
        let rdlen = usize::from(u16_at(b, after + 8)?);
        let rdata = after + 10;
        if b.len() < rdata + rdlen {
            return Err(DnsError::Truncated);
        }
        out.push(Answer { owner, rtype, ttl, rdata, rdlen });
        pos = rdata + rdlen;
    }
    Ok(out)
}

/// The name a reverse (PTR) query for `ip` asks about: `4.3.2.1.in-addr.arpa` for `1.2.3.4`,
/// the address's 32 nibbles backwards under `ip6.arpa` for an IPv6 one.
#[must_use]
pub fn reverse_name(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, d] = v4.octets();
            format!("{d}.{c}.{b}.{a}.in-addr.arpa")
        }
        IpAddr::V6(v6) => {
            let mut s = String::with_capacity(72);
            for byte in v6.octets().iter().rev() {
                let _ = write!(s, "{:x}.{:x}.", byte & 0xF, byte >> 4);
            }
            s.push_str("ip6.arpa");
            s
        }
    }
}

/// The host names of a response to a PTR query built by [`build_query`] with `id`, as the
/// server wrote them (case kept, no trailing dot), and the least TTL among them. A name the
/// server does not know (NXDOMAIN) is no names, not an error.
///
/// # Errors
/// Mismatched id, another server error, truncation or malformed data.
pub fn parse_ptr_response(b: &[u8], id: u16) -> Result<(Vec<String>, u32), DnsError> {
    let all = match answers(b, id) {
        Err(DnsError::ServerError(3)) => return Ok((Vec::new(), 0)),
        r => r?,
    };
    let mut names = Vec::new();
    let mut ttl = u32::MAX;
    for a in all.iter().filter(|a| a.rtype == TYPE_PTR) {
        let (name, _) = name_at(b, a.rdata, false)?;
        if !name.is_empty() {
            names.push(name);
            ttl = ttl.min(a.ttl);
        }
    }
    Ok((names, if ttl == u32::MAX { 0 } else { ttl }))
}

/// Parses a response to a query built by [`build_query`] with `id` for `name`.
/// Returns the A records for `name`, following CNAMEs within the answer.
///
/// # Errors
/// Mismatched id, server error, truncation or malformed data.
pub fn parse_a_response(b: &[u8], id: u16, name: &str) -> Result<Vec<ARecord>, DnsError> {
    let mut cnames: Vec<(String, String)> = Vec::new();
    let mut a: Vec<(String, ARecord)> = Vec::new();
    for Answer { owner, rtype, ttl, rdata, rdlen } in answers(b, id)? {
        match rtype {
            TYPE_A if rdlen == 4 => {
                let addr = Ipv4Addr::new(b[rdata], b[rdata + 1], b[rdata + 2], b[rdata + 3]);
                a.push((owner, ARecord { addr, ttl_secs: ttl }));
            }
            TYPE_CNAME => {
                let (target, _) = read_name(b, rdata)?;
                cnames.push((owner, target));
            }
            _ => {}
        }
    }
    // Follow the CNAME chain from the queried name.
    let mut current = name.trim_end_matches('.').to_ascii_lowercase();
    for _ in 0..16 {
        let found: Vec<ARecord> = a.iter().filter(|(o, _)| *o == current).map(|(_, r)| *r).collect();
        if !found.is_empty() {
            return Ok(found);
        }
        match cnames.iter().find(|(o, _)| *o == current) {
            Some((_, t)) => current = t.clone(),
            None => break,
        }
    }
    Ok(Vec::new())
}
