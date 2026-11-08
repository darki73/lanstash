//! Display names of games, for the status page and reports. Sources (a names
//! file, built-in tables, a platform's catalog) are adapters or platforms.

use alloc::string::String;

/// Names of game directories.
pub trait GameNames: Send + Sync {
    /// The name of the game directory `game` of `service`, if known.
    fn name(&self, service: &str, game: &str) -> Option<String>;

    /// The game `game` belongs to (for a Steam depot, its app id), if known.
    fn group(&self, _service: &str, _game: &str) -> Option<String> {
        None
    }
}

/// Knows no names.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoNames;

impl GameNames for NoNames {
    fn name(&self, _service: &str, _game: &str) -> Option<String> {
        None
    }
}
