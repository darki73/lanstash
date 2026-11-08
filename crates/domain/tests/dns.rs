//! Tests for `lanstash_domain::dns`.

use lanstash_domain::dns::*;
use std::net::Ipv4Addr;

/// Hand-built response: question + CNAME (compressed) + 2 A records.
fn sample_response(id: u16) -> Vec<u8> {
    let mut r = build_query(id, "lancache.steamcontent.com", TYPE_A).unwrap();
    r[2] = 0x81; // QR, RD
    r[3] = 0x80; // RA
    r[7] = 3; // ANCOUNT = 3
    // CNAME: owner = pointer to question name (offset 12) → cdn.example.net
    r.extend_from_slice(&[0xC0, 12, 0, 5, 0, 1, 0, 0, 0, 60]);
    let target = [3, b'c', b'd', b'n', 7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'n', b'e', b't', 0];
    r.extend_from_slice(&(target.len() as u16).to_be_bytes());
    let target_at = r.len();
    r.extend_from_slice(&target);
    for last in [10u8, 11] {
        r.extend_from_slice(&[0xC0, target_at as u8, 0, 1, 0, 1, 0, 0, 1, 44, 0, 4, 162, 254, 197, last]);
    }
    r
}

#[test]
fn query_layout() {
    let q = build_query(0x1234, "a.bc", TYPE_A).unwrap();
    assert_eq!(q, vec![0x12, 0x34, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, b'a', 2, b'b', b'c', 0, 0, 1, 0, 1]);
    assert!(build_query(1, "", TYPE_A).is_err());
    assert!(build_query(1, "a..b", TYPE_A).is_err());
    assert!(build_query(1, "bad host", TYPE_A).is_err());
}

#[test]
fn follows_cname_to_a_records() {
    let r = sample_response(7);
    let a = parse_a_response(&r, 7, "LANCACHE.steamcontent.com.").unwrap();
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].addr, Ipv4Addr::new(162, 254, 197, 10));
    assert_eq!(a[0].ttl_secs, 300);
}

#[test]
fn rejects_foreign_and_error_responses() {
    let r = sample_response(7);
    assert_eq!(parse_a_response(&r, 8, "lancache.steamcontent.com"), Err(DnsError::NotOurResponse));
    let mut nx = r.clone();
    nx[3] = 0x83; // RCODE 3 (NXDOMAIN)
    assert_eq!(parse_a_response(&nx, 7, "lancache.steamcontent.com"), Err(DnsError::ServerError(3)));
    let mut tc = r.clone();
    tc[2] |= 0x02;
    assert_eq!(parse_a_response(&tc, 7, "lancache.steamcontent.com"), Err(DnsError::TruncatedResponse));
    assert!(parse_a_response(&r[..r.len() - 3], 7, "lancache.steamcontent.com").is_err());
}

#[test]
fn pointer_loops_are_rejected() {
    let mut r = build_query(1, "x.y", TYPE_A).unwrap();
    r[2] = 0x81;
    r[7] = 1;
    let at = r.len();
    r.extend_from_slice(&[0xC0, at as u8, 0, 1, 0, 1, 0, 0, 0, 1, 0, 4, 1, 2, 3, 4]); // name points at itself
    assert!(parse_a_response(&r, 1, "x.y").is_err());
}

/// A PTR answer for `ip` naming `host` (labels as given, case kept).
fn ptr_response(id: u16, ip: std::net::IpAddr, host: &[&str], ttl: u32) -> Vec<u8> {
    let mut r = build_query(id, &reverse_name(ip), TYPE_PTR).unwrap();
    r[2] = 0x81;
    r[3] = 0x80;
    r[7] = 1;
    let mut name = Vec::new();
    for l in host {
        name.push(l.len() as u8);
        name.extend_from_slice(l.as_bytes());
    }
    name.push(0);
    r.extend_from_slice(&[0xC0, 12, 0, 12, 0, 1]);
    r.extend_from_slice(&ttl.to_be_bytes());
    r.extend_from_slice(&(name.len() as u16).to_be_bytes());
    r.extend_from_slice(&name);
    r
}

#[test]
fn reverse_names_of_v4_and_v6_addresses() {
    assert_eq!(reverse_name("10.0.1.100".parse().unwrap()), "100.1.0.10.in-addr.arpa");
    let v6 = reverse_name("2001:db8::1".parse().unwrap());
    assert!(v6.starts_with("1.0.0.0.") && v6.ends_with(".8.b.d.0.1.0.0.2.ip6.arpa"), "{v6}");
    assert_eq!(v6.split('.').count(), 34, "32 nibbles, then ip6.arpa");
    assert!(build_query(1, &v6, TYPE_PTR).is_ok());
}

#[test]
fn a_ptr_answer_names_the_host_as_written() {
    let ip = "10.0.1.100".parse().unwrap();
    let r = ptr_response(9, ip, &["Game-PC", "lan"], 120);
    assert_eq!(parse_ptr_response(&r, 9), Ok((vec!["Game-PC.lan".to_string()], 120)));
    assert_eq!(parse_ptr_response(&r, 8), Err(DnsError::NotOurResponse));
}

#[test]
fn an_unknown_address_has_no_name() {
    let ip = "10.0.1.7".parse().unwrap();
    let mut nx = build_query(4, &reverse_name(ip), TYPE_PTR).unwrap();
    nx[2] = 0x81;
    nx[3] = 0x83; // NXDOMAIN
    assert_eq!(parse_ptr_response(&nx, 4), Ok((Vec::new(), 0)));
    let mut fail = nx.clone();
    fail[3] = 0x82; // SERVFAIL
    assert_eq!(parse_ptr_response(&fail, 4), Err(DnsError::ServerError(2)));
}
