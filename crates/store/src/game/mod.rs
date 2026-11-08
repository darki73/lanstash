//! One game directory: its active segment, its journal and its object files.

mod evict;
mod load;
mod write;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::index::{FileId, Index, SegId, SegInfo};
use crate::journal::JournalFile;
use crate::layout::{GameDir, parse_segment_name};
use crate::segment::writer::SegmentAppender;
use lanstash_ports::storage::{FileRead, FileWrite, StoreError, StoreErrorKind, StoreFs};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A game's segment files as `(sequence number, path)`, in order; none when
/// the game has no segment directory.
pub(crate) fn segment_paths(fs: &dyn StoreFs, dir: &GameDir) -> Result<Vec<(u32, String)>, StoreError> {
    let names = match fs.list(&dir.segment_dir()) {
        Ok(n) => n,
        Err(e) if e.kind == StoreErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut segs: Vec<(u32, String)> = names
        .iter()
        .filter(|e| !e.is_dir)
        .filter_map(|e| parse_segment_name(&e.name).map(|seq| (seq, dir.segment(seq))))
        .collect();
    segs.sort();
    Ok(segs)
}

/// The whole content of a (small) file, e.g. a journal.
pub(crate) fn read_whole(fs: &dyn StoreFs, path: &str) -> Result<Vec<u8>, StoreError> {
    let f = fs.open_read(path)?;
    let mut data = vec![0u8; usize::try_from(f.size()?).map_err(|_| StoreError::other("file too large"))?];
    f.read_exact_at(0, &mut data)?;
    Ok(data)
}

/// Settings for new segments.
#[derive(Debug, Clone, Copy)]
pub struct SegmentOptions {
    pub max_bytes: u64,
    pub alignment: u32,
}

impl Default for SegmentOptions {
    /// 1 GiB segments, records aligned to 4 KiB.
    fn default() -> Self {
        Self { max_bytes: 1 << 30, alignment: 4096 }
    }
}

/// An object stored in its own sparse file.
pub struct ObjectFile {
    pub game: GameDir,
    /// Number within the game's journal.
    pub no: u32,
    pub key: String,
    /// Root-relative path of the file.
    pub path: String,
    pub total: u64,
    /// Stored response headers (`name=value` lines).
    pub meta: Arc<[u8]>,
    fs: Option<Arc<dyn StoreFs>>,
    id: AtomicU32,
    read: Mutex<Option<Arc<dyn FileRead>>>,
    write: Mutex<Option<Arc<dyn FileWrite>>>,
}

impl std::fmt::Debug for ObjectFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectFile").field("path", &self.path).field("total", &self.total).finish_non_exhaustive()
    }
}

impl ObjectFile {
    fn new(
        game: GameDir,
        no: u32,
        key: String,
        path: String,
        total: u64,
        meta: Arc<[u8]>,
        fs: Arc<dyn StoreFs>,
    ) -> Self {
        Self {
            game,
            no,
            key,
            path,
            total,
            meta,
            fs: Some(fs),
            id: AtomicU32::new(FileId::MAX),
            read: Mutex::new(None),
            write: Mutex::new(None),
        }
    }

    /// A file object without storage, for index tests.
    #[cfg(feature = "test-support")]
    pub fn for_tests(game: GameDir, key: &str, total: u64) -> Self {
        Self {
            game,
            no: 0,
            key: key.to_owned(),
            path: String::new(),
            total,
            meta: Arc::from(&b"content-type=x\n"[..]),
            fs: None,
            id: AtomicU32::new(FileId::MAX),
            read: Mutex::new(None),
            write: Mutex::new(None),
        }
    }

    /// Index id (set when registered).
    #[must_use]
    pub fn id(&self) -> FileId {
        self.id.load(Ordering::Relaxed)
    }

    fn fs(&self) -> Result<&Arc<dyn StoreFs>, StoreError> {
        self.fs.as_ref().ok_or_else(|| StoreError::other("object file without storage"))
    }

    /// A shared read handle (opened on first use).
    ///
    /// # Errors
    /// The file can't be opened.
    pub fn reader(&self) -> Result<Arc<dyn FileRead>, StoreError> {
        if let Some(w) = lock(&self.write).as_ref() {
            let r: Arc<dyn FileRead> = Arc::clone(w) as Arc<dyn FileRead>;
            return Ok(r);
        }
        let mut r = lock(&self.read);
        if let Some(h) = r.as_ref() {
            return Ok(Arc::clone(h));
        }
        let h = self.fs()?.open_read(&self.path)?;
        *r = Some(Arc::clone(&h));
        Ok(h)
    }

    /// A shared write handle (opened on first use).
    ///
    /// # Errors
    /// The file can't be opened for writing.
    pub fn writer(&self) -> Result<Arc<dyn FileWrite>, StoreError> {
        let mut w = lock(&self.write);
        if let Some(h) = w.as_ref() {
            return Ok(Arc::clone(h));
        }
        let h = self.fs()?.open_write(&self.path)?;
        *w = Some(Arc::clone(&h));
        Ok(h)
    }
}

/// The segment a game currently appends to.
pub struct ActiveSegment {
    pub id: SegId,
    pub path: String,
    pub appender: SegmentAppender<Arc<dyn FileWrite>>,
}

