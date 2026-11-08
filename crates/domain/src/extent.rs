//! Byte-range arithmetic for objects stored in pieces (extents).
//!
//! [`Span`] is a half-open range `start..end`. [`ExtentMap`] holds non-overlapping
//! spans in order, each with a value (where the bytes are stored). Nothing here
//! knows about storage.

use alloc::vec::Vec;

/// Half-open byte range `start..end` (`end` exclusive). Empty when `start >= end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Span {
    pub start: u64,
    pub end: u64,
}

impl Span {
    #[must_use]
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    /// `start..start+len` (saturating).
    #[must_use]
    pub const fn at(start: u64, len: u64) -> Self {
        Self { start, end: start.saturating_add(len) }
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    #[must_use]
    pub const fn contains(&self, pos: u64) -> bool {
        self.start <= pos && pos < self.end
    }

    /// The common part, if any.
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let s = Self { start: self.start.max(other.start), end: self.end.min(other.end) };
        (!s.is_empty()).then_some(s)
    }
}

/// Two spans that were expected to be disjoint overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overlap {
    pub existing: Span,
    pub new: Span,
}

/// Values that say whether two touching extents can be stored as one.
pub trait Joinable {
    /// `self` covers the bytes just before `next` (`self_end == next_start`): can
    /// the two be one extent with `self`'s value?
    fn joins(&self, next: &Self) -> bool;
}

/// Non-overlapping spans in ascending order, each with a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtentMap<T> {
    items: Vec<(Span, T)>,
}

impl<T> Default for ExtentMap<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T: Joinable> ExtentMap<T> {
    /// Adds an extent; touching neighbours are joined when their values allow it.
    ///
    /// # Errors
    /// [`Overlap`] if `span` overlaps an existing extent (nothing is changed).
    /// Empty spans are ignored.
    pub fn insert(&mut self, span: Span, value: T) -> Result<(), Overlap> {
        if span.is_empty() {
            return Ok(());
        }
        // First extent that ends after span.start.
        let i = self.items.partition_point(|(s, _)| s.end <= span.start);
        if let Some((s, _)) = self.items.get(i)
            && s.start < span.end
        {
            return Err(Overlap { existing: *s, new: span });
        }
        let join_prev = i > 0 && self.items[i - 1].0.end == span.start && self.items[i - 1].1.joins(&value);
        let join_next = self.items.get(i).is_some_and(|(s, v)| s.start == span.end && value.joins(v));
        match (join_prev, join_next) {
            (true, true) => {
                let next_end = self.items[i].0.end;
                self.items.remove(i);
                self.items[i - 1].0.end = next_end;
            }
            (true, false) => self.items[i - 1].0.end = span.end,
            (false, true) => {
                self.items[i].0.start = span.start;
                self.items[i].1 = value;
            }
            (false, false) => self.items.insert(i, (span, value)),
        }
        Ok(())
    }
}

impl<T> ExtentMap<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The extent containing `pos`.
    #[must_use]
    pub fn at(&self, pos: u64) -> Option<(Span, &T)> {
        let i = self.items.partition_point(|(s, _)| s.end <= pos);
        self.items.get(i).filter(|(s, _)| s.start <= pos).map(|(s, v)| (*s, v))
    }

    /// Start of the first extent that begins after `pos` (where a gap at `pos` ends).
    #[must_use]
    pub fn next_start_after(&self, pos: u64) -> Option<u64> {
        let i = self.items.partition_point(|(s, _)| s.start <= pos);
        self.items.get(i).map(|(s, _)| s.start)
    }

    /// Whether every byte of `span` is covered.
    #[must_use]
    pub fn covers(&self, span: Span) -> bool {
        span.is_empty() || self.gaps(span).is_empty()
    }

    /// The parts of `span` that no extent covers, in order.
    #[must_use]
    pub fn gaps(&self, span: Span) -> Vec<Span> {
        let mut out = Vec::new();
        let mut pos = span.start;
        let i = self.items.partition_point(|(s, _)| s.end <= span.start);
        for (s, _) in &self.items[i..] {
            if s.start >= span.end {
                break;
            }
            if s.start > pos {
                out.push(Span::new(pos, s.start));
            }
            pos = pos.max(s.end);
        }
        if pos < span.end {
            out.push(Span::new(pos, span.end));
        }
        out
    }

    /// Total bytes covered.
    #[must_use]
    pub fn covered_bytes(&self) -> u64 {
        self.items.iter().map(|(s, _)| s.len()).sum()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Span, &T)> {
        self.items.iter().map(|(s, v)| (*s, v))
    }

    /// Keeps only the extents for which `keep` returns true.
    pub fn retain(&mut self, mut keep: impl FnMut(Span, &T) -> bool) {
        self.items.retain(|(s, v)| keep(*s, v));
    }
}

/// Parts of `span` not covered by `taken` (sorted by start; spans may overlap each other).
#[must_use]
pub fn uncovered(span: Span, taken: &[Span]) -> Vec<Span> {
    let mut out = Vec::new();
    let mut pos = span.start;
    for t in taken {
        if t.end <= pos {
            continue;
        }
        if t.start >= span.end {
            break;
        }
        if t.start > pos {
            out.push(Span::new(pos, t.start.min(span.end)));
        }
        pos = pos.max(t.end);
        if pos >= span.end {
            break;
        }
    }
    if pos < span.end {
        out.push(Span::new(pos, span.end));
    }
    out
}
