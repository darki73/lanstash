//! Putting a game into the stash before a client asks for it.

use alloc::string::String;
use alloc::vec::Vec;

/// Where a prefill takes the bytes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrefillFrom {
    /// The CDN, through the cache.
    #[default]
    Cdn,
    /// lancache's nginx cache first (each piece checked), the CDN for the rest.
    Lancache,
    /// Only what lancache's nginx cache holds (each piece checked); the rest is
    /// left out (counted as missing), nothing comes from the CDN.
    LancacheOnly,
}

impl PrefillFrom {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cdn => "cdn",
            Self::Lancache => "lancache",
            Self::LancacheOnly => "lancache-only",
        }
    }

    /// Whether pieces are read out of lancache's nginx cache.
    /// The source `as_str` names.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Cdn, Self::Lancache, Self::LancacheOnly].into_iter().find(|f| f.as_str() == s)
    }

    #[must_use]
    pub fn reads_lancache(self) -> bool {
        matches!(self, Self::Lancache | Self::LancacheOnly)
    }
}

/// Where a prefill stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefillState {
    Running,
    Done,
    Stopped,
    Failed,
}

impl PrefillState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Done => "done",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }

    /// The state `as_str` names.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Running, Self::Done, Self::Stopped, Self::Failed].into_iter().find(|t| t.as_str() == s)
    }
}

/// The last prefill job, running or finished.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefillStatus {
    pub game: String,
    pub version: Option<String>,
    /// The pre-release build rather than the current one.
    pub prerelease: bool,
    /// The install it is for, when not the one the platform is set up for (Battle.net: the tags
    /// of another platform, `OSX,arm64`).
    pub install: Option<String>,
    pub from: PrefillFrom,
    pub state: PrefillState,
    /// Requests of the job (whole files and ranges), once known.
    pub files_total: u64,
    /// Requests dealt with so far (fetched, skipped, missing or taken from lancache).
    pub files_done: u64,
    /// Requests the stash already held, and their bytes (not fetched: not in `bytes`).
    pub files_skipped: u64,
    pub bytes_skipped: u64,
    /// Files the CDN did not have.
    pub files_missing: u64,
    /// Requests taken from lancache, and their bytes.
    pub files_local: u64,
    pub bytes_local: u64,
    /// Requests that failed their check: from lancache (or unreadable there; fetched from the CDN then),
    /// or what the CDN sent of a platform whose files are checked (nothing of it stored).
    pub files_rejected: u64,
    /// The job's bytes, once known.
    pub bytes_total: u64,
    /// Bytes received for the requests fetched or taken.
    pub bytes: u64,
    /// The file in flight.
    pub current: Option<String>,
    pub error: Option<String>,
    pub started_ms: u64,
    pub finished_ms: Option<u64>,
    /// While the job is being planned (before `files_total` is known): how far that has come.
    pub planning: Option<Planning>,
    /// The games waiting to be prefilled after this one, in order.
    pub queued: Vec<String>,
}

/// How far the planning of a prefill job has come: files read of all it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Planning {
    pub done: u64,
    pub total: u64,
}

impl PrefillStatus {
    /// A job of `game` just started at `now_ms`, nothing known yet.
    #[must_use]
    pub fn started(game: &str, prerelease: bool, from: PrefillFrom, now_ms: u64) -> Self {
        Self {
            game: game.into(),
            version: None,
            prerelease,
            install: None,
            from,
            state: PrefillState::Running,
            files_total: 0,
            files_done: 0,
            files_skipped: 0,
            bytes_skipped: 0,
            files_missing: 0,
            files_local: 0,
            bytes_local: 0,
            files_rejected: 0,
            bytes_total: 0,
            bytes: 0,
            current: None,
            error: None,
            started_ms: now_ms,
            finished_ms: None,
            planning: None,
            queued: Vec::new(),
        }
    }
}

/// Why a prefill did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// Another prefill is running.
    Busy,
    /// The directory is not a game (a launcher's own data).
    NotAGame,
    /// Prefill is not set up for this platform.
    NotSetUp,
    /// From lancache, without a lancache cache set up.
    NoLancache,
    /// For another install, on a platform that prefills only the one it is set up for.
    NoInstall,
    /// The job could not be started.
    Spawn(String),
}

/// One platform's prefill jobs, one at a time.
pub trait Prefill: Send + Sync + core::fmt::Debug {
    /// Starts prefilling `game`: its current build, or its pre-release build
    /// when `prerelease`, taking the bytes `from` the CDN or lancache first.
    ///
    /// # Errors
    /// [`StartError`].
    fn start(&self, game: &str, prerelease: bool, from: PrefillFrom) -> Result<(), StartError>;

    /// Whether [`PrefillFrom::Lancache`] is set up.
    fn lancache(&self) -> bool {
        false
    }

    /// Starts prefilling `game` now, or after the jobs before it when one runs.
    /// Returns how many wait before it (0: started).
    ///
    /// # Errors
    /// [`StartError`] other than `Busy`.
    fn queue(&self, game: &str, prerelease: bool, from: PrefillFrom) -> Result<usize, StartError> {
        self.start(game, prerelease, from).map(|()| 0)
    }

    /// As [`Prefill::queue`], for `install`: one other than the platform is set up for (Battle.net:
    /// another platform's tags, `OSX,arm64`); `None` is the one it is set up for.
    ///
    /// # Errors
    /// [`StartError`] other than `Busy`; [`StartError::NoInstall`] where the platform prefills only
    /// the install it is set up for.
    fn queue_for(
        &self,
        game: &str,
        prerelease: bool,
        from: PrefillFrom,
        install: Option<&str>,
    ) -> Result<usize, StartError> {
        match install {
            None => self.queue(game, prerelease, from),
            Some(_) => Err(StartError::NoInstall),
        }
    }

    /// Takes `game` out of the queue; whether it was queued.
    fn unqueue(&self, _game: &str) -> bool {
        false
    }

    /// Asks the running job to stop (and forgets the queued ones).
    fn stop(&self);

    /// The last job, running or finished.
    fn status(&self) -> Option<PrefillStatus>;

    /// The jobs that ended since the last call, oldest first: a queue of jobs ends several
    /// between two looks at [`Prefill::status`].
    fn take_ended(&self) -> Vec<PrefillStatus> {
        Vec::new()
    }
}

/// The prefill jobs that ran, kept beyond a restart.
pub trait PrefillLog: Send + Sync {
    /// Keeps `run`, a job of `platform` that ended (one already kept, by platform, game and
    /// start, is replaced). What is in flight or queued is not kept.
    ///
    /// # Errors
    /// It could not be kept.
    fn record(&self, platform: &str, run: &PrefillStatus) -> Result<(), String>;

    /// The last `limit` jobs kept, of `platform` or of all, newest first, with their platform.
    ///
    /// # Errors
    /// They could not be read.
    fn runs(&self, platform: Option<&str>, limit: usize) -> Result<Vec<(String, PrefillStatus)>, String>;
}
