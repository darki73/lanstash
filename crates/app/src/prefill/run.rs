//! Running one prefill job: its requests on parallel workers, each skipped,
//! taken from lancache or fetched through the cache.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use lanstash_domain::extent::Span;
use lanstash_domain::object::object_key;
use lanstash_ports::prefill::{PrefillFrom, PrefillState};
use lanstash_store::index::Lookup;

use super::checked::Fetched;
use super::{Job, Shared, Work};
use crate::lancache_read::Next;
use crate::via_cache::{self, Get, Watch};

/// The check of bytes `start..end` with the stored headers (job number already chosen).
pub(super) type StreamCheck<'a> = dyn Fn(u64, u64, &[u8]) -> Box<dyn super::PieceCheck> + 'a;

impl Shared {
    pub(super) fn run(&self, game: &str, prerelease: bool, from: PrefillFrom, install: Option<&str>) {
        let outcome = self.prefill(game, prerelease, from, install);
        let stopped = self.stop.load(Ordering::SeqCst);
        match outcome {
            Ok(()) => self.finish(PrefillState::Done, None),
            Err(_) if stopped => self.finish(PrefillState::Stopped, None),
            Err(e) => self.finish(PrefillState::Failed, Some(e)),
        }
    }

    fn prefill(&self, game: &str, prerelease: bool, from: PrefillFrom, install: Option<&str>) -> Result<(), String> {
        let found = |v: &str| self.update(|s| s.version = Some(v.to_owned()));
        let work = self.planner.work_for(game, prerelease, install, &found)?;
        self.update(|s| {
            s.files_total = work.jobs.len() as u64;
            s.bytes_total = work.bytes;
        });
        let next = AtomicUsize::new(0);
        let failed: Mutex<Option<String>> = Mutex::new(None);
        let abort = AtomicBool::new(false);
        let hosts = &work.hosts;
        if hosts.is_empty() {
            return Err(format!("{game}: no CDN host"));
        }
        std::thread::scope(|scope| {
            for worker in 0..self.opts.parallel.clamp(1, 64).min(work.jobs.len().max(1)) {
                let (next, failed, abort, work) = (&next, &failed, &abort, &work);
                scope.spawn(move || {
                    while !abort.load(Ordering::SeqCst) && !self.stop.load(Ordering::SeqCst) {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(job) = work.jobs.get(i) else { return };
                        if let Err(e) = self.step_tries(hosts, worker, work, i, job, from) {
                            abort.store(true, Ordering::SeqCst);
                            failed.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert(e);
                            return;
                        }
                    }
                });
            }
        });
        if let Some(e) = failed.into_inner().unwrap_or_else(PoisonError::into_inner) {
            return Err(e);
        }
        if self.stop.load(Ordering::SeqCst) {
            return Err("stopped".into());
        }
        Ok(())
    }

    /// Bytes `range` of `target` (or the whole file) out of lancache, checked
    /// by `check` as they stream and stored once it passes.
    fn stream(&self, target: &str, range: Option<(u64, u64)>, check: &StreamCheck<'_>) -> Result<Took, String> {
        let Some(l) = &self.lancache else { return Ok(Took::Absent) };
        let mut slices = match l.read.open(target, range) {
            Ok(Some(s)) => s,
            Ok(None) => return Ok(Took::Absent),
            Err(_) => return Ok(Took::Rejected),
        };
        let mut check = check(slices.start, slices.end, &slices.meta);
        let mut pending = l.put.begin(target, slices.start, slices.end, slices.total, &slices.meta)?;
        // Slice by slice: checked, written (not counting yet), shown as progress.
        let mut read = 0u64;
        let outcome = loop {
            if self.stop.load(Ordering::SeqCst) {
                break Err("stopped".to_owned());
            }
            match slices.next_piece() {
                Ok(Next::End) => break Ok(check.finish()),
                Ok(Next::Gone) | Err(_) => break Ok(false),
                Ok(Next::Bytes(at, bytes)) => {
                    check.feed(&bytes);
                    // Bytes lancache gives in a way they cannot be stored: not taken from it.
                    if pending.write(at, &bytes).is_err() {
                        break Ok(false);
                    }
                    read += bytes.len() as u64;
                    self.bytes.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                    self.local.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                }
            }
        };
        let take_back = || {
            self.bytes.fetch_sub(read, Ordering::Relaxed);
            self.local.fetch_sub(read, Ordering::Relaxed);
        };
        match outcome {
            Ok(true) => match pending.commit(true) {
                Ok(()) => Ok(Took::Stored(read)),
                Err(e) => {
                    take_back();
                    Err(e)
                }
            },
            Ok(false) => {
                take_back();
                Ok(Took::Rejected)
            }
            Err(e) => {
                take_back();
                Err(e)
            }
        }
    }

