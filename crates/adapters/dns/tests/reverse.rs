//! Tests for `lanstash_adapter_dns::reverse`: a reverse query against a fake LAN
//! DNS server on loopback.

use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use lanstash_adapter_dns::reverse::{PtrResolver, nameservers};
use lanstash_ports::clients::ReverseNames;

/// A fake server: names 100.1.0.10.in-addr.arpa `Game-PC` (TTL 600), knows no other address.
fn fake_lan_dns() -> SocketAddr {
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = sock.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut buf = [0u8; 512];
        while let Ok((n, from)) = sock.recv_from(&mut buf) {
            let mut r = buf[..n].to_vec();
            r[2] |= 0x80; // QR
            let known = r.windows(4).any(|w| w == b"\x03100");
            if known {
                r[7] = 1; // ANCOUNT
                r.extend_from_slice(&[0xC0, 12, 0, 12, 0, 1, 0, 0, 2, 0x58, 0, 9, 7]);
                r.extend_from_slice(b"Game-PC\0");
            } else {
                r[3] |= 3; // NXDOMAIN
            }
            let _ = sock.send_to(&r, from);
        }
    });
    addr
}

#[test]
fn an_address_is_named_by_the_lan_server() {
    let r = PtrResolver::new(vec![fake_lan_dns()], Duration::from_secs(2));
    assert_eq!(r.names_of("10.0.1.100".parse().unwrap()), Ok((vec!["Game-PC".to_string()], 600)));
    assert_eq!(r.names_of("::ffff:10.0.1.100".parse().unwrap()).unwrap().0, ["Game-PC"], "a mapped address");
    assert_eq!(r.names_of("10.0.1.7".parse().unwrap()), Ok((Vec::new(), 0)), "unknown: no name, no error");
}

#[test]
fn a_silent_server_is_an_error_and_the_next_one_is_asked() {
    let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
    let r = PtrResolver::new(vec![silent.local_addr().unwrap()], Duration::from_millis(150));
    assert!(r.names_of("10.0.1.100".parse().unwrap()).is_err());
    let r = PtrResolver::new(vec![silent.local_addr().unwrap(), fake_lan_dns()], Duration::from_millis(150));
    assert_eq!(r.names_of("10.0.1.100".parse().unwrap()).unwrap().0, ["Game-PC"]);
    assert!(PtrResolver::new(Vec::new(), Duration::from_millis(1)).names_of("10.0.1.1".parse().unwrap()).is_err());
}

#[test]
fn nameservers_come_from_resolv_conf() {
    let conf = "# a comment\nnameserver 127.0.0.53\nsearch lan\nnameserver fe80::1%eth0\nnameserver nonsense\n options edns0\n";
    let servers = nameservers(conf);
    assert_eq!(servers, ["127.0.0.53:53".parse::<SocketAddr>().unwrap(), "[fe80::1]:53".parse().unwrap()]);
    assert!(nameservers("").is_empty());
}