impl std::fmt::Debug for ActiveSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveSegment").field("id", &self.id).field("path", &self.path).finish_non_exhaustive()
    }
}

#[derive(Default)]
struct SegState {
    active: Option<Arc<ActiveSegment>>,
    next_seq: u32,
}

#[derive(Default)]
struct FilesState {
    journal: Option<JournalFile>,
    by_key: HashMap<String, Arc<ObjectFile>>,
    by_no: HashMap<u32, Arc<ObjectFile>>,
    /// Lower-cased paths in use (case-insensitive file systems).
    paths: HashSet<String>,
    next_no: u32,
    /// Journal entries per live object file (its `OBJECT` and `EXTENT`s).
    entries_of: HashMap<u32, u64>,
    /// Journal entries that describe nothing live any more (compaction drops them).
    dead: u64,
}

/// What loading a game directory found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameReport {
    pub segments_sealed: u64,
    pub segments_unsealed: u64,
    pub records: u64,
    pub record_bytes: u64,
    /// Records in unsealed segments whose body failed its CRC (dropped).
    pub bad_bodies: u64,
    /// v1 slice records (skipped: v1 stores are not migrated).
    pub legacy_slices: u64,
    /// Unsealed segments continued / sealed at startup.
    pub continued: u64,
    pub sealed_at_start: u64,
    pub object_files: u64,
    pub extents: u64,
    pub extent_bytes: u64,
    /// Journal bytes cut off after the last valid entry.
    pub journal_truncated: u64,
    /// Journal bytes of damaged entries skipped (valid entries followed them).
    pub journal_skipped: u64,
    /// Journal entries that made no sense (unknown object, out of range).
    pub journal_bad: u64,
    /// Journals rewritten without their dead entries.
    pub journals_compacted: u64,
    /// Files that could not be read: (path, error).
    pub failed: Vec<(String, String)>,
}

impl GameReport {
    pub fn add(&mut self, o: &Self) {
        self.segments_sealed += o.segments_sealed;
        self.segments_unsealed += o.segments_unsealed;
        self.records += o.records;
        self.record_bytes += o.record_bytes;
        self.bad_bodies += o.bad_bodies;
        self.legacy_slices += o.legacy_slices;
        self.continued += o.continued;
        self.sealed_at_start += o.sealed_at_start;
        self.object_files += o.object_files;
        self.extents += o.extents;
        self.extent_bytes += o.extent_bytes;
        self.journal_truncated += o.journal_truncated;
        self.journal_skipped += o.journal_skipped;
        self.journal_bad += o.journal_bad;
        self.journals_compacted += o.journals_compacted;
        self.failed.extend(o.failed.iter().cloned());
    }
}

/// Dead journal entries needed (and more than live ones) before a journal is rewritten.
pub const COMPACT_MIN_DEAD: u64 = 1000;

/// One of a game's segments.
#[derive(Debug, Clone)]
struct SegEntry {
    id: SegId,
    path: String,
    /// File size when registered (0 = not known yet).
    bytes: u64,
}

/// A game directory in one store root.
pub struct Game {
    pub dir: GameDir,
    fs: Arc<dyn StoreFs>,
    seg: Mutex<SegState>,
    /// Every segment of the game by sequence number (the active one included).
    segs: Mutex<BTreeMap<u32, SegEntry>>,
    files: Mutex<FilesState>,
    segment_ids: Arc<AtomicU64>,
}

impl std::fmt::Debug for Game {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Game").field("dir", &self.dir).finish_non_exhaustive()
    }
}

impl Game {
    /// A game with nothing stored yet.
    #[must_use]
    pub fn empty(dir: GameDir, fs: Arc<dyn StoreFs>, segment_ids: Arc<AtomicU64>) -> Self {
        Self { dir, fs, seg: Mutex::default(), segs: Mutex::default(), files: Mutex::default(), segment_ids }
    }

    /// The root this game lives in.
    #[must_use]
    pub fn fs(&self) -> &Arc<dyn StoreFs> {
        &self.fs
    }

    /// Loads a game directory into `index`: segment footers (or scans of
    /// unsealed segments), then the journal, then when each segment and
    /// object file was last used (unknown ones count as used at `now_ms`).
    /// The newest unsealed segment is continued, older unsealed ones are
    /// sealed (unless `read_only`).
    #[must_use]
    pub fn load(
        dir: GameDir,
        fs: Arc<dyn StoreFs>,
        segment_ids: Arc<AtomicU64>,
        index: &Index,
        read_only: bool,
        now_ms: u64,
    ) -> (Self, GameReport) {
        let game = Self::empty(dir, fs, segment_ids);
        let mut report = GameReport::default();
        game.load_segments(index, &mut report, read_only);
        if read_only {
            game.read_journal(index, &mut report);
        } else {
            game.load_journal(index, &mut report);
        }
        game.load_recency(index, now_ms);
        (game, report)
    }

    fn add_segment(&self, index: &Index, seq: u32, path: String, reader: Arc<dyn FileRead>, bytes: u64) -> SegId {
        let id = index.add_segment(SegInfo { path: path.clone(), game: self.dir.clone(), reader });
        lock(&self.segs).insert(seq, SegEntry { id, path, bytes });
        id
    }
}
