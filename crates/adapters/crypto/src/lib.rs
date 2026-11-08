//! lanstash adapter: password hashing (the `Passwords` port, PBKDF2), SHA-256 (the `Digest` port),
//! secure random bytes (the `Random` port) and the self-signed certificate of lanstash's own HTTPS,
//! through ring.

#![forbid(unsafe_code)]

pub mod certificate;
pub mod digest;
pub mod passwords;
pub mod random;
