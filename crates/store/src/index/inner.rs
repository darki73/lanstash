//! The index's state behind its lock: segments, object files, their last use,
//! and every object's stored pieces.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lanstash_domain::extent::{ExtentMap, Span};

use super::{FileId, Loc, SegId, SegInfo, Source, Usage};
use crate::game::ObjectFile;
use crate::layout::GameDir;

#[derive(Debug, Clone)]
pub(super) struct Obj {
    pub(super) total: u64,
    pub(super) pieces: ExtentMap<Loc>,
    /// The object file holding (some of) this object, if any.
    pub(super) file: Option<FileId>,
    pub(super) unusable: bool,
}

impl Obj {
    pub(super) fn new(total: u64) -> Self {
        Self { total, pieces: ExtentMap::new(), file: None, unusable: false }
    }
}

/// One record in a segment, for walking a segment in file order and for
/// removing its records.
#[derive(Debug, Clone, Copy)]
pub(super) struct RecordAt {
    pub(super) offset: u64,
    pub(super) header_len: u32,
    pub(super) body_len: u64,
    pub(super) hash: u64,
}

#[derive(Default)]
pub(super) struct Inner {
    /// `None` once evicted (ids are never reused).
    pub(super) segs: Vec<Option<SegInfo>>,
    pub(super) files: Vec<Option<Arc<ObjectFile>>>,
    /// Last use (ms) per segment / object file, for eviction.
    pub(super) seg_used: Vec<AtomicU64>,
    pub(super) file_used: Vec<AtomicU64>,
    pub(super) objs: HashMap<u64, Obj>,
    /// Per segment: its records by offset.
    pub(super) by_offset: Vec<Vec<RecordAt>>,
    /// Per service: how many times what it holds changed (stored or forgotten).
    pub(super) changes: HashMap<String, u64>,
}

impl Inner {
    /// What `service` holds changed.
    pub(super) fn changed(&mut self, service: &str) {
        match self.changes.get_mut(service) {
            Some(n) => *n += 1,
            None => {
                self.changes.insert(service.to_owned(), 1);
            }
        }
    }

    pub(super) fn seg(&self, seg: SegId) -> Option<&SegInfo> {
        self.segs.get(seg as usize)?.as_ref()
    }

    pub(super) fn file(&self, file: FileId) -> Option<&Arc<ObjectFile>> {
        self.files.get(file as usize)?.as_ref()
    }

    pub(super) fn source(&self, loc: Loc) -> Option<Source> {
        match loc {
            Loc::Record { seg, offset, header_len } => {
                self.seg(seg).map(|s| Source::Record { seg, reader: Arc::clone(&s.reader), offset, header_len })
            }
            Loc::File { file } => self.file(file).map(|f| Source::File { file: Arc::clone(f) }),
        }
    }

    pub(super) fn touch(&self, loc: Loc, now_ms: u64) {
        let slot = match loc {
            Loc::Record { seg, .. } => self.seg_used.get(seg as usize),
            Loc::File { file } => self.file_used.get(file as usize),
        };
        if let Some(a) = slot {
            a.fetch_max(now_ms, Ordering::Relaxed);
        }
    }

    pub(super) fn insert(&mut self, hash: u64, total: u64, span: Span, loc: Loc) {
        let obj = self.objs.entry(hash).or_insert_with(|| Obj::new(total));
        if obj.total != total || span.end > total {
            obj.unusable = true;
            return;
        }
        let gaps = obj.pieces.gaps(span);
        let ok = match loc {
            // Same key, same bytes: keep what is stored, add what is new. File
            // extents map object offsets 1:1, so any part of them can be added.
            Loc::File { .. } => gaps.into_iter().all(|g| obj.pieces.insert(g, loc).is_ok()),
            // A record body starts at object offset 0: only all or nothing.
            Loc::Record { .. } if gaps.is_empty() => true,
            Loc::Record { .. } => gaps == [span] && obj.pieces.insert(span, loc).is_ok(),
        };
        if !ok {
            obj.unusable = true;
        }
    }

    /// Drops the pieces of `hash` for which `gone` is true; an object left
    /// with nothing is forgotten completely (so it is fetched again, and a
    /// conflict that made it unusable is forgotten too).
    pub(super) fn drop_pieces(&mut self, hash: u64, gone: impl Fn(&Loc) -> bool, file: Option<FileId>) {
        let Some(obj) = self.objs.get_mut(&hash) else { return };
        obj.pieces.retain(|_, loc| !gone(loc));
        if file.is_some() && obj.file == file {
            obj.file = None;
        }
        if obj.pieces.is_empty() && obj.file.is_none() {
            self.objs.remove(&hash);
        }
    }

    /// Stored objects and bytes per game, and when each was last used.
    pub(super) fn usage(&self) -> BTreeMap<GameDir, Usage> {
        let mut out: BTreeMap<GameDir, Usage> = BTreeMap::new();
        for obj in self.objs.values() {
            let mut counted: Option<&GameDir> = None;
            for (span, loc) in obj.pieces.iter() {
                let game = match loc {
                    Loc::Record { seg, .. } => self.seg(*seg).map(|s| &s.game),
                    Loc::File { file } => self.file(*file).map(|f| &f.game),
                };
                let Some(game) = game else { continue };
                // No allocation per piece: millions of objects share a few games.
                if !out.contains_key(game) {
                    out.insert(game.clone(), Usage::default());
                }
                let Some(u) = out.get_mut(game) else { continue };
                u.bytes += span.len();
                if counted != Some(game) {
                    u.objects += 1;
                    counted = Some(game);
                }
            }
        }
        let used = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let segs = self.segs.iter().zip(&self.seg_used).filter_map(|(s, t)| Some((&s.as_ref()?.game, used(t))));
        let files = self.files.iter().zip(&self.file_used).filter_map(|(f, t)| Some((&f.as_ref()?.game, used(t))));
        for (game, t) in segs.chain(files) {
            if let Some(u) = out.get_mut(game) {
                u.last_used_ms = u.last_used_ms.max(t);
            }
        }
        out
    }
}
