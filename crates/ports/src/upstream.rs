//! Origin servers: one request out, a response whose body is read incrementally.

use alloc::boxed::Box;
use alloc::string::String;
use core::fmt;

use lanstash_domain::http::ResponseHeadIn;

/// An upstream (origin CDN) request failed before or during the response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamError {
    pub message: String,
    /// The origin did not connect or answer in time.
    pub timed_out: bool,
}

impl UpstreamError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), timed_out: false }
    }

    /// A failure because the origin did not connect or answer in time.
    #[must_use]
    pub fn timeout(message: impl Into<String>) -> Self {
        Self { message: message.into(), timed_out: true }
    }
}

impl fmt::Display for UpstreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "upstream: {}", self.message)
    }
}

impl core::error::Error for UpstreamError {}

/// One request to an origin server.
#[derive(Debug, Clone, Copy)]
pub struct UpstreamRequest<'a> {
    pub method: &'a str,
    /// Host name (no port); resolved by the adapter through upstream DNS.
    pub host: &'a str,
    /// TCP port; `None` is the adapter's default (80).
    pub port: Option<u16>,
    pub target: &'a str,
    /// Extra request headers (`Host` is added by the adapter).
    pub headers: &'a [(&'a str, &'a str)],
}

/// Where a file is on an origin server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub host: String,
    /// `None` is port 80.
    pub port: Option<u16>,
    pub target: String,
}

/// A response whose head has arrived; the body is read incrementally.
pub trait UpstreamResponse: Send {
    fn head(&self) -> &ResponseHeadIn;

    /// Reads decoded body bytes; `Ok(0)` at the end of the body.
    ///
    /// # Errors
    /// Connection failure, timeout or malformed framing.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, UpstreamError>;
}

/// Fetches from origin servers (HTTP client, DNS, pooling in the adapter).
pub trait Upstream: Send + Sync {
    /// Sends `req` and waits for the response head.
    ///
    /// # Errors
    /// Resolution, connection or protocol failure.
    fn fetch(&self, req: &UpstreamRequest<'_>) -> Result<Box<dyn UpstreamResponse>, UpstreamError>;
}
