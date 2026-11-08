use alloc::string::String;
use alloc::vec::Vec;

use super::bytes::{get_u16, get_u32, get_u64, put_u16, put_u32, put_u64};
use super::error::{FormatError, need};
use crate::checksum::crc::Crc32c;

/// First 4 bytes of every record.
pub const RECORD_MAGIC: [u8; 4] = *b"LSRC";
/// Fixed part of a record header; key and meta follow.
pub const RECORD_FIXED_LEN: usize = 48;

const O_MAGIC: usize = 0;
const O_FLAGS: usize = 4;
const O_BODY_LEN: usize = 8;
const O_RANGE_START: usize = 16;
const O_TOTAL_LEN: usize = 24;
const O_KEY_LEN: usize = 32;
const O_META_LEN: usize = 34;
const O_BODY_CRC: usize = 36;
// 40..44 reserved (zero)
const O_HDR_CRC: usize = 44;

/// Record flag bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecordFlags(pub u32);

impl RecordFlags {
    /// Body is one slice (`range_start..range_start+body_len`) of a larger object of `total_len` bytes.
    pub const SLICED: u32 = 1 << 0;
    /// Body was verified against an upstream-provided checksum before storing.
    pub const UPSTREAM_VERIFIED: u32 = 1 << 1;

    #[must_use]
    pub fn contains(self, bit: u32) -> bool {
        self.0 & bit == bit
    }
}

/// Fixed-size part of a record header, decodable before key/meta are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordFixed {
    pub flags: RecordFlags,
    pub body_len: u64,
    pub range_start: u64,
    pub total_len: u64,
    pub key_len: u16,
    pub meta_len: u16,
    pub body_crc: u32,
    pub hdr_crc: u32,
}

impl RecordFixed {
    /// Decodes the fixed part. Only checks magic; the header checksum covers
    /// key and meta too, so it is checked by [`RecordHeader::decode`].
    ///
    /// # Errors
    /// Short buffer or bad magic.
    pub fn decode(buf: &[u8]) -> Result<Self, FormatError> {
        need("record header", buf, RECORD_FIXED_LEN)?;
        if buf[O_MAGIC..O_MAGIC + 4] != RECORD_MAGIC {
            return Err(FormatError::BadMagic { what: "record header" });
        }
        Ok(Self {
            flags: RecordFlags(get_u32(buf, O_FLAGS)),
            body_len: get_u64(buf, O_BODY_LEN),
            range_start: get_u64(buf, O_RANGE_START),
            total_len: get_u64(buf, O_TOTAL_LEN),
            key_len: get_u16(buf, O_KEY_LEN),
            meta_len: get_u16(buf, O_META_LEN),
            body_crc: get_u32(buf, O_BODY_CRC),
            hdr_crc: get_u32(buf, O_HDR_CRC),
        })
    }

    /// Full header length: fixed part + key + meta.
    #[must_use]
    pub fn header_len(&self) -> usize {
        RECORD_FIXED_LEN + usize::from(self.key_len) + usize::from(self.meta_len)
    }
}

/// A complete record header (everything before the body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordHeader {
    pub flags: RecordFlags,
    pub body_len: u64,
    pub range_start: u64,
    pub total_len: u64,
    pub body_crc: u32,
    /// Cache key, e.g. `steam/depot/2357571/chunk/9de1…`.
    pub key: String,
    /// Opaque metadata (e.g. content type, upstream checksums).
    pub meta: Vec<u8>,
}

fn header_crc(fixed_without_crc: &[u8], key: &[u8], meta: &[u8]) -> u32 {
    let mut c = Crc32c::new();
    c.update(fixed_without_crc);
    c.update(key);
    c.update(meta);
    c.finish()
}

impl RecordHeader {
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        RECORD_FIXED_LEN + self.key.len() + self.meta.len()
    }

    /// Encodes fixed part + key + meta.
    ///
    /// # Errors
    /// Key or meta longer than 65535 bytes, or empty key.
    pub fn encode(&self) -> Result<Vec<u8>, FormatError> {
        let key_len = u16::try_from(self.key.len())
            .map_err(|_| FormatError::InvalidField { what: "key length", value: self.key.len() as u64 })?;
        if key_len == 0 {
            return Err(FormatError::InvalidField { what: "key length", value: 0 });
        }
        let meta_len = u16::try_from(self.meta.len())
            .map_err(|_| FormatError::InvalidField { what: "meta length", value: self.meta.len() as u64 })?;
        let mut b = Vec::with_capacity(self.encoded_len());
        b.resize(RECORD_FIXED_LEN, 0);
        b[O_MAGIC..O_MAGIC + 4].copy_from_slice(&RECORD_MAGIC);
        put_u32(&mut b, O_FLAGS, self.flags.0);
        put_u64(&mut b, O_BODY_LEN, self.body_len);
        put_u64(&mut b, O_RANGE_START, self.range_start);
        put_u64(&mut b, O_TOTAL_LEN, self.total_len);
        put_u16(&mut b, O_KEY_LEN, key_len);
        put_u16(&mut b, O_META_LEN, meta_len);
        put_u32(&mut b, O_BODY_CRC, self.body_crc);
        let crc = header_crc(&b[..O_HDR_CRC], self.key.as_bytes(), &self.meta);
        put_u32(&mut b, O_HDR_CRC, crc);
        b.extend_from_slice(self.key.as_bytes());
        b.extend_from_slice(&self.meta);
        Ok(b)
    }

    /// Decodes and verifies a full record header (fixed + key + meta).
    ///
    /// # Errors
    /// Short buffer, bad magic, checksum mismatch, invalid UTF-8 key.
    pub fn decode(buf: &[u8]) -> Result<Self, FormatError> {
        let fixed = RecordFixed::decode(buf)?;
        let total = fixed.header_len();
        need("record header + key + meta", buf, total)?;
        let key_end = RECORD_FIXED_LEN + usize::from(fixed.key_len);
        let key = &buf[RECORD_FIXED_LEN..key_end];
        let meta = &buf[key_end..total];
        let computed = header_crc(&buf[..O_HDR_CRC], key, meta);
        if computed != fixed.hdr_crc {
            return Err(FormatError::ChecksumMismatch { what: "record header", stored: fixed.hdr_crc, computed });
        }
        if fixed.key_len == 0 {
            return Err(FormatError::InvalidField { what: "key length", value: 0 });
        }
        let key = core::str::from_utf8(key).map_err(|_| FormatError::KeyNotUtf8)?;
        Ok(Self {
            flags: fixed.flags,
            body_len: fixed.body_len,
            range_start: fixed.range_start,
            total_len: fixed.total_len,
            body_crc: fixed.body_crc,
            key: String::from(key),
            meta: Vec::from(meta),
        })
    }
}

/// Rounds `value` up to a multiple of `alignment` (a power of two).
#[must_use]
pub fn align_up(value: u64, alignment: u32) -> u64 {
    let a = u64::from(alignment);
    debug_assert!(a.is_power_of_two());
    (value + a - 1) & !(a - 1)
}

/// Bytes a record occupies on disk, including padding to `alignment`.
#[must_use]
pub fn record_span(header_len: usize, body_len: u64, alignment: u32) -> u64 {
    align_up(header_len as u64 + body_len, alignment)
}
