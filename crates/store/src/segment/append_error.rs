//! Why an append to a segment, or its seal, failed.

use std::fmt;

use lanstash_domain::segment::error::FormatError;
use lanstash_ports::storage::StoreError;

/// Why an append or seal failed.
#[derive(Debug)]
pub enum AppendError {
    Store(StoreError),
    Format(FormatError),
    /// The segment reached its size target (or is closing); use a new segment.
    Full,
    /// The segment is already sealed.
    Sealed,
    /// Seal requested while appends are still writing.
    Busy {
        in_flight: usize,
    },
    /// The committed header does not match its reservation.
    Mismatch(&'static str),
}

impl fmt::Display for AppendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => write!(f, "storage: {e}"),
            Self::Format(e) => write!(f, "format: {e}"),
            Self::Full => write!(f, "segment full"),
            Self::Sealed => write!(f, "segment already sealed"),
            Self::Busy { in_flight } => write!(f, "{in_flight} appends still in flight"),
            Self::Mismatch(what) => write!(f, "record does not match its reservation: {what}"),
        }
    }
}

impl std::error::Error for AppendError {}

impl From<StoreError> for AppendError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

impl From<FormatError> for AppendError {
    fn from(e: FormatError) -> Self {
        Self::Format(e)
    }
}
