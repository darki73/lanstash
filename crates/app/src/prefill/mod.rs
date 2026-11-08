//! Prefill: puts a game's build into the stash before a client asks for it.
//! One job at a time, on a thread of its own. The platform says what a job
//! takes ([`Planner`]: the requests, whole files and ranges); the runner
//! here does them ([`run`]).
//!
//! Each request goes through the cache's own request path
//! ([`crate::via_cache`]) as a client would ask, so it is stored exactly as a
//! download would be. [`PrefillOptions::parallel`] workers take the requests
//! in turn (they are independent). What the stash holds already is skipped
//! without reading it; a file the CDN does not have (404) is counted, not an
//! error; a request that fails ends the job once the requests in flight are
//! done. Stopping aborts the requests in flight; what arrived of them stays
//! stored.
//!
//! From lancache ([`PrefillFrom::Lancache`]): each request is first read out
//! of lancache's old cache ([`crate::lancache_read`]) slice by slice, each
//! slice checked by the platform ([`Work::check`]) and written where a
//! download would have put it ([`crate::stash_put`]), counting only once the
//! check passed at the end; a request lancache does not hold, or whose check
//! fails, goes to the CDN as usual. When lancache holds only some of a range
//! (a slice missing), each file inside it whose slices are all there
//! ([`Work::parts`]) is taken and checked on its own, and the range then goes
//! to the cache, which fetches only what is still missing. With
//! [`PrefillFrom::LancacheOnly`] nothing goes to the cache: what lancache lacks
//! (or fails its check) is counted as missing. Either way, a whole file
//! lancache holds all but the end of gets that end from the CDN ([`tail`]).
//!
//! Checked from the CDN ([`Work::check_cdn`], with [`Prefiller::with_checked_cdn`]):
//! for a platform whose files can be checked without trusting where they come
//! from (Battle.net: every name is a hash of its content), a request goes to
//! the CDN directly instead of through the cache, is checked as it streams into
//! the stash, and is committed only once it passes ([`checked`]). One that fails
//! is asked once more; failing again, it ends the job: nothing that failed its
//! check is stored. A request its check cannot judge goes through the cache.

mod asked;
mod checked;
mod line;
mod run;
pub mod tail;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::{Clock, NoPause, Pause};
use lanstash_ports::prefill::{Planning, Prefill, PrefillFrom, PrefillState, PrefillStatus, StartError};
use lanstash_ports::upstream::Upstream;
use lanstash_store::store::Store;

use asked::Asked;

use crate::lancache_read::LancacheRead;
use crate::serve::RequestHandler;
use crate::stash_put::StashPut;

/// One request of a job: a whole file, or bytes `start..end` of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub target: String,
    pub range: Option<(u64, u64)>,
}

/// Checks bytes as they arrive, in order, so a file of any size is checked without holding it.
pub trait PieceCheck {
    /// The next bytes.
    fn feed(&mut self, data: &[u8]);
    /// Whether all of them are what the job asks for.
    fn finish(&self) -> bool;
    /// Whether it can judge these bytes at all (else a CDN download of them goes unchecked).
    fn checks(&self) -> bool {
        true
    }
}

/// The check of bytes `.1` to `.2` of job number `.0`'s file (the whole job, or
/// one of its parts, [`Work::parts`]), given `.3`: the response headers lancache
/// stored with it, as lanstash keeps them ([`lanstash_domain::segment::meta`]).
pub type Check = Box<dyn Fn(usize, u64, u64, &[u8]) -> Box<dyn PieceCheck> + Send + Sync>;

/// The files inside job number `.0` (their bytes in its file), each checkable
/// on its own; none for a job that is one file.
pub type Parts = Box<dyn Fn(usize) -> Vec<(u64, u64)> + Send + Sync>;

/// What one prefill job takes.
pub struct Work {
    /// The CDN hosts; workers take them in turn, as a launcher spreads over them.
    pub hosts: Vec<String>,
    pub jobs: Vec<Job>,
    /// The jobs' bytes.
    pub bytes: u64,
    pub check: Check,
    /// When lancache holds only some of a job: the parts it can still give.
    pub parts: Parts,
    /// What the CDN sends is checked too (by [`Work::check`], with no stored headers) before it is
    /// stored; else it goes through the cache, which checks what its profile can (Steam: `x-content-crc`).
    pub check_cdn: bool,
}

