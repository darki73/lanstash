//! Secure random bytes from the operating system's generator, through ring.

use lanstash_ports::random::Random;
use ring::rand::{SecureRandom, SystemRandom};

/// The operating system's generator.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRng;

impl Random for SystemRng {
    fn fill(&self, buf: &mut [u8]) -> Result<(), String> {
        SystemRandom::new().fill(buf).map_err(|_| "the system's random generator failed".to_string())
    }
}
