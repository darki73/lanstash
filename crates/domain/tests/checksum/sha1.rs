//! Tests for `lanstash_domain::checksum::sha1`: FIPS 180 and RFC 6455 / RFC 4648 vectors.

use lanstash_domain::checksum::sha1::{base64, sha1};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn fips_vectors() {
    assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
    assert_eq!(
        hex(&sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
        "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
    );
    let million = vec![b'a'; 1_000_000];
    assert_eq!(hex(&sha1(&million)), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
    // Block edges: 55, 56, 63, 64 and 65 bytes.
    for n in [55, 56, 63, 64, 65] {
        assert_eq!(sha1(&vec![1u8; n]).len(), 20);
    }
}

#[test]
fn base64_vectors() {
    for (raw, enc) in
        [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("foobar", "Zm9vYmFy")]
    {
        assert_eq!(base64(raw.as_bytes()), enc);
    }
}

#[test]
fn websocket_accept_key() {
    // RFC 6455 section 1.3.
    let key = "dGhlIHNhbXBsZSBub25jZQ==258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    assert_eq!(base64(&sha1(key.as_bytes())), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}
