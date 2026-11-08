//! In-memory test doubles: a file, a directory tree, and lancache's nginx
//! cache ([`nginx`]).

pub mod nginx;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use lanstash_domain::evict::Disk;
use lanstash_domain::segment::header::{SegmentHeader, Service};
use lanstash_domain::segment::record::RecordFlags;

use crate::segment::writer::NewRecord;
use lanstash_ports::storage::{DirEntry, FileRead, FileWrite, StoreError, StoreErrorKind, StoreFs};

/// Shared in-memory "file". Clones share the same bytes.
#[derive(Debug, Clone, Default)]
pub struct MemFile {
    data: Arc<Mutex<Vec<u8>>>,
    fail: Arc<AtomicBool>,
    /// Countdown for [`MemFile::fail_nth_write`]; 0 = off.
    fail_at: Arc<AtomicUsize>,
    syncs: Arc<AtomicUsize>,
    ops: Arc<Mutex<Vec<FileOp>>>,
}

/// What was done to a [`MemFile`], in order (for checking what reaches the disk first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOp {
    Write { offset: u64, len: u64 },
    Sync,
}

impl MemFile {
    /// Its writes and syncs so far, in order.
    pub fn ops(&self) -> Vec<FileOp> {
        self.ops.lock().unwrap().clone()
    }
    pub fn bytes(&self) -> Vec<u8> {
        self.data.lock().unwrap().clone()
    }
    pub fn fail_writes(&self, on: bool) {
        self.fail.store(on, Ordering::SeqCst);
    }
    /// The `n`-th write from now fails (1 = the next one); the others succeed.
    pub fn fail_nth_write(&self, n: usize) {
        self.fail_at.store(n, Ordering::SeqCst);
    }
    /// Mutate the raw bytes (to simulate corruption).
    pub fn with_bytes(&self, f: impl FnOnce(&mut Vec<u8>)) {
        f(&mut self.data.lock().unwrap());
    }
    pub fn syncs(&self) -> usize {
        self.syncs.load(Ordering::SeqCst)
    }
}

impl FileRead for MemFile {
    fn size(&self) -> Result<u64, StoreError> {
        Ok(self.data.lock().unwrap().len() as u64)
    }
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), StoreError> {
        let d = self.data.lock().unwrap();
        let start = usize::try_from(offset).unwrap();
        let end = start + buf.len();
        if end > d.len() {
            return Err(StoreError::new(StoreErrorKind::UnexpectedEof, "read past end"));
        }
        buf.copy_from_slice(&d[start..end]);
        Ok(())
    }
}

impl FileWrite for MemFile {
    fn write_all_at(&self, offset: u64, data: &[u8]) -> Result<(), StoreError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(StoreError::other("injected failure"));
        }
        let n = self.fail_at.load(Ordering::SeqCst);
        if n > 0 {
            self.fail_at.store(n - 1, Ordering::SeqCst);
            if n == 1 {
                return Err(StoreError::other("injected failure"));
            }
        }
        let mut d = self.data.lock().unwrap();
        let start = usize::try_from(offset).unwrap();
        let end = start + data.len();
        if d.len() < end {
            d.resize(end, 0);
        }
        d[start..end].copy_from_slice(data);
        self.ops.lock().unwrap().push(FileOp::Write { offset, len: data.len() as u64 });
        Ok(())
    }
    fn set_len(&self, len: u64) -> Result<(), StoreError> {
        self.data.lock().unwrap().resize(usize::try_from(len).unwrap(), 0);
        Ok(())
    }
    fn sync(&self) -> Result<(), StoreError> {
        self.syncs.fetch_add(1, Ordering::SeqCst);
        self.ops.lock().unwrap().push(FileOp::Sync);
        Ok(())
    }
}

/// In-memory [`StoreFs`]: path → file. Directories exist implicitly.
#[derive(Debug, Clone, Default)]
pub struct MemFs {
    files: Arc<Mutex<BTreeMap<String, MemFile>>>,
    /// What [`StoreFs::space`] reports (`None` = unknown).
    space: Arc<Mutex<Option<Disk>>>,
}

