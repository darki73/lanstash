//! Tests for `lanstash_battlenet_domain::blte`, on BLTE files built here.

use lanstash_battlenet_domain::blte::{BlteError, decode, verify};
use lanstash_domain::checksum::md5::md5;
use lanstash_domain::inflate::InflateError;

/// "abracadabra abracadabra abracadabra", zlib with the fixed Huffman code.
const FIXED: [u8; 21] = [
    0x78, 0x01, 0x4b, 0x4c, 0x2a, 0x4a, 0x4c, 0x4e, 0x4c, 0x49, 0x04, 0x52, 0x0a, 0x89, 0xd8, 0xd9, 0x00, 0xee, 0x28,
    0x0d, 0x3d,
];

/// A BLTE file of `chunks` (mode byte and data each, decoded size).
fn blte(chunks: &[(Vec<u8>, u32)]) -> Vec<u8> {
    let header = 12 + 24 * chunks.len() as u32;
    let mut d = b"BLTE".to_vec();
    d.extend_from_slice(&header.to_be_bytes());
    d.push(0x0F);
    d.extend_from_slice(&(chunks.len() as u32).to_be_bytes()[1..]);
    for (c, size) in chunks {
        d.extend_from_slice(&(c.len() as u32).to_be_bytes());
        d.extend_from_slice(&size.to_be_bytes());
        d.extend_from_slice(&md5(c));
    }
    for (c, _) in chunks {
        d.extend_from_slice(c);
    }
    d
}

fn chunk(mode: u8, data: &[u8]) -> Vec<u8> {
    let mut c = vec![mode];
    c.extend_from_slice(data);
    c
}

#[test]
fn stored_and_zlib_chunks() {
    let f = blte(&[(chunk(b'N', b"DL head "), 8), (chunk(b'Z', &FIXED), 35)]);
    assert_eq!(decode(&f, 1 << 20).unwrap(), b"DL head abracadabra abracadabra abracadabra");
    // Header size 0: one chunk, the rest of the file.
    let mut single = b"BLTE\0\0\0\0".to_vec();
    single.extend_from_slice(&chunk(b'N', b"whole"));
    assert_eq!(decode(&single, 100).unwrap(), b"whole");
}

#[test]
fn bad_files_are_refused() {
    assert_eq!(decode(b"BLTX\0\0\0\0N", 10), Err(BlteError::BadHeader));
    assert_eq!(decode(&blte(&[(chunk(b'E', b"secret"), 6)]), 10), Err(BlteError::Unsupported(b'E')));
    assert_eq!(decode(&blte(&[(chunk(b'N', b"abc"), 4)]), 10), Err(BlteError::BadSize));
    let f = blte(&[(chunk(b'N', b"abcdef"), 6)]);
    assert_eq!(decode(&f[..f.len() - 2], 10), Err(BlteError::Truncated));
    assert_eq!(decode(&f, 5), Err(BlteError::TooLarge));
    assert_eq!(decode(&blte(&[(chunk(b'Z', &FIXED), 35)]), 10), Err(BlteError::TooLarge));
    let mut broken = FIXED;
    broken[20] ^= 1;
    assert_eq!(decode(&blte(&[(chunk(b'Z', &broken), 35)]), 100), Err(BlteError::Inflate(InflateError::BadChecksum)));
}

#[test]
fn a_file_is_checked_against_its_encoding_key() {
    // With a header: the key is the header's MD5, each chunk checked against the MD5 listed for it.
    let f = blte(&[(chunk(b'N', b"DL head "), 8), (chunk(b'Z', &FIXED), 35)]);
    let header = 12 + 24 * 2;
    let key = md5(&f[..header]);
    assert!(verify(&f, &key));
    assert!(!verify(&f, &md5(&f)), "the whole file's MD5 is not the key");
    let mut bad_chunk = f.clone();
    *bad_chunk.last_mut().unwrap() ^= 1;
    assert!(!verify(&bad_chunk, &key), "a changed chunk");
    assert!(!verify(&f[..f.len() - 1], &key), "a short file");
    let mut long = f.clone();
    long.push(0);
    assert!(!verify(&long, &key), "trailing bytes");
    // Header size 0: the key is the whole file's MD5.
    let mut single = b"BLTE\0\0\0\0".to_vec();
    single.extend_from_slice(&chunk(b'N', b"whole"));
    assert!(verify(&single, &md5(&single)));
    assert!(!verify(&single[..12], &md5(&single)));
    assert!(!verify(b"BLTX\0\0\0\0Nx", &md5(b"BLTX\0\0\0\0Nx")), "not BLTE");
}

/// Real CDN files: every `<anything>-<encoding key>` file in `LANSTASH_BLTE_DIR`
/// verifies against its key, and fails against a changed one.
#[test]
#[ignore = "needs LANSTASH_BLTE_DIR with real BLTE files"]
fn real_files_verify_against_their_keys() {
    let dir = std::env::var("LANSTASH_BLTE_DIR").expect("LANSTASH_BLTE_DIR");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some(hex) = name.rsplit('-').next().filter(|h| h.len() == 32) else { continue };
        let mut key = [0u8; 16];
        for (i, b) in key.iter_mut().enumerate() {
            *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
        }
        let data = std::fs::read(&path).unwrap();
        let header = u32::from_be_bytes(data[4..8].try_into().unwrap());
        assert!(verify(&data, &key), "{name} ({} bytes, header {header}) does not verify", data.len());
        let mut parts = lanstash_battlenet_domain::blte::Verifier::new(key);
        for p in data.chunks(1 << 20) {
            parts.feed(p);
        }
        assert!(parts.finish(), "{name} in 1 MiB parts");
        key[0] ^= 1;
        assert!(!verify(&data, &key), "{name} verifies against a wrong key");
        println!("{name}: {} bytes, header {header}: ok", data.len());
        checked += 1;
    }
    assert!(checked > 0, "no files named <...>-<32 hex>");
}

#[test]
fn checked_in_parts_as_whole() {
    use lanstash_battlenet_domain::blte::Verifier;
    let f = blte(&[(chunk(b'N', b"DL head "), 8), (chunk(b'Z', &FIXED), 35)]);
    let key = md5(&f[..12 + 24 * 2]);
    for part in 1..=f.len() {
        let mut v = Verifier::new(key);
        for p in f.chunks(part) {
            v.feed(p);
        }
        assert!(v.finish(), "in parts of {part}");
        let mut short = Verifier::new(key);
        short.feed(&f[..f.len() - 1]);
        assert!(!short.finish());
    }
    let mut single = b"BLTE\0\0\0\0".to_vec();
    single.extend_from_slice(&chunk(b'N', b"whole"));
    let mut v = Verifier::new(md5(&single));
    for b in &single {
        v.feed(std::slice::from_ref(b));
    }
    assert!(v.finish(), "byte by byte, header size 0");
}

#[test]
fn a_header_too_long_to_be_real_fails_without_being_collected() {
    use lanstash_battlenet_domain::blte::{MAX_HEADER, Verifier};
    for size in [u32::MAX, MAX_HEADER as u32 + 24, 13] {
        let mut v = Verifier::new([0; 16]);
        let mut start = b"BLTE".to_vec();
        start.extend_from_slice(&size.to_be_bytes());
        v.feed(&start);
        // Megabytes more: none of it kept, the answer already known.
        let more = vec![0u8; 1 << 20];
        for _ in 0..8 {
            v.feed(&more);
        }
        assert!(!v.finish(), "header size {size}");
    }
}
