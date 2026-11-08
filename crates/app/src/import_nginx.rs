//! Use case: import an existing nginx (lancache) cache into the store. The
//! nginx cache is only read, never modified.
//!
//! nginx keeps every object as slices (1 MiB by default) in separate files,
//! named by `md5(key)`, the key being `<service><uri>bytes=<a>-<b>`. Every
//! file is handled independently, and each slice exactly once:
//!
//! - Objects smaller than `object_file_min` are reassembled by the worker that
//!   finds their **first** slice: complete objects become one segment record
//!   (checked against the profile's body check, e.g. Steam's `x-content-crc`);
//!   incomplete ones have their present slices written to an object file.
//!   Later slices of such objects are skipped when the first slice exists.
//! - Larger objects: every slice becomes an extent of the object file.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::checksum::crc::{crc32, crc32c};
use lanstash_domain::extent::Span;
use lanstash_domain::nginx_cache::{self, NginxCacheFile};
use lanstash_domain::object::object_key;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_domain::service::{BodyCheck, Profile, Profiles, Route};
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::storage::{FileRead, StoreErrorKind};
use lanstash_store::index::Lookup;
use lanstash_store::store::{ObjectRef, Store};

use crate::nginx_file::{FileError, bad, fatal, meta, read_cache_file, response_head};

/// Import settings (passed in from the composition root).
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// nginx top-level cache directories to import (`"00"`–`"ff"`).
    pub top_dirs: Vec<String>,
    pub threads: usize,
    /// Services to import (the cache identifiers nginx used); others are skipped.
    pub services: Vec<String>,
    /// The caching rules of each service.
    pub profiles: Profiles,
    /// nginx `slice` size (lancache: 1 MiB).
    pub slice_bytes: u64,
    /// Objects at least this large go to object files (as when serving).
    pub object_file_min: u64,
}

/// Counters describing an import run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub files_listed: u64,
    pub files_done: u64,
    /// Objects stored as segment records.
    pub records: u64,
    /// Slices stored as object-file extents.
    pub extents: u64,
    pub imported_bytes: u64,
    pub verified: u64,
    pub already_stored: u64,
    pub skipped_service: u64,
    pub skipped_uncacheable: u64,
    /// Later slices imported together with their first slice.
    pub joined_slices: u64,
    /// Small objects with missing slices (their present slices went to an object file).
    pub incomplete: u64,
    pub check_failed: u64,
    pub bad_files: u64,
    pub segments_closed: u64,
    /// First few per-file problems, for the operator.
    pub sample_errors: Vec<String>,
}

/// Import stopped because the store could not be written.
#[derive(Debug)]
pub struct ImportError(pub String);

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "import failed: {}", self.0)
    }
}

impl std::error::Error for ImportError {}

#[derive(Default)]
struct Counters {
    files_done: AtomicU64,
    records: AtomicU64,
    extents: AtomicU64,
    imported_bytes: AtomicU64,
    verified: AtomicU64,
    already_stored: AtomicU64,
    skipped_service: AtomicU64,
    skipped_uncacheable: AtomicU64,
    joined_slices: AtomicU64,
    incomplete: AtomicU64,
    check_failed: AtomicU64,
    bad_files: AtomicU64,
}

fn inc(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

const MAX_SAMPLE_ERRORS: usize = 20;
const PROGRESS_EVERY: u64 = 2000;

struct Importer<'a> {
    source: &'a dyn NginxCacheSource,
    store: &'a Store,
    opts: &'a ImportOptions,
    profiles: Vec<(String, Arc<dyn Profile>)>,
    c: Counters,
}

