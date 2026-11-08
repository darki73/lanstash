//! A self-signed X.509 certificate for lanstash's own HTTPS (the admin port), written as DER (RFC
//! 5280): ECDSA P-256 with SHA-256; the issuer and subject one common name; valid from a day
//! before it was made; not a CA; for a TLS server only (key usage: signatures; extended: server
//! authentication); its names (host names, addresses) as subject alternative names, which a
//! browser matches the address against. The signing is the caller's: [`to_be_signed`] gives the
//! bytes to sign, [`certificate`] puts the signature on.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

/// A name the certificate is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AltName {
    Dns(String),
    V4([u8; 4]),
    V6([u8; 16]),
}

/// What the certificate says.
#[derive(Debug, Clone)]
pub struct CertSpec<'a> {
    /// Random; made positive and without a leading zero byte here.
    pub serial: [u8; 16],
    pub not_before_ms: u64,
    pub not_after_ms: u64,
    pub common_name: &'a str,
    pub names: &'a [AltName],
    /// The public key as an uncompressed P-256 point (65 bytes, starting with 4).
    pub public_key: &'a [u8],
}

const ECDSA_SHA256: &[u8] = &[0x06, 0x08, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x04, 0x03, 0x02];
const EC_PUBLIC_KEY: &[u8] = &[0x06, 0x07, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01];
const P256: &[u8] = &[0x06, 0x08, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];
const COMMON_NAME: &[u8] = &[0x06, 0x03, 0x55, 0x04, 0x03];
const BASIC_CONSTRAINTS: &[u8] = &[0x06, 0x03, 0x55, 0x1D, 0x13];
const KEY_USAGE: &[u8] = &[0x06, 0x03, 0x55, 0x1D, 0x0F];
const EXT_KEY_USAGE: &[u8] = &[0x06, 0x03, 0x55, 0x1D, 0x25];
const SUBJECT_ALT_NAME: &[u8] = &[0x06, 0x03, 0x55, 0x1D, 0x11];
const SERVER_AUTH: &[u8] = &[0x06, 0x08, 0x2B, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x01];
const CRITICAL: &[u8] = &[0x01, 0x01, 0xFF];

/// A DER element: its tag, its length, its content.
#[must_use]
pub fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let n = content.len();
    let mut out = Vec::with_capacity(n + 4);
    out.push(tag);
    match n {
        0..=0x7F => out.push(u8::try_from(n).unwrap_or(0)),
        0x80..=0xFF => out.extend([0x81, u8::try_from(n).unwrap_or(0)]),
        _ => out.extend([0x82, u8::try_from(n >> 8).unwrap_or(0xFF), u8::try_from(n & 0xFF).unwrap_or(0)]),
    }
    out.extend_from_slice(content);
    out
}

fn seq(parts: &[&[u8]]) -> Vec<u8> {
    der(0x30, &parts.concat())
}

fn bits(content: &[u8]) -> Vec<u8> {
    der(0x03, &[&[0u8][..], content].concat())
}

/// A day number since 1970-01-01 as (year, month, day) (proleptic Gregorian).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// A time as X.509 writes it: UTCTime (`YYMMDDHHMMSSZ`) up to 2049, GeneralizedTime after.
#[must_use]
pub fn time(ms: u64) -> Vec<u8> {
    let secs = i64::try_from(ms / 1000).unwrap_or(i64::MAX);
    let (y, mo, d) = civil(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    let (h, mi, se) = (s / 3600, s % 3600 / 60, s % 60);
    let mut text = String::new();
    let tag = if (1950..2050).contains(&y) {
        let _ = write!(text, "{:02}", y % 100);
        0x17
    } else {
        let _ = write!(text, "{y:04}");
        0x18
    };
    let _ = write!(text, "{mo:02}{d:02}{h:02}{mi:02}{se:02}Z");
    der(tag, text.as_bytes())
}

fn name(common_name: &str) -> Vec<u8> {
    let atv = seq(&[COMMON_NAME, &der(0x0C, common_name.as_bytes())]);
    seq(&[&der(0x31, &atv)])
}

fn extension(oid: &[u8], critical: bool, value: &[u8]) -> Vec<u8> {
    let crit: &[u8] = if critical { CRITICAL } else { &[] };
    seq(&[oid, crit, &der(0x04, value)])
}

/// The certificate's body, the bytes the issuer signs (ECDSA with SHA-256).
#[must_use]
pub fn to_be_signed(spec: &CertSpec<'_>) -> Vec<u8> {
    let mut serial = spec.serial;
    serial[0] = (serial[0] & 0x7F) | 0x40;
    let names: Vec<u8> = spec
        .names
        .iter()
        .flat_map(|n| match n {
            AltName::Dns(d) => der(0x82, d.as_bytes()),
            AltName::V4(a) => der(0x87, a),
            AltName::V6(a) => der(0x87, a),
        })
        .collect();
    let extensions = [
        extension(BASIC_CONSTRAINTS, true, &seq(&[])),
        // digitalSignature only.
        extension(KEY_USAGE, true, &der(0x03, &[0x07, 0x80])),
        extension(EXT_KEY_USAGE, false, &seq(&[SERVER_AUTH])),
        extension(SUBJECT_ALT_NAME, false, &der(0x30, &names)),
    ]
    .concat();
    let who = name(spec.common_name);
    seq(&[
        &der(0xA0, &der(0x02, &[2])),
        &der(0x02, &serial),
        &seq(&[ECDSA_SHA256]),
        &who,
        &seq(&[&time(spec.not_before_ms), &time(spec.not_after_ms)]),
        &who,
        &seq(&[&seq(&[EC_PUBLIC_KEY, P256]), &bits(spec.public_key)]),
        &der(0xA3, &der(0x30, &extensions)),
    ])
}

/// The certificate: its body and the issuer's signature of it (ECDSA, DER-encoded).
#[must_use]
pub fn certificate(to_be_signed: &[u8], signature: &[u8]) -> Vec<u8> {
    seq(&[to_be_signed, &seq(&[ECDSA_SHA256]), &bits(signature)])
}
