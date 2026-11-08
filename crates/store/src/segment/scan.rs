//! Use cases: read a segment's index fast (startup path), or scan it fully
//! (recovery of an unsealed segment, `dump`, `verify`).
//!
//! The full scan walks records from the header region. When a position does not
//! hold a valid record it searches forward, at alignment boundaries, for the next
//! valid record header. The gap is reported as unwritten (all zero, e.g. space
//! reserved by a writer that crashed) or corrupt (non-zero).

use std::fmt;

use lanstash_domain::checksum::crc::Crc32c;
use lanstash_domain::segment::error::FormatError;
use lanstash_domain::segment::footer::{FooterEntry, TRAILER_LEN, Trailer, decode_footer, key_hash};
use lanstash_domain::segment::header::{HEADER_ENCODED_LEN, HEADER_REGION, SegmentHeader};
use lanstash_domain::segment::record::{
    RECORD_FIXED_LEN, RECORD_MAGIC, RecordFixed, RecordHeader, align_up, record_span,
};

use lanstash_ports::storage::{FileRead, StoreError};

/// Fatal scan failure (the segment can't be interpreted at all).
#[derive(Debug)]
pub enum ScanError {
    Store(StoreError),
    /// The segment header itself is invalid.
    Header(FormatError),
    /// A trailer is present but the footer it points to is invalid.
    Footer(FormatError),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(e) => write!(f, "storage: {e}"),
            Self::Header(e) => write!(f, "segment header: {e}"),
            Self::Footer(e) => write!(f, "footer: {e}"),
        }
    }
}

impl std::error::Error for ScanError {}

impl From<StoreError> for ScanError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

/// Index of a sealed segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedIndex {
    pub trailer: Trailer,
    /// Sorted by `(key_hash, offset)`.
    pub entries: Vec<FooterEntry>,
}

/// One record found by a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedRecord {
    pub offset: u64,
    pub span: u64,
    pub header: RecordHeader,
    /// `Some(ok)` when bodies were verified.
    pub body_ok: Option<bool>,
}

/// Something wrong (or notable) found by a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Non-zero bytes that are not a valid record.
    Corrupt { offset: u64, len: u64, detail: String },
    /// Space between records that holds no record: all zero (reserved but never
    /// written), or a body whose header was never written (not committed yet, or never).
    Unwritten { offset: u64, len: u64 },
    /// Record body does not match its checksum.
    BodyChecksum { offset: u64, key: String },
    /// Trailer present but footer unreadable; segment treated as unsealed.
    FooterUnreadable { detail: String },
    /// Footer and scanned records disagree.
    FooterMismatch { detail: String },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Corrupt { offset, len, detail } => write!(f, "corrupt region at {offset} ({len} bytes): {detail}"),
            Self::Unwritten { offset, len } => write!(f, "unwritten region at {offset} ({len} bytes)"),
            Self::BodyChecksum { offset, key } => write!(f, "body checksum mismatch at {offset} ({key})"),
            Self::FooterUnreadable { detail } => write!(f, "footer unreadable: {detail}"),
            Self::FooterMismatch { detail } => write!(f, "footer mismatch: {detail}"),
        }
    }
}

/// Scan settings.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScanOptions {
    /// Read every body and check its CRC (slow: reads the whole segment).
    pub verify_bodies: bool,
}

/// Full scan result.
#[derive(Debug, Clone)]
pub struct SegmentReport {
    pub header: SegmentHeader,
    pub file_len: u64,
    pub sealed: Option<SealedIndex>,
    pub records: Vec<ScannedRecord>,
    pub problems: Vec<Problem>,
    /// Offset after the last valid record: where an unsealed segment can continue.
    pub resume_offset: u64,
}

impl SegmentReport {
    /// No corruption or mismatch found (unwritten gaps are not errors).
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.problems.iter().all(|p| matches!(p, Problem::Unwritten { .. }))
    }
}

/// Reads and validates the segment header.
///
/// # Errors
/// Storage failure or invalid header.
pub fn read_header(r: &dyn FileRead) -> Result<SegmentHeader, ScanError> {
    let mut buf = [0u8; HEADER_ENCODED_LEN];
    r.read_exact_at(0, &mut buf)?;
    SegmentHeader::decode(&buf).map_err(ScanError::Header)
}

