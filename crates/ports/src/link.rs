//! A connection that carries whole messages both ways (a WebSocket to a
//! Steam CM, say), and how to open one.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// An open message connection.
pub trait Link: Send {
    /// Sends one message.
    ///
    /// # Errors
    /// The connection failed.
    fn send(&mut self, message: &[u8]) -> Result<(), String>;

    /// The next message; `Ok(None)` when none came within the link's read
    /// timeout (the connection is still usable).
    ///
    /// # Errors
    /// The connection failed or was closed.
    fn recv(&mut self) -> Result<Option<Vec<u8>>, String>;
}

/// Opens links and fetches small documents over HTTPS.
pub trait Dialer: Send + Sync {
    /// A link to `host:port` at `path` (for a WebSocket, `wss://host:port/path`).
    ///
    /// # Errors
    /// Connection, TLS or handshake.
    fn dial(&self, host: &str, port: u16, path: &str) -> Result<Box<dyn Link>, String>;

    /// The body of `GET https://host/path`.
    ///
    /// # Errors
    /// Connection, TLS, or a status other than 200.
    fn get(&self, host: &str, path: &str) -> Result<Vec<u8>, String>;
}