    /// A request out of lancache: all of it, else (a range lancache holds
    /// only some of) the parts inside it lancache holds whole.
    fn take_from_lancache(&self, host: &str, work: &Work, i: usize, job: &Job) -> Result<Took, String> {
        let check = |a: u64, b: u64, meta: &[u8]| (work.check)(i, a, b, meta);
        match self.stream(&job.target, job.range, &check)? {
            Took::Stored(n) => return Ok(Took::Stored(n)),
            Took::Rejected => {
                self.update(|s| s.files_rejected += 1);
                return Ok(Took::Rejected);
            }
            Took::Absent | Took::Partly(_) => {}
        }
        if job.range.is_none() {
            return match self.take_with_tail(host, &job.target, &check)? {
                Took::Rejected => {
                    self.update(|s| s.files_rejected += 1);
                    Ok(Took::Rejected)
                }
                took => Ok(took),
            };
        }
        let mut taken = 0u64;
        for (a, b) in (work.parts)(i) {
            match self.stream(&job.target, Some((a, b)), &check)? {
                Took::Stored(n) => taken += n,
                Took::Rejected => self.update(|s| s.files_rejected += 1),
                Took::Absent | Took::Partly(_) => {}
            }
        }
        Ok(if taken > 0 { Took::Partly(taken) } else { Took::Absent })
    }

    /// One request, tried up to [`super::PrefillOptions::attempts`] times, each on the next CDN
    /// host (`worker`'s first): a CDN's passing error (a 502) costs a pause, not the job. A try
    /// stores only what it got whole and checked, so the next one fetches the rest. A stop is
    /// never tried again.
    fn step_tries(
        &self,
        hosts: &[String],
        worker: usize,
        work: &Work,
        i: usize,
        job: &Job,
        from: PrefillFrom,
    ) -> Result<(), String> {
        let attempts = self.opts.attempts.max(1);
        let mut tried = 0;
        loop {
            let host = &hosts[(worker + tried) % hosts.len()];
            tried += 1;
            match self.step(host, work, i, job, from) {
                Ok(()) => return Ok(()),
                Err(Failure::Final(e)) => return Err(e),
                Err(Failure::Again(e)) if self.stop.load(Ordering::SeqCst) => return Err(e),
                Err(Failure::Again(e)) if tried >= attempts => {
                    return Err(if attempts > 1 { format!("{e} (tried {attempts} times)") } else { e });
                }
                Err(Failure::Again(_)) => self.pause.pause_ms(self.opts.retry_pause_ms.saturating_mul(tried as u64)),
            }
        }
    }

