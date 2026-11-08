//! Puts bytes of a CDN file that came from somewhere other than the CDN (such
//! as lancache's cache) into the stash, exactly where a download through the
//! cache would have put them ([`crate::fetch`]): a whole object below
//! `object_file_min` as one segment record, anything else as an extent of the
//! object's file. The caller has checked the bytes, or checks them as they
//! pass and commits only then ([`StashPut::begin`]).

use std::sync::Arc;

use lanstash_domain::checksum::crc::{Crc32c, crc32c};
use lanstash_domain::extent::Span;
use lanstash_domain::object::object_key;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_domain::service::{Profile, Route};
use lanstash_store::game::ObjectFile;
use lanstash_store::store::{ObjectRef, Store};

pub struct StashPut {
    store: Arc<Store>,
    service: String,
    profile: Arc<dyn Profile>,
    object_file_min: u64,
}

impl std::fmt::Debug for StashPut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StashPut").field("service", &self.service).finish_non_exhaustive()
    }
}

impl StashPut {
    /// `profile`: the caching rules of the platform the bytes belong to;
    /// `object_file_min` as the cache uses it.
    #[must_use]
    pub fn new(store: Arc<Store>, profile: Arc<dyn Profile>, object_file_min: u64) -> Self {
        Self { store, service: profile.service().to_owned(), profile, object_file_min }
    }

    /// Stores `bytes` at offset `start` of `target` (a CDN path), a file of
    /// `total` bytes, with the response headers `meta`; `verified` marks a
    /// whole record as checked against its upstream hash.
    ///
    /// # Errors
    /// The path is not one the platform caches, or the store refused the write.
    pub fn put(
        &self,
        target: &str,
        start: u64,
        bytes: &[u8],
        total: u64,
        meta: &[u8],
        verified: bool,
    ) -> Result<(), String> {
        let Route::Object { game, path } = self.profile.route(target) else {
            return Err(format!("{target}: not a path {} caches", self.service));
        };
        let key = object_key(self.profile.service(), path);
        let obj = ObjectRef { service: &self.service, game: &game, key: &key };
        let span = Span::at(start, bytes.len() as u64);
        if span.is_empty() || span.end > total {
            return Err(format!("{target}: bytes {span:?} outside {total}"));
        }
        let crc = crc32c(bytes);
        if start == 0 && span.end == total && total < self.object_file_min {
            let mut flags = RecordFlags::default();
            if verified {
                flags.0 |= RecordFlags::UPSTREAM_VERIFIED;
            }
            let w = self.store.reserve_record(obj, meta, total).map_err(|e| e.to_string())?;
            w.write(0, bytes).map_err(|e| e.to_string())?;
            return w.commit(flags, crc).map_err(|e| e.to_string());
        }
        let file = self.store.object_file(obj, path, total, meta).map_err(|e| e.to_string())?;
        file.writer().and_then(|w| w.write_all_at(start, bytes)).map_err(|e| e.to_string())?;
        self.store.commit_extent(&file, span, crc).map_err(|e| e.to_string())
    }

    /// Starts storing bytes `start..end` of `target`, a file of `total` bytes,
    /// that arrive in parts ([`Pending::write`]) and count only once
    /// committed ([`Pending::commit`]): a whole object below
    /// `object_file_min` is gathered and stored as one record then; anything
    /// else is written into the object's file as it comes, invisible until its
    /// extent is committed. Dropping it uncommitted stores nothing.
    ///
    /// # Errors
    /// As [`Self::put`].
    pub fn begin(&self, target: &str, start: u64, end: u64, total: u64, meta: &[u8]) -> Result<Pending<'_>, String> {
        let Route::Object { game, path } = self.profile.route(target) else {
            return Err(format!("{target}: not a path {} caches", self.service));
        };
        if end <= start || end > total {
            return Err(format!("{target}: bytes {start}..{end} outside {total}"));
        }
        let key = object_key(self.profile.service(), path);
        let target_file = if start == 0 && end == total && total < self.object_file_min {
            None
        } else {
            let obj = ObjectRef { service: &self.service, game: &game, key: &key };
            Some(self.store.object_file(obj, path, total, meta).map_err(|e| e.to_string())?)
        };
        Ok(Pending {
            put: self,
            target: target.to_owned(),
            game,
            key,
            meta: meta.to_vec(),
            start,
            end,
            total,
            file: target_file,
            held: Vec::new(),
            next: start,
            crc: Crc32c::new(),
        })
    }
}

/// Bytes being stored in parts, in order: see [`StashPut::begin`].
pub struct Pending<'a> {
    put: &'a StashPut,
    target: String,
    game: String,
    key: String,
    meta: Vec<u8>,
    start: u64,
    end: u64,
    total: u64,
    /// The object's file; `None`: a small whole object, gathered in `held`.
    file: Option<Arc<ObjectFile>>,
    held: Vec<u8>,
    /// Where the next part starts.
    next: u64,
    crc: Crc32c,
}

impl std::fmt::Debug for Pending<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pending").field("target", &self.target).field("next", &self.next).finish_non_exhaustive()
    }
}

impl Pending<'_> {
    /// The next part, at `at` (where the previous one ended).
    ///
    /// # Errors
    /// Out of order, past the end, or the store refused the write.
    pub fn write(&mut self, at: u64, bytes: &[u8]) -> Result<(), String> {
        let end = at + bytes.len() as u64;
        if at != self.next || end > self.end {
            return Err(format!("{}: part {at}..{end} out of order", self.target));
        }
        match &self.file {
            Some(f) => f.writer().and_then(|w| w.write_all_at(at, bytes)).map_err(|e| e.to_string())?,
            None => self.held.extend_from_slice(bytes),
        }
        self.crc.update(bytes);
        self.next = end;
        Ok(())
    }

    /// Makes the bytes count: the record, or the extent `start..end`.
    ///
    /// # Errors
    /// Not all bytes were written, or the store refused.
    pub fn commit(self, verified: bool) -> Result<(), String> {
        if self.next != self.end {
            return Err(format!(
                "{}: {} of {} bytes written",
                self.target,
                self.next - self.start,
                self.end - self.start
            ));
        }
        let crc = self.crc.finish();
        let p = self.put;
        match &self.file {
            Some(f) => p.store.commit_extent(f, Span::new(self.start, self.end), crc).map_err(|e| e.to_string()),
            None => {
                let mut flags = RecordFlags::default();
                if verified {
                    flags.0 |= RecordFlags::UPSTREAM_VERIFIED;
                }
                let obj = ObjectRef { service: &p.service, game: &self.game, key: &self.key };
                let w = p.store.reserve_record(obj, &self.meta, self.total).map_err(|e| e.to_string())?;
                w.write(0, &self.held).map_err(|e| e.to_string())?;
                w.commit(flags, crc).map_err(|e| e.to_string())
            }
        }
    }
}
