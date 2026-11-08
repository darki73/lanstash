//! In-memory index: object key hash → total length and stored extents.
//!
//! Keys are hashed with FNV-1a 64 (the hash segment footers already carry).
//! Readers confirm the key from the record header (segments) or the object
//! file's key; an object whose entries conflict (a hash collision, or
//! inconsistent data) is marked unusable and passed through instead of served.

pub mod counts;
mod inner;
pub mod records;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use lanstash_domain::extent::{Joinable, Span};
use lanstash_domain::segment::footer::key_hash;

use crate::game::ObjectFile;
use crate::layout::GameDir;
use lanstash_ports::storage::FileRead;

use inner::{Inner, Obj, RecordAt};

/// Index-local segment number.
pub type SegId = u32;
/// Index-local object file number.
pub type FileId = u32;

/// Where an extent is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loc {
    /// A whole-object segment record (the extent is the whole body).
    Record { seg: SegId, offset: u64, header_len: u32 },
    /// An object file; the bytes sit at their own offset in the file.
    File { file: FileId },
}

impl Joinable for Loc {
    fn joins(&self, next: &Self) -> bool {
        matches!((self, next), (Self::File { file: a }, Self::File { file: b }) if a == b)
    }
}

/// A registered segment.
#[derive(Clone)]
pub struct SegInfo {
    pub path: String,
    pub game: GameDir,
    pub reader: Arc<dyn FileRead>,
}

impl std::fmt::Debug for SegInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SegInfo").field("path", &self.path).field("game", &self.game).finish_non_exhaustive()
    }
}

/// Where to read one stored extent from.
#[derive(Clone)]
pub enum Source {
    /// Read `header_len + body` at `offset`; the header must carry the requested key.
    Record { seg: SegId, reader: Arc<dyn FileRead>, offset: u64, header_len: u32 },
    /// Read the extent's bytes at their own offset in the object file.
    File { file: Arc<ObjectFile> },
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Record { seg, offset, header_len, .. } => {
                write!(f, "Record {{ seg: {seg}, offset: {offset}, header_len: {header_len} }}")
            }
            Self::File { file } => write!(f, "File {{ {} }}", file.path),
        }
    }
}

/// A stored object as seen by one lookup.
#[derive(Debug, Clone)]
pub struct StoredObject {
    pub total: u64,
    pub pieces: Vec<(Span, Source)>,
    /// Stored response headers, when kept in memory (object files). For segment
    /// records they are read from the record header.
    pub meta: Option<Arc<[u8]>>,
}

impl StoredObject {
    /// Whether `span` is completely stored.
    #[must_use]
    pub fn covers(&self, span: Span) -> bool {
        let mut pos = span.start;
        for (s, _) in &self.pieces {
            if s.start > pos {
                break;
            }
            pos = pos.max(s.end);
            if pos >= span.end {
                return true;
            }
        }
        span.is_empty() || pos >= span.end
    }

    /// The stored piece containing `pos`.
    #[must_use]
    pub fn at(&self, pos: u64) -> Option<&(Span, Source)> {
        self.pieces.iter().find(|(s, _)| s.contains(pos))
    }
}

/// Outcome of a lookup.
#[derive(Debug, Clone)]
pub enum Lookup {
    Missing,
    /// Known but must not be served (conflict or hash collision): pass through.
    Unusable,
    Found(StoredObject),
}

/// Stored bytes and objects of one game, and when it was last used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub objects: u64,
    pub bytes: u64,
    /// Last read or write (ms) of any of its segments and object files; 0 if never.
    pub last_used_ms: u64,
}

/// The index. Shared with `Arc`; lookups take a read lock, inserts a write lock.
pub struct Index {
    inner: RwLock<Inner>,
}

impl std::fmt::Debug for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let i = self.read();
        f.debug_struct("Index")
            .field("segments", &i.segs.len())
            .field("files", &i.files.len())
            .field("objects", &i.objs.len())
            .finish()
    }
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

