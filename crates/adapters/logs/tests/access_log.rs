//! Tests for `lanstash_adapter_logs::access_log`.

use lanstash_adapter_http::server::AccessEntry;
use lanstash_adapter_logs::access_log::*;
use lanstash_domain::http::{Limits, parse_request_head};

#[test]
fn formats_one_safe_line() {
    let raw =
        b"GET /tpr/a b HTTP/1.1\r\nHost: level3.blizzard.com\r\nRange: bytes=0-9\r\nUser-Agent: Battle\"net\r\n\r\n";
    // "/tpr/a b" is not a valid target; use a valid one with odd characters instead.
    let raw = std::str::from_utf8(raw).unwrap().replace("/tpr/a b", "/tpr/a%20b");
    let req = parse_request_head(raw.as_bytes(), Limits::default()).unwrap().unwrap().0;
    let e = AccessEntry {
        peer: Some("10.0.1.100:5555".parse().unwrap()),
        req: &req,
        status: 206,
        cache: Some("MISS"),
        bytes: 10,
        complete: false,
    };
    let base = "7 10.0.1.100 GET level3.blizzard.com /tpr/a%20b range=bytes=0-9 status=206 cache=MISS bytes=10 INCOMPLETE ua=\"Battle'net\"";
    assert_eq!(format_entry(7, &e, false), base);
    assert_eq!(
        format_entry(7, &e, true),
        format!("{base} hdr=\"host: level3.blizzard.com | range: bytes=0-9 | user-agent: Battle'net\"")
    );
}
