//! Use case: the store checked in the background, every stored byte against
//! its checksum (`store verify`, while serving), once per period and paced so
//! it never takes the disks from clients: after each segment or object file it
//! waits until the bytes read stay under the rate. Reading is safe next to the
//! running server (`lanstash_store::verify`); a file deleted while the round
//! runs is not damage, so whatever looked wrong is checked once more against a
//! fresh listing before it is reported. Items go in path order and the round's
//! place is kept in [`Keep`] every few minutes, so a restart resumes it rather
//! than starting over; the last finished round and its problems are kept too.

use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::{Clock, Pause};
use lanstash_ports::keep::Keep;
use lanstash_store::verify::VerifyPlan;

/// The name the patrol's place and last result are kept under.
pub const KEPT: &str = "verify";

/// How often a round runs, and how fast it may read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatrolOptions {
    /// A round starts once this long has passed since the last one finished.
    pub every_ms: u64,
    /// Bytes per second at most (0: as fast as the disks go).
    pub rate: u64,
}

/// Lists what there is to check (`lanstash_store::verify::plan` over the store's roots).
pub type Planner = Box<dyn Fn() -> Result<VerifyPlan, String> + Send + Sync>;

/// Where the patrol stands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatrolState {
    pub running: bool,
    /// When the current (or last) round started, and when the last one finished (0: never).
    pub started_ms: u64,
    pub finished_ms: u64,
    pub items_done: u64,
    pub items: u64,
    pub bytes_done: u64,
    pub bytes: u64,
    /// The last item checked of the current round (its place).
    pub cursor: String,
    /// (path, message): what the last finished round found, or the current round so far.
    pub problems: Vec<(String, String)>,
}

impl PatrolState {
    fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for (k, v) in text.lines().filter_map(|l| l.split_once('=')) {
            match k {
                "started_ms" => s.started_ms = v.parse().unwrap_or(0),
                "finished_ms" => s.finished_ms = v.parse().unwrap_or(0),
                "cursor" => s.cursor = v.to_owned(),
                "problem" => {
                    let (p, m) = v.split_once('\t').unwrap_or((v, ""));
                    s.problems.push((p.to_owned(), m.to_owned()));
                }
                _ => {}
            }
        }
        s
    }

    fn text(&self) -> String {
        let mut t =
            format!("started_ms={}\nfinished_ms={}\ncursor={}\n", self.started_ms, self.finished_ms, self.cursor);
        for (p, m) in &self.problems {
            t.push_str(&format!("problem={p}\t{}\n", m.replace(['\n', '\t'], " ")));
        }
        t
    }
}

/// How often the round's place is kept.
const SAVE_EVERY_MS: u64 = 5 * 60_000;

/// The background check of the store.
pub struct Patrol {
    plan: Planner,
    clock: Arc<dyn Clock>,
    pause: Arc<dyn Pause>,
    keep: Arc<dyn Keep>,
    options: PatrolOptions,
    state: Mutex<PatrolState>,
}

impl std::fmt::Debug for Patrol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Patrol").field("options", &self.options).finish_non_exhaustive()
    }
}

