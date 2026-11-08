//! The `.lcseg` segment file format, version 1.
//!
//! ```text
//! 0               SegmentHeader (64 B encoded, zero-padded to HEADER_REGION)
//! HEADER_REGION   Record*  each = RecordHeader (48 B fixed + key + meta) + body, padded to `alignment`
//! footer_offset   Footer   (sealed only) FooterEntry* sorted by (key_hash, offset)
//! EOF - 64        Trailer  (sealed only)
//! ```
//!
//! All integers are little-endian. Every structure carries its own CRC-32C.

mod bytes;
pub mod error;
pub mod footer;
pub mod header;
pub mod layout;
pub mod meta;
pub mod record;

use error::FormatError;

/// Largest supported record alignment. Must divide [`header::HEADER_REGION`].
pub const MAX_ALIGNMENT: u32 = 4096;

/// Validates a record alignment: a power of two in `1..=MAX_ALIGNMENT`.
pub fn validate_alignment(alignment: u32) -> Result<(), FormatError> {
    if alignment == 0 || !alignment.is_power_of_two() || alignment > MAX_ALIGNMENT {
        return Err(FormatError::InvalidField { what: "alignment", value: u64::from(alignment) });
    }
    Ok(())
}
