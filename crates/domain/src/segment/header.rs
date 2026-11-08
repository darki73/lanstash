use super::bytes::{get_u16, get_u32, get_u64, put_u16, put_u32, put_u64};
use super::error::{FormatError, need};
use super::validate_alignment;
use crate::checksum::crc::crc32c;

/// First 8 bytes of every segment file.
pub const SEGMENT_MAGIC: [u8; 8] = *b"LANSTASH";
/// Segment format version written by this build.
pub const FORMAT_VERSION: u16 = 1;
/// Bytes reserved for the header; the first record starts here.
pub const HEADER_REGION: u64 = 4096;
/// Encoded header size (the rest of [`HEADER_REGION`] is zero).
pub const HEADER_ENCODED_LEN: usize = 64;

// Field offsets.
const O_MAGIC: usize = 0;
const O_VERSION: usize = 8;
const O_FLAGS: usize = 10;
const O_ALIGNMENT: usize = 12;
const O_SEGMENT_ID: usize = 16;
const O_SERVICE: usize = 24;
const O_GAME_ID: usize = 32;
const O_CREATED: usize = 40;
const O_MAX_BYTES: usize = 48;
const O_CRC: usize = 56;

/// Content source of a segment. Unknown values are preserved, not rejected,
/// so newer services do not break older tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Service(pub u16);

impl Service {
    pub const UNKNOWN: Self = Self(0);
    pub const STEAM: Self = Self(1);
    pub const EPIC: Self = Self(2);
    pub const BLIZZARD: Self = Self(3);
    pub const RIOT: Self = Self(4);

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::UNKNOWN => "unknown",
            Self::STEAM => "steam",
            Self::EPIC => "epic",
            Self::BLIZZARD => "blizzard",
            Self::RIOT => "riot",
            _ => "unrecognised",
        }
    }
}

/// Segment-wide metadata, stored at offset 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentHeader {
    pub flags: u16,
    /// Record alignment in bytes (power of two, ≤ 4096).
    pub alignment: u32,
    pub segment_id: u64,
    pub service: Service,
    /// Game/app id within the service; 0 = unknown.
    pub game_id: u64,
    /// Creation time, unix milliseconds.
    pub created_at_ms: u64,
    /// Size target the writer used for this segment.
    pub max_bytes: u64,
}

impl SegmentHeader {
    /// Encodes the header (version [`FORMAT_VERSION`]).
    ///
    /// # Errors
    /// Invalid alignment.
    pub fn encode(&self) -> Result<[u8; HEADER_ENCODED_LEN], FormatError> {
        validate_alignment(self.alignment)?;
        let mut b = [0u8; HEADER_ENCODED_LEN];
        b[O_MAGIC..O_MAGIC + 8].copy_from_slice(&SEGMENT_MAGIC);
        put_u16(&mut b, O_VERSION, FORMAT_VERSION);
        put_u16(&mut b, O_FLAGS, self.flags);
        put_u32(&mut b, O_ALIGNMENT, self.alignment);
        put_u64(&mut b, O_SEGMENT_ID, self.segment_id);
        put_u16(&mut b, O_SERVICE, self.service.0);
        put_u64(&mut b, O_GAME_ID, self.game_id);
        put_u64(&mut b, O_CREATED, self.created_at_ms);
        put_u64(&mut b, O_MAX_BYTES, self.max_bytes);
        let crc = crc32c(&b[..O_CRC]);
        put_u32(&mut b, O_CRC, crc);
        Ok(b)
    }

    /// Decodes and validates a header from the start of a segment.
    ///
    /// # Errors
    /// Short buffer, bad magic, unsupported version, checksum or field errors.
    pub fn decode(buf: &[u8]) -> Result<Self, FormatError> {
        need("segment header", buf, HEADER_ENCODED_LEN)?;
        if buf[O_MAGIC..O_MAGIC + 8] != SEGMENT_MAGIC {
            return Err(FormatError::BadMagic { what: "segment header" });
        }
        let version = get_u16(buf, O_VERSION);
        if version != FORMAT_VERSION {
            return Err(FormatError::UnsupportedVersion(version));
        }
        let stored = get_u32(buf, O_CRC);
        let computed = crc32c(&buf[..O_CRC]);
        if stored != computed {
            return Err(FormatError::ChecksumMismatch { what: "segment header", stored, computed });
        }
        // Bytes after the CRC are reserved and outside it, so they must be zero.
        if buf[O_CRC + 4..HEADER_ENCODED_LEN].iter().any(|&b| b != 0) {
            return Err(FormatError::InvalidField { what: "segment header reserved bytes", value: 0 });
        }
        let h = Self {
            flags: get_u16(buf, O_FLAGS),
            alignment: get_u32(buf, O_ALIGNMENT),
            segment_id: get_u64(buf, O_SEGMENT_ID),
            service: Service(get_u16(buf, O_SERVICE)),
            game_id: get_u64(buf, O_GAME_ID),
            created_at_ms: get_u64(buf, O_CREATED),
            max_bytes: get_u64(buf, O_MAX_BYTES),
        };
        validate_alignment(h.alignment)?;
        Ok(h)
    }
}
