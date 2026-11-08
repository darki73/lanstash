//! Use case: the games kept private. A game (`<platform>/<game>`: a Steam app id, a Battle.net
//! product code) marked hidden is still cached, updated and prefilled as before; whoever reads
//! the list masks it where it is shown (its name, art, page and builds). The marks are kept in
//! [`Keep`] as one game a line, so they outlive a restart.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::keep::Keep;

/// The name the marks are kept under.
pub const KEPT: &str = "hidden";

/// Whether `key` names a game as `<platform>/<game>`: a platform of 1 to 32 lower-case letters,
/// digits or `-`, and a game of 1 to 64 letters, digits, `_` or `-`.
#[must_use]
pub fn is_game_key(key: &str) -> bool {
    let Some((platform, game)) = key.split_once('/') else { return false };
    let ok = |s: &str, max: usize, more: &dyn Fn(char) -> bool| {
        (1..=max).contains(&s.len()) && s.chars().all(|c| c.is_ascii_digit() || c == '-' || more(c))
    };
    ok(platform, 32, &|c| c.is_ascii_lowercase()) && ok(game, 64, &|c| c.is_ascii_alphabetic() || c == '_')
}

/// The games marked hidden.
pub struct HiddenGames {
    keep: Arc<dyn Keep>,
    games: Mutex<BTreeSet<String>>,
}

impl std::fmt::Debug for HiddenGames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HiddenGames").field("games", &self.list().len()).finish_non_exhaustive()
    }
}

impl HiddenGames {
    /// The marks `keep` holds (a line that names no game is skipped).
    ///
    /// # Errors
    /// They are kept but could not be read.
    pub fn new(keep: Arc<dyn Keep>) -> Result<Self, String> {
        let kept = keep.load(KEPT)?.unwrap_or_default();
        let text = String::from_utf8_lossy(&kept);
        let games = text.lines().map(str::trim).filter(|l| is_game_key(l)).map(String::from).collect();
        Ok(Self { keep, games: Mutex::new(games) })
    }

    /// The games marked hidden, in order.
    #[must_use]
    pub fn list(&self) -> Vec<String> {
        self.games.lock().unwrap_or_else(PoisonError::into_inner).iter().cloned().collect()
    }

    /// Marks `game` (`<platform>/<game>`) hidden, or takes the mark away, and keeps the marks.
    ///
    /// # Errors
    /// `game` names no game, or the marks could not be kept (then nothing changed).
    pub fn set(&self, game: &str, hidden: bool) -> Result<(), String> {
        if !is_game_key(game) {
            return Err(format!("not a game: {game:?} (<platform>/<game>)"));
        }
        let mut games = self.games.lock().unwrap_or_else(PoisonError::into_inner);
        let mut next = games.clone();
        if hidden {
            next.insert(game.to_owned());
        } else {
            next.remove(game);
        }
        let text: String = next.iter().map(|g| format!("{g}\n")).collect();
        self.keep.save(KEPT, text.as_bytes())?;
        *games = next;
        Ok(())
    }
}
