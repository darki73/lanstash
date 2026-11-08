//! One file on a file system (local disk, SMB/NFS mount).
//!
//! Reads and writes are positional (`pread`/`pwrite` on Linux), so many
//! threads share one handle without any seek state.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use lanstash_ports::storage::{FileRead, FileWrite, StoreError, StoreErrorKind};

/// An open file.
#[derive(Debug)]
pub struct FsFile {
    file: File,
    path: PathBuf,
}

impl FsFile {
    /// Opens an existing file read-only.
    ///
    /// # Errors
    /// File missing or unreadable.
    pub fn open_read(path: &Path) -> Result<Self, StoreError> {
        refuse_link(path)?;
        let file = File::open(path).map_err(|e| io_error(path, &e))?;
        Ok(Self { file, path: path.to_path_buf() })
    }

    /// Opens an existing file for reading and writing.
    ///
    /// # Errors
    /// File missing or not writable.
    pub fn open_write(path: &Path) -> Result<Self, StoreError> {
        refuse_link(path)?;
        let file = OpenOptions::new().read(true).write(true).open(path).map_err(|e| io_error(path, &e))?;
        Ok(Self { file, path: path.to_path_buf() })
    }

    /// Creates a new, empty file. Fails if it already exists.
    ///
    /// # Errors
    /// File exists or can't be created.
    pub fn create_new(path: &Path) -> Result<Self, StoreError> {
        let file =
            OpenOptions::new().read(true).write(true).create_new(true).open(path).map_err(|e| io_error(path, &e))?;
        Ok(Self { file, path: path.to_path_buf() })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The stash holds only files lanstash created: a symbolic link in it was put there by
/// someone else, and is never followed (it could point anywhere the service may write).
fn refuse_link(path: &Path) -> Result<(), StoreError> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => {
            Err(StoreError::other(format!("{}: a symbolic link, not followed", path.display())))
        }
        _ => Ok(()),
    }
}

/// Maps an I/O error on `path` to the storage port's error.
pub(crate) fn io_error(path: &Path, e: &io::Error) -> StoreError {
    let kind = match e.kind() {
        io::ErrorKind::NotFound => StoreErrorKind::NotFound,
        io::ErrorKind::AlreadyExists => StoreErrorKind::AlreadyExists,
        io::ErrorKind::UnexpectedEof => StoreErrorKind::UnexpectedEof,
        _ => StoreErrorKind::Other,
    };
    StoreError::new(kind, format!("{}: {e}", path.display()))
}

impl FileRead for FsFile {
    fn size(&self) -> Result<u64, StoreError> {
        Ok(self.file.metadata().map_err(|e| io_error(&self.path, &e))?.len())
    }

    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), StoreError> {
        pos_io::read_exact_at(&self.file, buf, offset).map_err(|e| io_error(&self.path, &e))
    }

    fn os_file(&self) -> Option<&dyn std::any::Any> {
        Some(&self.file)
    }
}

impl FileWrite for FsFile {
    fn write_all_at(&self, offset: u64, data: &[u8]) -> Result<(), StoreError> {
        pos_io::write_all_at(&self.file, data, offset).map_err(|e| io_error(&self.path, &e))
    }

    /// Extending leaves a hole: sparse on ext4, ZFS, NTFS-over-SMB.
    fn set_len(&self, len: u64) -> Result<(), StoreError> {
        self.file.set_len(len).map_err(|e| io_error(&self.path, &e))
    }

    fn sync(&self) -> Result<(), StoreError> {
        self.file.sync_data().map_err(|e| io_error(&self.path, &e))
    }
}

/// Positional I/O. Linux is the supported target; Windows is best effort.
mod pos_io {
    use std::fs::File;
    use std::io;

    #[cfg(unix)]
    pub fn read_exact_at(f: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
        std::os::unix::fs::FileExt::read_exact_at(f, buf, offset)
    }

    #[cfg(unix)]
    pub fn write_all_at(f: &File, data: &[u8], offset: u64) -> io::Result<()> {
        std::os::unix::fs::FileExt::write_all_at(f, data, offset)
    }

    // Windows `seek_read`/`seek_write` take an explicit offset per call (and
    // also move the handle's file pointer, which we never use).
    #[cfg(windows)]
    pub fn read_exact_at(f: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        use std::os::windows::fs::FileExt;
        while !buf.is_empty() {
            match f.seek_read(buf, offset) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => {
                    buf = &mut buf[n..];
                    offset += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    pub fn write_all_at(f: &File, mut data: &[u8], mut offset: u64) -> io::Result<()> {
        use std::os::windows::fs::FileExt;
        while !data.is_empty() {
            match f.seek_write(data, offset) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    data = &data[n..];
                    offset += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
