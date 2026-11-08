//! Tests for `lanstash_adapter_http::server`.

use lanstash_adapter_http::server::*;
use lanstash_app::serve::{ConnState, RequestHandler, ResponseHead, ResponseSink, SinkError};
use lanstash_domain::http::Limits;
use lanstash_domain::http::RequestHead;
use lanstash_ports::storage::{FileRead, StoreError};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

struct WallClock;
impl lanstash_ports::clock::Clock for WallClock {
    fn now_ms(&self) -> u64 {
        let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
        u64::try_from(since.as_millis()).unwrap()
    }
}

struct Echo;
impl RequestHandler for Echo {
    fn handle(&self, req: &RequestHead, conn: &mut ConnState, sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
        if req.target == "/stream" {
            sink.send_stream_head(200, &[("Content-Type", "text/event-stream")])?;
            sink.send_body(b"data: 1\n\n")?;
            return sink.send_body(b"data: 2\n\n");
        }
        if req.target == "/file" {
            // Bytes 10..110 of a file: by the sink where it can, else read and sent.
            let file = OsFile(std::fs::File::open(file_path()).unwrap());
            sink.send_head(&ResponseHead { status: 200, reason: "OK", headers: &[], content_length: 100 })?;
            if !sink.send_file(&file, 10, 100)? {
                let mut buf = vec![0u8; 100];
                file.read_exact_at(10, &mut buf).unwrap();
                sink.send_body(&buf)?;
            }
            return Ok(());
        }
        let who;
        let body = if req.target == "/who" {
            who = format!("{:?} {}", conn.client(), conn.is_local());
            who.as_bytes()
        } else {
            req.target.as_bytes()
        };
        sink.send_head(&ResponseHead {
            status: 200,
            reason: "OK",
            headers: &[("X-Test", "1")],
            content_length: body.len() as u64,
        })?;
        sink.send_body(body)
    }
}

/// A file of 300 bytes (each its position's low byte), written once for the test process.
fn file_path() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("lanstash-http-test-{}-file", std::process::id()));
    if !path.exists() {
        let bytes: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
        std::fs::write(&path, bytes).unwrap();
    }
    path
}

/// A stored file with the OS file under it, as the file-system adapter's.
struct OsFile(std::fs::File);

impl FileRead for OsFile {
    fn size(&self) -> Result<u64, StoreError> {
        Ok(self.0.metadata().map_err(|e| StoreError::other(e.to_string()))?.len())
    }
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), StoreError> {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = &self.0;
        f.seek(SeekFrom::Start(offset)).map_err(|e| StoreError::other(e.to_string()))?;
        f.read_exact(buf).map_err(|e| StoreError::other(e.to_string()))
    }
    fn os_file(&self) -> Option<&dyn std::any::Any> {
        Some(&self.0)
    }
}

#[test]
fn a_file_body_is_sent_whole_and_a_head_answer_sends_none() {
    let addr = start();
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"GET /file HTTP/1.1\r\nConnection: close\r\n\r\n").unwrap();
    let mut got = Vec::new();
    s.read_to_end(&mut got).unwrap();
    let body_at = got.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    let want: Vec<u8> = (10..110u32).map(|i| i as u8).collect();
    assert_eq!(&got[body_at..], &want[..]);
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"HEAD /file HTTP/1.1\r\nConnection: close\r\n\r\n").unwrap();
    let text = read_all(&mut s);
    assert!(text.contains("Content-Length: 100\r\n") && text.ends_with("\r\n\r\n"), "{text}");
}

fn start() -> SocketAddr {
    start_with(Duration::from_secs(5), 8)
}

