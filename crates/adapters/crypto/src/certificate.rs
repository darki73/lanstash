//! A self-signed certificate for lanstash's own HTTPS: a new ECDSA P-256 key from the system's
//! generator, and the certificate ([`lanstash_domain::x509`]) for its names, signed with it; valid
//! from a day before it is made for [`VALID_DAYS`].

use lanstash_domain::x509::{AltName, CertSpec, certificate, to_be_signed};
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, KeyPair};

/// How long a certificate made here is valid.
pub const VALID_DAYS: u64 = 5 * 365;

const DAY_MS: u64 = 86_400_000;

/// A certificate and its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    /// The certificate, DER.
    pub certificate: Vec<u8>,
    /// The private key, PKCS#8 DER.
    pub key: Vec<u8>,
}

/// A new key and a certificate for `names`, made at `now_ms`, called `common_name`.
///
/// # Errors
/// The system's random generator or the signing failed.
pub fn make(common_name: &str, names: &[AltName], now_ms: u64) -> Result<Made, String> {
    let rng = SystemRandom::new();
    let key = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng)
        .map_err(|_| "no key could be made".to_string())?;
    let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, key.as_ref(), &rng)
        .map_err(|_| "the key made could not be read".to_string())?;
    let mut serial = [0u8; 16];
    rng.fill(&mut serial).map_err(|_| "the system's random generator failed".to_string())?;
    let spec = CertSpec {
        serial,
        not_before_ms: now_ms.saturating_sub(DAY_MS),
        not_after_ms: now_ms + VALID_DAYS * DAY_MS,
        common_name,
        names,
        public_key: pair.public_key().as_ref(),
    };
    let body = to_be_signed(&spec);
    let signature = pair.sign(&rng, &body).map_err(|_| "the certificate could not be signed".to_string())?;
    Ok(Made { certificate: certificate(&body, signature.as_ref()), key: key.as_ref().to_vec() })
}
