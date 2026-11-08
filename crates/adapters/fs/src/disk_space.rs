//! Free and total bytes of a file system, from `df` (GNU coreutils).
//!
//! The standard library cannot ask the OS for this, the workspace forbids
//! `unsafe` (no `statvfs` call of our own) and takes no third-party crates,
//! so it runs `df -B1 --output=avail,size <dir>`. That costs a process per
//! check, which is fine every 30 seconds. A file system that does not answer
//! (a hung network mount) is given up on after a timeout.

use std::path::Path;
use std::time::Duration;

use lanstash_domain::evict::Disk;
use lanstash_ports::storage::StoreError;

use lanstash_adapter_os::command::run;

/// Parses `df -B1 --output=avail,size` output: a header line, then `avail size`.
#[must_use]
pub fn parse_df(out: &str) -> Option<Disk> {
    let line = out.lines().nth(1)?;
    let mut it = line.split_ascii_whitespace();
    let free = it.next()?.parse().ok()?;
    let total = it.next()?.parse().ok()?;
    Some(Disk { free, total })
}

/// Asks `df` about the file system holding `dir`.
///
/// # Errors
/// `df` is missing, fails, prints something unexpected, or takes longer than `timeout`.
pub fn df_space(dir: &Path, timeout: Duration) -> Result<Disk, StoreError> {
    let dir = dir.to_string_lossy();
    let out = run("df", &["-B1", "--output=avail,size", &dir], timeout).map_err(StoreError::other)?;
    if !out.success {
        return Err(StoreError::other(format!("df {dir}: exit code {:?}", out.code)));
    }
    parse_df(&out.stdout).ok_or_else(|| StoreError::other(format!("df: unexpected output {:?}", out.stdout)))
}
