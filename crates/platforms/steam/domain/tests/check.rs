//! Tests for `lanstash_steam_domain::check`.

use lanstash_domain::checksum::crc::crc32;
use lanstash_domain::segment::meta::encode_meta;
use lanstash_steam_domain::check::ChunkCheck;

#[test]
fn a_chunk_passes_on_its_crc_and_size_only() {
    let body = b"an encrypted, compressed chunk".to_vec();
    let crc = crc32(&body).to_string();
    let stored = encode_meta(&[("content-type", "application/x-steam-chunk"), ("x-content-crc", &crc)]);
    let check = |size: u64, stored: &[u8], data: &[u8]| {
        let mut c = ChunkCheck::new(size, stored);
        c.feed(data);
        c.finish()
    };
    assert!(check(body.len() as u64, &stored, &body));
    assert!(!check(body.len() as u64 + 1, &stored, &body), "the size the manifest gives");
    let mut other = body.clone();
    other[0] ^= 1;
    assert!(!check(body.len() as u64, &stored, &other), "the CRC");
    assert!(!check(body.len() as u64, &encode_meta(&[("etag", "x")]), &body), "no header: no pass");
}
