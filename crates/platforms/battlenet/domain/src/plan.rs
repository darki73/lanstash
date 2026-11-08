//! What an install of a Battle.net build downloads ([`Plan`]), and how much
//! of it is held ([`Coverage`], [`PlanDetail`]).

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use lanstash_domain::extent::Span;

use crate::download::Installs;
use crate::locate::{Located, Place, Range, ranges};
use crate::tact::{Version, target};

/// The files of one build an install downloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub product: String,
    pub version: Version,
    /// CDN host and path the files are read from (planning reads from the first).
    pub host: String,
    pub path: String,
    /// Every CDN host of the region, the first being `host`: prefill spreads its requests over them.
    pub hosts: Vec<String>,
    /// Files fetched whole: (target, size; 0 when unknown).
    pub files: Vec<(String, u64)>,
    /// The archives' targets, by number.
    pub archives: Vec<String>,
    /// Archive files: bytes of archive number `.0`, and their encoding key (hex), sorted.
    pub parts: Vec<(usize, Span, String)>,
    /// The ranged requests that fetch them.
    pub requests: Vec<Range>,
    /// Wanted files the CDN does not have.
    pub absent: u64,
    /// The platforms, architectures and locales the build has installs for (its download
    /// manifest's tags), for an install other than the one planned.
    pub installs: Installs,
}

impl Plan {
    /// The bytes an install downloads.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.1).sum::<u64>() + self.parts.iter().map(|p| p.1.end - p.1.start).sum::<u64>()
    }
}

/// Where a build's files are: its product and version, the CDN path, the
/// region's hosts (the first is read from), the archives' targets and the
/// files fetched whole so far (target, size).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    pub product: String,
    pub version: Version,
    pub path: String,
    pub hosts: Vec<String>,
    pub archives: Vec<String>,
    pub files: Vec<(String, u64)>,
}

/// The plan of `build` from where its wanted files lie: archive files become
/// parts (sorted, once each) fetched by ranged requests at most `gap` bytes
/// apart, loose files are added to the whole files (once each), absent ones
/// are counted.
///
/// # Errors
/// A loose file's key is not a hash.
pub fn assemble(build: Build, located: &[Located], gap: u64) -> Result<Plan, String> {
    let Build { product, version, path, hosts, archives, mut files } = build;
    let mut absent = 0;
    let mut seen: BTreeSet<String> = files.iter().map(|f| f.0.clone()).collect();
    let mut parts: Vec<(usize, Span, String)> = Vec::new();
    for l in located {
        match l.place {
            Place::Archive { archive, offset, size } => {
                parts.push((archive, Span::new(offset, offset + size), l.key.clone()));
            }
            Place::Loose { size } => {
                let t = target(&path, "data", &l.key).ok_or("bad key")?;
                if seen.insert(t.clone()) {
                    files.push((t, size));
                }
            }
            Place::Absent => absent += 1,
        }
    }
    parts.sort_unstable_by_key(|p| (p.0, p.1.start));
    parts.dedup();
    let requests = ranges(located, gap);
    let host = hosts.first().cloned().unwrap_or_default();
    Ok(Plan {
        product,
        version,
        host,
        path,
        hosts,
        files,
        archives,
        parts,
        requests,
        absent,
        installs: Default::default(),
    })
}

/// How much of a plan is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Coverage {
    pub have: u64,
    pub want: u64,
}

impl Coverage {
    /// All of it.
    #[must_use]
    pub fn full(&self) -> bool {
        self.want > 0 && self.have >= self.want
    }
}

/// A plan in figures: what is held of whole files (configs, indexes, loose
/// files) and of archive ranges, and how many of each there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanDetail {
    pub files: Coverage,
    pub archived: Coverage,
    /// Whole files.
    pub file_count: usize,
    /// Files inside archives.
    pub part_count: usize,
    /// Archives they are in.
    pub archive_count: usize,
    /// Requests a prefill sends (whole files and merged ranges).
    pub requests: usize,
    /// Wanted files the CDN does not have.
    pub absent: u64,
}

impl PlanDetail {
    /// Whole files and archive ranges together.
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        Coverage { have: self.files.have + self.archived.have, want: self.files.want + self.archived.want }
    }
}
