//! The platform contract: what the core asks of a platform (Battle.net,
//! Steam, ...) beyond routing, which is the platform's
//! [`lanstash_domain::service::Profile`]. Every capability is optional; a
//! platform answers `None` for what it does not have.
//!
//! Views are JSON documents the platform renders itself; the admin API sends
//! them as they are.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::account::Account;
use crate::art::ArtSource;
use crate::names::GameNames;
use crate::prefill::Prefill;

/// Why a view has no document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    /// No such game on this platform.
    NotFound(String),
    /// The platform could not answer now (a lookup failed or is not done).
    Unavailable(String),
}

/// One thing a prune deletes from the stash: an object file (by journal number) or a
/// segment (by sequence number), of the store directory `dir`; or a segment
/// compacted: the objects of `keep` (keys) copied out of it, then it goes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PruneItem {
    File { dir: String, no: u32 },
    Segment { dir: String, seq: u32 },
    Compact { dir: String, seq: u32, keep: Vec<String> },
}

/// What a prune would delete: a view of it, and the items.
pub type Pruning = (String, Vec<PruneItem>);

/// A JSON document, or why there is none.
pub type View = Result<String, ViewError>;

/// A platform's [`Platform::builds_mark`], read when asked.
pub type BuildsMarkOf = Arc<dyn Fn() -> u64 + Send + Sync>;

/// A platform's builds view and build status, worked out together ([`Platform::builds_and_status`]).
pub type BuildsAndStatus = (Option<View>, Option<Vec<BuildStatus>>);

/// Where one stashed game's current build stands, for keeping it current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStatus {
    /// What a prefill of it takes (a Battle.net product code, a Steam app id).
    pub game: String,
    /// The current build, as the platform names it.
    pub version: String,
    /// Whether how much of the build the stash holds is known yet.
    pub known: bool,
    /// Whether the stash holds all of it.
    pub current: bool,
}

/// An operating system a PC runs: what a platform's setup file is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientOs {
    Windows,
    MacOs,
    Linux,
}

impl ClientOs {
    /// Its name in API paths: `windows`, `macos`, `linux`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::MacOs => "macos",
            Self::Linux => "linux",
        }
    }

    /// The system an API path names.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Windows, Self::MacOs, Self::Linux].into_iter().find(|os| os.name() == name)
    }
}

/// A file a PC runs to set itself up for a platform (where the platform's
/// client needs that before lanstash keeps its downloads).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupFile {
    /// The name it is saved under.
    pub name: String,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

/// One platform.
pub trait Platform: Send + Sync + core::fmt::Debug {
    /// The service identifier (`blizzard`, `steam`, ...): the object key
    /// prefix, the store directory and the platform's name in API paths.
    fn id(&self) -> &str;

    /// Display names of its games.
    fn names(&self) -> Option<Arc<dyn GameNames>> {
        None
    }

    /// Its games' images.
    fn art(&self) -> Option<Arc<dyn ArtSource>> {
        None
    }

    /// Every game in the stash with its current build and how much of it is held.
    fn builds(&self) -> Option<View> {
        None
    }

    /// Where each stashed game's current build stands (for automatic updates).
    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        None
    }

    /// [`Platform::builds`] and [`Platform::build_status`] together: a platform that works
    /// both out from one look at the stash gives them from that one look.
    fn builds_and_status(&self) -> BuildsAndStatus {
        (self.builds(), self.build_status())
    }

    /// A number that moves when the platform's own idea of its builds changes without the
    /// stash changing (a game's file lists read, a game forgotten): what keeps the builds
    /// looks at it with the stash's changes. 0 (the default): never moves.
    fn builds_mark(&self) -> u64 {
        0
    }

    /// When the builds view last changed (ms), where that is known: the web asks for
    /// the builds again when it moves instead of asking every so often.
    fn builds_changed_ms(&self) -> Option<u64> {
        None
    }

    /// Every game the platform offers, cached or not.
    fn catalog(&self) -> Option<View> {
        None
    }

    /// One game's builds in detail; `game` is a safe path segment.
    fn game(&self, _game: &str) -> Option<View> {
        None
    }

    /// How much of one game's current build lancache's nginx cache holds.
    fn nginx(&self, _game: &str) -> Option<View> {
        None
    }

    /// Its prefill jobs.
    fn prefill(&self) -> Option<Arc<dyn Prefill>> {
        None
    }

    /// What of one game no current build needs: a view of it, and the items a
    /// prune deletes.
    fn prune(&self, _game: &str) -> Option<Result<Pruning, ViewError>> {
        None
    }

    /// The store directories `game` is made of, when it is more than its own
    /// (a Steam app: its depots); deleting the game deletes them all.
    fn parts(&self, _game: &str) -> Option<Vec<String>> {
        None
    }

    /// `game` is being deleted: drop what is kept of it between runs (a Steam
    /// app's manifests' chunk lists). Losing such state costs time, never correctness.
    fn forget(&self, _game: &str) {}

    /// Its account (signing in, for platforms whose downloads need one).
    fn account(&self) -> Option<Arc<dyn Account>> {
        None
    }

    /// The file that sets up a PC running `os` for the platform, where one is needed and made.
    fn setup(&self, _os: ClientOs) -> Option<SetupFile> {
        None
    }
}
