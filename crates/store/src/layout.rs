//! Where things live inside a store root:
//!
//! ```text
//! <service>/<game>/seg/<seq:05>.lcseg
//! <service>/<game>/obj/<safe cdn path>
//! <service>/<game>/journal.lsj
//! <service>/<game>/used.lsu
//! ```

use lanstash_domain::object::{game_dir, safe_component};

pub const SEGMENT_DIR: &str = "seg";
pub const OBJECT_DIR: &str = "obj";
pub const JOURNAL_FILE: &str = "journal.lsj";
/// When each segment and object file was last used.
pub const RECENCY_FILE: &str = "used.lsu";
pub const SEGMENT_EXT: &str = ".lcseg";

/// A game's directory within a root.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GameDir {
    /// Service identifier (`steam`, `blizzard`, …), already a safe component.
    pub service: String,
    /// Game directory name (safe component).
    pub game: String,
}

impl GameDir {
    /// From a service and a raw game id (made safe here).
    #[must_use]
    pub fn new(service: &str, game: &str) -> Self {
        Self { service: safe_component(service), game: game_dir(game) }
    }

    /// From names read back from a directory listing (already safe).
    #[must_use]
    pub fn from_listing(service: &str, game: &str) -> Self {
        Self { service: service.to_owned(), game: game.to_owned() }
    }

    #[must_use]
    pub fn path(&self) -> String {
        format!("{}/{}", self.service, self.game)
    }

    #[must_use]
    pub fn segment_dir(&self) -> String {
        format!("{}/{SEGMENT_DIR}", self.path())
    }

    #[must_use]
    pub fn segment(&self, seq: u32) -> String {
        format!("{}/{SEGMENT_DIR}/{seq:05}{SEGMENT_EXT}", self.path())
    }

    #[must_use]
    pub fn journal(&self) -> String {
        format!("{}/{JOURNAL_FILE}", self.path())
    }

    /// Temporary journal name while rewriting it.
    #[must_use]
    pub fn journal_tmp(&self) -> String {
        format!("{}/{JOURNAL_FILE}.tmp", self.path())
    }

    #[must_use]
    pub fn recency(&self) -> String {
        format!("{}/{RECENCY_FILE}", self.path())
    }

    /// Temporary name while replacing the recency file.
    #[must_use]
    pub fn recency_tmp(&self) -> String {
        format!("{}/{RECENCY_FILE}.tmp", self.path())
    }

    /// Absolute (root-relative) path of an object file from its game-relative path.
    #[must_use]
    pub fn file(&self, rel: &str) -> String {
        format!("{}/{rel}", self.path())
    }
}

/// Game-relative path for an object file (`obj/<safe path>`).
#[must_use]
pub fn object_rel_path(safe_path: &str) -> String {
    format!("{OBJECT_DIR}/{safe_path}")
}

/// `00042.lcseg` → 42.
#[must_use]
pub fn parse_segment_name(name: &str) -> Option<u32> {
    let digits = name.strip_suffix(SEGMENT_EXT)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}