/// Fast path for startup: reads only the trailer and footer.
/// Returns `Ok(None)` if the segment is not sealed (no valid trailer).
///
/// # Errors
/// Storage failure, or [`ScanError::Footer`] if a valid trailer points to a bad footer.
pub fn read_sealed_index(r: &dyn FileRead) -> Result<Option<SealedIndex>, ScanError> {
    let len = r.size()?;
    if len < HEADER_REGION + TRAILER_LEN as u64 {
        return Ok(None);
    }
    let mut tbuf = [0u8; TRAILER_LEN];
    r.read_exact_at(len - TRAILER_LEN as u64, &mut tbuf)?;
    let Ok(trailer) = Trailer::decode(&tbuf) else { return Ok(None) };
    let footer_end = trailer.footer_offset.checked_add(trailer.footer_len());
    if trailer.footer_offset < HEADER_REGION || footer_end != Some(len - TRAILER_LEN as u64) {
        return Err(ScanError::Footer(FormatError::InvalidField {
            what: "trailer footer_offset",
            value: trailer.footer_offset,
        }));
    }
    let footer_len = usize::try_from(trailer.footer_len()).map_err(|_| {
        ScanError::Footer(FormatError::InvalidField { what: "footer length", value: trailer.footer_len() })
    })?;
    let mut fbuf = vec![0u8; footer_len];
    r.read_exact_at(trailer.footer_offset, &mut fbuf)?;
    let computed = lanstash_domain::checksum::crc::crc32c(&fbuf);
    if computed != trailer.footer_crc {
        return Err(ScanError::Footer(FormatError::ChecksumMismatch {
            what: "footer",
            stored: trailer.footer_crc,
            computed,
        }));
    }
    let entries = decode_footer(&fbuf).map_err(ScanError::Footer)?;
    Ok(Some(SealedIndex { trailer, entries }))
}

/// Scans every record of a segment. See the module docs.
///
/// # Errors
/// Storage failure or invalid segment header. Damage inside the segment is
/// reported in [`SegmentReport::problems`], not as an error.
pub fn scan(r: &dyn FileRead, opts: ScanOptions) -> Result<SegmentReport, ScanError> {
    let header = read_header(r)?;
    let file_len = r.size()?;
    let mut problems = Vec::new();

    let sealed = match read_sealed_index(r) {
        Ok(s) => s,
        Err(ScanError::Footer(e)) => {
            problems.push(Problem::FooterUnreadable { detail: e.to_string() });
            None
        }
        Err(e) => return Err(e),
    };
    let records_end = sealed.as_ref().map_or(file_len, |s| s.trailer.footer_offset);
    let align = header.alignment;

    let mut records: Vec<ScannedRecord> = Vec::new();
    let mut pos = HEADER_REGION;
    let mut resume_offset = HEADER_REGION;
    while pos + RECORD_FIXED_LEN as u64 <= records_end {
        match try_record(r, pos, records_end, align, opts)? {
            Ok(rec) => {
                if rec.body_ok == Some(false) {
                    problems.push(Problem::BodyChecksum { offset: rec.offset, key: rec.header.key.clone() });
                }
                pos += rec.span;
                resume_offset = pos;
                records.push(rec);
            }
            Err(detail) => {
                let gap = find_next_record(r, pos, records_end, align, opts)?;
                let gap_end = gap.next.unwrap_or(records_end);
                let len = gap_end - pos;
                // A body written whose header never was: a write still going on, or one that
                // never finished. Not damage.
                let mut head = [0u8; RECORD_FIXED_LEN];
                let no_header = r.read_exact_at(pos, &mut head).is_ok() && head.iter().all(|&b| b == 0);
                problems.push(if gap.all_zero || no_header {
                    Problem::Unwritten { offset: pos, len }
                } else {
                    Problem::Corrupt { offset: pos, len, detail }
                });
                match gap.next {
                    Some(n) => pos = n,
                    None => break,
                }
            }
        }
    }
    // A trailing all-zero gap in an unsealed segment is just the unwritten tail.
    if sealed.is_none()
        && let Some(Problem::Unwritten { offset, len }) = problems.last()
        && offset + len == records_end
    {
        problems.pop();
    }

    if let Some(s) = &sealed {
        cross_check_footer(s, &records, &mut problems);
    }
    Ok(SegmentReport { header, file_len, sealed, records, problems, resume_offset })
}