impl MemFs {
    /// The file at `path`, created empty if missing.
    pub fn file(&self, path: &str) -> MemFile {
        self.files.lock().unwrap().entry(path.to_owned()).or_default().clone()
    }
    pub fn get(&self, path: &str) -> Option<MemFile> {
        self.files.lock().unwrap().get(path).cloned()
    }
    pub fn paths(&self) -> Vec<String> {
        self.files.lock().unwrap().keys().cloned().collect()
    }
    /// Sets what [`StoreFs::space`] reports.
    pub fn set_space(&self, d: Option<Disk>) {
        *self.space.lock().unwrap() = d;
    }
    pub fn remove(&self, path: &str) {
        self.files.lock().unwrap().remove(path);
    }
}

fn not_found(p: &str) -> StoreError {
    StoreError::new(StoreErrorKind::NotFound, p)
}

impl StoreFs for MemFs {
    fn list(&self, dir: &str) -> Result<Vec<DirEntry>, StoreError> {
        let prefix = if dir.is_empty() { String::new() } else { format!("{dir}/") };
        let mut out: Vec<DirEntry> = Vec::new();
        for path in self.files.lock().unwrap().keys() {
            let Some(rest) = path.strip_prefix(&prefix) else { continue };
            let e = match rest.split_once('/') {
                Some((d, _)) => DirEntry { name: d.to_owned(), is_dir: true },
                None => DirEntry { name: rest.to_owned(), is_dir: false },
            };
            if !out.contains(&e) {
                out.push(e);
            }
        }
        if out.is_empty() && !dir.is_empty() {
            return Err(not_found(dir));
        }
        out.sort();
        Ok(out)
    }
    fn open_read(&self, path: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        Ok(Arc::new(self.get(path).ok_or_else(|| not_found(path))?))
    }
    fn create_new(&self, path: &str) -> Result<Arc<dyn FileWrite>, StoreError> {
        let mut files = self.files.lock().unwrap();
        if files.contains_key(path) {
            return Err(StoreError::new(StoreErrorKind::AlreadyExists, path));
        }
        let f = MemFile::default();
        files.insert(path.to_owned(), f.clone());
        Ok(Arc::new(f))
    }
    fn open_write(&self, path: &str) -> Result<Arc<dyn FileWrite>, StoreError> {
        Ok(Arc::new(self.get(path).ok_or_else(|| not_found(path))?))
    }
    fn rename(&self, from: &str, to: &str) -> Result<(), StoreError> {
        let mut files = self.files.lock().unwrap();
        let f = files.remove(from).ok_or_else(|| not_found(from))?;
        files.insert(to.to_owned(), f);
        Ok(())
    }
    fn space(&self) -> Result<Option<Disk>, StoreError> {
        Ok(*self.space.lock().unwrap())
    }
    fn remove_file(&self, path: &str) -> Result<(), StoreError> {
        self.files.lock().unwrap().remove(path).map(|_| ()).ok_or_else(|| not_found(path))
    }
}

pub fn sample_header(alignment: u32, max_bytes: u64) -> SegmentHeader {
    SegmentHeader {
        flags: 0,
        alignment,
        segment_id: 1,
        service: Service::STEAM,
        game_id: 2_357_570,
        created_at_ms: 1_793_822_984_000,
        max_bytes,
    }
}

pub fn rec(key: &str) -> NewRecord<'_> {
    NewRecord { key, meta: b"content-type=application/x-steam-chunk\n", flags: RecordFlags::default() }
}

/// A clock the test moves by hand.
#[derive(Debug, Default)]
pub struct ManualClock(pub AtomicU64);

impl ManualClock {
    pub fn set(&self, ms: u64) {
        self.0.store(ms, Ordering::SeqCst);
    }
}

impl lanstash_ports::clock::Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
