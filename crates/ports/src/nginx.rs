//! Read access to an nginx (lancache) cache directory.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::storage::{FileRead, StoreError, StoreErrorKind};

/// An nginx cache directory, read-only (implemented by an adapter).
pub trait NginxCacheSource: Send + Sync {
    /// Cache files below the given top-level directories (`"00"` to `"ff"`), as
    /// paths relative to the cache root (`"ab/cd/<md5>"`).
    ///
    /// # Errors
    /// The directory could not be listed.
    fn list_files(&self, top_dirs: &[String]) -> Result<Vec<String>, StoreError>;

    /// Opens one cache file.
    ///
    /// # Errors
    /// `NotFound` when missing; other storage failures.
    fn open(&self, rel: &str) -> Result<Arc<dyn FileRead>, StoreError>;

    /// Whether a cache file exists (without reading it; by opening it unless
    /// the source knows a cheaper way).
    ///
    /// # Errors
    /// A storage failure other than the file being absent.
    fn exists(&self, rel: &str) -> Result<bool, StoreError> {
        match self.open(rel) {
            Ok(_) => Ok(true),
            Err(e) if e.kind == StoreErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }
}