/// `Ok(Err(detail))`: no valid record at `pos`. Outer `Err`: storage failure.
fn try_record(
    r: &dyn FileRead,
    pos: u64,
    end: u64,
    align: u32,
    opts: ScanOptions,
) -> Result<Result<ScannedRecord, String>, ScanError> {
    let mut fixed_buf = [0u8; RECORD_FIXED_LEN];
    r.read_exact_at(pos, &mut fixed_buf)?;
    let fixed = match RecordFixed::decode(&fixed_buf) {
        Ok(f) => f,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let header_len = fixed.header_len();
    let body_start = pos + header_len as u64;
    let Some(body_end) = body_start.checked_add(fixed.body_len) else {
        return Ok(Err(String::from("record length overflows")));
    };
    if body_end > end {
        return Ok(Err(format!("record claims {} body bytes, past end of records at {end}", fixed.body_len)));
    }
    let mut hbuf = vec![0u8; header_len];
    r.read_exact_at(pos, &mut hbuf)?;
    let header = match RecordHeader::decode(&hbuf) {
        Ok(h) => h,
        Err(e) => return Ok(Err(e.to_string())),
    };
    let body_ok =
        if opts.verify_bodies { Some(body_crc(r, body_start, header.body_len)? == header.body_crc) } else { None };
    let span = record_span(header_len, header.body_len, align);
    Ok(Ok(ScannedRecord { offset: pos, span, header, body_ok }))
}

fn body_crc(r: &dyn FileRead, start: u64, len: u64) -> Result<u32, ScanError> {
    Ok(crc32c_at(r, start, len)?)
}

/// CRC-32C of `len` bytes of `r` at `start`, read in 1 MiB pieces.
///
/// # Errors
/// Storage failure, or the file ends early.
pub fn crc32c_at(r: &dyn FileRead, start: u64, len: u64) -> Result<u32, StoreError> {
    const CHUNK: u64 = 1 << 20;
    let mut c = Crc32c::new();
    let mut buf = vec![0u8; usize::try_from(CHUNK.min(len)).unwrap_or(0)];
    let mut done = 0;
    while done < len {
        let n = usize::try_from(CHUNK.min(len - done)).unwrap_or(0);
        r.read_exact_at(start + done, &mut buf[..n])?;
        c.update(&buf[..n]);
        done += n as u64;
    }
    Ok(c.finish())
}

struct Gap {
    next: Option<u64>,
    all_zero: bool,
}

/// Searches aligned positions after `from` for the next valid record.
fn find_next_record(r: &dyn FileRead, from: u64, end: u64, align: u32, opts: ScanOptions) -> Result<Gap, ScanError> {
    const WINDOW: u64 = 1 << 20;
    let mut all_zero = true;
    let mut win_start = from;
    let mut candidate = align_up(from + 1, align);
    let mut buf = vec![0u8; WINDOW as usize];
    while win_start < end {
        let win_end = (win_start + WINDOW).min(end);
        let n = (win_end - win_start) as usize;
        r.read_exact_at(win_start, &mut buf[..n])?;
        let mut checked_to = win_start; // bytes [win_start, checked_to) examined for zero-ness
        while candidate + RECORD_MAGIC.len() as u64 <= win_end {
            let i = (candidate - win_start) as usize;
            if buf[i..i + RECORD_MAGIC.len()] == RECORD_MAGIC {
                let zero_upto = (candidate - win_start) as usize;
                let before = (checked_to - win_start) as usize;
                all_zero &= buf[before..zero_upto].iter().all(|&b| b == 0);
                checked_to = candidate;
                if let Ok(_rec) = try_record(r, candidate, end, align, opts)? {
                    return Ok(Gap { next: Some(candidate), all_zero });
                }
                all_zero = false; // a magic that isn't a valid record is damage
            }
            candidate += u64::from(align);
        }
        // Only bytes before the first unchecked candidate are final for this window.
        let settle = candidate.min(win_end);
        let before = (checked_to - win_start) as usize;
        let upto = (settle - win_start) as usize;
        if upto > before {
            all_zero &= buf[before..upto].iter().all(|&b| b == 0);
        }
        if settle <= win_start {
            break;
        }
        win_start = settle;
    }
    Ok(Gap { next: None, all_zero })
}

fn cross_check_footer(s: &SealedIndex, records: &[ScannedRecord], problems: &mut Vec<Problem>) {
    if s.entries.len() != records.len() {
        problems.push(Problem::FooterMismatch {
            detail: format!("footer lists {} records, scan found {}", s.entries.len(), records.len()),
        });
    }
    let mut by_offset: Vec<&FooterEntry> = s.entries.iter().collect();
    by_offset.sort_unstable_by_key(|e| e.offset);
    for rec in records {
        match by_offset.binary_search_by_key(&rec.offset, |e| e.offset) {
            Ok(i) => {
                let e = by_offset[i];
                if e.key_hash != key_hash(rec.header.key.as_bytes())
                    || e.body_len != rec.header.body_len
                    || e.header_len as usize != rec.header.encoded_len()
                {
                    problems.push(Problem::FooterMismatch {
                        detail: format!("entry at {} disagrees with record ({})", rec.offset, rec.header.key),
                    });
                }
            }
            Err(_) => problems.push(Problem::FooterMismatch {
                detail: format!("record at {} ({}) missing from footer", rec.offset, rec.header.key),
            }),
        }
    }
}
