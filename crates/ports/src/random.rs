//! Cryptographically secure random bytes (RSA padding, WebSocket keys).

use alloc::string::String;

/// A source of secure random bytes.
pub trait Random: Send + Sync {
    /// Fills `buf`.
    ///
    /// # Errors
    /// The system's generator failed.
    fn fill(&self, buf: &mut [u8]) -> Result<(), String>;
}
