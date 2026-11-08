//! The admin port's servers: plain HTTP on `admin_listen` and, with `admin_listen_tls`, HTTPS with
//! lanstash's own certificate. The certificate is made once (ECDSA P-256, self-signed) for the
//! names `admin_tls_names` gives (else this machine's name and `localhost`, with the HTTPS
//! address when it is one), kept in `secrets`, and made again only when those names change. With
//! HTTPS on, the plain port answers this machine itself (the command line) and sends every other
//! request to the same path over HTTPS (308), so no password crosses the network in the clear.

use std::net::IpAddr;
use std::sync::Arc;

use lanstash::config::Config;
use lanstash_adapter_admin_api::guard::https_redirect;
use lanstash_adapter_crypto::certificate::make;
use lanstash_adapter_fs::private_dir::PrivateDir;
use lanstash_adapter_http::link::tls_config;
use lanstash_app::serve::{ConnState, RequestHandler, ResponseHead, ResponseSink, SinkError};
use lanstash_domain::accounts::{hex, unhex};
use lanstash_domain::http::{RequestHead, reason_phrase};
use lanstash_domain::x509::AltName;
use lanstash_ports::clock::Clock;
use lanstash_ports::secrets::Secrets;

use lanstash_adapter_http::server::ServerConfig;

const CERTIFICATE: &str = "admin-tls-certificate";
const KEY: &str = "admin-tls-key";
const NAMES: &str = "admin-tls-names";

/// The plain port with HTTPS on: this machine is answered, every other request sent to HTTPS.
struct ToHttps {
    inner: Arc<dyn RequestHandler>,
    port: u16,
}

impl RequestHandler for ToHttps {
    fn handle(&self, req: &RequestHead, conn: &mut ConnState, sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
        if conn.is_local() {
            return self.inner.handle(req, conn, sink);
        }
        // Only to a name of the network's own: never an open redirect to wherever `Host` says.
        let to = https_redirect(req, self.port);
        let status = if to.is_some() { 308 } else { 400 };
        let location = to.unwrap_or_default();
        let headers: &[(&str, &str)] = if status == 308 { &[("Location", &location)] } else { &[] };
        sink.send_head(&ResponseHead { status, reason: reason_phrase(status), headers, content_length: 0 })
    }
}

/// The names the certificate is for.
fn names(c: &Config) -> Vec<String> {
    if !c.admin_tls_names.is_empty() {
        return c.admin_tls_names.clone();
    }
    let host = std::fs::read_to_string("/etc/hostname").map(|h| h.trim().to_string()).unwrap_or_default();
    let mut out: Vec<String> = [host, "localhost".into()].into_iter().filter(|n| !n.is_empty()).collect();
    if let Some(a) = c.admin_listen_tls.filter(|a| !a.ip().is_unspecified()) {
        out.push(a.ip().to_string());
    }
    out
}

fn alt(name: &str) -> AltName {
    match name.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) => AltName::V4(a.octets()),
        Ok(IpAddr::V6(a)) => AltName::V6(a.octets()),
        Err(_) => AltName::Dns(name.to_string()),
    }
}

/// The kept certificate and key for `names`, or new ones (kept).
fn certificate(secrets: &dyn Secrets, names: &[String], now_ms: u64) -> Result<(Vec<u8>, Vec<u8>), String> {
    let joined = names.join(",");
    let kept = (secrets.load(CERTIFICATE)?, secrets.load(KEY)?, secrets.load(NAMES)?);
    if let (Some(cert), Some(key), Some(for_names)) = kept
        && for_names.trim() == joined
        && let (Some(cert), Some(key)) = (unhex(cert.trim()), unhex(key.trim()))
    {
        return Ok((cert, key));
    }
    let made = make("lanstash", &names.iter().map(|n| alt(n)).collect::<Vec<_>>(), now_ms)?;
    secrets.save(KEY, &hex(&made.key))?;
    secrets.save(CERTIFICATE, &hex(&made.certificate))?;
    secrets.save(NAMES, &joined)?;
    println!("made the HTTPS certificate for {joined}");
    Ok((made.certificate, made.key))
}

fn spawn(
    name: &str,
    server: lanstash_adapter_http::server::HttpServer,
    page: Arc<dyn RequestHandler>,
) -> Result<(), String> {
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || server.run(&page))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Starts the admin servers for `page`.
pub(super) fn start(c: &Config, page: Arc<dyn RequestHandler>, clock: &Arc<dyn Clock>) -> Result<(), String> {
    let Some(addr) = c.admin_listen else { return Ok(()) };
    // Event streams take a quarter at most (`MAX_STREAMS`); a dev server's proxy is one address.
    let config = |listen| ServerConfig { listen, max_connections: 32, max_per_client: 24, ..super::server_config(c) };
    let plain: Arc<dyn RequestHandler> = match c.admin_listen_tls {
        Some(tls) => Arc::new(ToHttps { inner: Arc::clone(&page), port: tls.port() }),
        None => Arc::clone(&page),
    };
    let admin = super::bind(config(addr), clock)?;
    println!("status page on http://{}", admin.local_addr().map_err(|e| e.to_string())?);
    spawn("lanstash-admin", admin, plain)?;
    if let Some(tls) = c.admin_listen_tls {
        let secrets = PrivateDir::new(&c.secrets);
        let (cert, key) = certificate(&secrets, &names(c), clock.now_ms())?;
        let https = super::bind(config(tls), clock)?.with_tls(tls_config(cert, key)?);
        println!("web UI on https://{}", https.local_addr().map_err(|e| e.to_string())?);
        spawn("lanstash-admin-tls", https, page)?;
    }
    Ok(())
}
