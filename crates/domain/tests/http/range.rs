//! Tests for `lanstash_domain::http::range`.

use lanstash_domain::http::range::{ByteRange, parse_content_range, parse_range, parse_unsatisfied_range};

#[test]
fn parses_and_resolves_ranges() {
    assert_eq!(parse_range("bytes=0-1048575"), Some(ByteRange::FromTo(0, 1_048_575)));
    assert_eq!(parse_range(" bytes=500- "), Some(ByteRange::From(500)));
    assert_eq!(parse_range("bytes=-100"), Some(ByteRange::Suffix(100)));
    for bad in ["bytes=5-1", "bytes=0-1,5-9", "items=0-1", "bytes=-", "bytes=a-b", "bytes=+5-+9", "bytes=-+3"] {
        assert_eq!(parse_range(bad), None, "{bad}");
    }
    assert_eq!(ByteRange::FromTo(10, 99_999).resolve(1000), Some((10, 999)));
    assert_eq!(ByteRange::From(1000).resolve(1000), None);
    assert_eq!(ByteRange::Suffix(100).resolve(1000), Some((900, 999)));
    assert_eq!(ByteRange::Suffix(5000).resolve(1000), Some((0, 999)));
    assert_eq!(ByteRange::From(0).resolve(0), None);
    assert_eq!(ByteRange::Suffix(1).known_start(), None);
}

#[test]
fn content_range_parsing() {
    assert_eq!(parse_content_range("bytes 1048576-1048655/1048656").map(|r| r.total), Some(1_048_656));
    assert_eq!(parse_content_range("bytes 5-4/10"), None);
    assert_eq!(parse_content_range("bytes 0-10/10"), None);
    assert_eq!(parse_content_range("items 0-1/2"), None);
    assert_eq!(parse_content_range("bytes */795075223"), None);
    assert_eq!(parse_unsatisfied_range("bytes */795075223"), Some(795_075_223));
    assert_eq!(parse_unsatisfied_range("bytes 0-1/2"), None);
}
