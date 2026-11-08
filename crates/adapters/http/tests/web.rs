//! Tests for `lanstash_adapter_http::web`, against a one-shot local server.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use lanstash_adapter_http::web::HttpWeb;
use lanstash_ports::web::{Web, WebRequest};

/// Answers one connection with `response`; returns its port and the raw request it got.
fn server(response: &'static [u8]) -> (u16, mpsc::Receiver<String>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut c, _) = l.accept().unwrap();
        let mut req = Vec::new();
        let mut b = [0u8; 1];
        // The head, then the body by its length.
        while !req.ends_with(b"\r\n\r\n") && c.read(&mut b).unwrap() == 1 {
            req.push(b[0]);
        }
        let head = String::from_utf8_lossy(&req).to_lowercase();
        let n: usize =
            head.lines().find_map(|l| l.strip_prefix("content-length: ")).map_or(0, |v| v.trim().parse().unwrap());
        let mut body = vec![0u8; n];
        c.read_exact(&mut body).unwrap();
        req.extend(body);
        // A test that does not look at the request has dropped the receiver.
        let _ = tx.send(String::from_utf8_lossy(&req).into_owned());
        c.write_all(response).unwrap();
    });
    (port, rx)
}

fn post(port: u16, body: &[u8]) -> WebRequest<'_> {
    WebRequest {
        method: "POST",
        host: "127.0.0.1",
        port,
        target: "/control/rewrite/add",
        headers: &[("Authorization", "Basic eDp5"), ("Content-Type", "application/json")],
        body,
        tls: None,
    }
}

#[test]
fn sends_the_body_with_its_length_and_reads_a_whole_response() {
    let (port, got) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK");
    let r = HttpWeb::default().send(&post(port, br#"{"a":1}"#)).unwrap();
    assert_eq!((r.status, r.body.as_slice()), (200, b"OK".as_slice()));
    let req = got.recv().unwrap();
    assert!(req.starts_with("POST /control/rewrite/add HTTP/1.1\r\n"), "{req}");
    for h in ["Host: 127.0.0.1:", "Authorization: Basic eDp5", "Content-Length: 7", "Connection: close"] {
        assert!(req.contains(h), "{h} in {req}");
    }
    assert!(req.ends_with(r#"{"a":1}"#));
}

#[test]
fn reads_a_chunked_body_and_one_until_the_close() {
    let (port, _) = server(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\n[1,\r\n2\r\n2]\r\n0\r\n\r\n");
    assert_eq!(HttpWeb::default().send(&post(port, b"")).unwrap().body, b"[1,2]");
    let (port, _) = server(b"HTTP/1.1 403 Forbidden\r\n\r\nno");
    let r = HttpWeb::default().send(&post(port, b"")).unwrap();
    assert_eq!((r.status, r.body.as_slice()), (403, b"no".as_slice()));
}

#[test]
fn refuses_a_body_over_the_limit() {
    let (port, _) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n");
    let small = HttpWeb { max_body: 10, ..HttpWeb::default() };
    assert!(small.send(&post(port, b"")).unwrap_err().message.contains("larger than 10"));
}
