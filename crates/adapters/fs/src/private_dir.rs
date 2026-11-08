//! Private files in one directory, for [`Secrets`] and for [`Keep`]: the
//! directory is mode 0700 and each file 0600 (on Unix), written to a
//! temporary name and renamed, so a file is either the old one or the new
//! one, never half.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use lanstash_ports::keep::Keep;
use lanstash_ports::secrets::Secrets;

/// Files kept under `dir`.
#[derive(Debug, Clone)]
pub struct PrivateDir {
    dir: PathBuf,
}

impl PrivateDir {
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The file for `name`; only `[a-z0-9-]` names are accepted.
    fn path(&self, name: &str) -> Result<PathBuf, String> {
        let ok = !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !ok {
            return Err(format!("a secret's name must be [a-z0-9-]: {name:?}"));
        }
        Ok(self.dir.join(name))
    }
}

fn err(p: &Path, e: &std::io::Error) -> String {
    format!("{}: {e}", p.display())
}

#[cfg(unix)]
fn private_dir(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    fs::DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(|e| err(dir, &e))?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| err(dir, &e))
}

#[cfg(not(unix))]
fn private_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| err(dir, &e))
}

#[cfg(unix)]
fn create_private(p: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(p)
}

#[cfg(not(unix))]
fn create_private(p: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new().write(true).create(true).truncate(true).open(p)
}

/// Numbers the temp files of this process's writes.
static TEMP_NO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl PrivateDir {
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let p = self.path(name)?;
        match fs::read(&p) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(err(&p, &e)),
        }
    }

    fn write(&self, name: &str, data: &[u8]) -> Result<(), String> {
        let p = self.path(name)?;
        private_dir(&self.dir)?;
        // A name of its own per write: two saves at once never share (and cut) one file.
        let n = TEMP_NO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = self.dir.join(format!(".{name}.{}.{n}.tmp", std::process::id()));
        let mut f = create_private(&tmp).map_err(|e| err(&tmp, &e))?;
        f.write_all(data).and_then(|()| f.sync_all()).map_err(|e| err(&tmp, &e))?;
        fs::rename(&tmp, &p).map_err(|e| err(&p, &e))
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        let p = self.path(name)?;
        match fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(err(&p, &e)),
        }
    }
}

impl Keep for PrivateDir {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        self.read(name)
    }

    fn save(&self, name: &str, data: &[u8]) -> Result<(), String> {
        self.write(name, data)
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        self.delete(name)
    }
}

impl Secrets for PrivateDir {
    fn load(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.read(name)?.map(|b| String::from_utf8_lossy(&b).into_owned()))
    }

    fn save(&self, name: &str, value: &str) -> Result<(), String> {
        self.write(name, value.as_bytes())
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        self.delete(name)
    }
}
