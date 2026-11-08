//! Import from an in-memory nginx cache into an in-memory store.

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

use lanstash_app::import_nginx::{ImportOptions, ImportReport, import_nginx};
use lanstash_domain::checksum::crc::crc32;
use lanstash_domain::extent::Span;
use lanstash_domain::nginx_cache::{CACHE_VERSION, HEADER_STRUCT_LEN, KEY_MARKER, cache_file_path, slice_key};
use lanstash_ports::clock::FixedClock;
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::storage::{FileRead, StoreError, StoreErrorKind};
use lanstash_store::index::{Lookup, Source};
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::{MemFile, MemFs};

const SLICE: u64 = 64;

#[derive(Default)]
struct MemNginx {
    files: BTreeMap<String, Vec<u8>>,
}

impl MemNginx {
    /// Stores `body[span]` as the slice of `base` that nginx would write.
    fn slice(&mut self, base: &str, body: &[u8], start: u64, extra: &str) {
        self.slice_keeping(base, body, start, extra, usize::MAX);
    }

    /// Like [`Self::slice`], with at most `kept` bytes of its body in the file (0: an empty slice).
    fn slice_keeping(&mut self, base: &str, body: &[u8], start: u64, extra: &str, kept: usize) {
        let end = (start + SLICE).min(body.len() as u64);
        let part = &body[start as usize..end as usize];
        let part = &part[..part.len().min(kept)];
        let key = slice_key(base, start, SLICE);
        let http = format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\
             Content-Range: bytes {start}-{}/{}\r\n{extra}\r\n",
            part.len(),
            end - 1,
            body.len()
        );
        let mut f = vec![0u8; HEADER_STRUCT_LEN];
        f[0..8].copy_from_slice(&CACHE_VERSION.to_le_bytes());
        f.extend_from_slice(KEY_MARKER);
        f.extend_from_slice(key.as_bytes());
        f.push(b'\n');
        let header_start = u16::try_from(f.len()).unwrap();
        f.extend_from_slice(http.as_bytes());
        let body_start = u16::try_from(f.len()).unwrap();
        f[54..56].copy_from_slice(&header_start.to_le_bytes());
        f[56..58].copy_from_slice(&body_start.to_le_bytes());
        f.extend_from_slice(part);
        self.files.insert(cache_file_path(&key), f);
    }

    fn all_slices(&mut self, base: &str, body: &[u8], extra: &str) {
        for start in (0..body.len() as u64).step_by(SLICE as usize) {
            self.slice(base, body, start, extra);
        }
    }
}

impl NginxCacheSource for MemNginx {
    fn list_files(&self, _top_dirs: &[String]) -> Result<Vec<String>, StoreError> {
        Ok(self.files.keys().cloned().collect())
    }
    fn open(&self, rel: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        let b = self.files.get(rel).ok_or_else(|| StoreError::new(StoreErrorKind::NotFound, rel))?;
        let f = MemFile::default();
        f.with_bytes(|d| d.extend_from_slice(b));
        Ok(Arc::new(f))
    }
}

fn body(n: usize, seed: usize) -> Vec<u8> {
    (0..n).map(|i| ((i + seed) * 13 % 251) as u8).collect()
}

fn opts() -> ImportOptions {
    ImportOptions {
        top_dirs: vec![],
        threads: 3,
        services: vec!["steam".into(), "blizzard".into()],
        profiles: support::profiles::profiles(),
        slice_bytes: SLICE,
        object_file_min: 1000,
    }
}

fn open(fs: &MemFs) -> Store {
    Store::open(Roots::single(Arc::new(fs.clone())), StoreOptions::default(), Arc::new(FixedClock(5))).unwrap().0
}

fn run(src: &MemNginx, store: &Store) -> ImportReport {
    import_nginx(src, store, &opts(), &|_| {}).unwrap()
}

/// The stored bytes of `span`, or `None` when not all stored.
fn read(store: &Store, key: &str, span: Span) -> Option<Vec<u8>> {
    let Lookup::Found(o) = store.lookup(key) else { return None };
    if !o.covers(span) {
        return None;
    }
    let mut out = vec![0u8; span.len() as usize];
    let mut pos = span.start;
    while pos < span.end {
        let (s, src) = o.at(pos).unwrap();
        let to = s.end.min(span.end);
        let buf = &mut out[(pos - span.start) as usize..(to - span.start) as usize];
        match src {
            Source::Record { reader, offset, header_len, .. } => {
                reader.read_exact_at(offset + u64::from(*header_len) + pos, buf).unwrap();
            }
            Source::File { file } => file.reader().unwrap().read_exact_at(pos, buf).unwrap(),
        }
        pos = to;
    }
    Some(out)
}