impl std::fmt::Debug for Work {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Work").field("hosts", &self.hosts).field("jobs", &self.jobs.len()).finish_non_exhaustive()
    }
}

/// A platform's side of prefill.
pub trait Planner: Send + Sync {
    /// Whether `game` can be prefilled at all, asked before a job starts.
    ///
    /// # Errors
    /// [`StartError::NotAGame`], [`StartError::NotSetUp`].
    fn admit(&self, game: &str) -> Result<(), StartError>;

    /// What prefilling `game` takes; `version` is told the build's name as
    /// soon as it is known.
    ///
    /// # Errors
    /// Why there is nothing to do; the job fails with it.
    fn work(&self, game: &str, prerelease: bool, version: &dyn Fn(&str)) -> Result<Work, String>;

    /// As [`Planner::work`], for `install`: another install than the platform is set up for
    /// (only where [`Planner::installs`] says it prefills others).
    ///
    /// # Errors
    /// Why there is nothing to do; the job fails with it.
    fn work_for(
        &self,
        game: &str,
        prerelease: bool,
        install: Option<&str>,
        version: &dyn Fn(&str),
    ) -> Result<Work, String> {
        match install {
            None => self.work(game, prerelease, version),
            Some(i) => Err(format!("this platform prefills only the install it is set up for, not {i}")),
        }
    }

    /// Whether it prefills other installs than the one it is set up for ([`Planner::work_for`]).
    fn installs(&self) -> bool {
        false
    }

    /// While [`Planner::work`] is still working out the job: how far it has come.
    fn planning(&self) -> Option<Planning> {
        None
    }
}

/// How prefill runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefillOptions {
    /// Requests in flight at once (at least 1).
    pub parallel: usize,
    /// Tries of one request before the job fails (at least 1): a CDN answers an error now and
    /// then, and one such error should not end a job.
    pub attempts: usize,
    /// The pause after the first failed try, longer by as much after each further one.
    pub retry_pause_ms: u64,
}

impl Default for PrefillOptions {
    fn default() -> Self {
        Self { parallel: 8, attempts: 3, retry_pause_ms: 1_000 }
    }
}

/// lancache's old cache: where its pieces are read, and how they are stored.
#[derive(Debug)]
struct Lancache {
    read: LancacheRead,
    put: StashPut,
}

struct Shared {
    opts: PrefillOptions,
    planner: Arc<dyn Planner>,
    /// The platform's service identifier (the store's key prefix).
    service: String,
    lancache: Option<Lancache>,
    /// Fetches the missing ends of files lancache holds the start of ([`tail`]).
    tails: Option<Arc<dyn Upstream>>,
    /// Fetches and stores checked CDN downloads ([`checked`]).
    checked: Option<(Arc<dyn Upstream>, StashPut)>,
    cache: Arc<dyn RequestHandler>,
    store: Arc<Store>,
    clock: Arc<dyn Clock>,
    /// Waits between the tries of a request.
    pause: Arc<dyn Pause>,
    status: Mutex<Option<PrefillStatus>>,
    stop: AtomicBool,
    bytes: AtomicU64,
    /// Of `bytes`, those read out of lancache (counted as they stream, taken back if their check fails).
    local: AtomicU64,
    /// Jobs waiting for the running one.
    queue: Mutex<std::collections::VecDeque<Asked>>,
    /// A thread runs the jobs, from a start until the queue is empty (read and set under the
    /// status lock): between two queued jobs, the state is not `Running` but the line is taken.
    worker: AtomicBool,
    /// The jobs that ended, until [`Prefill::take_ended`] (the last few).
    ended: Mutex<std::collections::VecDeque<PrefillStatus>>,
}

/// One platform's prefill jobs.
pub struct Prefiller {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Prefiller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prefiller").field("service", &self.shared.service).finish_non_exhaustive()
    }
}

