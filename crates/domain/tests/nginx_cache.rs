//! Tests for `lanstash_domain::nginx_cache`.

use lanstash_domain::http::range::ContentRange;
use lanstash_domain::nginx_cache::*;

/// Builds a cache file the way nginx lays it out.
fn build(key: &str, http: &str, body: &[u8]) -> Vec<u8> {
    let mut f = vec![0u8; HEADER_STRUCT_LEN];
    f[0..8].copy_from_slice(&CACHE_VERSION.to_le_bytes());
    f.extend_from_slice(KEY_MARKER);
    f.extend_from_slice(key.as_bytes());
    f.push(b'\n');
    let header_start = f.len() as u16;
    f.extend_from_slice(http.as_bytes());
    let body_start = f.len() as u16;
    f[54..56].copy_from_slice(&header_start.to_le_bytes());
    f[56..58].copy_from_slice(&body_start.to_le_bytes());
    f.extend_from_slice(body);
    f
}

const KEY: &str = "steam/depot/2357571/chunk/9de1d873a59f1c1d108f63112ffe8903dc4b5ae1bytes=0-1048575";

fn http(len: usize) -> String {
    format!(
        "HTTP/1.1 206 Partial Content\r\nContent-Type: application/x-steam-chunk\r\nContent-Length: {len}\r\n\
         x-content-crc: 3666761081\r\ncontent-range: bytes 0-{}/1048656\r\n\r\n",
        len - 1
    )
}

#[test]
fn parses_nginx_layout() {
    let file = build(KEY, &http(10), &[7u8; 10]);
    // KEY at 337, header_start = 336 + 6 + 81 + 1 = 424.
    assert_eq!(u16::from_le_bytes([file[54], file[55]]), 424);
    let bs = body_start(&file).unwrap();
    let p = parse(&file[..usize::from(bs)], file.len() as u64).unwrap();
    assert_eq!(p.key, KEY);
    assert_eq!(p.status, 206);
    assert_eq!(p.body_len, 10);
    assert_eq!(p.header("CONTENT-TYPE"), Some("application/x-steam-chunk"));
    assert_eq!(p.content_range(), Some(ContentRange { start: 0, end: 9, total: 1_048_656 }));
}

#[test]
fn rejects_bad_files() {
    let good = build(KEY, &http(10), &[0u8; 10]);
    let mut v4 = good.clone();
    v4[0] = 4;
    assert_eq!(body_start(&v4), Err(NginxCacheError::UnsupportedVersion(4)));
    assert!(matches!(body_start(&good[..20]), Err(NginxCacheError::TooShort { .. })));
    let mut nokey = good.clone();
    nokey[HEADER_STRUCT_LEN + 1] = b'X';
    assert!(matches!(parse(&nokey, nokey.len() as u64), Err(NginxCacheError::BadLayout(_))));
    // Truncated body: Content-Length says 10, file has 5.
    let short = &good[..good.len() - 5];
    assert_eq!(parse(short, short.len() as u64), Err(NginxCacheError::ContentLengthMismatch { header: 10, actual: 5 }));
}

#[test]
fn file_path_matches_nginx() {
    assert_eq!(cache_file_path(KEY), "7a/00/009e7e74e5f962fcbd151381e957007a");
    // Second slice of the same chunk.
    let base = "steam/depot/2357571/chunk/9de1d873a59f1c1d108f63112ffe8903dc4b5ae1";
    assert_eq!(cache_file_path(&slice_key(base, 1_048_576, 1_048_576)), "81/af/e3b3f773e80c891cf848f1abbbd9af81");
}

#[test]
fn slices_of_a_range() {
    let starts = |a, b, s| slice_starts(a, b, s).collect::<Vec<u64>>();
    assert_eq!(starts(0, 1, 1 << 20), vec![0]);
    assert_eq!(starts(0, 1 << 20, 1 << 20), vec![0]);
    assert_eq!(starts(0, (1 << 20) + 1, 1 << 20), vec![0, 1 << 20]);
    assert_eq!(starts((1 << 20) - 1, (2 << 20) + 5, 1 << 20), vec![0, 1 << 20, 2 << 20]);
    assert_eq!(starts(5, 5, 1 << 20), Vec::<u64>::new());
    assert_eq!(starts(0, 10, 0), Vec::<u64>::new());
}

#[test]
fn slice_keys() {
    let (base, r) = split_slice_key(KEY);
    assert_eq!(base, "steam/depot/2357571/chunk/9de1d873a59f1c1d108f63112ffe8903dc4b5ae1");
    assert_eq!(r, Some((0, 1_048_575)));
    assert_eq!(split_slice_key("steam/x"), ("steam/x", None));
    assert_eq!(slice_key(base, 1_048_576, 1_048_576), format!("{base}bytes=1048576-2097151"));
}
