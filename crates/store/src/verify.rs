//! Checking stored bytes against their checksums (`store verify`).
//!
//! Every segment record carries the CRC-32C of its body, every journal extent
//! the CRC-32C of its bytes. [`plan`] lists what there is to check, one item
//! per segment or object file; [`VerifyItem::check`] reads one item completely.
//! Nothing is written, so this is safe next to a running server: records being
//! written are not committed yet (their space reads as unwritten), and an
//! extent is journaled only after its bytes are synced.

use std::sync::Arc;

use lanstash_domain::extent::Span;
use lanstash_domain::journal::{live_objects, read_journal};

use crate::game::{read_whole, segment_paths};
use crate::layout::GameDir;
use crate::segment::scan::{Problem, ScanOptions, crc32c_at, scan};
use crate::store::Roots;
use lanstash_ports::storage::{FileRead, StoreError, StoreErrorKind, StoreFs};

/// What one item holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Segment,
    ObjectFile,
}

/// One segment or object file to check.
pub struct VerifyItem {
    pub game: GameDir,
    /// Root-relative path.
    pub path: String,
    pub kind: ItemKind,
    /// Bytes to read (for progress).
    pub bytes: u64,
    fs: Arc<dyn StoreFs>,
    /// Object files: the journaled extents and their CRCs.
    extents: Vec<(Span, u32)>,
}

impl std::fmt::Debug for VerifyItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyItem").field("path", &self.path).field("kind", &self.kind).finish_non_exhaustive()
    }
}

/// Everything to check, and what could not even be listed.
#[derive(Debug, Default)]
pub struct VerifyPlan {
    pub items: Vec<VerifyItem>,
    /// Problems found while planning: (path, message).
    pub problems: Vec<(String, String)>,
    /// Service directories that belong to another root.
    pub ignored: Vec<String>,
}

/// The outcome of checking one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemResult {
    pub game: GameDir,
    pub path: String,
    pub kind: ItemKind,
    /// Records or extents whose bytes match their CRC.
    pub ok: u64,
    pub ok_bytes: u64,
    /// Each damaged record or extent, or why the item could not be read.
    pub problems: Vec<String>,
}

/// Lists every segment and object file of every game in `roots`.
///
/// # Errors
/// A root exists but can't be listed.
pub fn plan(roots: &Roots) -> Result<VerifyPlan, StoreError> {
    let found = roots.games()?;
    let mut out = VerifyPlan { ignored: found.ignored, problems: found.failed, ..VerifyPlan::default() };
    for (fs, dir) in found.games {
        match segment_paths(&*fs, &dir) {
            Ok(segs) => {
                for (_, path) in segs {
                    let bytes = fs.open_read(&path).and_then(|f| f.size()).unwrap_or(0);
                    out.items.push(VerifyItem {
                        game: dir.clone(),
                        path,
                        kind: ItemKind::Segment,
                        bytes,
                        fs: Arc::clone(&fs),
                        extents: Vec::new(),
                    });
                }
            }
            Err(e) => out.problems.push((dir.segment_dir(), e.to_string())),
        }
        let journal = dir.journal();
        let data = match read_whole(&*fs, &journal) {
            Ok(d) => d,
            Err(e) if e.kind == StoreErrorKind::NotFound => continue,
            Err(e) => {
                out.problems.push((journal, e.to_string()));
                continue;
            }
        };
        let read = read_journal(&data);
        if read.refused {
            let why = read.stopped.unwrap_or_default();
            out.problems.push((journal, format!("not read: {why}")));
            continue;
        }
        // A torn tail is normal after a crash (and cut at the next start); a damaged
        // entry in the middle loses what it said.
        let lost = data.len() as u64 - read.valid_len;
        if let Some(why) = &read.stopped
            && lost > 0
        {
            out.problems.push((journal.clone(), format!("{lost} bytes after the last valid entry ({why})")));
        }
        if read.skipped > 0 {
            out.problems.push((journal.clone(), format!("{} bytes of damaged entries skipped", read.skipped)));
        }
        let live = live_objects(read.entries);
        if live.bad > 0 {
            out.problems.push((journal.clone(), format!("{} entries that make no sense", live.bad)));
        }
        for o in live.objects {
            let extents: Vec<(Span, u32)> = o.extents.iter().map(|&(s, l, crc)| (Span::at(s, l), crc)).collect();
            out.items.push(VerifyItem {
                game: dir.clone(),
                path: dir.file(&o.path),
                kind: ItemKind::ObjectFile,
                bytes: extents.iter().map(|(s, _)| s.len()).sum(),
                fs: Arc::clone(&fs),
                extents,
            });
        }
    }
    Ok(out)
}

impl VerifyItem {
    /// Reads the item completely and compares every record or extent with its CRC.
    #[must_use]
    pub fn check(&self) -> ItemResult {
        let mut r = ItemResult {
            game: self.game.clone(),
            path: self.path.clone(),
            kind: self.kind,
            ok: 0,
            ok_bytes: 0,
            problems: Vec::new(),
        };
        let file = match self.fs.open_read(&self.path) {
            Ok(f) => f,
            Err(e) => {
                r.problems.push(format!("can't open: {e}"));
                return r;
            }
        };
        match self.kind {
            ItemKind::Segment => self.check_segment(&*file, &mut r),
            ItemKind::ObjectFile => self.check_extents(&*file, &mut r),
        }
        r
    }

    fn check_segment(&self, file: &dyn FileRead, r: &mut ItemResult) {
        let rep = match scan(file, ScanOptions { verify_bodies: true }) {
            Ok(rep) => rep,
            Err(e) => {
                r.problems.push(e.to_string());
                return;
            }
        };
        for rec in rep.records.iter().filter(|rec| rec.body_ok == Some(true)) {
            r.ok += 1;
            r.ok_bytes += rec.header.body_len;
        }
        // Unwritten gaps are space of writes that never finished: not damage.
        r.problems
            .extend(rep.problems.iter().filter(|p| !matches!(p, Problem::Unwritten { .. })).map(ToString::to_string));
    }

    fn check_extents(&self, file: &dyn FileRead, r: &mut ItemResult) {
        for &(span, crc) in &self.extents {
            match crc32c_at(file, span.start, span.len()) {
                Ok(c) if c == crc => {
                    r.ok += 1;
                    r.ok_bytes += span.len();
                }
                Ok(c) => r
                    .problems
                    .push(format!("bytes {}..{}: checksum {c:08x}, journal says {crc:08x}", span.start, span.end)),
                Err(e) => r.problems.push(format!("bytes {}..{}: {e}", span.start, span.end)),
            }
        }
    }
}