impl Index {
    #[must_use]
    pub fn new() -> Self {
        Self { inner: RwLock::new(Inner::default()) }
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers a segment; records are added with [`Index::insert_record`].
    pub fn add_segment(&self, info: SegInfo) -> SegId {
        let mut i = self.write();
        let id = SegId::try_from(i.segs.len()).unwrap_or(SegId::MAX);
        i.segs.push(Some(info));
        i.seg_used.push(AtomicU64::new(0));
        i.by_offset.push(Vec::new());
        id
    }

    /// Replaces a segment's read handle (e.g. once it is sealed and immutable).
    pub fn set_segment_reader(&self, seg: SegId, reader: Arc<dyn FileRead>) {
        if let Some(Some(s)) = self.write().segs.get_mut(seg as usize) {
            s.reader = reader;
        }
    }

    /// Adds a whole-object record of `key_hash` (body `body_len` bytes).
    pub fn insert_record(&self, hash: u64, seg: SegId, offset: u64, header_len: u32, body_len: u64) {
        let mut i = self.write();
        let Some(service) = i.seg(seg).map(|s| s.game.service.clone()) else {
            return; // evicted while the record was being written
        };
        i.changed(&service);
        i.insert(hash, body_len, Span::at(0, body_len), Loc::Record { seg, offset, header_len });
        if let Some(list) = i.by_offset.get_mut(seg as usize) {
            let at = list.partition_point(|r| r.offset < offset);
            list.insert(at, RecordAt { offset, header_len, body_len, hash });
        }
    }

    /// Registers an object file (no extents yet). Its total becomes known; a second file of
    /// a key makes it unusable, which changes what its service holds.
    pub fn add_file(&self, file: Arc<ObjectFile>) -> FileId {
        let hash = key_hash(file.key.as_bytes());
        let (total, service) = (file.total, file.game.service.clone());
        let mut i = self.write();
        let id = FileId::try_from(i.files.len()).unwrap_or(FileId::MAX);
        i.files.push(Some(file));
        i.file_used.push(AtomicU64::new(0));
        let obj = i.objs.entry(hash).or_insert_with(|| Obj::new(total));
        if obj.total != total || obj.file.is_some() {
            obj.unusable = true;
            i.changed(&service);
        } else {
            obj.file = Some(id);
        }
        id
    }

    /// Marks `span` of an object file as stored.
    pub fn insert_file_extent(&self, file: FileId, span: Span) {
        let mut i = self.write();
        let Some(f) = i.file(file) else { return };
        let (hash, total, service) = (key_hash(f.key.as_bytes()), f.total, f.game.service.clone());
        i.changed(&service);
        i.insert(hash, total, span, Loc::File { file });
    }

    /// Looks up `key`.
    #[must_use]
    pub fn lookup(&self, key: &str) -> Lookup {
        self.lookup_at(key, None)
    }

    /// Looks up `key` and, if found, marks where it is stored as used at `now_ms`.
    #[must_use]
    pub fn lookup_touch(&self, key: &str, now_ms: u64) -> Lookup {
        self.lookup_at(key, Some(now_ms))
    }

    fn lookup_at(&self, key: &str, touch: Option<u64>) -> Lookup {
        let i = self.read();
        let Some(obj) = i.objs.get(&key_hash(key.as_bytes())) else { return Lookup::Missing };
        if obj.unusable {
            return Lookup::Unusable;
        }
        let file = obj.file.and_then(|f| i.file(f));
        if file.is_some_and(|f| f.key != key) {
            return Lookup::Unusable;
        }
        let mut pieces = Vec::with_capacity(obj.pieces.len());
        for (span, loc) in obj.pieces.iter() {
            match i.source(*loc) {
                Some(src) => pieces.push((span, src)),
                None => return Lookup::Unusable,
            }
            if let Some(now) = touch {
                i.touch(*loc, now);
            }
        }
        if let (Some(now), Some(f)) = (touch, obj.file) {
            i.touch(Loc::File { file: f }, now);
        }
        Lookup::Found(StoredObject { total: obj.total, pieces, meta: file.map(|f| Arc::clone(&f.meta)) })
    }

    /// Marks a segment used at `now_ms` (times only move forward).
    pub fn touch_segment(&self, seg: SegId, now_ms: u64) {
        self.read().touch(Loc::Record { seg, offset: 0, header_len: 0 }, now_ms);
    }

    /// Marks an object file used at `now_ms` (times only move forward).
    pub fn touch_file(&self, file: FileId, now_ms: u64) {
        self.read().touch(Loc::File { file }, now_ms);
    }

    /// When a segment was last used (0 = never, or unknown).
    #[must_use]
    pub fn segment_used(&self, seg: SegId) -> u64 {
        self.read().seg_used.get(seg as usize).map_or(0, |a| a.load(Ordering::Relaxed))
    }

    /// When an object file was last used (0 = never, or unknown).
    #[must_use]
    pub fn file_used(&self, file: FileId) -> u64 {
        self.read().file_used.get(file as usize).map_or(0, |a| a.load(Ordering::Relaxed))
    }

    /// Bytes of an object file that are stored (its extents).
    #[must_use]
    pub fn file_bytes(&self, file: FileId) -> u64 {
        let i = self.read();
        let Some(f) = i.file(file) else { return 0 };
        i.objs
            .get(&key_hash(f.key.as_bytes()))
            .map_or(0, |o| o.pieces.iter().filter(|(_, l)| **l == Loc::File { file }).map(|(s, _)| s.len()).sum())
    }

    /// Forgets a segment and every record in it (eviction). Returns its info; dropping that closes
    /// the index's handle. Readers that looked an object up before keep their own and can finish.
    pub fn remove_segment(&self, seg: SegId) -> Option<SegInfo> {
        let mut i = self.write();
        let info = i.segs.get_mut(seg as usize)?.take()?;
        i.changed(&info.game.service);
        let records = std::mem::take(&mut i.by_offset[seg as usize]);
        for r in records {
            i.drop_pieces(r.hash, |l| matches!(l, Loc::Record { seg: s, .. } if *s == seg), None);
        }
        Some(info)
    }

    /// Forgets an object file and its extents (eviction).
    pub fn remove_file(&self, file: FileId) -> Option<Arc<ObjectFile>> {
        let mut i = self.write();
        let f = i.files.get_mut(file as usize)?.take()?;
        i.changed(&f.game.service);
        i.drop_pieces(key_hash(f.key.as_bytes()), |l| *l == Loc::File { file }, Some(file));
        Some(f)
    }

    /// The object file registered for `key`, if any.
    #[must_use]
    pub fn file_of(&self, key: &str) -> Option<Arc<ObjectFile>> {
        let i = self.read();
        let obj = i.objs.get(&key_hash(key.as_bytes()))?;
        let f = i.file(obj.file?)?;
        (f.key == key).then(|| Arc::clone(f))
    }

    /// Marks an object unusable (e.g. a record header carried another key): what its service
    /// holds changed (a key is its service, then its path).
    pub fn mark_unusable(&self, key: &str) {
        let mut i = self.write();
        let marked =
            i.objs.get_mut(&key_hash(key.as_bytes())).is_some_and(|o| !std::mem::replace(&mut o.unusable, true));
        if marked {
            i.changed(key.split('/').next().unwrap_or(""));
        }
    }

    /// The record stored right after the one at `offset` in segment `seg`:
    /// `(offset, header_len, body_len)` and a reader.
    #[must_use]
    pub fn next_record(&self, seg: SegId, offset: u64) -> Option<(u64, u32, u64, Arc<dyn FileRead>)> {
        let i = self.read();
        let list = i.by_offset.get(seg as usize)?;
        let at = list.binary_search_by_key(&offset, |r| r.offset).ok()?;
        let r = *list.get(at + 1)?;
        Some((r.offset, r.header_len, r.body_len, Arc::clone(&i.seg(seg)?.reader)))
    }

    /// Number of objects (distinct key hashes).
    #[must_use]
    pub fn len(&self) -> usize {
        self.read().objs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.read().objs.is_empty()
    }

    #[must_use]
    pub fn segment(&self, seg: SegId) -> Option<SegInfo> {
        self.read().seg(seg).cloned()
    }

    /// Stored objects and bytes per game, and when each was last used.
    #[must_use]
    pub fn usage(&self) -> BTreeMap<GameDir, Usage> {
        self.read().usage()
    }

    /// How many times what `service` holds changed: a cheap way to tell whether it did.
    #[must_use]
    pub fn changes(&self, service: &str) -> u64 {
        self.read().changes.get(service).copied().unwrap_or(0)
    }

    /// How many times what any service holds changed.
    #[must_use]
    pub fn all_changes(&self) -> u64 {
        self.read().changes.values().sum()
    }

    /// Whether `game` has a segment or an object file (a look over them, not over every object).
    #[must_use]
    pub fn holds(&self, game: &GameDir) -> bool {
        let r = self.read();
        r.segs.iter().flatten().any(|s| &s.game == game) || r.files.iter().flatten().any(|f| &f.game == game)
    }

    /// Total stored bytes.
    #[must_use]
    pub fn stored_bytes(&self) -> u64 {
        self.read().objs.values().map(|o| o.pieces.covered_bytes()).sum()
    }
}
