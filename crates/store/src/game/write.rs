//! Writing: segment records, object files and their extents, and keeping
//! the journal compact.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_domain::extent::Span;
use lanstash_domain::journal::{Entry, JOURNAL_VERSION, live_objects, read_journal};
use lanstash_domain::object::{fallback_rel_path, safe_rel_path};
use lanstash_domain::segment::header::SegmentHeader;
use lanstash_domain::service::service_code;

use crate::index::Index;
use crate::journal::JournalFile;
use crate::layout::object_rel_path;
use crate::segment::append_error::AppendError;
use crate::segment::writer::{Reservation, SegmentAppender};
use lanstash_ports::clock::Clock;
use lanstash_ports::storage::{FileRead, FileWrite, StoreError, StoreErrorKind};

use super::{ActiveSegment, COMPACT_MIN_DEAD, FilesState, Game, ObjectFile, SegmentOptions, lock, read_whole};

impl Game {
    /// Rewrites the journal without dead entries once they are at least
    /// [`COMPACT_MIN_DEAD`] and outnumber the live ones. The live entries are
    /// read back from the file (with their CRCs), written to a temp file,
    /// synced and renamed over the journal: a crash leaves the old or the new
    /// one. Object numbers stay the same. Called with the files lock held.
    pub(super) fn maybe_compact(&self, st: &mut FilesState) -> Result<bool, StoreError> {
        let live: u64 = 1 + st.entries_of.values().sum::<u64>();
        if st.dead < COMPACT_MIN_DEAD || st.dead <= live || st.journal.is_none() {
            return Ok(false);
        }
        let path = self.dir.journal();
        let read = read_journal(&read_whole(&*self.fs, &path)?);
        let mut out =
            Entry::Header { version: JOURNAL_VERSION }.encode().map_err(|e| StoreError::other(e.to_string()))?;
        let mut entries_of = HashMap::new();
        for o in live_objects(read.entries).objects {
            if !st.by_no.contains_key(&o.no) {
                continue; // not loaded (a conflict at startup): keep it out
            }
            let mut seen = HashSet::new();
            let extents: Vec<(u64, u64, u32)> = o.extents.into_iter().filter(|e| seen.insert(*e)).collect();
            entries_of.insert(o.no, 1 + extents.len() as u64);
            let obj = Entry::Object { no: o.no, total_len: o.total_len, key: o.key, path: o.path, meta: o.meta };
            for e in std::iter::once(obj).chain(extents.into_iter().map(|(start, len, crc)| Entry::Extent {
                no: o.no,
                start,
                len,
                crc,
            })) {
                out.extend(e.encode().map_err(|err| StoreError::other(err.to_string()))?);
            }
        }
        let tmp = self.dir.journal_tmp();
        let _ = self.fs.remove_file(&tmp);
        let f = self.fs.create_new(&tmp)?;
        f.write_all_at(0, &out)?;
        f.sync()?;
        drop(f);
        st.journal = None; // close it before replacing it (Windows refuses otherwise)
        let renamed = self.fs.rename(&tmp, &path);
        // Either way a journal is there again: reopen it for appending.
        st.journal = Some(JournalFile::open(&*self.fs, &path)?.0);
        renamed?;
        st.entries_of = entries_of;
        st.dead = 0;
        Ok(true)
    }

    /// Compacts the journal if it is mostly dead entries (see `maybe_compact`).
    ///
    /// # Errors
    /// Storage failure (the old journal is then still in place).
    pub fn compact_journal(&self) -> Result<bool, StoreError> {
        self.maybe_compact(&mut lock(&self.files))
    }

    /// Reserves a whole-object record of `body_len` bytes in the active segment,
    /// starting a new segment when the current one is full.
    ///
    /// # Errors
    /// Storage failure.
    pub fn reserve_record(
        &self,
        index: &Index,
        opts: SegmentOptions,
        clock: &dyn Clock,
        key: &str,
        meta: &[u8],
        body_len: u64,
    ) -> Result<(Arc<ActiveSegment>, Reservation), StoreError> {
        // A fresh segment always takes its first record; more rounds only cover
        // races with other threads rotating the same game.
        for _ in 0..16 {
            let active = self.active_segment(index, opts, clock)?;
            match active.appender.reserve(key, meta, body_len) {
                Ok(r) => return Ok((active, r)),
                Err(AppendError::Full | AppendError::Sealed) => self.retire(&active),
                Err(e) => return Err(StoreError::other(e.to_string())),
            }
        }
        Err(StoreError::other("could not find room in a segment"))
    }