    /// One request: skipped when the stash holds it (without counting as a
    /// use), else taken from lancache (when asked and it can) or sent
    /// through the cache; for a range lancache gave part of, the cache
    /// fetches only the rest.
    fn step(&self, host: &str, work: &Work, i: usize, job: &Job, from: PrefillFrom) -> Result<(), Failure> {
        // The bytes the stash holds of it, when it holds all of it.
        let held = match self.store.index().lookup(&object_key(&self.service, &job.target)) {
            Lookup::Found(o) => match job.range {
                Some((a, b)) => o.covers(Span::new(a, b)).then_some(b - a),
                None => (o.total > 0 && o.covers(Span::new(0, o.total))).then_some(o.total),
            },
            _ => None,
        };
        if let Some(bytes) = held {
            self.update(|s| {
                s.files_done += 1;
                s.files_skipped += 1;
                s.bytes_skipped += bytes;
            });
            return Ok(());
        }
        let target = job.target.as_str();
        self.update(|s| s.current = Some(target.to_owned()));
        let partly = match from {
            PrefillFrom::Lancache | PrefillFrom::LancacheOnly => match self.take_from_lancache(host, work, i, job)? {
                Took::Stored(_) => {
                    self.update(|s| {
                        s.files_done += 1;
                        s.files_local += 1;
                    });
                    return Ok(());
                }
                Took::Partly(n) if from == PrefillFrom::Lancache => Some(n),
                Took::Absent | Took::Rejected if from == PrefillFrom::Lancache => None,
                // Only lancache: what it lacks is left out.
                Took::Partly(_) | Took::Absent | Took::Rejected => {
                    self.update(|s| {
                        s.files_done += 1;
                        s.files_missing += 1;
                    });
                    return Ok(());
                }
            },
            PrefillFrom::Cdn => None,
        };
        // A platform whose files can be checked: from the CDN straight into the stash, once checked.
        if work.check_cdn && partly.is_none() {
            match self.fetch_checked(host, work, i, job)? {
                Fetched::Stored => {
                    self.update(|s| s.files_done += 1);
                    return Ok(());
                }
                Fetched::Missing => {
                    self.update(|s| {
                        s.files_done += 1;
                        s.files_missing += 1;
                    });
                    return Ok(());
                }
                Fetched::Rejected => {
                    self.update(|s| s.files_rejected += 1);
                    // Asked twice already: bad data, not a passing error.
                    return Err(Failure::Final(format!(
                        "{target}: what the CDN sent failed its check twice; nothing of it was stored"
                    )));
                }
                Fetched::Failed(e) => return Err(Failure::Again(format!("{target}: {e}"))),
                Fetched::Unchecked => {}
            }
        }
        // What lancache gave of the range is read back from the stash with the
        // rest; only the rest counts as fetched.
        let mine = AtomicU64::new(0);
        let watch = match partly {
            None => Watch { bytes: Some(&self.bytes), mine: Some(&mine), stop: Some(&self.stop) },
            Some(_) => Watch { bytes: None, mine: None, stop: Some(&self.stop) },
        };
        let got = via_cache::get(self.cache.as_ref(), Get { host, target, range: job.range, keep: None }, watch)
            .inspect_err(|_| {
                // A failed try's bytes come again with the next one: counted once.
                self.bytes.fetch_sub(mine.load(Ordering::Relaxed), Ordering::Relaxed);
            })?;
        if let (Some(n), Some((a, b))) = (partly, job.range) {
            self.bytes.fetch_add((b - a).saturating_sub(n), Ordering::Relaxed);
        }
        self.update(|s| {
            s.files_done += 1;
            if got.is_none() {
                s.files_missing += 1;
            }
        });
        Ok(())
    }
}

/// Why a request failed.
pub(super) enum Failure {
    /// A passing error (a CDN's 5xx, a connection lost, a storage hiccup): worth another try.
    Again(String),
    /// Trying again would give the same (what the CDN sent failed its check twice).
    Final(String),
}

impl From<String> for Failure {
    fn from(e: String) -> Self {
        Self::Again(e)
    }
}

/// What lancache gave of a request.
pub(super) enum Took {
    /// All of it (bytes).
    Stored(u64),
    /// Some parts of a range (bytes); the cache fetches the rest.
    Partly(u64),
    /// Nothing: lancache does not hold it.
    Absent,
    /// Its check failed or it could not be read.
    Rejected,
}
