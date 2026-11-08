//! What was updated when: the events of the build history and the games as
//! last seen, kept beyond a restart (see the app's `build_history`).

use alloc::string::String;
use alloc::vec::Vec;

/// What changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A game seen for the first time.
    Added,
    /// A new build of a game.
    NewBuild,
    /// The stash came to hold all of the game's build.
    Current,
}

impl Change {
    /// Its name in the API and in what is kept.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::NewBuild => "new-build",
            Self::Current => "current",
        }
    }

    /// The change `as_str` names.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Added, Self::NewBuild, Self::Current].into_iter().find(|c| c.as_str() == s)
    }
}

/// One change of one game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub at_ms: u64,
    pub platform: String,
    pub game: String,
    pub change: Change,
    /// The build it is about.
    pub version: String,
    /// The build before (a new build).
    pub from: Option<String>,
}

/// A game as last seen: its build and whether the stash held all of it (`None`: not known when
/// it was first taken in, as right after a start; its first known answer is no change).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub platform: String,
    pub game: String,
    pub version: String,
    pub held: Option<bool>,
}

/// What was seen: every game, and the platforms whose status was taken in once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeenAll {
    pub games: Vec<Seen>,
    pub platforms: Vec<String>,
}

/// Which events: of a platform, of games, at most how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventQuery<'a> {
    pub platform: Option<&'a str>,
    /// Of these games (of the platform) only; empty: any.
    pub games: &'a [String],
    pub limit: usize,
}

/// The kept build history.
pub trait HistoryLog: Send + Sync {
    /// What was seen.
    ///
    /// # Errors
    /// It could not be read.
    fn seen(&self) -> Result<SeenAll, String>;

    /// Keeps, all at once, that `platform`'s status was taken in, the `events` it gave, and
    /// the games now `seen` as they are.
    ///
    /// # Errors
    /// It could not be kept (then nothing of it is).
    fn note(&self, platform: &str, events: &[Event], seen: &[Seen]) -> Result<(), String>;

    /// The events `query` asks for, newest first.
    ///
    /// # Errors
    /// They could not be read.
    fn events(&self, query: &EventQuery<'_>) -> Result<Vec<Event>, String>;
}
