//! Tests for `lanstash_domain::tls`.

use lanstash_domain::tls::*;

/// A minimal ClientHello with the given extensions, split into records of `record_max` bytes.
fn hello(exts: &[(u16, Vec<u8>)], record_max: usize) -> Vec<u8> {
    let mut body = vec![0x03, 0x03];
    body.extend_from_slice(&[7u8; 32]);
    body.push(0); // session id
    body.extend_from_slice(&[0, 2, 0x13, 0x01]); // one cipher suite
    body.extend_from_slice(&[1, 0]); // null compression
    let mut ext_bytes = Vec::new();
    for (ty, data) in exts {
        ext_bytes.extend_from_slice(&ty.to_be_bytes());
        ext_bytes.extend_from_slice(&(data.len() as u16).to_be_bytes());
        ext_bytes.extend_from_slice(data);
    }
    body.extend_from_slice(&(ext_bytes.len() as u16).to_be_bytes());
    body.extend_from_slice(&ext_bytes);
    let mut hs = vec![0x01];
    hs.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
    hs.extend_from_slice(&body);
    let mut out = Vec::new();
    for frag in hs.chunks(record_max) {
        out.extend_from_slice(&[0x16, 0x03, 0x01]);
        out.extend_from_slice(&(frag.len() as u16).to_be_bytes());
        out.extend_from_slice(frag);
    }
    out
}

fn sni_ext(host: &str) -> (u16, Vec<u8>) {
    let mut entry = vec![0];
    entry.extend_from_slice(&(host.len() as u16).to_be_bytes());
    entry.extend_from_slice(host.as_bytes());
    let mut data = (entry.len() as u16).to_be_bytes().to_vec();
    data.extend(entry);
    (0, data)
}

#[test]
fn finds_the_host_in_one_or_many_records() {
    for record_max in [16_384, 50, 7] {
        let h = hello(&[(10, vec![0, 2, 0, 29]), sni_ext("Level3.Blizzard.com")], record_max);
        assert_eq!(parse_client_hello(&h), Sni::Host("level3.blizzard.com".into()), "records of {record_max}");
        for cut in [0, 3, 5, h.len() / 2, h.len() - 1] {
            assert_eq!(parse_client_hello(&h[..cut]), Sni::NeedMore, "cut {cut} of records of {record_max}");
        }
    }
}

#[test]
fn no_name_and_not_tls() {
    assert_eq!(parse_client_hello(&hello(&[(10, vec![0, 2, 0, 29])], 16_384)), Sni::NoName);
    assert_eq!(parse_client_hello(b"GET / HTTP/1.1\r\n\r\n"), Sni::NotTls);
    assert_eq!(parse_client_hello(&[0x16, 0x03, 0x01, 0x00, 0x05, 0x02]), Sni::NotTls, "not a ClientHello");
    assert_eq!(parse_client_hello(&hello(&[sni_ext("bad host!")], 16_384)), Sni::NotTls);
    let mut truncated = hello(&[sni_ext("x.example")], 16_384);
    truncated[7] = 0xFF; // handshake length (bytes 6..9) now claims more than the record holds
    assert_eq!(parse_client_hello(&truncated), Sni::NeedMore);
}
