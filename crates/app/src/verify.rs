//! Use case: check every stored byte against its checksum (`store verify`).
//!
//! The items of a [`VerifyPlan`] (segments and object files) are read by a few
//! threads in parallel, as fast as the disks go or held to a rate ([`Paced`]);
//! the result is a summary per game and every problem.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use lanstash_ports::clock::{Clock, Pause};
use lanstash_store::layout::GameDir;
use lanstash_store::verify::{ItemKind, ItemResult, VerifyPlan};

/// Checked data of one game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameVerify {
    pub segments: u64,
    pub records: u64,
    pub record_bytes: u64,
    pub files: u64,
    pub extents: u64,
    pub extent_bytes: u64,
    pub problems: u64,
}

/// Progress after each checked item.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VerifyProgress {
    pub items_done: u64,
    pub items: u64,
    pub bytes_done: u64,
    pub bytes: u64,
}

/// The outcome of a whole check.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerifyReport {
    pub games: BTreeMap<GameDir, GameVerify>,
    /// (path, message), sorted by path.
    pub problems: Vec<(String, String)>,
    pub ignored: Vec<String>,
}

impl VerifyReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.problems.is_empty()
    }

    fn add(&mut self, r: ItemResult) {
        let g = self.games.entry(r.game).or_default();
        match r.kind {
            ItemKind::Segment => {
                g.segments += 1;
                g.records += r.ok;
                g.record_bytes += r.ok_bytes;
            }
            ItemKind::ObjectFile => {
                g.files += 1;
                g.extents += r.ok;
                g.extent_bytes += r.ok_bytes;
            }
        }
        g.problems += r.problems.len() as u64;
        self.problems.extend(r.problems.into_iter().map(|p| (r.path.clone(), p)));
    }
}

/// A check held to a rate: after each item, it waits until the bytes read so far have taken at
/// least as long as `bytes_per_s` allows (so serving keeps the disks).
pub struct Paced<'a> {
    pub bytes_per_s: u64,
    pub clock: &'a dyn Clock,
    pub pause: &'a dyn Pause,
}

impl std::fmt::Debug for Paced<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Paced").field("bytes_per_s", &self.bytes_per_s).finish_non_exhaustive()
    }
}

impl Paced<'_> {
    /// Waits until `done` bytes, read since `start_ms`, are within the rate.
    fn hold(&self, start_ms: u64, done: u64) {
        let Some(due_ms) = done.saturating_mul(1000).checked_div(self.bytes_per_s) else { return };
        let spent = self.clock.now_ms().saturating_sub(start_ms);
        if due_ms > spent {
            self.pause.pause_ms(due_ms - spent);
        }
    }
}

/// Checks every item of `plan` with `threads` threads, as fast as the disks go; `progress` is
/// called after each item (from any of the threads).
#[must_use]
pub fn verify(plan: VerifyPlan, threads: usize, progress: &(dyn Fn(VerifyProgress) + Sync)) -> VerifyReport {
    verify_paced(plan, threads, None, progress)
}

/// [`verify`], held to a rate when `paced` is given.
#[must_use]
pub fn verify_paced(
    plan: VerifyPlan,
    threads: usize,
    paced: Option<&Paced<'_>>,
    progress: &(dyn Fn(VerifyProgress) + Sync),
) -> VerifyReport {
    let start_ms = paced.map_or(0, |p| p.clock.now_ms());
    let mut report = VerifyReport { ignored: plan.ignored, problems: plan.problems, ..VerifyReport::default() };
    // Largest first: the big items don't end up alone at the end.
    let mut items = plan.items;
    items.sort_by_key(|i| std::cmp::Reverse(i.bytes));
    let state = Mutex::new((
        VerifyProgress {
            items: items.len() as u64,
            bytes: items.iter().map(|i| i.bytes).sum(),
            ..VerifyProgress::default()
        },
        Vec::with_capacity(items.len()),
    ));
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads.clamp(1, 64) {
            s.spawn(|| {
                while let Some(item) = items.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let r = item.check();
                    let p = {
                        let mut st = state.lock().unwrap_or_else(PoisonError::into_inner);
                        st.0.items_done += 1;
                        st.0.bytes_done += item.bytes;
                        st.1.push(r);
                        st.0
                    };
                    progress(p);
                    if let Some(paced) = paced {
                        paced.hold(start_ms, p.bytes_done);
                    }
                }
            });
        }
    });
    let (_, results) = state.into_inner().unwrap_or_else(PoisonError::into_inner);
    for r in results {
        report.add(r);
    }
    report.problems.sort();
    report
}
