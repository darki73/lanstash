//! Record metadata convention: stored response headers as `name=value\n` lines
//! (names lower-case). The format treats `meta` as opaque bytes; this is the
//! convention lanstash itself writes and reads.

use alloc::vec::Vec;

/// Encodes header pairs. Pairs whose name or value contains `=` in the name,
/// a newline, or non-ASCII control bytes are skipped (they cannot round-trip).
#[must_use]
pub fn encode_meta(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (n, v) in pairs {
        let ok = !n.is_empty()
            && !n.contains('=')
            && !n.bytes().chain(v.bytes()).any(|b| b == b'\n' || b == b'\r' || (b < 0x20 && b != b'\t'));
        if ok {
            out.extend_from_slice(n.to_ascii_lowercase().as_bytes());
            out.push(b'=');
            out.extend_from_slice(v.as_bytes());
            out.push(b'\n');
        }
    }
    out
}

/// Decodes `name=value` lines, skipping anything malformed.
pub fn decode_meta(meta: &[u8]) -> impl Iterator<Item = (&str, &str)> {
    core::str::from_utf8(meta)
        .unwrap_or("")
        .split('\n')
        .filter_map(|line| line.split_once('='))
        .filter(|(n, _)| !n.is_empty())
}
