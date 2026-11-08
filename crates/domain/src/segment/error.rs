use core::fmt;

/// A structure in a segment file could not be decoded or encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// Buffer shorter than the structure requires.
    TooShort { what: &'static str, needed: usize, got: usize },
    /// Magic bytes do not match.
    BadMagic { what: &'static str },
    /// Segment format version this build does not understand.
    UnsupportedVersion(u16),
    /// Stored checksum does not match the data.
    ChecksumMismatch { what: &'static str, stored: u32, computed: u32 },
    /// A field holds a value outside its allowed range.
    InvalidField { what: &'static str, value: u64 },
    /// Record key is not valid UTF-8.
    KeyNotUtf8,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { what, needed, got } => write!(f, "{what}: need {needed} bytes, got {got}"),
            Self::BadMagic { what } => write!(f, "{what}: bad magic"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported segment format version {v}"),
            Self::ChecksumMismatch { what, stored, computed } => {
                write!(f, "{what}: checksum mismatch (stored {stored:#010x}, computed {computed:#010x})")
            }
            Self::InvalidField { what, value } => write!(f, "{what}: invalid value {value}"),
            Self::KeyNotUtf8 => write!(f, "record key is not valid UTF-8"),
        }
    }
}

impl core::error::Error for FormatError {}

pub(crate) fn need(what: &'static str, buf: &[u8], needed: usize) -> Result<(), FormatError> {
    if buf.len() < needed { Err(FormatError::TooShort { what, needed, got: buf.len() }) } else { Ok(()) }
}
