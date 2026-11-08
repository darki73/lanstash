//! Tests for `lanstash_domain::http`.

mod chunked;
mod range;

use lanstash_domain::http::*;

#[test]
fn parses_upstream_response_head() {
    let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/x-steam-chunk\r\nContent-Length: 1048656\r\nx-content-crc: 3666761081\r\n\r\nBODY";
    let (h, used) = parse_response_head(raw, Limits::default()).unwrap().unwrap();
    assert_eq!((h.status, h.content_length(), h.keep_alive()), (200, Some(1_048_656), true));
    assert_eq!(h.header("x-content-crc"), Some("3666761081"));
    assert_eq!(&raw[used..], b"BODY");
    let (h, _) = parse_response_head(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\n",
        Limits::default(),
    )
    .unwrap()
    .unwrap();
    assert!(h.is_chunked() && h.content_length().is_none());
    assert!(parse_response_head(b"HTTP/2 200\r\n\r\n", Limits::default()).is_err());
    assert!(parse_response_head(b"HTTP/1.1 99x OK\r\n\r\n", Limits::default()).is_err());
    assert_eq!(parse_response_head(b"HTTP/1.1 200 OK\r\n", Limits::default()), Ok(None));
}

#[test]
fn builds_request() {
    let r = request_head("GET", "/depot/1/chunk/a", &[("Host", "cdn.example"), ("User-Agent", "x")]);
    assert_eq!(r, b"GET /depot/1/chunk/a HTTP/1.1\r\nHost: cdn.example\r\nUser-Agent: x\r\n\r\n");
}

fn parse(s: &str) -> Result<Option<(RequestHead, usize)>, HttpError> {
    parse_request_head(s.as_bytes(), Limits::default())
}

#[test]
fn parses_steam_request() {
    let req = "GET /depot/1/chunk/abc?x=1 HTTP/1.1\r\nHost: cache1-fra1.steamcontent.com:80\r\nUser-Agent: Valve/Steam HTTP Client 1.0\r\n\r\nGET /next";
    let (h, used) = parse(req).unwrap().unwrap();
    assert_eq!(h.method, "GET");
    assert_eq!(h.target, "/depot/1/chunk/abc?x=1");
    assert_eq!(h.host(), Some("cache1-fra1.steamcontent.com"));
    assert_eq!(h.header("user-agent"), Some("Valve/Steam HTTP Client 1.0"));
    assert!(h.keep_alive());
    assert!(!h.has_body());
    assert_eq!(&req[used..], "GET /next");
}

#[test]
fn incomplete_needs_more() {
    assert_eq!(parse("GET / HTTP/1.1\r\nHost: x\r\n"), Ok(None));
}

#[test]
fn keep_alive_rules() {
    let ka = |s: &str| parse(s).unwrap().unwrap().0.keep_alive();
    assert!(!ka("GET / HTTP/1.1\r\nConnection: close\r\n\r\n"));
    assert!(!ka("GET / HTTP/1.0\r\n\r\n"));
    assert!(ka("GET / HTTP/1.0\r\nConnection: Keep-Alive\r\n\r\n"));
}

#[test]
fn limits_and_malformed() {
    let lim = Limits { max_head_bytes: 64, max_headers: 2 };
    assert_eq!(parse_request_head(&[b'a'; 65], lim), Err(HttpError::HeadTooLarge));
    assert_eq!(
        parse_request_head(b"GET / HTTP/1.1\r\na: 1\r\nb: 2\r\nc: 3\r\n\r\n", lim),
        Err(HttpError::TooManyHeaders)
    );
    assert_eq!(parse("GET / HTTP/2.0\r\n\r\n"), Err(HttpError::UnsupportedVersion));
    assert!(matches!(parse("get / HTTP/1.1\r\n\r\n"), Err(HttpError::Malformed(_))));
    assert!(matches!(parse("GET  / HTTP/1.1\r\n\r\n"), Err(HttpError::Malformed(_))));
    assert!(matches!(parse("GET / HTTP/1.1\r\nbad header\r\n\r\n"), Err(HttpError::Malformed(_))));
    assert!(matches!(parse("GET / HTTP/1.1\r\nBad Name: x\r\n\r\n"), Err(HttpError::Malformed(_))));
    assert!(parse("POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\n").unwrap().unwrap().0.has_body());
}

#[test]
fn a_header_value_with_a_line_break_or_control_is_refused() {
    // A bare LF would become a header, or a request, of its own once sent on to an origin.
    for bad in [
        "GET / HTTP/1.1\r\nUser-Agent: v\nX: 1\r\n\r\n",
        "GET / HTTP/1.1\r\nRange: bytes=0-1\rX: 1\r\n\r\n",
        "GET / HTTP/1.1\r\nUser-Agent: a\u{0}b\r\n\r\n",
    ] {
        assert_eq!(parse(bad), Err(HttpError::Malformed("header value")), "{bad:?}");
    }
    assert!(parse("GET / HTTP/1.1\r\nUser-Agent: a\tb\r\n\r\n").unwrap().is_some(), "a tab is text");
    let up = parse_response_head(b"HTTP/1.1 200 OK\r\nX-A: 1\nSet-Cookie: x\r\n\r\n", Limits::default());
    assert!(matches!(up, Err(HttpError::Malformed(_))), "nor from an origin");
}

#[test]
fn content_length_is_one_number() {
    let twice = "Content-Length: 0\r\nContent-Length: 20";
    for bad in [twice, "Content-Length: +0", "Content-Length: 1,2", "Content-Length:"] {
        let req = format!("POST / HTTP/1.1\r\n{bad}\r\n\r\n");
        assert!(matches!(parse(&req), Err(HttpError::Malformed(_))), "{bad}");
    }
    assert!(parse("GET / HTTP/1.1\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n").unwrap().is_some());
}

#[test]
fn only_a_path_is_a_target() {
    assert!(matches!(parse("GET http://other.example/x HTTP/1.1\r\n\r\n"), Err(HttpError::Malformed("target"))));
    assert!(matches!(parse("OPTIONS * HTTP/1.1\r\n\r\n"), Err(HttpError::Malformed("target"))));
}

#[test]
fn no_header_written_breaks_the_head() {
    let r = request_head("GET", "/a", &[("Host", "cdn"), ("User-Agent", "v\r\nX: 1"), ("Range", "bytes=0-1")]);
    assert_eq!(r, b"GET /a HTTP/1.1\r\nHost: cdn\r\nRange: bytes=0-1\r\n\r\n");
    let r = response_head(200, "OK", &[("Location", "/x\nSet-Cookie: y")], 0, false);
    assert!(!String::from_utf8(r).unwrap().contains("Set-Cookie"));
}

#[test]
fn serialises_response() {
    let r = response_head(200, "OK", &[("Content-Type", "application/x-steam-chunk")], 42, true);
    assert_eq!(
        String::from_utf8(r).unwrap(),
        "HTTP/1.1 200 OK\r\nContent-Length: 42\r\nContent-Type: application/x-steam-chunk\r\nConnection: keep-alive\r\n\r\n"
    );
}

#[test]
fn stream_head_has_no_length_and_closes() {
    let h = stream_response_head(200, "OK", &[("Content-Type", "text/event-stream")]);
    assert_eq!(h, b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n");
}
