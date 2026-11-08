//! An append-only log file that rotates itself by size: when the next write
//! would pass `max_bytes`, `log` becomes `log.1` (`log.1` becomes `log.2`, …,
//! the oldest beyond `keep` is deleted) and a new `log` is started. No
//! logrotate and no signal needed; `tail -F` follows the new file.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// When and how much to rotate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rotation {
    /// Rotate before the file would grow past this (0 = never).
    pub max_bytes: u64,
    /// Rotated files kept (`log.1` … `log.<keep>`); 0 = none, the file is just started over.
    pub keep: u32,
}

impl Default for Rotation {
    /// 100 MiB, 5 old files.
    fn default() -> Self {
        Self { max_bytes: 100 << 20, keep: 5 }
    }
}

#[derive(Debug)]
pub struct RotatingFile {
    path: PathBuf,
    rotation: Rotation,
    file: File,
    len: u64,
}

fn numbered(path: &Path, n: u32) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".{n}"));
    PathBuf::from(s)
}

impl RotatingFile {
    /// Opens (creating if needed) `path` for appending.
    ///
    /// # Errors
    /// The file can't be opened.
    pub fn open(path: &Path, rotation: Rotation) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let len = file.metadata()?.len();
        Ok(Self { path: path.to_path_buf(), rotation, file, len })
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let keep = self.rotation.keep;
        if keep == 0 {
            fs::remove_file(&self.path)?;
        } else {
            let _ = fs::remove_file(numbered(&self.path, keep));
            for n in (1..keep).rev() {
                let from = numbered(&self.path, n);
                if from.exists() {
                    fs::rename(&from, numbered(&self.path, n + 1))?;
                }
            }
            fs::rename(&self.path, numbered(&self.path, 1))?;
        }
        self.file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        self.len = 0;
        Ok(())
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let max = self.rotation.max_bytes;
        if max > 0 && self.len > 0 && self.len + buf.len() as u64 > max {
            self.rotate()?;
        }
        let n = self.file.write(buf)?;
        self.len += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
