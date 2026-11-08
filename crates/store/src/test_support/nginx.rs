//! An in-memory nginx (lancache) cache: files written as nginx writes its
//! slices (header struct, `KEY:` line, stored response, body), by key.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lanstash_domain::nginx_cache::{CACHE_VERSION, HEADER_STRUCT_LEN, KEY_MARKER, cache_file_path, slice_key};
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::storage::{FileRead, StoreError, StoreErrorKind};

use super::MemFile;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Cache files by their path below the cache directory.
#[derive(Debug, Default)]
pub struct MemNginx {
    pub files: Mutex<BTreeMap<String, Vec<u8>>>,
    /// Cache files opened, in order.
    pub opened: Mutex<Vec<String>>,
}

/// One cache file as nginx writes it.
///
/// # Panics
/// When the key and headers are over 64 KiB.
#[must_use]
pub fn cache_file(key: &str, status_line: &str, headers: &str, body: &[u8]) -> Vec<u8> {
    let http = format!("{status_line}\r\n{headers}\r\n");
    let mut f = vec![0u8; HEADER_STRUCT_LEN];
    f[0..8].copy_from_slice(&CACHE_VERSION.to_le_bytes());
    f.extend_from_slice(KEY_MARKER);
    f.extend_from_slice(key.as_bytes());
    f.push(b'\n');
    let header_start = u16::try_from(f.len()).expect("key under 64 KiB");
    f.extend_from_slice(http.as_bytes());
    let body_start = u16::try_from(f.len()).expect("headers under 64 KiB");
    f[54..56].copy_from_slice(&header_start.to_le_bytes());
    f[56..58].copy_from_slice(&body_start.to_le_bytes());
    f.extend_from_slice(body);
    f
}

impl MemNginx {
    /// Stores slice `start` (of `slice` bytes) of `body` under `base` (`<service><uri>`).
    ///
    /// # Panics
    /// When `start` is past the end of `body`.
    pub fn slice(&self, base: &str, body: &[u8], start: u64, slice: u64) {
        let end = (start + slice).min(body.len() as u64);
        let key = slice_key(base, start, slice);
        let headers = format!(
            "Content-Type: application/octet-stream\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{}/{}\r\n",
            end - start,
            end.saturating_sub(1),
            body.len()
        );
        let at = |n: u64| usize::try_from(n).expect("in memory");
        let f = cache_file(&key, "HTTP/1.1 206 Partial Content", &headers, &body[at(start)..at(end)]);
        self.put(&key, f);
    }

    /// Stores every slice of `body`.
    ///
    /// # Panics
    /// When `slice` is 0.
    pub fn whole(&self, base: &str, body: &[u8], slice: u64) {
        let step = usize::try_from(slice).expect("in memory");
        for start in (0..body.len().max(1) as u64).step_by(step) {
            self.slice(base, body, start, slice);
        }
    }

    /// Stores a cache file for `key` as given.
    pub fn put(&self, key: &str, file: Vec<u8>) {
        lock(&self.files).insert(cache_file_path(key), file);
    }

    /// Drops the cache file of `key`.
    pub fn remove(&self, key: &str) {
        lock(&self.files).remove(&cache_file_path(key));
    }
}

impl NginxCacheSource for MemNginx {
    fn list_files(&self, _: &[String]) -> Result<Vec<String>, StoreError> {
        Ok(lock(&self.files).keys().cloned().collect())
    }

    fn open(&self, rel: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        lock(&self.opened).push(rel.to_owned());
        let files = lock(&self.files);
        let b = files.get(rel).ok_or_else(|| StoreError::new(StoreErrorKind::NotFound, rel))?;
        let f = MemFile::default();
        f.with_bytes(|d| d.extend_from_slice(b));
        Ok(Arc::new(f))
    }
}