const CHUNK: &str = "steam/depot/7/chunk/0123456789abcdef0123456789abcdef01234567";
const BAD_CHUNK: &str = "steam/depot/7/chunk/1123456789abcdef0123456789abcdef01234567";
const ARCHIVE: &str = "blizzard/tpr/hs/data/8e/fc/8efccb8531df1859cffa8f081a8eddd1";
const SMALL: &str = "blizzard/tpr/hs/config/aa/bb/aabb0000000000000000000000000000";
const ORPHAN: &str = "blizzard/tpr/hs/config/cc/dd/ccdd0000000000000000000000000000";

#[test]
fn imports_records_extents_and_skips_the_rest() {
    let mut src = MemNginx::default();
    let chunk = body(150, 1);
    src.all_slices(CHUNK, &chunk, &format!("x-content-crc: {}\r\n", crc32(&chunk)));
    src.all_slices(BAD_CHUNK, &body(100, 2), "x-content-crc: 1\r\n");
    let archive = body(5000, 3);
    src.slice(ARCHIVE, &archive, 0, "");
    src.slice(ARCHIVE, &archive, 640, "");
    let small = body(200, 4);
    src.slice(SMALL, &small, 0, "");
    src.slice(SMALL, &small, 128, ""); // 64..128 and 192..200 missing
    let orphan = body(150, 5);
    src.slice(ORPHAN, &orphan, 64, ""); // first slice missing
    src.all_slices("blizzard/tpr/hs/x.version", b"v1", "");
    src.all_slices("example.org/file", b"zz", "");

    let fs = MemFs::default();
    let store = open(&fs);
    let r = run(&src, &store);
    assert_eq!(r.files_listed, 12);
    assert_eq!((r.records, r.verified, r.check_failed), (1, 1, 1));
    assert_eq!((r.extents, r.incomplete), (5, 1));
    assert_eq!((r.skipped_service, r.skipped_uncacheable, r.bad_files), (1, 1, 0), "{:?}", r.sample_errors);
    assert_eq!(r.joined_slices, 2 + 1 + 1);

    assert_eq!(read(&store, CHUNK, Span::new(0, 150)).unwrap(), chunk);
    // Everything is readable from a store opened afresh.
    drop(store);
    let store = open(&fs);
    assert_eq!(read(&store, CHUNK, Span::new(0, 150)).unwrap(), chunk);
    assert!(matches!(store.lookup(BAD_CHUNK), Lookup::Missing));
    assert_eq!(read(&store, ARCHIVE, Span::new(640, 704)).unwrap(), &archive[640..704]);
    assert!(read(&store, ARCHIVE, Span::new(0, 65)).is_none());
    assert_eq!(read(&store, SMALL, Span::new(128, 192)).unwrap(), &small[128..192]);
    assert!(read(&store, SMALL, Span::new(0, 65)).is_none());
    assert_eq!(read(&store, ORPHAN, Span::new(64, 128)).unwrap(), &orphan[64..128]);

    // A second run adds nothing.
    let again = run(&src, &store);
    assert_eq!((again.records, again.extents), (0, 0));
    assert!(again.already_stored >= 6, "{again:?}");
}

#[test]
fn an_empty_slice_ends_the_walk_as_incomplete() {
    let mut src = MemNginx::default();
    let small = body(200, 6);
    src.slice(SMALL, &small, 0, "");
    src.slice_keeping(SMALL, &small, 64, "", 0); // a slice file with nothing in it
    src.slice(SMALL, &small, 128, "");
    src.slice(SMALL, &small, 192, "");
    let mut first_empty = MemNginx::default();
    first_empty.slice_keeping(SMALL, &small, 0, "", 0);
    let store = open(&MemFs::default());
    // Before the fix the walk stood still on the empty slice: give it a limit.
    let (done, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let r = run(&src, &store);
        let again = run(&first_empty, &store);
        let _ = done.send((r, again, read(&store, SMALL, Span::new(0, 64)).is_some()));
    });
    let (r, again, first_kept) = wait.recv_timeout(std::time::Duration::from_secs(10)).expect("the walk never ended");
    assert_eq!(r.records, 0, "not whole: {r:?}");
    assert!(r.incomplete >= 1 && first_kept, "kept as what it is: {r:?}");
    assert_eq!(again.records, 0, "{again:?}");
}
