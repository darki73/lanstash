//! When segments and object files were last used, and evicting them.

use std::collections::BTreeMap;

use lanstash_domain::evict::Recency;
use lanstash_domain::journal::Entry;

use crate::evict::EvictItem;
use crate::index::records::RecordLoc;
use crate::index::{Index, SegId};
use crate::segment::scan::read_header;
use lanstash_ports::storage::{FileRead, FileWrite, StoreError};

use super::{Game, lock, read_whole};

impl Game {
    /// Sets the last-use times from `used.lsu`. Items missing from it are newer
    /// than its last save: they count as used at `now_ms`. A game without the
    /// file at all (stored before eviction existed) starts from its segments'
    /// creation times, and its object files from the newest of those.
    pub(super) fn load_recency(&self, index: &Index, now_ms: u64) {
        let saved = read_whole(&*self.fs, &self.dir.recency()).map(|d| Recency::parse(&String::from_utf8_lossy(&d)));
        let segs = lock(&self.segs).clone();
        let (r, seg_default, file_default) = match saved {
            Ok(r) => (r, BTreeMap::new(), now_ms),
            Err(_) => {
                let created: BTreeMap<u32, u64> = segs
                    .iter()
                    .filter_map(|(seq, e)| {
                        let reader = index.segment(e.id)?.reader;
                        read_header(&*reader).ok().map(|h| (*seq, h.created_at_ms.min(now_ms)))
                    })
                    .collect();
                let newest = created.values().copied().max().unwrap_or(now_ms);
                (Recency::default(), created, newest)
            }
        };
        for (seq, e) in &segs {
            let t = r.segments.get(seq).or_else(|| seg_default.get(seq)).copied().unwrap_or(now_ms);
            index.touch_segment(e.id, t);
        }
        for (no, f) in &lock(&self.files).by_no {
            index.touch_file(f.id(), r.files.get(no).copied().unwrap_or(file_default));
        }
    }

    /// Writes when each segment and object file was last used (replacing the
    /// previous file in one rename).
    ///
    /// # Errors
    /// Storage failure.
    pub fn save_recency(&self, index: &Index) -> Result<(), StoreError> {
        let mut r = Recency::default();
        for (seq, e) in lock(&self.segs).iter() {
            r.segments.insert(*seq, index.segment_used(e.id));
        }
        for (no, f) in &lock(&self.files).by_no {
            r.files.insert(*no, index.file_used(f.id()));
        }
        let (path, tmp) = (self.dir.recency(), self.dir.recency_tmp());
        let _ = self.fs.remove_file(&tmp); // left over from a crash, or absent
        let f = self.fs.create_new(&tmp)?;
        f.write_all_at(0, r.encode().as_bytes())?;
        f.sync()?;
        drop(f);
        self.fs.rename(&tmp, &path)
    }

    /// Segments and object files that may be evicted (not the segment being
    /// written), with their size and last use.
    /// The object files: (journal number, object key).
    pub(crate) fn file_keys(&self) -> Vec<(u32, String)> {
        lock(&self.files).by_no.iter().map(|(no, f)| (*no, f.key.clone())).collect()
    }

    pub(crate) fn candidates(&self, index: &Index) -> Vec<(EvictItem, String, u64, u64)> {
        let active = lock(&self.seg).active.as_ref().map(|a| a.id);
        let mut out = Vec::new();
        for (seq, e) in lock(&self.segs).iter_mut() {
            if Some(e.id) == active {
                continue;
            }
            if e.bytes == 0 {
                e.bytes = index.segment(e.id).and_then(|s| s.reader.size().ok()).unwrap_or(0);
            }
            out.push((EvictItem::Segment(*seq), e.path.clone(), e.bytes, index.segment_used(e.id)));
        }
        for (no, f) in &lock(&self.files).by_no {
            out.push((EvictItem::File(*no), f.path.clone(), index.file_bytes(f.id()), index.file_used(f.id())));
        }
        out
    }