impl Prefiller {
    /// `service`: the platform's identifier; `cache`: the request path clients use.
    #[must_use]
    pub fn new(
        planner: Arc<dyn Planner>,
        service: &str,
        cache: Arc<dyn RequestHandler>,
        store: Arc<Store>,
        clock: Arc<dyn Clock>,
        opts: PrefillOptions,
    ) -> Self {
        let shared = Shared {
            opts,
            planner,
            service: service.to_owned(),
            lancache: None,
            tails: None,
            checked: None,
            cache,
            store,
            clock,
            pause: Arc::new(NoPause),
            status: Mutex::new(None),
            stop: AtomicBool::new(false),
            bytes: AtomicU64::new(0),
            local: AtomicU64::new(0),
            queue: Mutex::new(std::collections::VecDeque::new()),
            worker: AtomicBool::new(false),
            ended: Mutex::new(std::collections::VecDeque::new()),
        };
        Self { shared: Arc::new(shared) }
    }

    /// Waits with `pause` between the tries of a failed request (none by default: tries follow
    /// at once).
    ///
    /// # Panics
    /// When a prefill was started already (call it while composing).
    #[must_use]
    pub fn with_pause(mut self, pause: Arc<dyn Pause>) -> Self {
        Arc::get_mut(&mut self.shared).expect("with_pause before any start").pause = pause;
        self
    }

    /// Enables [`PrefillFrom::Lancache`]: pieces read by `read`, stored by `put`.
    ///
    /// # Panics
    /// When a prefill was started already (call it while composing).
    #[must_use]
    pub fn with_lancache(mut self, read: LancacheRead, put: StashPut) -> Self {
        Arc::get_mut(&mut self.shared).expect("with_lancache before any start").lancache = Some(Lancache { read, put });
        self
    }

    /// Checks what the CDN sends for a platform whose [`Work::check_cdn`] says so: fetched from
    /// `upstream`, stored by `put` once it passes ([`checked`]).
    ///
    /// # Panics
    /// When a prefill was started already (call it while composing).
    #[must_use]
    pub fn with_checked_cdn(mut self, upstream: Arc<dyn Upstream>, put: StashPut) -> Self {
        Arc::get_mut(&mut self.shared).expect("with_checked_cdn before any start").checked = Some((upstream, put));
        self
    }

    /// Lets a lancache prefill complete a whole file lancache holds all but
    /// the end of, fetching that end from `upstream` ([`tail`]).
    ///
    /// # Panics
    /// When a prefill was started already (call it while composing).
    #[must_use]
    pub fn with_tails(mut self, upstream: Arc<dyn Upstream>) -> Self {
        Arc::get_mut(&mut self.shared).expect("with_tails before any start").tails = Some(upstream);
        self
    }
}

impl Prefill for Prefiller {
    fn start(&self, game: &str, prerelease: bool, from: PrefillFrom) -> Result<(), StartError> {
        self.begin(Asked::new(game, prerelease, from, None))
    }

    fn lancache(&self) -> bool {
        self.shared.lancache.is_some()
    }

    fn queue(&self, game: &str, prerelease: bool, from: PrefillFrom) -> Result<usize, StartError> {
        self.enqueue(Asked::new(game, prerelease, from, None))
    }

    fn queue_for(
        &self,
        game: &str,
        prerelease: bool,
        from: PrefillFrom,
        install: Option<&str>,
    ) -> Result<usize, StartError> {
        self.enqueue(Asked::new(game, prerelease, from, install))
    }

    fn unqueue(&self, game: &str) -> bool {
        let mut q = self.shared.queue.lock().unwrap_or_else(PoisonError::into_inner);
        let before = q.len();
        q.retain(|a| a.game != game);
        q.len() != before
    }

    /// The running job stops after the reads in flight; the queued ones are forgotten.
    fn stop(&self) {
        self.shared.queue.lock().unwrap_or_else(PoisonError::into_inner).clear();
        self.shared.stop.store(true, Ordering::SeqCst);
    }

    fn status(&self) -> Option<PrefillStatus> {
        let mut s = self.shared.lock().clone()?;
        if s.state == PrefillState::Running {
            s.bytes = self.shared.bytes.load(Ordering::Relaxed);
            s.bytes_local = self.shared.local.load(Ordering::Relaxed);
            if s.files_total == 0 {
                s.planning = self.shared.planner.planning();
            }
        }
        s.queued =
            self.shared.queue.lock().unwrap_or_else(PoisonError::into_inner).iter().map(|a| a.game.clone()).collect();
        Some(s)
    }

    fn take_ended(&self) -> Vec<PrefillStatus> {
        self.shared.ended.lock().unwrap_or_else(PoisonError::into_inner).drain(..).collect()
    }
}
