//! A web API on the network (a DNS server's admin API, say): one request with
//! a body, its whole response, over plain HTTP or TLS (trusted by the public
//! roots, or by one pinned certificate). Not for origin servers: those go
//! through [`crate::upstream`], which streams and pools.

use alloc::string::String;
use alloc::vec::Vec;

/// One request.
#[derive(Debug, Clone, Copy)]
pub struct WebRequest<'a> {
    pub method: &'a str,
    /// Host name or address (resolved the way the machine resolves names).
    pub host: &'a str,
    pub port: u16,
    pub target: &'a str,
    /// Extra request headers (`Host`, `Content-Length` and `Connection` are the adapter's).
    pub headers: &'a [(&'a str, &'a str)],
    pub body: &'a [u8],
    /// Over TLS (HTTPS), when given.
    pub tls: Option<Tls<'a>>,
}

/// How a TLS server is trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tls<'a> {
    /// The SHA-256 of the one certificate trusted (hex, colons allowed): a self-signed or
    /// privately issued one. None: the usual public roots, and the certificate must name the host.
    pub pin: Option<&'a str>,
    /// Any certificate is accepted (still encrypted, but anyone on the way could pose as the server).
    pub unchecked: bool,
}

/// Where a web API is: its host and port, and over HTTPS how its certificate is trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub https: bool,
    /// The one certificate trusted (its SHA-256), for a self-signed or privately issued one.
    pub pin: Option<String>,
    /// Any certificate is accepted.
    pub unchecked: bool,
}

impl Endpoint {
    /// How a request to it is made secure: none over plain HTTP.
    #[must_use]
    pub fn tls(&self) -> Option<Tls<'_>> {
        self.https.then_some(Tls { pin: self.pin.as_deref(), unchecked: self.unchecked })
    }
}

/// A web request's failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebError {
    pub message: String,
    /// The SHA-256 of the certificate the server presented when it was not trusted, so it can be pinned.
    pub untrusted: Option<String>,
}

impl core::fmt::Display for WebError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.message)?;
        match &self.untrusted {
            Some(fp) => write!(f, " ({UNTRUSTED}{fp})"),
            None => Ok(()),
        }
    }
}

/// What precedes an untrusted certificate's SHA-256 in an error's words, so it survives
/// being passed on as text and can be read back ([`untrusted_in`]).
pub const UNTRUSTED: &str = "untrusted certificate, SHA-256 ";

/// The SHA-256 of an untrusted certificate an error's words name, if they name one.
#[must_use]
pub fn untrusted_in(message: &str) -> Option<&str> {
    let rest = &message[message.find(UNTRUSTED)? + UNTRUSTED.len()..];
    let end = rest.find(|c: char| !(c.is_ascii_hexdigit() || c == ':')).unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

impl From<String> for WebError {
    fn from(message: String) -> Self {
        Self { message, untrusted: None }
    }
}

impl From<&str> for WebError {
    fn from(message: &str) -> Self {
        Self { message: message.into(), untrusted: None }
    }
}

/// A whole response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Sends requests to web APIs.
pub trait Web: Send + Sync {
    /// Sends `req` and reads the whole response.
    ///
    /// # Errors
    /// Resolution, connection, timeout, an untrusted certificate, a malformed or too large response.
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError>;
}