impl Patrol {
    /// A patrol listing with `plan`, resuming what `keep` holds of an earlier one.
    #[must_use]
    pub fn new(
        plan: Planner,
        clock: Arc<dyn Clock>,
        pause: Arc<dyn Pause>,
        keep: Arc<dyn Keep>,
        options: PatrolOptions,
    ) -> Self {
        let kept = keep.load(KEPT).ok().flatten().map(|b| PatrolState::parse(&String::from_utf8_lossy(&b)));
        Self { plan, clock, pause, keep, options, state: Mutex::new(kept.unwrap_or_default()) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, PatrolState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn save(&self) {
        let text = self.lock().text();
        let _ = self.keep.save(KEPT, text.as_bytes());
    }

    /// Where it stands.
    #[must_use]
    pub fn state(&self) -> PatrolState {
        self.lock().clone()
    }

    /// Whether a round is due: one was cut short, or the period has passed since the last one.
    #[must_use]
    pub fn due(&self) -> bool {
        let s = self.lock();
        !s.cursor.is_empty() || self.clock.now_ms() >= s.finished_ms.saturating_add(self.options.every_ms)
    }

    /// For the status: `running`, `started_ms`, `finished_ms`, items and bytes done of all, `problems`.
    #[must_use]
    pub fn values(&self) -> Vec<(&'static str, u64)> {
        let s = self.lock();
        vec![
            ("running", u64::from(s.running)),
            ("started_ms", s.started_ms),
            ("finished_ms", s.finished_ms),
            ("items_done", s.items_done),
            ("items", s.items),
            ("bytes_done", s.bytes_done),
            ("bytes", s.bytes),
            ("problems", s.problems.len() as u64),
            ("every_ms", self.options.every_ms),
        ]
    }

    /// Runs one round to its end (from its kept place, if it was cut short); its problems.
    pub fn round(&self) -> Vec<(String, String)> {
        let plan = match (self.plan)() {
            Ok(p) => p,
            // Nothing checked: the round keeps its place and is still due (tried at the next look).
            Err(e) => return vec![("store".to_owned(), e)],
        };
        let mut items = plan.items;
        items.sort_by(|a, b| a.path.cmp(&b.path));
        let now = self.clock.now_ms();
        let mut found = {
            let mut s = self.lock();
            let resumed = !s.cursor.is_empty();
            if !resumed {
                s.started_ms = now;
                s.problems.clear();
            }
            let skipped = items.iter().take_while(|i| resumed && i.path <= s.cursor).count();
            items.drain(..skipped);
            s.running = true;
            s.items = items.len() as u64;
            s.bytes = items.iter().map(|i| i.bytes).sum();
            (s.items_done, s.bytes_done) = (0, 0);
            std::mem::take(&mut s.problems)
        };
        found.extend(plan.problems);
        let (start, mut saved) = (now, now);
        for item in &items {
            let r = item.check();
            found.extend(r.problems.into_iter().map(|p| (r.path.clone(), p)));
            let now = {
                let mut s = self.lock();
                s.items_done += 1;
                s.bytes_done += item.bytes;
                s.cursor.clone_from(&item.path);
                s.problems.clone_from(&found);
                self.clock.now_ms()
            };
            if now.saturating_sub(saved) >= SAVE_EVERY_MS {
                self.save();
                saved = now;
            }
            // Paced: the bytes read so far take at least this long at the rate.
            let done = self.lock().bytes_done;
            if let Some(due_ms) = done.saturating_mul(1000).checked_div(self.options.rate) {
                let spent = now.saturating_sub(start);
                if due_ms > spent {
                    self.pause.pause_ms(due_ms - spent);
                }
            }
        }
        let confirmed = if found.is_empty() { found } else { self.confirm(&found) };
        self.finish(confirmed)
    }

    /// Of `found`, what is still wrong against a fresh listing (a file gone since is no damage).
    fn confirm(&self, found: &[(String, String)]) -> Vec<(String, String)> {
        let Ok(again) = (self.plan)() else { return found.to_vec() };
        let mut out: Vec<(String, String)> =
            again.problems.into_iter().filter(|(p, _)| found.iter().any(|(f, _)| f == p)).collect();
        let mut paths: Vec<&str> = found.iter().map(|(p, _)| p.as_str()).collect();
        paths.dedup();
        for item in again.items.iter().filter(|i| paths.contains(&i.path.as_str())) {
            let r = item.check();
            out.extend(r.problems.into_iter().map(|p| (r.path.clone(), p)));
        }
        out.sort();
        out.dedup();
        out
    }

    /// On a thread of its own: every `check_ms` (the first time after it), a round when one is
    /// due; what a round finds is printed (the service's journal).
    ///
    /// # Errors
    /// The thread could not be started.
    pub fn keep_going(self: &Arc<Self>, check_ms: u64) -> Result<(), String> {
        let me = Arc::clone(self);
        crate::rounds::forever("verify", Arc::clone(&self.pause), move || {
            me.pause.pause_ms(check_ms);
            if me.due() {
                let found = me.round();
                let s = me.state();
                println!("store check: {} files, {} bytes, {} problems", s.items_done, s.bytes_done, found.len());
                for (path, why) in &found {
                    println!("  {path}: {why}");
                }
            }
            crate::rounds::Next::Again
        })
    }

    fn finish(&self, problems: Vec<(String, String)>) -> Vec<(String, String)> {
        {
            let mut s = self.lock();
            s.running = false;
            s.finished_ms = self.clock.now_ms();
            s.cursor.clear();
            s.problems.clone_from(&problems);
        }
        self.save();
        problems
    }
}
