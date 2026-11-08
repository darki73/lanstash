//! A connection the HTTP server reads requests from and writes answers to: a plain TCP socket, or
//! TLS over one (rustls, the handshake done as the first bytes are read). Only a plain socket can
//! be handed a file to send without copying it ([`Link::socket`]).

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};

/// A connection of the HTTP server.
pub trait Link: Read + Write {
    /// How long a read may wait.
    ///
    /// # Errors
    /// The socket refused it.
    fn set_read_timeout(&self, wait: Option<Duration>) -> io::Result<()>;

    /// The plain socket, when the bytes go to it as they are (not over TLS).
    fn socket(&mut self) -> Option<&mut TcpStream>;

    /// Says the connection ends, where the protocol has a way to (TLS's close notice).
    fn close(&mut self) {}
}

impl Link for TcpStream {
    fn set_read_timeout(&self, wait: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, wait)
    }
    fn socket(&mut self) -> Option<&mut TcpStream> {
        Some(self)
    }
}

/// TLS over a TCP socket, as a server.
pub type TlsLink = StreamOwned<ServerConnection, TcpStream>;

impl Link for TlsLink {
    fn set_read_timeout(&self, wait: Option<Duration>) -> io::Result<()> {
        self.sock.set_read_timeout(wait)
    }
    fn socket(&mut self) -> Option<&mut TcpStream> {
        None
    }
    fn close(&mut self) {
        self.conn.send_close_notify();
        let _ = self.flush();
    }
}

/// A TLS server configuration for one certificate (DER) and its key (PKCS#8 DER), with ring.
///
/// # Errors
/// rustls refused the certificate or the key.
pub fn tls_config(certificate: Vec<u8>, key: Vec<u8>) -> Result<Arc<ServerConfig>, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(vec![CertificateDer::from(certificate)], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)))
        .map_err(|e| format!("the certificate or its key: {e}"))?;
    Ok(Arc::new(config))
}

/// `stream` as the server's side of a TLS connection under `config`.
///
/// # Errors
/// rustls could not start a connection with that configuration.
pub fn tls(config: &Arc<ServerConfig>, stream: TcpStream) -> io::Result<TlsLink> {
    let conn = ServerConnection::new(Arc::clone(config)).map_err(io::Error::other)?;
    Ok(StreamOwned::new(conn, stream))
}
