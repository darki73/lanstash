//! An nginx proxy cache directory (`levels=2:2`), read-only: the source of
//! [`lanstash_app::import_nginx`].

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::storage::{FileRead, StoreError};

use crate::fs_file::{FsFile, io_error};

#[derive(Debug, Clone)]
pub struct NginxDir {
    root: PathBuf,
}

impl NginxDir {
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self { root: root.to_path_buf() }
    }
}

fn is_level_dir(name: &str) -> bool {
    name.len() == 2 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_cache_file(name: &str) -> bool {
    name.len() == 32 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

impl NginxCacheSource for NginxDir {
    fn list_files(&self, top_dirs: &[String]) -> Result<Vec<String>, StoreError> {
        let mut out = Vec::new();
        for top in top_dirs {
            if !is_level_dir(top) {
                return Err(StoreError::other(format!("not a cache level directory: {top:?}")));
            }
            let top_path = self.root.join(top);
            let subs = match fs::read_dir(&top_path) {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(io_error(&top_path, &e)),
            };
            for sub in subs {
                let sub = sub.map_err(|e| io_error(&top_path, &e))?;
                let Some(sub_name) = sub.file_name().to_str().map(str::to_owned) else { continue };
                if !is_level_dir(&sub_name) {
                    continue;
                }
                let sub_path = sub.path();
                for f in fs::read_dir(&sub_path).map_err(|e| io_error(&sub_path, &e))? {
                    let f = f.map_err(|e| io_error(&sub_path, &e))?;
                    if let Some(n) = f.file_name().to_str()
                        && is_cache_file(n)
                    {
                        out.push(format!("{top}/{sub_name}/{n}"));
                    }
                }
            }
        }
        Ok(out)
    }

    fn open(&self, rel: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        Ok(Arc::new(FsFile::open_read(&self.path(rel)?)?))
    }

    fn exists(&self, rel: &str) -> Result<bool, StoreError> {
        let path = self.path(rel)?;
        match fs::metadata(&path) {
            Ok(m) => Ok(m.is_file()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_error(&path, &e)),
        }
    }
}

impl NginxDir {
    /// The full path of a cache file (`ab/cd/<md5>`), refusing anything else.
    fn path(&self, rel: &str) -> Result<PathBuf, StoreError> {
        let parts: Vec<&str> = rel.split('/').collect();
        match parts.as_slice() {
            [a, b, n] if is_level_dir(a) && is_level_dir(b) && is_cache_file(n) => {
                Ok(self.root.join(a).join(b).join(n))
            }
            _ => Err(StoreError::other(format!("invalid cache path {rel:?}"))),
        }
    }
}