    fn active_segment(
        &self,
        index: &Index,
        opts: SegmentOptions,
        clock: &dyn Clock,
    ) -> Result<Arc<ActiveSegment>, StoreError> {
        let mut st = lock(&self.seg);
        if let Some(a) = &st.active {
            return Ok(Arc::clone(a));
        }
        // Creating a file is rare (once per segment of data); doing it under the
        // lock keeps naming simple.
        loop {
            let seq = st.next_seq;
            st.next_seq = seq.checked_add(1).ok_or_else(|| StoreError::other("segment numbers used up"))?;
            let path = self.dir.segment(seq);
            let file = match self.fs.create_new(&path) {
                Ok(f) => f,
                Err(e) if e.kind == StoreErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            };
            let header = SegmentHeader {
                flags: 0,
                alignment: opts.alignment,
                segment_id: self.segment_ids.fetch_add(1, Ordering::Relaxed),
                service: service_code(&self.dir.service),
                game_id: self.dir.game.parse().unwrap_or(0),
                created_at_ms: clock.now_ms(),
                max_bytes: opts.max_bytes,
            };
            let appender =
                SegmentAppender::create(Arc::clone(&file), header).map_err(|e| StoreError::other(e.to_string()))?;
            let reader: Arc<dyn FileRead> = file as Arc<dyn FileRead>;
            let id = self.add_segment(index, seq, path.clone(), reader, 0);
            let a = Arc::new(ActiveSegment { id, path, appender });
            st.active = Some(Arc::clone(&a));
            return Ok(a);
        }
    }

    /// Replaces `full` as the active segment (if nobody did yet) and closes it;
    /// it seals once its open reservations end.
    fn retire(&self, full: &Arc<ActiveSegment>) {
        {
            let mut st = lock(&self.seg);
            if st.active.as_ref().is_some_and(|a| Arc::ptr_eq(a, full)) {
                st.active = None;
            }
        }
        let _ = full.appender.close(); // `Sealed`: someone else closed it already
    }

    /// Closes the active segment (seals it once idle), e.g. at the end of an import.
    pub fn close_active(&self) -> Option<Arc<ActiveSegment>> {
        let a = lock(&self.seg).active.take()?;
        let _ = a.appender.close();
        Some(a)
    }

    /// The object file for `key`, creating it (at `total` bytes, sparse) if needed.
    ///
    /// # Errors
    /// A different total is already recorded for `key`, or storage failure.
    pub fn object_file(
        &self,
        index: &Index,
        key: &str,
        cdn_path: &str,
        total: u64,
        meta: &[u8],
    ) -> Result<Arc<ObjectFile>, StoreError> {
        let mut st = lock(&self.files);
        if let Some(f) = st.by_key.get(key) {
            if f.total != total {
                return Err(StoreError::other(format!("{key}: stored total {} differs from {total}", f.total)));
            }
            return Ok(Arc::clone(f));
        }
        if st.journal.is_none() {
            st.journal = Some(JournalFile::open_or_create(&*self.fs, &self.dir.journal())?.0);
        }
        let no = st.next_no;
        let next_no = no.checked_add(1).ok_or_else(|| StoreError::other("object file numbers used up"))?;
        // The CDN path, else a name from the key's hash; never a name a live object has.
        let free = |rel: &String| !st.paths.contains(&rel.to_lowercase());
        let rel = core::iter::once(object_rel_path(&safe_rel_path(cdn_path)))
            .chain((0u32..).map(|n| {
                let base = fallback_rel_path(key);
                object_rel_path(&if n == 0 { base } else { format!("{base}-{n}") })
            }))
            .find(free)
            .unwrap_or_default();
        let path = self.dir.file(&rel);
        let file = match self.fs.create_new(&path) {
            Ok(f) => f,
            // A file no journal entry knows: left over from a crash. It goes, and a new one
            // takes its name (removed, not opened: whatever it is, it is not written through).
            Err(e) if e.kind == StoreErrorKind::AlreadyExists => {
                self.fs.remove_file(&path)?;
                self.fs.create_new(&path)?
            }
            Err(e) => return Err(e),
        };
        file.set_len(total)?;
        let entry = Entry::Object { no, total_len: total, key: key.to_owned(), path: rel.clone(), meta: meta.to_vec() };
        st.journal.as_mut().ok_or_else(|| StoreError::other("journal missing"))?.append(&entry)?;
        st.entries_of.insert(no, 1);
        st.next_no = next_no;
        let obj = Arc::new(ObjectFile::new(
            self.dir.clone(),
            no,
            key.to_owned(),
            path,
            total,
            Arc::from(meta),
            Arc::clone(&self.fs),
        ));
        *lock(&obj.write) = Some(file);
        obj.id.store(index.add_file(Arc::clone(&obj)), Ordering::Relaxed);
        st.paths.insert(rel.to_lowercase());
        st.by_no.insert(no, Arc::clone(&obj));
        st.by_key.insert(key.to_owned(), Arc::clone(&obj));
        Ok(obj)
    }

    /// Makes `span` of `file` durable and valid: sync, journal, index.
    ///
    /// # Errors
    /// Storage failure (the bytes then count as missing).
    pub fn commit_extent(&self, index: &Index, file: &ObjectFile, span: Span, crc: u32) -> Result<(), StoreError> {
        if span.is_empty() || span.end > file.total {
            return Err(StoreError::other(format!("extent {span:?} outside {}", file.total)));
        }
        file.writer()?.sync()?;
        let mut st = lock(&self.files);
        let journal = st.journal.as_mut().ok_or_else(|| StoreError::other("journal missing"))?;
        journal.append(&Entry::Extent { no: file.no, start: span.start, len: span.len(), crc })?;
        match st.entries_of.get_mut(&file.no) {
            Some(n) => *n += 1,
            None => st.dead += 1, // evicted while this piece was being written
        }
        drop(st);
        index.insert_file_extent(file.id(), span);
        Ok(())
    }
}
