use alloc::vec::Vec;

use super::bytes::{get_u32, get_u64, put_u32, put_u64};
use super::error::{FormatError, need};
use crate::checksum::crc::crc32c;

/// Size of one footer entry.
pub const FOOTER_ENTRY_LEN: usize = 32;
/// Size of the trailer at the very end of a sealed segment.
pub const TRAILER_LEN: usize = 64;
/// Trailer magic.
pub const TRAILER_MAGIC: [u8; 8] = *b"LSTRAILR";

/// One index entry: where a record lives in this segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FooterEntry {
    /// [`key_hash`] of the record key.
    pub key_hash: u64,
    /// Offset of the record header.
    pub offset: u64,
    pub body_len: u64,
    /// Record flags (copied from the record header).
    pub flags: u32,
    /// Record header length (fixed part + key + meta): the body starts at `offset + header_len`.
    pub header_len: u32,
}

/// Stable 64-bit key hash (FNV-1a). Collisions are resolved by comparing the
/// full key stored in the record header, so distribution matters, strength does not.
#[must_use]
pub fn key_hash(key: &[u8]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for &b in key {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Encodes entries, sorted by `(key_hash, offset)` in place.
pub fn encode_footer(entries: &mut [FooterEntry]) -> Vec<u8> {
    entries.sort_unstable_by_key(|e| (e.key_hash, e.offset));
    let mut b = Vec::with_capacity(entries.len() * FOOTER_ENTRY_LEN);
    let mut e_buf = [0u8; FOOTER_ENTRY_LEN];
    for e in entries.iter() {
        put_u64(&mut e_buf, 0, e.key_hash);
        put_u64(&mut e_buf, 8, e.offset);
        put_u64(&mut e_buf, 16, e.body_len);
        put_u32(&mut e_buf, 24, e.flags);
        put_u32(&mut e_buf, 28, e.header_len);
        b.extend_from_slice(&e_buf);
    }
    b
}

/// Decodes a footer previously produced by [`encode_footer`].
///
/// # Errors
/// Length not a multiple of the entry size, or entries not sorted.
pub fn decode_footer(buf: &[u8]) -> Result<Vec<FooterEntry>, FormatError> {
    if !buf.len().is_multiple_of(FOOTER_ENTRY_LEN) {
        return Err(FormatError::InvalidField { what: "footer length", value: buf.len() as u64 });
    }
    let mut out = Vec::with_capacity(buf.len() / FOOTER_ENTRY_LEN);
    for c in buf.chunks_exact(FOOTER_ENTRY_LEN) {
        out.push(FooterEntry {
            key_hash: get_u64(c, 0),
            offset: get_u64(c, 8),
            body_len: get_u64(c, 16),
            flags: get_u32(c, 24),
            header_len: get_u32(c, 28),
        });
    }
    if out.windows(2).any(|w| (w[0].key_hash, w[0].offset) > (w[1].key_hash, w[1].offset)) {
        return Err(FormatError::InvalidField { what: "footer order", value: 0 });
    }
    Ok(out)
}

/// All entries with the given hash (binary search over a sorted footer).
#[must_use]
pub fn find_entries(sorted: &[FooterEntry], hash: u64) -> &[FooterEntry] {
    let start = sorted.partition_point(|e| e.key_hash < hash);
    let end = start + sorted[start..].partition_point(|e| e.key_hash == hash);
    &sorted[start..end]
}

/// Last [`TRAILER_LEN`] bytes of a sealed segment; locates the footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trailer {
    pub footer_offset: u64,
    pub entry_count: u64,
    pub footer_crc: u32,
}

impl Trailer {
    #[must_use]
    pub fn encode(&self) -> [u8; TRAILER_LEN] {
        let mut b = [0u8; TRAILER_LEN];
        b[0..8].copy_from_slice(&TRAILER_MAGIC);
        put_u64(&mut b, 8, self.footer_offset);
        put_u64(&mut b, 16, self.entry_count);
        put_u32(&mut b, 24, self.footer_crc);
        let crc = crc32c(&b[..60]);
        put_u32(&mut b, 60, crc);
        b
    }

    /// # Errors
    /// Short buffer, bad magic or checksum mismatch.
    pub fn decode(buf: &[u8]) -> Result<Self, FormatError> {
        need("trailer", buf, TRAILER_LEN)?;
        if buf[0..8] != TRAILER_MAGIC {
            return Err(FormatError::BadMagic { what: "trailer" });
        }
        let stored = get_u32(buf, 60);
        let computed = crc32c(&buf[..60]);
        if stored != computed {
            return Err(FormatError::ChecksumMismatch { what: "trailer", stored, computed });
        }
        Ok(Self { footer_offset: get_u64(buf, 8), entry_count: get_u64(buf, 16), footer_crc: get_u32(buf, 24) })
    }

    /// Footer size in bytes implied by `entry_count` (`u64::MAX` for a count no file
    /// could hold, which then fails every bounds check).
    #[must_use]
    pub fn footer_len(&self) -> u64 {
        self.entry_count.saturating_mul(FOOTER_ENTRY_LEN as u64)
    }
}
