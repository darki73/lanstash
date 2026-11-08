//! Text reports for the operator (pure: results in, text out).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use lanstash_app::import_nginx::ImportReport;
use lanstash_app::verify::VerifyReport;
use lanstash_ports::names::GameNames;
use lanstash_store::index::Usage;
use lanstash_store::layout::GameDir;
use lanstash_store::segment::scan::SegmentReport;

/// Outcome of a command, mapped to an exit code by `main`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    ProblemsFound,
}

/// Summary of an import run that took `secs`.
#[must_use]
pub fn import(r: &ImportReport, secs: f64) -> String {
    let mut s = String::new();
    let gb = r.imported_bytes as f64 / 1e9;
    let _ = writeln!(s, "import finished in {secs:.1}s");
    let _ = writeln!(s, "  files listed              {}", r.files_listed);
    let _ = writeln!(
        s,
        "  imported                  {gb:.2} GB ({:.0} MB/s)",
        r.imported_bytes as f64 / 1e6 / secs.max(0.001)
    );
    let rows = [
        ("  as segment records        ", r.records),
        ("  as object-file extents    ", r.extents),
        ("  verified (body check)     ", r.verified),
        ("  already stored            ", r.already_stored),
        ("  later slices joined       ", r.joined_slices),
        ("  incomplete small objects  ", r.incomplete),
        ("  skipped: other service    ", r.skipped_service),
        ("  skipped: never cached     ", r.skipped_uncacheable),
        ("  failed the body check     ", r.check_failed),
        ("  bad files                 ", r.bad_files),
        ("  segments closed           ", r.segments_closed),
    ];
    for (label, n) in rows {
        let _ = writeln!(s, "{label}{n}");
    }
    for e in &r.sample_errors {
        let _ = writeln!(s, "    e.g. {e}");
    }
    s
}

/// Objects and bytes per service and game, with service totals.
#[must_use]
pub fn usage(u: &BTreeMap<GameDir, Usage>, names: &dyn GameNames) -> String {
    let mut s = String::new();
    let mut per_service: BTreeMap<&str, Usage> = BTreeMap::new();
    for (g, x) in u {
        let t = per_service.entry(g.service.as_str()).or_default();
        t.objects += x.objects;
        t.bytes += x.bytes;
    }
    for (service, t) in &per_service {
        let _ = writeln!(s, "{service:<12} {:>10} objects {:>10.2} GB", t.objects, t.bytes as f64 / 1e9);
        for (g, x) in u.iter().filter(|(g, _)| g.service == *service) {
            let label = names.name(&g.service, &g.game).map_or_else(|| g.game.clone(), |n| format!("{n} ({})", g.game));
            let _ = writeln!(s, "  {label:<40} {:>10} objects {:>10.2} GB", x.objects, x.bytes as f64 / 1e9);
        }
    }
    if per_service.is_empty() {
        let _ = writeln!(s, "store is empty");
    }
    s
}

/// A segment's header, records (at most `limit`, 0 = all) and problems.
#[must_use]
pub fn segment(name: &str, rep: &SegmentReport, limit: usize) -> String {
    let h = &rep.header;
    let mut s = String::new();
    let _ = writeln!(s, "segment {}", name);
    let _ = writeln!(
        s,
        "  id {}  service {} ({})  game {}  alignment {}  max {}  created {} ms",
        h.segment_id,
        h.service.name(),
        h.service.0,
        h.game_id,
        h.alignment,
        h.max_bytes,
        h.created_at_ms
    );
    match &rep.sealed {
        Some(x) => {
            let _ = writeln!(
                s,
                "  {} bytes, sealed: {} footer entries at offset {}",
                rep.file_len, x.trailer.entry_count, x.trailer.footer_offset
            );
        }
        None => {
            let _ = writeln!(s, "  {} bytes, NOT sealed (would resume at offset {})", rep.file_len, rep.resume_offset);
        }
    }
    let body: u64 = rep.records.iter().map(|r| r.header.body_len).sum();
    let overhead = if rep.file_len > 0 {
        (rep.file_len - body.min(rep.file_len)) as f64 * 100.0 / rep.file_len as f64
    } else {
        0.0
    };
    let _ = writeln!(s, "  {} records, {} body bytes, overhead {:.3}%", rep.records.len(), body, overhead);
    let shown = if limit == 0 { rep.records.len() } else { limit.min(rep.records.len()) };
    if shown > 0 {
        let _ = writeln!(s, "  {:>12} {:>10} {:>10}  {:<6} key", "offset", "span", "body", "body");
    }
    for r in rep.records.iter().take(shown) {
        let check = match r.body_ok {
            Some(true) => "ok",
            Some(false) => "BAD",
            None => "-",
        };
        let _ =
            writeln!(s, "  {:>12} {:>10} {:>10}  {:<6} {}", r.offset, r.span, r.header.body_len, check, r.header.key);
    }
    if shown < rep.records.len() {
        let _ = writeln!(s, "  … {} more (use --limit 0 for all)", rep.records.len() - shown);
    }
    if rep.problems.is_empty() {
        let _ = writeln!(s, "  no problems");
    } else {
        let _ = writeln!(s, "  {} problem(s):", rep.problems.len());
        for p in &rep.problems {
            let _ = writeln!(s, "    {p}");
        }
    }
    s
}

/// Problems listed in full by [`verify`]; the rest are counted.
pub const VERIFY_PROBLEMS_SHOWN: usize = 50;

/// Checked data per game and every problem (up to [`VERIFY_PROBLEMS_SHOWN`]).
#[must_use]
pub fn verify(r: &VerifyReport, names: &dyn GameNames, secs: f64) -> String {
    let mut s = String::new();
    let bytes: u64 = r.games.values().map(|g| g.record_bytes + g.extent_bytes).sum();
    let _ = writeln!(
        s,
        "verified {:.2} GB in {secs:.1}s ({:.0} MB/s)",
        bytes as f64 / 1e9,
        bytes as f64 / 1e6 / secs.max(0.001)
    );
    let _ = writeln!(s, "  {:<46} {:>19} {:>19} {:>9}", "game", "records ok", "extents ok", "problems");
    for (g, v) in &r.games {
        let label = names.name(&g.service, &g.game).map_or_else(|| g.path(), |n| format!("{n} ({})", g.path()));
        let _ = writeln!(
            s,
            "  {label:<46} {:>8} {:>7.2} GB {:>8} {:>7.2} GB {:>9}",
            v.records,
            v.record_bytes as f64 / 1e9,
            v.extents,
            v.extent_bytes as f64 / 1e9,
            v.problems
        );
    }
    for x in &r.ignored {
        let _ = writeln!(s, "  {x}: belongs to another root, not checked");
    }
    if r.problems.is_empty() {
        let _ = writeln!(s, "no problems: every stored byte matches its checksum");
    } else {
        let _ = writeln!(s, "{} problem(s):", r.problems.len());
        for (path, p) in r.problems.iter().take(VERIFY_PROBLEMS_SHOWN) {
            let _ = writeln!(s, "  {path}: {p}");
        }
        if r.problems.len() > VERIFY_PROBLEMS_SHOWN {
            let _ = writeln!(s, "  … {} more", r.problems.len() - VERIFY_PROBLEMS_SHOWN);
        }
    }
    s
}
