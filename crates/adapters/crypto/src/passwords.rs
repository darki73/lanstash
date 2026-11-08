//! The `Passwords` port as PBKDF2-HMAC-SHA256: a password is kept as
//! `pbkdf2-sha256$<iterations>$<salt hex>$<hash hex>` (a 16-byte salt from the system's generator, a
//! 32-byte hash), and checked by ring in constant time. The iterations are passed in (600,000 for
//! use, as OWASP advises for this hash; a few in tests); a kept hash says its own, so changing them
//! leaves old hashes working.

use lanstash_domain::accounts::{hex, unhex};
use lanstash_ports::passwords::Passwords;
use lanstash_ports::random::Random;
use ring::pbkdf2::{self, PBKDF2_HMAC_SHA256};

use crate::random::SystemRng;

/// The iterations a new hash uses.
pub const ITERATIONS: u32 = 600_000;

const KIND: &str = "pbkdf2-sha256";
const SALT: usize = 16;
const HASH: usize = 32;
/// The most iterations a kept hash may ask for (a bigger number is no hash of ours, and would hang).
const MAX_ITERATIONS: u32 = 10_000_000;

/// PBKDF2-HMAC-SHA256 passwords.
#[derive(Debug, Clone, Copy)]
pub struct Pbkdf2 {
    iterations: u32,
}

impl Pbkdf2 {
    /// New hashes use `iterations` (at least 1).
    #[must_use]
    pub fn new(iterations: u32) -> Self {
        Self { iterations: iterations.max(1) }
    }
}

impl Default for Pbkdf2 {
    fn default() -> Self {
        Self::new(ITERATIONS)
    }
}

/// The parts of a kept hash: iterations, salt and hash.
fn parts(kept: &str) -> Option<(std::num::NonZeroU32, Vec<u8>, Vec<u8>)> {
    let mut it = kept.split('$');
    if it.next()? != KIND {
        return None;
    }
    let n: u32 = it.next()?.parse().ok().filter(|n| *n <= MAX_ITERATIONS)?;
    let (salt, hash) = (unhex(it.next()?)?, unhex(it.next()?)?);
    if it.next().is_some() || salt.is_empty() || hash.len() != HASH {
        return None;
    }
    Some((std::num::NonZeroU32::new(n)?, salt, hash))
}

impl Passwords for Pbkdf2 {
    fn hash(&self, password: &str) -> Result<String, String> {
        let mut salt = [0u8; SALT];
        SystemRng.fill(&mut salt)?;
        let n = std::num::NonZeroU32::new(self.iterations).ok_or("no iterations")?;
        let mut hash = [0u8; HASH];
        pbkdf2::derive(PBKDF2_HMAC_SHA256, n, &salt, password.as_bytes(), &mut hash);
        Ok(format!("{KIND}${}${}${}", self.iterations, hex(&salt), hex(&hash)))
    }

    fn verify(&self, password: &str, kept: &str) -> bool {
        match parts(kept) {
            Some((n, salt, hash)) => pbkdf2::verify(PBKDF2_HMAC_SHA256, n, &salt, password.as_bytes(), &hash).is_ok(),
            None => {
                // As long as a real check: whether a name exists does not show in the time taken.
                let mut waste = [0u8; HASH];
                let n = std::num::NonZeroU32::new(self.iterations).unwrap_or(std::num::NonZeroU32::MIN);
                pbkdf2::derive(PBKDF2_HMAC_SHA256, n, &[0; SALT], password.as_bytes(), &mut waste);
                false
            }
        }
    }
}