/// Runs the import. `progress` is called periodically from worker threads.
///
/// # Errors
/// Listing the source or writing the store failed. Problems with individual
/// cache files are counted in the report instead.
pub fn import_nginx(
    source: &dyn NginxCacheSource,
    store: &Store,
    opts: &ImportOptions,
    progress: &(dyn Fn(&ImportReport) + Sync),
) -> Result<ImportReport, ImportError> {
    let files = source.list_files(&opts.top_dirs).map_err(|e| ImportError(e.to_string()))?;
    let profiles = opts.services.iter().map(|s| (s.clone(), opts.profiles.get(s))).collect();
    let im = Importer { source, store, opts, profiles, c: Counters::default() };
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let failure: Mutex<Option<String>> = Mutex::new(None);
    let abort = AtomicBool::new(false);
    let next = AtomicUsize::new(0);
    let listed = files.len() as u64;

    std::thread::scope(|s| {
        for _ in 0..opts.threads.max(1) {
            s.spawn(|| {
                while !abort.load(Ordering::Relaxed) {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(rel) = files.get(i) else { break };
                    match im.file(rel) {
                        Ok(()) => {}
                        Err(FileError::Bad(msg)) => {
                            inc(&im.c.bad_files);
                            let mut e = errors.lock().unwrap_or_else(PoisonError::into_inner);
                            if e.len() < MAX_SAMPLE_ERRORS {
                                e.push(format!("{rel}: {msg}"));
                            }
                        }
                        Err(FileError::Fatal(msg)) => {
                            *failure.lock().unwrap_or_else(PoisonError::into_inner) = Some(format!("{rel}: {msg}"));
                            abort.store(true, Ordering::Relaxed);
                        }
                    }
                    let done = im.c.files_done.fetch_add(1, Ordering::Relaxed) + 1;
                    if done.is_multiple_of(PROGRESS_EVERY) {
                        progress(&im.snapshot(listed, &errors, 0));
                    }
                }
            });
        }
    });

    let closed = store.close_segments().len() as u64;
    if let Some(msg) = failure.into_inner().unwrap_or_else(PoisonError::into_inner) {
        return Err(ImportError(msg));
    }
    Ok(im.snapshot(listed, &errors, closed))
}

/// One parsed slice file.
struct Slice {
    file: Arc<dyn FileRead>,
    head: NginxCacheFile,
    span: Span,
}

/// What a cache key refers to.
struct Target<'k> {
    service: &'k str,
    profile: Arc<dyn Profile>,
    game: String,
    path: &'k str,
    key: String,
}

