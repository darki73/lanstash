//! Loading a game directory: segments (footers or scans), then the journal.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_domain::extent::Span;
use lanstash_domain::journal::{JournalRead, live_objects, read_journal};
use lanstash_domain::segment::footer::key_hash;
use lanstash_domain::segment::record::RecordFlags;

use crate::index::{Index, SegId};
use crate::journal::JournalFile;
use crate::segment::scan::{ScanOptions, SegmentReport, read_sealed_index, scan};
use crate::segment::writer::SegmentAppender;
use lanstash_ports::storage::{FileRead, StoreErrorKind};

use super::{ActiveSegment, Game, GameReport, ObjectFile, lock, read_whole, segment_paths};

impl Game {
    pub(super) fn load_segments(&self, index: &Index, report: &mut GameReport, read_only: bool) {
        let segs = match segment_paths(&*self.fs, &self.dir) {
            Ok(s) => s,
            Err(e) => {
                report.failed.push((self.dir.segment_dir(), e.to_string()));
                return;
            }
        };
        let mut unsealed: Vec<(u32, String, SegId, SegmentReport)> = Vec::new();
        let mut next_seq = 0;
        for (seq, path) in segs {
            // The last number is never handed out again (a new segment then fails to open).
            next_seq = next_seq.max(seq.saturating_add(1));
            let reader = match self.fs.open_read(&path) {
                Ok(r) => r,
                Err(e) => {
                    report.failed.push((path, e.to_string()));
                    continue;
                }
            };
            match read_sealed_index(&*reader) {
                Ok(Some(sealed)) => {
                    let reader = match self.fs.open_read_immutable(&path) {
                        Ok(r) => r,
                        Err(e) => {
                            report.failed.push((path, e.to_string()));
                            continue;
                        }
                    };
                    report.segments_sealed += 1;
                    let bytes = reader.size().unwrap_or(0);
                    let id = self.add_segment(index, seq, path, reader, bytes);
                    for e in &sealed.entries {
                        if RecordFlags(e.flags).contains(RecordFlags::SLICED) {
                            report.legacy_slices += 1;
                            continue;
                        }
                        index.insert_record(e.key_hash, id, e.offset, e.header_len, e.body_len);
                        report.records += 1;
                        report.record_bytes += e.body_len;
                    }
                }
                // Unsealed (or unreadable footer): scan with body checks.
                Ok(None) | Err(_) => match scan(&*reader, ScanOptions { verify_bodies: true }) {
                    Ok(rep) => {
                        report.segments_unsealed += 1;
                        let id = self.add_segment(index, seq, path.clone(), reader, rep.file_len);
                        for r in &rep.records {
                            if r.body_ok == Some(false) {
                                report.bad_bodies += 1;
                            } else if r.header.flags.contains(RecordFlags::SLICED) {
                                report.legacy_slices += 1;
                            } else {
                                let header_len = u32::try_from(r.header.encoded_len()).unwrap_or(u32::MAX);
                                index.insert_record(
                                    key_hash(r.header.key.as_bytes()),
                                    id,
                                    r.offset,
                                    header_len,
                                    r.header.body_len,
                                );
                                report.records += 1;
                                report.record_bytes += r.header.body_len;
                            }
                        }
                        unsealed.push((seq, path, id, rep));
                    }
                    Err(e) => report.failed.push((path, e.to_string())),
                },
            }
        }
        if read_only {
            return;
        }
        // Continue the newest unsealed segment, seal the others.
        let newest = unsealed.pop();
        for (_, path, _, rep) in unsealed {
            let sealed = self
                .fs
                .open_write(&path)
                .map_err(|e| e.to_string())
                .and_then(|f| SegmentAppender::resume(f, &rep).map_err(|e| e.to_string()))
                .and_then(|a| a.seal().map_err(|e| e.to_string()));
            match sealed {
                Ok(_) => report.sealed_at_start += 1,
                Err(e) => report.failed.push((path, format!("sealing at startup: {e}"))),
            }
        }
        let mut st = lock(&self.seg);
        st.next_seq = next_seq;
        if let Some((_, path, id, rep)) = newest {
            let resumed = self
                .fs
                .open_write(&path)
                .map_err(|e| e.to_string())
                .and_then(|f| SegmentAppender::resume(f, &rep).map_err(|e| e.to_string()));
            match resumed {
                Ok(appender) => {
                    st.active = Some(Arc::new(ActiveSegment { id, path, appender }));
                    report.continued += 1;
                }
                Err(e) => report.failed.push((path, format!("continuing: {e}"))),
            }
        }
    }

    pub(super) fn load_journal(&self, index: &Index, report: &mut GameReport) {
        let path = self.dir.journal();
        let (journal, opened) = match JournalFile::open(&*self.fs, &path) {
            Ok(j) => j,
            Err(e) if e.kind == StoreErrorKind::NotFound => return,
            Err(e) => {
                report.failed.push((path, e.to_string()));
                return;
            }
        };
        report.journal_truncated += opened.truncated;
        report.journal_skipped += opened.read.skipped;
        self.apply_journal(index, report, opened.read);
        let mut st = lock(&self.files);
        st.journal = Some(journal);
        match self.maybe_compact(&mut st) {
            Ok(true) => report.journals_compacted += 1,
            Ok(false) => {}
            Err(e) => report.failed.push((path, format!("compacting: {e}"))),
        }
    }

    /// Like `load_journal`, without opening the journal for writing (a torn tail is
    /// counted, not cut).
    pub(super) fn read_journal(&self, index: &Index, report: &mut GameReport) {
        let path = self.dir.journal();
        match read_whole(&*self.fs, &path) {
            Ok(data) => {
                let r = read_journal(&data);
                if r.refused {
                    report.failed.push((path, r.stopped.unwrap_or_default()));
                    return;
                }
                report.journal_truncated += data.len() as u64 - r.valid_len;
                report.journal_skipped += r.skipped;
                self.apply_journal(index, report, r);
            }
            Err(e) if e.kind == StoreErrorKind::NotFound => {}
            Err(e) => report.failed.push((path, e.to_string())),
        }
    }

    fn apply_journal(&self, index: &Index, report: &mut GameReport, read: JournalRead) {
        let total_entries = read.entries.len() as u64;
        let live = live_objects(read.entries);
        report.journal_bad += live.bad;
        let mut st = lock(&self.files);
        st.next_no = st.next_no.max(live.next_no);
        let mut live_entries = 1; // the header
        for o in live.objects {
            if st.by_no.contains_key(&o.no) || st.by_key.contains_key(&o.key) {
                report.journal_bad += 1;
                continue;
            }
            let entries = 1 + o.extents.len() as u64;
            live_entries += entries;
            st.entries_of.insert(o.no, entries);
            let file = Arc::new(ObjectFile::new(
                self.dir.clone(),
                o.no,
                o.key.clone(),
                self.dir.file(&o.path),
                o.total_len,
                Arc::from(o.meta),
                Arc::clone(&self.fs),
            ));
            file.id.store(index.add_file(Arc::clone(&file)), Ordering::Relaxed);
            for &(start, len, _) in &o.extents {
                index.insert_file_extent(file.id(), Span::at(start, len));
                report.extents += 1;
                report.extent_bytes += len;
            }
            st.paths.insert(o.path.to_lowercase());
            st.by_no.insert(o.no, Arc::clone(&file));
            st.by_key.insert(o.key, file);
            report.object_files += 1;
        }
        st.dead = total_entries.saturating_sub(live_entries);
    }
}
