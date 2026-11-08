//! What the storage layer needs from a file system.
//!
//! Paths are relative to a store root, `/`-separated, and built only by the
//! store's layout; adapters must still refuse anything that could leave the
//! root (`..`, absolute paths, backslashes).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt;

use lanstash_domain::evict::Disk;

/// Category of a storage failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreErrorKind {
    NotFound,
    AlreadyExists,
    /// Read past the end of the file.
    UnexpectedEof,
    Other,
}

/// A storage operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError {
    pub kind: StoreErrorKind,
    pub message: String,
}

impl StoreError {
    pub fn new(kind: StoreErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::new(StoreErrorKind::Other, message)
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl core::error::Error for StoreError {}

/// Positional, thread-safe read access to one file. Many threads may call
/// `read_exact_at` on one instance at once.
pub trait FileRead: Send + Sync {
    /// Current size in bytes.
    ///
    /// # Errors
    /// Storage failure.
    fn size(&self) -> Result<u64, StoreError>;

    /// Fills `buf` from `offset`.
    ///
    /// # Errors
    /// [`StoreErrorKind::UnexpectedEof`] past the end, or another storage failure.
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), StoreError>;

    /// The operating system's file under it, where there is one (a `std::fs::File`), for an
    /// adapter that can send its bytes to a socket without copying them through lanstash.
    /// `None` (the default): read it with [`FileRead::read_exact_at`].
    fn os_file(&self) -> Option<&dyn core::any::Any> {
        None
    }
}

/// Positional, thread-safe write access to one file. Concurrent writes to
/// non-overlapping ranges must be safe.
pub trait FileWrite: FileRead {
    /// Writes all of `data` at `offset`, extending the file if needed.
    ///
    /// # Errors
    /// Storage failure.
    fn write_all_at(&self, offset: u64, data: &[u8]) -> Result<(), StoreError>;

    /// Truncates, or extends with zeros (sparse where the file system supports it).
    ///
    /// # Errors
    /// Storage failure.
    fn set_len(&self, len: u64) -> Result<(), StoreError>;

    /// Flushes written data to durable storage (`fdatasync`).
    ///
    /// # Errors
    /// Storage failure.
    fn sync(&self) -> Result<(), StoreError>;
}

/// One directory entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// A directory tree (one store root).
pub trait StoreFs: Send + Sync {
    /// Entries of `dir` (`""` = the root), sorted by name.
    ///
    /// # Errors
    /// [`StoreErrorKind::NotFound`] if `dir` does not exist, or another storage failure.
    fn list(&self, dir: &str) -> Result<Vec<DirEntry>, StoreError>;

    /// Opens a file for shared positional reads.
    ///
    /// # Errors
    /// Missing or unreadable file.
    fn open_read(&self, path: &str) -> Result<Arc<dyn FileRead>, StoreError>;

    /// Opens a file that will not change any more (a sealed segment). Stores with
    /// several access paths to the same files (e.g. one SMB session each) may
    /// spread these handles across paths.
    ///
    /// # Errors
    /// Missing or unreadable file.
    fn open_read_immutable(&self, path: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        self.open_read(path)
    }

    /// Creates a new empty file, and missing parent directories.
    ///
    /// # Errors
    /// [`StoreErrorKind::AlreadyExists`] if it exists, or another storage failure.
    fn create_new(&self, path: &str) -> Result<Arc<dyn FileWrite>, StoreError>;

    /// Opens an existing file for reading and writing.
    ///
    /// # Errors
    /// Missing file or storage failure.
    fn open_write(&self, path: &str) -> Result<Arc<dyn FileWrite>, StoreError>;

    /// Renames `from` to `to`, replacing `to` if it exists; creates `to`'s parent directories.
    ///
    /// # Errors
    /// Missing `from` or storage failure.
    fn rename(&self, from: &str, to: &str) -> Result<(), StoreError>;

    /// Deletes a file.
    ///
    /// # Errors
    /// Missing file or storage failure.
    fn remove_file(&self, path: &str) -> Result<(), StoreError>;

    /// Free and total bytes of the file system holding this root, for
    /// eviction's `keep_free`; `None` when not known.
    ///
    /// # Errors
    /// The file system could not be asked.
    fn space(&self) -> Result<Option<Disk>, StoreError> {
        Ok(None)
    }
}

impl<T: FileRead + ?Sized> FileRead for Arc<T> {
    fn size(&self) -> Result<u64, StoreError> {
        (**self).size()
    }
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), StoreError> {
        (**self).read_exact_at(offset, buf)
    }
}

impl<T: FileWrite + ?Sized> FileWrite for Arc<T> {
    fn write_all_at(&self, offset: u64, data: &[u8]) -> Result<(), StoreError> {
        (**self).write_all_at(offset, data)
    }
    fn set_len(&self, len: u64) -> Result<(), StoreError> {
        (**self).set_len(len)
    }
    fn sync(&self) -> Result<(), StoreError> {
        (**self).sync()
    }
}
