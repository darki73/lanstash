//! Secure random bytes from ring (the provider rustls already uses).

use lanstash_ports::random::Random;

/// The operating system's generator, through ring.
#[derive(Debug, Clone, Copy, Default)]
pub struct RingRandom;

impl Random for RingRandom {
    fn fill(&self, buf: &mut [u8]) -> Result<(), String> {
        rustls::crypto::ring::default_provider().secure_random.fill(buf).map_err(|e| format!("random: {e:?}"))
    }
}