    /// Deletes a sealed segment: out of the index first (no new reader finds
    /// it), then off the disk. Returns its size.
    ///
    /// # Errors
    /// Unknown or active segment, or the file can't be removed.
    pub(crate) fn evict_segment(&self, index: &Index, seq: u32) -> Result<u64, StoreError> {
        self.replace_segment(index, seq, &[])
    }

    /// A sealed segment (not the one being written): its index id and path.
    ///
    /// # Errors
    /// Unknown or active segment.
    pub(crate) fn sealed_segment(&self, seq: u32) -> Result<(SegId, String), StoreError> {
        let active = lock(&self.seg).active.as_ref().map(|a| a.id);
        let segs = lock(&self.segs);
        let e = segs.get(&seq).ok_or_else(|| StoreError::other("no such segment"))?;
        if Some(e.id) == active {
            return Err(StoreError::other("the segment is being written"));
        }
        Ok((e.id, e.path.clone()))
    }

    /// Deletes a sealed segment whose objects `copies` were copied elsewhere:
    /// out of the index, with the copies served instead, in one step; then off
    /// the disk. Returns its size.
    ///
    /// # Errors
    /// Unknown or active segment, or the file can't be removed.
    pub(crate) fn replace_segment(&self, index: &Index, seq: u32, copies: &[RecordLoc]) -> Result<u64, StoreError> {
        let active = lock(&self.seg).active.clone();
        // The copies are in the active segment: on the disk before the originals go.
        if let Some(a) = active.as_ref().filter(|a| copies.iter().any(|c| c.seg == a.id)) {
            a.appender.file().sync()?;
        }
        let active = active.map(|a| a.id);
        let e = {
            let mut segs = lock(&self.segs);
            let e = segs.get(&seq).ok_or_else(|| StoreError::other("no such segment"))?;
            if Some(e.id) == active {
                return Err(StoreError::other("the segment is being written"));
            }
            segs.remove(&seq).ok_or_else(|| StoreError::other("no such segment"))?
        };
        let bytes = index.segment(e.id).and_then(|s| s.reader.size().ok()).unwrap_or(e.bytes);
        index.replace_segment(e.id, copies);
        if let Err(err) = self.fs.remove_file(&e.path) {
            // Still on the disk: still counted, and deleted again by a later pass.
            lock(&self.segs).insert(seq, e);
            return Err(err);
        }
        Ok(bytes)
    }

    /// Deletes an object file: a `FORGET` in the journal first, synced (so a crash
    /// can't bring it back, nor leave the journal describing a file that is gone),
    /// then out of the index, then off the disk. Its name stays taken until the file
    /// is gone, so a new object of the same name can't be created in between and
    /// deleted with it. Returns its stored bytes.
    ///
    /// # Errors
    /// Unknown object file, or storage failure.
    pub(crate) fn evict_file(&self, index: &Index, no: u32) -> Result<u64, StoreError> {
        let mut st = lock(&self.files);
        let f = st.by_no.get(&no).cloned().ok_or_else(|| StoreError::other("no such object file"))?;
        let journal = st.journal.as_mut().ok_or_else(|| StoreError::other("journal missing"))?;
        journal.append(&Entry::Forget { no })?;
        journal.sync()?;
        st.dead += st.entries_of.remove(&no).unwrap_or(1) + 1;
        st.by_no.remove(&no);
        st.by_key.remove(&f.key);
        let bytes = index.file_bytes(f.id());
        index.remove_file(f.id());
        *lock(&f.read) = None;
        *lock(&f.write) = None;
        let removed = self.fs.remove_file(&f.path);
        let prefix = format!("{}/", self.dir.path());
        if let Some(rel) = f.path.strip_prefix(&prefix) {
            st.paths.remove(&rel.to_lowercase());
        }
        removed?;
        Ok(bytes)
    }
}
