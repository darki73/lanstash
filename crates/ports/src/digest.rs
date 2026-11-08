//! A one-way digest of a secret (a session token), so that what is kept of it cannot be used as it.

/// SHA-256.
pub trait Digest: Send + Sync {
    /// The SHA-256 digest of `data`.
    fn sha256(&self, data: &[u8]) -> [u8; 32];
}
