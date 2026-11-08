//! Tests for `lanstash_domain::inflate`, on streams made by zlib (Python's
//! `zlib.compress`) from text written for these tests.

use lanstash_domain::inflate::{InflateError, adler32, gzip, inflate, zlib};

fn unhex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn text() -> Vec<u8> {
    let mut t = b"lanstash keeps game downloads on the LAN. ".repeat(40);
    t.extend(0..=255u8);
    t
}

/// `zlib.compress(text(), 9)`: one dynamic-Huffman block.
const DYNAMIC: &str = "
78dacb49cc2b2e492cce50c84e4d2d2856484fcc4d5548c92fcfcbc94f4c2956c8cf5328c94855f071f4d353c8195539
aa7254e5a8ca5195a32a69a6928191899985958d9d83938b9b87978f5f4050485844544c5c42524a5a46564e5e415149
5945554d5d43534b5b47574fdfc0d0c8d8c4d4ccdcc2d2cadac6d6cedec1d1c9d9c5d5cdddc3d3cbdbc7d7cf3f203028
3824342c3c22322a3a26362e3e2131293925352d3d23332b3b27372fbfa0b0a8b8a4b4acbca2b2aabaa6b6aebea1b1a9
b9a5b5adbda3b3abbba7b7af7fc2c44993a74c9d367dc6cc59b3e7cc9d377fc1c2458b972c5db67cc5ca55abd7ac5db7
7ec3c64d9bb76cddb67dc7ce5dbbf7ecddb7ffc0c143878f1c3d76fcc4c953a7cf9c3d77fec2c54b97af5cbd76fdc6cd
5bb7efdcbd77ffc1c3478f9f3c7df6fcc5cb57afdfbc7df7fec3c74f9fbf7cfdf6fdc7cf5fbffffcfdf71f008804d2ef";

/// "abracadabra abracadabra abracadabra" with the fixed Huffman code (`Z_FIXED`).
const FIXED: &str = "78014b4c2a4a4c4e4c4904520a89d8d900ee280d3d";

#[test]
fn dynamic_and_fixed_blocks() {
    assert_eq!(zlib(&unhex(DYNAMIC), 1 << 20).unwrap(), text());
    assert_eq!(zlib(&unhex(FIXED), 1 << 20).unwrap(), b"abracadabra abracadabra abracadabra");
}

#[test]
fn stored_blocks() {
    // Two stored blocks, the last one flagged: "hello" and " world".
    let mut raw = vec![0x00, 5, 0, !5, 0xff];
    raw.extend_from_slice(b"hello");
    raw.extend_from_slice(&[0x01, 6, 0, !6, 0xff]);
    raw.extend_from_slice(b" world");
    let (out, used) = inflate(&raw, 100).unwrap();
    assert_eq!((out.as_slice(), used), (&b"hello world"[..], raw.len()));
    let mut z = vec![0x78, 0x01];
    z.extend_from_slice(&raw);
    z.extend_from_slice(&adler32(b"hello world").to_be_bytes());
    assert_eq!(zlib(&z, 100).unwrap(), b"hello world");
}

#[test]
fn adler32_of_known_text() {
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    assert_eq!(adler32(b""), 1);
}

#[test]
fn bad_streams_are_refused() {
    let good = unhex(DYNAMIC);
    assert_eq!(zlib(&good, 100), Err(InflateError::TooLarge), "the limit holds");
    assert_eq!(zlib(&good[..good.len() / 2], 1 << 20), Err(InflateError::Truncated));
    let mut sum = good.clone();
    *sum.last_mut().unwrap() ^= 1;
    assert_eq!(zlib(&sum, 1 << 20), Err(InflateError::BadChecksum));
    assert_eq!(zlib(&[0x78, 0x02], 10), Err(InflateError::BadHeader), "check bits");
    assert_eq!(zlib(&[0x78, 0xbb, 0, 0, 0, 0], 10), Err(InflateError::BadHeader), "preset dictionary");
    assert_eq!(inflate(&[0x07], 10).map(|_| ()), Err(InflateError::Corrupt), "block type 3");
    assert_eq!(inflate(&[0x01, 5, 0, 5, 0], 10).map(|_| ()), Err(InflateError::Corrupt), "stored length check");
    // A fixed block starting with a match (length 3, distance 1): nothing to copy yet.
    assert_eq!(inflate(&[0x03, 0x02, 0x00], 10).map(|_| ()), Err(InflateError::Corrupt));
}

/// Python's `gzip.GzipFile(filename="text.txt", mtime=0)` of `text()`: a header with a name.
const GZIP: &str = "1f8b08080000000002ff746578742e74787400cb49cc2b2e492cce50c84e4d2d2856484fcc4d5548c92fcfcbc94f4c2956c8cf5328c94855f071f4d353c8195539aa7254e5a8ca5195a32a69a6928191899985958d9d83938b9b87978f5f4050485844544c5c42524a5a46564e5e4151495945554d5d43534b5b47574fdfc0d0c8d8c4d4ccdcc2d2cadac6d6cedec1d1c9d9c5d5cdddc3d3cbdbc7d7cf3f2030283824342c3c22322a3a26362e3e2131293925352d3d23332b3b27372fbfa0b0a8b8a4b4acbca2b2aabaa6b6aebea1b1a9b9a5b5adbda3b3abbba7b7af7fc2c44993a74c9d367dc6cc59b3e7cc9d377fc1c2458b972c5db67cc5ca55abd7ac5db77ec3c64d9bb76cddb67dc7ce5dbbf7ecddb7ffc0c143878f1c3d76fcc4c953a7cf9c3d77fec2c54b97af5cbd76fdc6cd5bb7efdcbd77ffc1c3478f9f3c7df6fcc5cb57afdfbc7df7fec3c74f9fbf7cfdf6fdc7cf5fbffffcfdf71f0015cb58d690070000";

#[test]
fn gzip_members_are_checked_by_crc_and_size() {
    let g = unhex(GZIP);
    assert_eq!(gzip(&g, 1 << 20).unwrap(), text());
    let mut bad = g.clone();
    let n = bad.len();
    bad[n - 5] ^= 1; // the CRC-32
    assert_eq!(gzip(&bad, 1 << 20), Err(InflateError::BadChecksum));
    assert_eq!(gzip(&g[..g.len() - 2], 1 << 20), Err(InflateError::Truncated));
    assert_eq!(gzip(&g[1..], 1 << 20), Err(InflateError::BadHeader));
    assert_eq!(gzip(&g, 100), Err(InflateError::TooLarge));
}
