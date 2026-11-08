//! A store root on a file system: [`StoreFs`] over one directory tree.
//!
//! Paths from the store are relative, `/`-separated and validated here so that
//! nothing can escape the root. Files that never change again (sealed segments)
//! can be opened through extra paths to the same directory, e.g. one SMB mount
//! per session: each session is served by its own `smbd` process.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use lanstash_domain::evict::Disk;
use lanstash_ports::storage::{DirEntry, FileRead, FileWrite, StoreError, StoreErrorKind, StoreFs};

use crate::disk_space::df_space;
use crate::fs_file::{FsFile, io_error};

/// How long `df` may take before the free space counts as unknown.
const SPACE_TIMEOUT: Duration = Duration::from_secs(10);

/// A directory tree, plus optional read-only views of the same tree.
#[derive(Debug)]
pub struct DirFs {
    root: PathBuf,
    /// `root` first, then the extra views.
    read_roots: Vec<PathBuf>,
    next: AtomicUsize,
}

/// Checks a store path: relative, `/`-separated, no empty, `.` or `..`
/// components, nothing a platform would read as a drive or separator.
fn valid(path: &str) -> bool {
    path.is_empty()
        || path
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != ".." && !c.contains(['\\', ':', '\0']) && c.len() <= 255)
}

impl DirFs {
    /// Uses `root`, creating it if missing.
    ///
    /// # Errors
    /// The directory can't be created.
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        Self::with_read_paths(root, &[])
    }

    /// Like [`DirFs::open`], with extra views of the same directory for
    /// [`StoreFs::open_read_immutable`]. They must exist and list the same entries.
    ///
    /// # Errors
    /// `root` can't be created, or a view is missing or lists different entries.
    pub fn with_read_paths(root: &Path, views: &[PathBuf]) -> Result<Self, StoreError> {
        fs::create_dir_all(root).map_err(|e| io_error(root, &e))?;
        let me = Self { root: root.to_path_buf(), read_roots: vec![root.to_path_buf()], next: AtomicUsize::new(0) };
        let names = me.list("")?;
        for v in views {
            if !v.is_dir() {
                return Err(StoreError::new(StoreErrorKind::NotFound, format!("{} is not a directory", v.display())));
            }
            let other = list_dir(v)?;
            if other != names {
                return Err(StoreError::other(format!(
                    "{} lists {} entries but {} lists {}: not the same directory?",
                    v.display(),
                    other.len(),
                    root.display(),
                    names.len()
                )));
            }
        }
        Ok(Self { read_roots: std::iter::once(root.to_path_buf()).chain(views.iter().cloned()).collect(), ..me })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Number of paths sealed files are opened through.
    #[must_use]
    pub fn path_count(&self) -> usize {
        self.read_roots.len()
    }

    fn under(base: &Path, path: &str) -> Result<PathBuf, StoreError> {
        if !valid(path) {
            return Err(StoreError::other(format!("invalid store path {path:?}")));
        }
        Ok(if path.is_empty() {
            base.to_path_buf()
        } else {
            path.split('/').fold(base.to_path_buf(), |p, c| p.join(c))
        })
    }

    fn path(&self, path: &str) -> Result<PathBuf, StoreError> {
        Self::under(&self.root, path)
    }

    fn create_parent(p: &Path) -> Result<(), StoreError> {
        match p.parent() {
            Some(d) => fs::create_dir_all(d).map_err(|e| io_error(d, &e)),
            None => Ok(()),
        }
    }
}

fn list_dir(dir: &Path) -> Result<Vec<DirEntry>, StoreError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| io_error(dir, &e))? {
        let entry = entry.map_err(|e| io_error(dir, &e))?;
        // Names that are not UTF-8 are not ours.
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        let is_dir = entry.file_type().map_err(|e| io_error(&entry.path(), &e))?.is_dir();
        out.push(DirEntry { name, is_dir });
    }
    out.sort();
    Ok(out)
}

/// Makes a directory entry change (create, rename) durable.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> Result<(), StoreError> {
    fs::File::open(dir).and_then(|d| d.sync_all()).map_err(|e| io_error(dir, &e))
}

/// Windows has no directory fsync; NTFS journals metadata itself.
#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn sync_dir(_dir: &Path) -> Result<(), StoreError> {
    Ok(())
}

impl StoreFs for DirFs {
    fn list(&self, dir: &str) -> Result<Vec<DirEntry>, StoreError> {
        list_dir(&self.path(dir)?)
    }

    fn open_read(&self, path: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        Ok(Arc::new(FsFile::open_read(&self.path(path)?)?))
    }

    fn open_read_immutable(&self, path: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        let i = self.next.fetch_add(1, Ordering::Relaxed) % self.read_roots.len();
        Ok(Arc::new(FsFile::open_read(&Self::under(&self.read_roots[i], path)?)?))
    }

    fn create_new(&self, path: &str) -> Result<Arc<dyn FileWrite>, StoreError> {
        let p = self.path(path)?;
        Self::create_parent(&p)?;
        let f = FsFile::create_new(&p)?;
        if let Some(d) = p.parent() {
            sync_dir(d)?;
        }
        Ok(Arc::new(f))
    }

    fn open_write(&self, path: &str) -> Result<Arc<dyn FileWrite>, StoreError> {
        Ok(Arc::new(FsFile::open_write(&self.path(path)?)?))
    }

    fn rename(&self, from: &str, to: &str) -> Result<(), StoreError> {
        let (a, b) = (self.path(from)?, self.path(to)?);
        Self::create_parent(&b)?;
        fs::rename(&a, &b).map_err(|e| io_error(&a, &e))?;
        match b.parent() {
            Some(d) => sync_dir(d),
            None => Ok(()),
        }
    }

    fn space(&self) -> Result<Option<Disk>, StoreError> {
        df_space(&self.root, SPACE_TIMEOUT).map(Some)
    }

    fn remove_file(&self, path: &str) -> Result<(), StoreError> {
        let p = self.path(path)?;
        fs::remove_file(&p).map_err(|e| io_error(&p, &e))
    }
}
