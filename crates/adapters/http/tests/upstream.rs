//! Tests for `lanstash_adapter_http::upstream`.

use lanstash_adapter_dns::resolver::Resolve;
use lanstash_adapter_http::upstream::*;
use lanstash_ports::upstream::{Upstream, UpstreamError, UpstreamRequest};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Localhost;
impl Resolve for Localhost {
    fn resolve(&self, _host: &str) -> Result<Vec<Ipv4Addr>, String> {
        Ok(vec![Ipv4Addr::LOCALHOST])
    }
}

/// Serves `responses` in order; each entry is (raw response, close connection after it).
/// Returns the port and the number of accepted connections.
fn server(responses: Vec<(Vec<u8>, bool)>) -> (u16, Arc<AtomicUsize>, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let (a, rq) = (Arc::clone(&accepted), Arc::clone(&requests));
    std::thread::spawn(move || {
        let mut queue = responses.into_iter();
        for conn in l.incoming() {
            let Ok(mut c) = conn else { return };
            a.fetch_add(1, Ordering::SeqCst);
            loop {
                let mut req = Vec::new();
                let mut b = [0u8; 1];
                while !req.ends_with(b"\r\n\r\n") {
                    match c.read(&mut b) {
                        Ok(1) => req.push(b[0]),
                        _ => break,
                    }
                }
                if req.is_empty() {
                    break;
                }
                rq.lock().unwrap().push(String::from_utf8_lossy(&req).into_owned());
                let Some((resp, close)) = queue.next() else { return };
                let _ = c.write_all(&resp);
                if close {
                    break;
                }
            }
        }
    });
    (port, accepted, requests)
}

fn client(port: u16) -> HttpUpstream {
    HttpUpstream::new(
        Arc::new(Localhost),
        UpstreamConfig { port, io_timeout: Duration::from_secs(5), ..UpstreamConfig::default() },
    )
}

fn get(c: &HttpUpstream, target: &str) -> Result<(u16, Vec<u8>), UpstreamError> {
    let req = UpstreamRequest { method: "GET", host: "cdn.test", port: None, target, headers: &[("User-Agent", "t")] };
    let mut r = c.fetch(&req)?;
    let status = r.head().status;
    let mut body = Vec::new();
    let mut buf = [0u8; 5];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            return Ok((status, body));
        }
        body.extend_from_slice(&buf[..n]);
    }
}

#[test]
fn content_length_and_chunked_bodies_reuse_one_connection() {
    let (port, accepted, requests) = server(vec![
        (b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\nhello world".to_vec(), false),
        (
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n".to_vec(),
            false,
        ),
        (b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec(), false),
    ]);
    let c = client(port);
    assert_eq!(get(&c, "/a").unwrap(), (200, b"hello world".to_vec()));
    assert_eq!(get(&c, "/b").unwrap(), (200, b"hello world".to_vec()));
    assert_eq!(get(&c, "/c").unwrap(), (404, Vec::new()));
    assert_eq!(accepted.load(Ordering::SeqCst), 1);
    let first = requests.lock().unwrap()[0].clone();
    assert!(first.starts_with("GET /a HTTP/1.1\r\nHost: cdn.test\r\nUser-Agent: t\r\n"), "{first}");
}

#[test]
fn stale_pooled_connection_is_retried_on_a_fresh_one() {
    // The server says keep-alive but closes after the first response.
    let (port, accepted, _) = server(vec![
        (b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec(), true),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nyes".to_vec(), true),
    ]);
    let c = client(port);
    assert_eq!(get(&c, "/1").unwrap().1, b"ok");
    assert_eq!(get(&c, "/2").unwrap().1, b"yes");
    assert_eq!(accepted.load(Ordering::SeqCst), 2);
}

#[test]
fn short_body_is_an_error_and_close_delimited_bodies_work() {
    let (port, _, _) = server(vec![(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nshort".to_vec(), true)]);
    assert!(get(&client(port), "/x").is_err());
    let (port, accepted, _) = server(vec![
        (b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nuntil close".to_vec(), true),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nz".to_vec(), true),
    ]);
    let c = client(port);
    assert_eq!(get(&c, "/y").unwrap().1, b"until close");
    assert_eq!(get(&c, "/z").unwrap().1, b"z");
    assert_eq!(accepted.load(Ordering::SeqCst), 2, "close-delimited connection is not reused");
}

#[test]
fn a_request_may_name_its_own_port() {
    let (port, _, requests) = server(vec![(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok".to_vec(), false)]);
    let c = client(1); // the default port is wrong on purpose
    let req = UpstreamRequest { method: "GET", host: "patch.test", port: Some(port), target: "/v", headers: &[] };
    let mut r = c.fetch(&req).unwrap();
    assert_eq!(r.head().status, 200);
    let mut buf = [0u8; 8];
    assert_eq!(r.read(&mut buf).unwrap(), 2);
    assert!(requests.lock().unwrap()[0].contains(&format!("Host: patch.test:{port}\r\n")));
}

#[test]
fn an_origin_that_says_nothing_is_a_timeout() {
    // Accepts and reads the request, then says nothing until the client gives up.
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let Ok((mut c, _)) = l.accept() else { return };
        let mut b = [0u8; 1024];
        let _ = c.read(&mut b);
        std::thread::sleep(Duration::from_secs(2));
    });
    let c = HttpUpstream::new(
        Arc::new(Localhost),
        UpstreamConfig { port, io_timeout: Duration::from_millis(200), ..UpstreamConfig::default() },
    );
    let e = get(&c, "/slow").unwrap_err();
    assert!(e.timed_out, "{e}");
    assert_eq!(e.message, "receive: timed out", "said in words, not as an OS error");
    let refused = client(free_port());
    assert!(!get(&refused, "/x").unwrap_err().timed_out, "a refused connection is no timeout");
}

/// A port nothing listens on.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}
