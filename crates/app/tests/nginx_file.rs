//! One nginx cache file: its head, its stored response, the headers kept.

use lanstash_app::nginx_file::{FileError, meta, read_cache_file, response_head};
use lanstash_domain::segment::meta::decode_meta;
use lanstash_store::test_support::MemFile;
use lanstash_store::test_support::nginx::cache_file;

fn file(bytes: &[u8]) -> MemFile {
    let f = MemFile::default();
    f.with_bytes(|d| d.extend_from_slice(bytes));
    f
}

#[test]
fn a_cache_file_gives_its_key_response_and_kept_headers() {
    let headers = "Content-Type: application/octet-stream\r\nX-Junk: 1\r\nContent-Length: 4\r\n";
    let f = file(&cache_file("steam/depot/7/chunk/ab", "HTTP/1.1 200 OK", headers, b"body"));
    let head = read_cache_file(&f).unwrap();
    assert_eq!((head.key.as_str(), head.status, head.body_len), ("steam/depot/7/chunk/ab", 200, 4));
    let r = response_head(&head);
    assert_eq!(r.header("content-type"), Some("application/octet-stream"), "names lower-cased");
    let m = meta(&head);
    let kept: Vec<(&str, &str)> = decode_meta(&m).collect();
    assert!(kept.iter().any(|(n, _)| *n == "content-type"));
    assert!(!kept.iter().any(|(n, _)| *n == "x-junk"), "only the headers lanstash keeps: {kept:?}");
}

#[test]
fn what_is_not_a_cache_file_is_bad() {
    let e = read_cache_file(&file(b"tiny")).unwrap_err();
    assert!(matches!(&e, FileError::Bad(m) if m.contains("too small")), "{e:?}");
    assert!(e.message().contains("too small"));
    assert!(matches!(read_cache_file(&file(&[0xAB; 600])), Err(FileError::Bad(_))));
}
