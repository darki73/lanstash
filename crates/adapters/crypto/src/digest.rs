//! The `Digest` port as ring's SHA-256.

use lanstash_ports::digest::Digest;
use ring::digest::{SHA256, digest};

/// SHA-256 through ring.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sha256;

impl Digest for Sha256 {
    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(digest(&SHA256, data).as_ref());
        out
    }
}
