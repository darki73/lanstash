//! Eviction policy: what to delete when the store
//! outgrows its limits. Pure: sizes and times in, choices out.
//!
//! Two limits, each optional, the stricter one wins:
//! - `max_size`: the store may hold at most this many bytes;
//! - `keep_free`: the file system must keep this much free (bytes or a
//!   percentage of its size), for disks shared with other data.
//!
//! There is no eviction by age: CDN objects are named by their content and
//! never go stale, so deleting old ones while space is left only loses hits.
//!
//! Victims are the least recently used items (a sealed segment or a whole
//! object file). Items used recently are protected: a download still writing
//! them, or a client still reading them.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

/// How much a file system must keep free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepFree {
    Bytes(u64),
    /// Percent of the file system's size (1–99).
    Percent(u8),
}

impl KeepFree {
    /// The bytes to keep free on a file system of `total` bytes.
    #[must_use]
    pub fn bytes(self, total: u64) -> u64 {
        match self {
            Self::Bytes(b) => b,
            Self::Percent(p) => (u128::from(total) * u128::from(p) / 100) as u64,
        }
    }
}

/// The limits; `None` = no such limit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    pub max_size: Option<u64>,
    pub keep_free: Option<KeepFree>,
}

impl Limits {
    #[must_use]
    pub fn is_off(&self) -> bool {
        self.max_size.is_none() && self.keep_free.is_none()
    }

    /// Bytes over `max_size` with `stored` bytes in the store.
    #[must_use]
    pub fn over_max_size(&self, stored: u64) -> u64 {
        self.max_size.map_or(0, |m| stored.saturating_sub(m))
    }

    /// Bytes missing to `keep_free` on a file system.
    #[must_use]
    pub fn short_of_free(&self, disk: Disk) -> u64 {
        self.keep_free.map_or(0, |k| k.bytes(disk.total).saturating_sub(disk.free))
    }
}

/// Free and total bytes of a file system (free = available to this process).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Disk {
    pub free: u64,
    pub total: u64,
}

/// Something that can be evicted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub bytes: u64,
    pub last_used_ms: u64,
}

/// Indices into `cands` to evict, least recently used first, until at least
/// `need` bytes are chosen (or nothing is left). Candidates used at or after
/// `protect_since_ms` are never chosen.
#[must_use]
pub fn choose(cands: &[Candidate], need: u64, protect_since_ms: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].last_used_ms < protect_since_ms).collect();
    order.sort_by_key(|&i| (cands[i].last_used_ms, core::cmp::Reverse(cands[i].bytes)));
    let mut out = Vec::new();
    let mut got = 0u64;
    for i in order {
        if got >= need {
            break;
        }
        got = got.saturating_add(cands[i].bytes);
        out.push(i);
    }
    out
}

/// When a game's segments and object files were last used, by segment
/// sequence number and object file number.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recency {
    pub segments: BTreeMap<u32, u64>,
    pub files: BTreeMap<u32, u64>,
}

impl Recency {
    /// Text form, one item per line: `s <seq> <ms>` or `o <no> <ms>`.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut s = String::from("# lanstash last-used times (ms since 1970)\n");
        for (seq, ms) in &self.segments {
            let _ = writeln!(s, "s {seq} {ms}");
        }
        for (no, ms) in &self.files {
            let _ = writeln!(s, "o {no} {ms}");
        }
        s
    }

    /// Parses [`Recency::encode`] output; malformed lines are skipped (the
    /// times are a hint, never a reason to fail).
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut r = Self::default();
        for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
            let mut it = line.split_ascii_whitespace();
            let (Some(kind), Some(id), Some(ms), None) = (it.next(), it.next(), it.next(), it.next()) else { continue };
            let (Ok(id), Ok(ms)) = (id.parse::<u32>(), ms.parse::<u64>()) else { continue };
            match kind {
                "s" => {
                    r.segments.insert(id, ms);
                }
                "o" => {
                    r.files.insert(id, ms);
                }
                _ => {}
            }
        }
        r
    }
}

/// The percentage form of a `keep_free` setting (`10%`); sizes (`500G`) are
/// parsed by the caller.
#[must_use]
pub fn parse_percent(v: &str) -> Option<u8> {
    let n: u8 = v.strip_suffix('%')?.trim().parse().ok()?;
    (1..=99).contains(&n).then_some(n)
}