impl Importer<'_> {
    fn snapshot(&self, listed: u64, errors: &Mutex<Vec<String>>, segments_closed: u64) -> ImportReport {
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        let c = &self.c;
        ImportReport {
            files_listed: listed,
            files_done: g(&c.files_done),
            records: g(&c.records),
            extents: g(&c.extents),
            imported_bytes: g(&c.imported_bytes),
            verified: g(&c.verified),
            already_stored: g(&c.already_stored),
            skipped_service: g(&c.skipped_service),
            skipped_uncacheable: g(&c.skipped_uncacheable),
            joined_slices: g(&c.joined_slices),
            incomplete: g(&c.incomplete),
            check_failed: g(&c.check_failed),
            bad_files: g(&c.bad_files),
            segments_closed,
            sample_errors: errors.lock().unwrap_or_else(PoisonError::into_inner).clone(),
        }
    }

    /// `steam/depot/…` → service `steam`, path `/depot/…`, routed by its profile.
    fn target<'k>(&self, base: &'k str) -> Option<Option<Target<'k>>> {
        let i = base.find('/')?;
        let (service, path) = (&base[..i], &base[i..]);
        let (_, profile) = self.profiles.iter().find(|(s, _)| s == service)?;
        Some(match profile.route(path) {
            Route::Pass => None,
            Route::Object { game, path } => Some(Target {
                service,
                profile: Arc::clone(profile),
                game,
                key: object_key(profile.service(), path),
                path,
            }),
        })
    }

    fn open_slice(&self, rel: &str) -> Result<Option<Slice>, FileError> {
        let file = match self.source.open(rel) {
            Ok(f) => f,
            Err(e) if e.kind == StoreErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(bad(e)),
        };
        let head = read_cache_file(&*file)?;
        let start = head.content_range().map_or(0, |r| r.start);
        let span = Span::at(start, head.body_len);
        Ok(Some(Slice { file, head, span }))
    }

    fn file(&self, rel: &str) -> Result<(), FileError> {
        let Some(slice) = self.open_slice(rel)? else { return Err(FileError::Bad("vanished".into())) };
        let key = slice.head.key.clone();
        let (base, range) = nginx_cache::split_slice_key(&key);
        let t = match self.target(base) {
            None => {
                inc(&self.c.skipped_service);
                return Ok(());
            }
            Some(None) => {
                inc(&self.c.skipped_uncacheable);
                return Ok(());
            }
            Some(Some(t)) => t,
        };
        let total = slice.head.content_range().map_or(slice.head.body_len, |r| r.total);
        if slice.span.end > total || range.is_some_and(|(a, _)| a != slice.span.start) {
            return Err(FileError::Bad(format!("slice {:?} does not fit its key or total {total}", slice.span)));
        }
        if total >= self.opts.object_file_min {
            return self.extents(&t, total, &[slice]);
        }
        if slice.span.start != 0 {
            // Imported with its first slice, if that exists.
            let first = nginx_cache::slice_key(base, 0, self.opts.slice_bytes);
            return match self.source.open(&nginx_cache::cache_file_path(&first)) {
                Ok(_) => Ok(()),
                Err(e) if e.kind == StoreErrorKind::NotFound => self.extents(&t, total, &[slice]),
                Err(e) => Err(bad(e)),
            };
        }
        self.small_object(&t, base, total, slice)
    }

    /// Reassembles an object from its first slice; a record when complete.
    fn small_object(&self, t: &Target<'_>, base: &str, total: u64, first: Slice) -> Result<(), FileError> {
        if matches!(self.store.lookup(&t.key), Lookup::Found(o) if o.covers(Span::new(0, total))) {
            inc(&self.c.already_stored);
            return Ok(());
        }
        // The first slice's length, unless it says nothing (the last slice, or an empty one).
        let slice_size = if first.span.end < total && !first.span.is_empty() {
            first.span.len()
        } else {
            self.opts.slice_bytes.max(1)
        };
        let mut slices = vec![first];
        let mut complete = true;
        let mut pos = slices[0].span.end;
        while pos < total {
            let key = nginx_cache::slice_key(base, pos, slice_size);
            match self.open_slice(&nginx_cache::cache_file_path(&key))? {
                // A slice that moves the position on (an empty one would never end the walk).
                Some(s) if s.head.key == key && s.span.start == pos && pos < s.span.end && s.span.end <= total => {
                    pos = s.span.end;
                    slices.push(s);
                }
                Some(s) if s.head.key != key => {
                    return Err(FileError::Bad(format!("{key} is stored as {}", s.head.key)));
                }
                _ => {
                    complete = false;
                    pos = pos.saturating_add(slice_size);
                }
            }
        }
        self.c.joined_slices.fetch_add(slices.len() as u64 - 1, Ordering::Relaxed);
        if !complete {
            inc(&self.c.incomplete);
            return self.extents(t, total, &slices);
        }
        let mut body = vec![0u8; usize::try_from(total).map_err(bad)?];
        for s in &slices {
            let (a, b) = (usize::try_from(s.span.start).map_err(bad)?, usize::try_from(s.span.end).map_err(bad)?);
            s.file.read_exact_at(s.head.body_start, &mut body[a..b]).map_err(bad)?;
        }
        let head = &slices[0].head;
        let mut flags = RecordFlags::default();
        if let Some(BodyCheck::Crc32(expected)) = t.profile.body_check(&response_head(head)) {
            if crc32(&body) != expected {
                inc(&self.c.check_failed);
                return Ok(());
            }
            flags.0 |= RecordFlags::UPSTREAM_VERIFIED;
            inc(&self.c.verified);
        }
        let obj = ObjectRef { service: t.service, game: &t.game, key: &t.key };
        let w = self.store.reserve_record(obj, &meta(head), total).map_err(fatal)?;
        w.write(0, &body).map_err(fatal)?;
        w.commit(flags, crc32c(&body)).map_err(fatal)?;
        inc(&self.c.records);
        self.c.imported_bytes.fetch_add(total, Ordering::Relaxed);
        Ok(())
    }

    /// Writes slices into the object file as extents (skipping stored ones).
    fn extents(&self, t: &Target<'_>, total: u64, slices: &[Slice]) -> Result<(), FileError> {
        let stored = match self.store.lookup(&t.key) {
            Lookup::Found(o) => Some(o),
            Lookup::Unusable => return Err(FileError::Bad(format!("{} is marked unusable", t.key))),
            Lookup::Missing => None,
        };
        let Some(first) = slices.first() else { return Ok(()) };
        let obj = ObjectRef { service: t.service, game: &t.game, key: &t.key };
        let file = self.store.object_file(obj, t.path, total, &meta(&first.head)).map_err(bad)?;
        let writer = file.writer().map_err(fatal)?;
        for s in slices {
            if stored.as_ref().is_some_and(|o| o.covers(s.span)) {
                inc(&self.c.already_stored);
                continue;
            }
            let mut buf = vec![0u8; usize::try_from(s.span.len()).map_err(bad)?];
            s.file.read_exact_at(s.head.body_start, &mut buf).map_err(bad)?;
            writer.write_all_at(s.span.start, &buf).map_err(fatal)?;
            self.store.commit_extent(&file, s.span, crc32c(&buf)).map_err(fatal)?;
            inc(&self.c.extents);
            self.c.imported_bytes.fetch_add(s.span.len(), Ordering::Relaxed);
        }
        Ok(())
    }
}
