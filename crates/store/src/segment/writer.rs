//! Appending records to an active segment, then sealing it.
//!
//! Space is reserved under a short lock (pure [`SegmentLayout`] planning). The
//! body and header writes happen outside the lock, so parallel downloads into
//! the same segment don't serialise on I/O. A record enters the footer only
//! after both writes succeeded.
//!
//! The body is written before the record header: the header (with its own CRC)
//! is the commit marker, so a write cut short by a crash never leaves a valid
//! header in front of a missing body. Recovery also checks the body CRCs of
//! unsealed segments, for storage that reorders writes on power loss.
//!
//! A reservation can be filled piece by piece as a download arrives
//! ([`SegmentAppender::reserve`], [`SegmentAppender::write_body`],
//! [`SegmentAppender::commit`]). Closing a segment that still has reservations
//! open defers the seal to the moment the last one is committed or abandoned.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::segment::footer::{FooterEntry, Trailer, encode_footer, key_hash};
use lanstash_domain::segment::header::{HEADER_ENCODED_LEN, HEADER_REGION, SegmentHeader};
use lanstash_domain::segment::layout::SegmentLayout;
use lanstash_domain::segment::record::{RecordFlags, RecordHeader};
use lanstash_ports::storage::{FileWrite, StoreError};

use super::append_error::AppendError;
use super::scan::SegmentReport;

/// Description of a whole-object record to append (body passed separately).
#[derive(Debug, Clone, Copy)]
pub struct NewRecord<'a> {
    pub key: &'a str,
    pub meta: &'a [u8],
    pub flags: RecordFlags,
}

/// Where an appended record landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordLocation {
    pub offset: u64,
    pub span: u64,
    pub body_len: u64,
    /// Encoded record header length (the body starts at `offset + header_len`).
    pub header_len: u32,
}

/// Space reserved for one record whose body is still being written.
#[derive(Debug)]
#[must_use = "a reservation must be committed or abandoned, or the segment never seals"]
pub struct Reservation {
    pub location: RecordLocation,
    key: String,
    meta: Vec<u8>,
}

impl Reservation {
    /// File offset of the first body byte.
    #[must_use]
    pub fn body_offset(&self) -> u64 {
        self.location.offset + u64::from(self.location.header_len)
    }
}

/// Result of sealing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealInfo {
    pub footer_offset: u64,
    pub entry_count: u64,
    pub file_len: u64,
}

#[derive(Debug)]
struct State {
    layout: SegmentLayout,
    entries: Vec<FooterEntry>,
    in_flight: usize,
    /// No new reservations; seal when `in_flight` reaches zero.
    closing: bool,
    sealed: bool,
    /// Outcome of a deferred seal (set by whoever finished the last reservation).
    deferred: Option<Result<SealInfo, String>>,
}

/// An active (writable) segment.
#[derive(Debug)]
pub struct SegmentAppender<W: FileWrite> {
    file: W,
    header: SegmentHeader,
    state: Mutex<State>,
    /// Signalled when `in_flight` drops to zero.
    idle: Condvar,
}

fn whole_header(key: &str, meta: &[u8], flags: RecordFlags, body_len: u64, body_crc: u32) -> RecordHeader {
    RecordHeader {
        flags,
        body_len,
        range_start: 0,
        total_len: body_len,
        body_crc,
        key: key.to_owned(),
        meta: meta.to_vec(),
    }
}

impl<W: FileWrite> SegmentAppender<W> {
    fn with_state(file: W, header: SegmentHeader, layout: SegmentLayout, entries: Vec<FooterEntry>) -> Self {
        Self {
            file,
            header,
            state: Mutex::new(State { layout, entries, in_flight: 0, closing: false, sealed: false, deferred: None }),
            idle: Condvar::new(),
        }
    }

    /// Starts a new segment in an empty file: writes the header region.
    ///
    /// # Errors
    /// Invalid header or storage failure.
    pub fn create(file: W, header: SegmentHeader) -> Result<Self, AppendError> {
        let layout = SegmentLayout::new(header.alignment, header.max_bytes)?;
        let encoded = header.encode()?;
        let mut region = vec![0u8; HEADER_REGION as usize];
        region[..HEADER_ENCODED_LEN].copy_from_slice(&encoded);
        file.write_all_at(0, &region)?;
        Ok(Self::with_state(file, header, layout, Vec::new()))
    }

