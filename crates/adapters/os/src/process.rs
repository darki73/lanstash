//! A process's open files and the most it may open, from Linux's `/proc`
//! (`/proc/<pid>/fd` and the `Max open files` line of `/proc/<pid>/limits`).
//! Elsewhere there is no `/proc`: the answer is an error, not a guess.

use std::fs;
use std::path::PathBuf;

/// Whose files: a process by id, or this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whose {
    Pid(u32),
    This,
}

impl Whose {
    fn dir(self) -> PathBuf {
        match self {
            Self::Pid(p) => PathBuf::from(format!("/proc/{p}")),
            Self::This => PathBuf::from("/proc/self"),
        }
    }
}

/// How many files the process has open, and the most it may (its soft limit).
///
/// # Errors
/// No `/proc` (not Linux), no such process, or not allowed to look (another user's process).
pub fn open_files(whose: Whose) -> Result<(u64, u64), String> {
    let dir = whose.dir();
    let fds = fs::read_dir(dir.join("fd")).map_err(|e| format!("{}: {e}", dir.join("fd").display()))?.count() as u64;
    let limits =
        fs::read_to_string(dir.join("limits")).map_err(|e| format!("{}: {e}", dir.join("limits").display()))?;
    let limit =
        max_open_files(&limits).ok_or_else(|| format!("{}: no open-file limit", dir.join("limits").display()))?;
    Ok((fds, limit))
}

/// The soft `Max open files` of a `/proc/<pid>/limits` text (`unlimited`: the largest number).
#[must_use]
pub fn max_open_files(limits: &str) -> Option<u64> {
    let line = limits.lines().find(|l| l.starts_with("Max open files"))?;
    let soft = line.trim_start_matches("Max open files").split_whitespace().next()?;
    if soft == "unlimited" { Some(u64::MAX) } else { soft.parse().ok() }
}
