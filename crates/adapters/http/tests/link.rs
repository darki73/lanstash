//! Tests for `lanstash_adapter_http::link`: the server over TLS with a certificate lanstash made,
//! checked by a client the way a browser checks it (trusting that certificate).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use lanstash_adapter_crypto::certificate::make;
use lanstash_adapter_http::link::tls_config;
use lanstash_adapter_http::server::{HttpServer, ServerConfig};
use lanstash_app::serve::{ConnState, RequestHandler, ResponseHead, ResponseSink, SinkError};
use lanstash_domain::http::{Limits, RequestHead};
use lanstash_domain::x509::AltName;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

/// Answers whether the connection was HTTPS.
struct Secure;
impl RequestHandler for Secure {
    fn handle(&self, _: &RequestHead, conn: &mut ConnState, sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
        let body = format!("secure={}", conn.is_secure());
        sink.send_head(&ResponseHead { status: 200, reason: "OK", headers: &[], content_length: body.len() as u64 })?;
        sink.send_body(body.as_bytes())
    }
}

fn now_ms() -> u64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    u64::try_from(since.as_millis()).unwrap()
}

/// An HTTPS server on a free port, and the certificate it shows.
fn https() -> (SocketAddr, Vec<u8>) {
    let made = make("lanstash", &[AltName::Dns("lanstash.lan".into()), AltName::V4([127, 0, 0, 1])], now_ms()).unwrap();
    let server = HttpServer::bind(ServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        max_connections: 8,
        idle_timeout: Duration::from_secs(5),
        head_timeout: Duration::from_secs(5),
        max_per_client: 8,
        write_timeout: Duration::from_secs(5),
        limits: Limits::default(),
        thread_stack_bytes: 256 * 1024,
        zero_copy: true,
    })
    .unwrap()
    .with_tls(tls_config(made.certificate.clone(), made.key).unwrap());
    let addr = server.local_addr().unwrap();
    let handler: Arc<dyn RequestHandler> = Arc::new(Secure);
    std::thread::spawn(move || server.run(&handler));
    (addr, made.certificate)
}

fn client(trusted: &[u8]) -> Arc<ClientConfig> {
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from(trusted.to_vec())).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Arc::new(
        ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}

fn get(addr: SocketAddr, config: Arc<ClientConfig>, name: &str) -> std::io::Result<String> {
    let conn = ClientConnection::new(config, ServerName::try_from(name.to_string()).unwrap()).unwrap();
    let mut tls = StreamOwned::new(conn, TcpStream::connect(addr)?);
    tls.write_all(b"GET / HTTP/1.1\r\nHost: lanstash.lan\r\nConnection: close\r\n\r\n")?;
    let mut out = String::new();
    tls.read_to_string(&mut out)?;
    Ok(out)
}

#[test]
fn a_client_that_trusts_the_certificate_gets_an_answer_over_https() {
    let (addr, certificate) = https();
    let answer = get(addr, client(&certificate), "lanstash.lan").unwrap();
    assert!(answer.starts_with("HTTP/1.1 200") && answer.ends_with("secure=true"), "{answer}");
    let by_address = get(addr, client(&certificate), "127.0.0.1").unwrap();
    assert!(by_address.ends_with("secure=true"), "its address is one of its names");
}

#[test]
fn a_name_the_certificate_is_not_for_is_refused() {
    let (addr, certificate) = https();
    assert!(get(addr, client(&certificate), "elsewhere.lan").is_err());
}

#[test]
fn plain_http_to_the_https_port_gets_no_answer() {
    let (addr, _) = https();
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    assert!(!String::from_utf8_lossy(&out).contains("secure="));
}