    /// Continues an unsealed segment after a restart. `report` must be a scan of
    /// this file with verified bodies: records with a bad body stay out of the
    /// footer, and everything after the last valid record (writes cut short by
    /// the crash) is cut off, so new records reuse that space.
    ///
    /// # Errors
    /// Already sealed, invalid header, or storage failure.
    pub fn resume(file: W, report: &SegmentReport) -> Result<Self, AppendError> {
        if report.sealed.is_some() {
            return Err(AppendError::Sealed);
        }
        let header = report.header;
        let records = report.records.len() as u64;
        let layout = SegmentLayout::resume(header.alignment, header.max_bytes, report.resume_offset, records)?;
        let entries = report
            .records
            .iter()
            .filter(|r| r.body_ok != Some(false))
            .map(|r| FooterEntry {
                key_hash: key_hash(r.header.key.as_bytes()),
                offset: r.offset,
                body_len: r.header.body_len,
                flags: r.header.flags.0,
                header_len: u32::try_from(r.header.encoded_len()).unwrap_or(u32::MAX),
            })
            .collect();
        file.set_len(layout.end())?;
        Ok(Self::with_state(file, header, layout, entries))
    }

    #[must_use]
    pub fn header(&self) -> &SegmentHeader {
        &self.header
    }

    /// Records committed so far.
    #[must_use]
    pub fn record_count(&self) -> usize {
        self.lock().entries.len()
    }

    /// Reserves space for a whole-object record of `body_len` bytes.
    ///
    /// # Errors
    /// [`AppendError::Full`] when the segment reached its target size or is
    /// closing, [`AppendError::Sealed`], or an invalid key or metadata.
    pub fn reserve(&self, key: &str, meta: &[u8], body_len: u64) -> Result<Reservation, AppendError> {
        // The header length depends only on key and meta, not on the CRC.
        let header_len = whole_header(key, meta, RecordFlags::default(), body_len, 0).encode()?.len();
        let mut s = self.lock();
        if s.sealed {
            return Err(AppendError::Sealed);
        }
        if s.closing {
            return Err(AppendError::Full);
        }
        let slot = s.layout.reserve(header_len, body_len).map_err(|_| AppendError::Full)?;
        s.in_flight += 1;
        let header_len = u32::try_from(header_len).unwrap_or(u32::MAX);
        Ok(Reservation {
            location: RecordLocation { offset: slot.offset, span: slot.span, body_len, header_len },
            key: key.to_owned(),
            meta: meta.to_vec(),
        })
    }

    /// Writes body bytes at `at` (relative to the body start) of a reservation.
    ///
    /// # Errors
    /// Out of the reserved body, or storage failure.
    pub fn write_body(&self, r: &Reservation, at: u64, data: &[u8]) -> Result<(), StoreError> {
        if at + data.len() as u64 > r.location.body_len {
            return Err(StoreError::other("write past the reserved body"));
        }
        self.file.write_all_at(r.body_offset() + at, data)
    }

    /// The file being written (to read back body bytes written so far).
    pub fn file(&self) -> &W {
        &self.file
    }

    /// Writes the record header, committing the record. `body_crc` is the
    /// CRC-32C of the whole body as written.
    ///
    /// # Errors
    /// Storage failure (the record is then lost, the space stays unused).
    pub fn commit(&self, r: Reservation, flags: RecordFlags, body_crc: u32) -> Result<RecordLocation, AppendError> {
        let header = whole_header(&r.key, &r.meta, flags, r.location.body_len, body_crc);
        let result = header.encode().map_err(AppendError::from).and_then(|bytes| {
            if bytes.len() != r.location.header_len as usize {
                return Err(AppendError::Mismatch("header length"));
            }
            self.file.write_all_at(r.location.offset, &bytes).map_err(AppendError::from)
        });
        let mut s = self.lock();
        if result.is_ok() {
            s.entries.push(FooterEntry {
                key_hash: key_hash(r.key.as_bytes()),
                offset: r.location.offset,
                body_len: r.location.body_len,
                flags: flags.0,
                header_len: r.location.header_len,
            });
        }
        self.finish_one(s);
        result.map(|()| r.location)
    }

