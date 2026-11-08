use super::error::FormatError;
use super::header::HEADER_REGION;
use super::record::{align_up, record_span};
use super::validate_alignment;

/// Space reserved for one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// Offset of the record header.
    pub offset: u64,
    /// Bytes reserved, including alignment padding.
    pub span: u64,
}

/// The segment has reached its size target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentFull;

/// Pure space planner for an active segment: hands out non-overlapping,
/// aligned slots. Callers serialise access (e.g. under a mutex) and do the
/// actual writes outside it.
#[derive(Debug, Clone)]
pub struct SegmentLayout {
    alignment: u32,
    max_bytes: u64,
    next: u64,
    records: u64,
}

impl SegmentLayout {
    /// # Errors
    /// Invalid alignment, or `max_bytes` not larger than the header region.
    pub fn new(alignment: u32, max_bytes: u64) -> Result<Self, FormatError> {
        validate_alignment(alignment)?;
        if max_bytes <= HEADER_REGION {
            return Err(FormatError::InvalidField { what: "max_bytes", value: max_bytes });
        }
        Ok(Self { alignment, max_bytes, next: HEADER_REGION, records: 0 })
    }

    /// Resumes planning after recovery: the next record goes at `next`.
    ///
    /// # Errors
    /// See [`SegmentLayout::new`]; also `next` before the header region.
    pub fn resume(alignment: u32, max_bytes: u64, next: u64, records: u64) -> Result<Self, FormatError> {
        let mut l = Self::new(alignment, max_bytes)?;
        if next < HEADER_REGION {
            return Err(FormatError::InvalidField { what: "resume offset", value: next });
        }
        l.next = align_up(next, alignment);
        l.records = records;
        Ok(l)
    }

    /// Reserves space for a record. The first record always fits, so objects
    /// larger than `max_bytes` still get a (single-record) segment.
    ///
    /// # Errors
    /// [`SegmentFull`] when the record would exceed `max_bytes`.
    pub fn reserve(&mut self, header_len: usize, body_len: u64) -> Result<Slot, SegmentFull> {
        let span = record_span(header_len, body_len, self.alignment);
        if self.records > 0 && self.next + span > self.max_bytes {
            return Err(SegmentFull);
        }
        let slot = Slot { offset: self.next, span };
        self.next += span;
        self.records += 1;
        Ok(slot)
    }

    /// Where the footer of this segment would start if sealed now.
    #[must_use]
    pub fn end(&self) -> u64 {
        self.next
    }

    #[must_use]
    pub fn records(&self) -> u64 {
        self.records
    }

    #[must_use]
    pub fn alignment(&self) -> u32 {
        self.alignment
    }
}