fn start_with(head_timeout: Duration, max_per_client: usize) -> SocketAddr {
    let server = HttpServer::bind(ServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        max_connections: 8,
        idle_timeout: Duration::from_secs(5),
        head_timeout,
        max_per_client,
        write_timeout: Duration::from_secs(5),
        limits: Limits::default(),
        thread_stack_bytes: 256 * 1024,
        zero_copy: true,
    })
    .unwrap();
    let server = server.with_clock(Arc::new(WallClock));
    let addr = server.local_addr().unwrap();
    let handler: Arc<dyn RequestHandler> = Arc::new(Echo);
    std::thread::spawn(move || server.run(&handler));
    addr
}

fn read_all(s: &mut TcpStream) -> String {
    let mut out = Vec::new();
    s.read_to_end(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn the_handler_is_told_the_clients_address() {
    let addr = start();
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"GET /who HTTP/1.1\r\nConnection: close\r\n\r\n").unwrap();
    let text = read_all(&mut s);
    assert!(text.ends_with("\r\n\r\nSome(127.0.0.1) true"), "{text}");
}

#[test]
fn keep_alive_and_pipelining() {
    let addr = start();
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"GET /a HTTP/1.1\r\nHost: x\r\n\r\nGET /bb HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap();
    let text = read_all(&mut s);
    assert_eq!(text.matches("HTTP/1.1 200 OK").count(), 2, "{text}");
    assert!(text.contains("Content-Length: 2\r\n"));
    assert!(text.ends_with("\r\n\r\n/bb"));
}

#[test]
fn head_has_no_body_and_bad_requests_close() {
    let addr = start();
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"HEAD /abc HTTP/1.1\r\nConnection: close\r\n\r\n").unwrap();
    let text = read_all(&mut s);
    assert!(text.contains("Content-Length: 4\r\n") && text.ends_with("\r\n\r\n"), "{text}");

    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"GARBAGE\r\n\r\n").unwrap();
    assert!(read_all(&mut s).starts_with("HTTP/1.1 400"));

    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"POST /x HTTP/1.1\r\nContent-Length: 3\r\n\r\nabc").unwrap();
    assert!(read_all(&mut s).starts_with("HTTP/1.1 400"));
}

#[test]
fn a_head_that_trickles_in_is_closed_at_its_deadline() {
    let addr = start_with(Duration::from_millis(600), 8);
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let started = std::time::Instant::now();
    // A byte every 100 ms: each read is quick, the head never whole.
    for b in b"GET /slow HTTP/1.1\r\nX-Pad: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" {
        if s.write_all(std::slice::from_ref(b)).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        if started.elapsed() > Duration::from_secs(3) {
            break;
        }
    }
    let mut rest = Vec::new();
    let _ = s.read_to_end(&mut rest);
    assert!(started.elapsed() < Duration::from_secs(3), "closed after {:?}", started.elapsed());
}

#[test]
fn one_client_holds_only_its_share_of_the_connections() {
    let addr = start_with(Duration::from_secs(5), 2);
    let held: Vec<TcpStream> = (0..2).map(|_| TcpStream::connect(addr).unwrap()).collect();
    std::thread::sleep(Duration::from_millis(100));
    let mut third = TcpStream::connect(addr).unwrap();
    assert!(read_all(&mut third).starts_with("HTTP/1.1 503"));
    drop(held);
    std::thread::sleep(Duration::from_millis(200));
    let mut again = TcpStream::connect(addr).unwrap();
    again.write_all(b"GET /x HTTP/1.1\r\nConnection: close\r\n\r\n").unwrap();
    assert!(read_all(&mut again).starts_with("HTTP/1.1 200"), "free again once they closed");
}

#[test]
fn a_stream_has_no_length_and_ends_the_connection() {
    let addr = start();
    let mut s = TcpStream::connect(addr).unwrap();
    // Keep-alive and a second request behind it: the stream still ends the connection.
    s.write_all(b"GET /stream HTTP/1.1\r\nHost: x\r\n\r\nGET /next HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let text = read_all(&mut s);
    assert_eq!(
        text,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: 1\n\ndata: 2\n\n"
    );
}