    /// Gives up a reservation (e.g. the download failed). Its space stays unused.
    pub fn abandon(&self, r: Reservation) {
        drop(r);
        let s = self.lock();
        self.finish_one(s);
    }

    /// One reservation ended; seal if the segment is closing and now idle.
    fn finish_one(&self, mut s: MutexGuard<'_, State>) {
        s.in_flight -= 1;
        if s.in_flight == 0 {
            self.idle.notify_all();
            if s.closing && !s.sealed {
                let r = self.seal_locked(&mut s).map_err(|e| e.to_string());
                s.deferred = Some(r);
            }
        }
    }

    /// Appends a whole record in one call.
    ///
    /// # Errors
    /// See [`SegmentAppender::reserve`]; storage failures.
    pub fn append(&self, rec: NewRecord<'_>, body: &[u8]) -> Result<RecordLocation, AppendError> {
        let r = self.reserve(rec.key, rec.meta, body.len() as u64)?;
        if let Err(e) = self.write_body(&r, 0, body) {
            self.abandon(r);
            return Err(e.into());
        }
        self.commit(r, rec.flags, crc32c(body))
    }

    fn seal_locked(&self, s: &mut State) -> Result<SealInfo, AppendError> {
        let footer_offset = s.layout.end();
        let footer = encode_footer(&mut s.entries);
        let trailer =
            Trailer { footer_offset, entry_count: s.entries.len() as u64, footer_crc: crc32c(&footer) }.encode();
        // The records first: a sealed segment's bodies are trusted without their checksums,
        // so the footer must never reach the disk before them.
        self.file.sync()?;
        self.file.write_all_at(footer_offset, &footer)?;
        let trailer_offset = footer_offset + footer.len() as u64;
        self.file.write_all_at(trailer_offset, &trailer)?;
        self.file.sync()?;
        s.sealed = true;
        Ok(SealInfo {
            footer_offset,
            entry_count: s.entries.len() as u64,
            file_len: trailer_offset + trailer.len() as u64,
        })
    }

    /// Writes footer and trailer and syncs. The segment is immutable afterwards.
    ///
    /// # Errors
    /// [`AppendError::Busy`] while reservations are open, [`AppendError::Sealed`], or storage failure.
    pub fn seal(&self) -> Result<SealInfo, AppendError> {
        let mut s = self.lock();
        if s.sealed {
            return Err(AppendError::Sealed);
        }
        if s.in_flight > 0 {
            return Err(AppendError::Busy { in_flight: s.in_flight });
        }
        self.seal_locked(&mut s)
    }

    /// Stops new reservations. Seals now if none are open (returning the seal),
    /// otherwise when the last open one ends (see [`SegmentAppender::deferred_seal`]).
    ///
    /// # Errors
    /// [`AppendError::Sealed`] if already sealed, or storage failure of an immediate seal.
    pub fn close(&self) -> Result<Option<SealInfo>, AppendError> {
        let mut s = self.lock();
        if s.sealed {
            return Err(AppendError::Sealed);
        }
        s.closing = true;
        if s.in_flight == 0 { self.seal_locked(&mut s).map(Some) } else { Ok(None) }
    }

    /// The outcome of a seal deferred by [`SegmentAppender::close`], once it happened.
    #[must_use]
    pub fn deferred_seal(&self) -> Option<Result<SealInfo, String>> {
        self.lock().deferred.clone()
    }

    /// Stops new reservations, waits for open ones, then seals.
    ///
    /// # Errors
    /// [`AppendError::Sealed`] if already sealed, or storage failure.
    pub fn seal_when_idle(&self) -> Result<SealInfo, AppendError> {
        let mut s = self.lock();
        s.closing = true;
        while s.in_flight > 0 {
            s = self.idle.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
        if s.sealed {
            return match s.deferred.clone() {
                Some(Ok(info)) => Ok(info),
                _ => Err(AppendError::Sealed),
            };
        }
        self.seal_locked(&mut s)
    }

    #[must_use]
    pub fn is_sealed(&self) -> bool {
        self.lock().sealed
    }

    /// Gives back the underlying file.
    pub fn into_inner(self) -> W {
        self.file
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock can't leave `State` half-updated in a
        // way that matters for safety; continue with the inner value.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
