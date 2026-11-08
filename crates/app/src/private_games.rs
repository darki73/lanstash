//! Use case: the games each account keeps private ([`lanstash_domain::private`]). The marks are
//! kept in [`Keep`] as `<account> <platform>/<game>` a line, so they outlive a restart; an
//! account's marks go with the account.

use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::accounts::name_problem;
use lanstash_domain::private::Marks;
use lanstash_ports::keep::Keep;

use crate::hidden::is_game_key;

/// The name the marks are kept under.
pub const KEPT: &str = "private";

/// Every account's private games.
pub struct PrivateGames {
    keep: Arc<dyn Keep>,
    marks: Mutex<Marks>,
}

impl std::fmt::Debug for PrivateGames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateGames").field("games", &self.marks().len()).finish_non_exhaustive()
    }
}

impl PrivateGames {
    /// The marks `keep` holds (a line that names no account and game is skipped).
    ///
    /// # Errors
    /// They are kept but could not be read.
    pub fn new(keep: Arc<dyn Keep>) -> Result<Self, String> {
        let kept = keep.load(KEPT)?.unwrap_or_default();
        let mut marks = Marks::new();
        for line in String::from_utf8_lossy(&kept).lines() {
            if let Some((account, game)) = line.trim().split_once(' ')
                && name_problem(account).is_none()
                && is_game_key(game)
            {
                marks.entry(game.to_owned()).or_default().insert(account.to_owned());
            }
        }
        Ok(Self { keep, marks: Mutex::new(marks) })
    }

    /// Every private game, with the accounts that marked it.
    #[must_use]
    pub fn marks(&self) -> Marks {
        self.marks.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The games `account` keeps private, in order.
    #[must_use]
    pub fn of(&self, account: &str) -> Vec<String> {
        self.marks().into_iter().filter(|(_, by)| by.contains(account)).map(|(g, _)| g).collect()
    }

    /// Marks `game` private for `account`, or takes the mark away, and keeps the marks.
    ///
    /// # Errors
    /// No such account name or game, or the marks could not be kept (then nothing changed).
    pub fn set(&self, account: &str, game: &str, private: bool) -> Result<(), String> {
        if let Some(why) = name_problem(account) {
            return Err(why.to_owned());
        }
        if !is_game_key(game) {
            return Err(format!("not a game: {game:?} (<platform>/<game>)"));
        }
        self.change(|m| {
            if private {
                m.entry(game.to_owned()).or_default().insert(account.to_owned());
            } else if let Some(by) = m.get_mut(game) {
                by.remove(account);
                if by.is_empty() {
                    m.remove(game);
                }
            }
        })
    }

    /// Takes away every mark of `account` (its account is gone).
    ///
    /// # Errors
    /// The marks could not be kept (then nothing changed).
    pub fn forget(&self, account: &str) -> Result<(), String> {
        self.change(|m| {
            m.values_mut().for_each(|by| {
                by.remove(account);
            });
            m.retain(|_, by| !by.is_empty());
        })
    }

    fn change(&self, f: impl FnOnce(&mut Marks)) -> Result<(), String> {
        let mut marks = self.marks.lock().unwrap_or_else(PoisonError::into_inner);
        let mut next = marks.clone();
        f(&mut next);
        let text: String = next.iter().flat_map(|(g, by)| by.iter().map(move |a| format!("{a} {g}\n"))).collect();
        self.keep.save(KEPT, text.as_bytes())?;
        *marks = next;
        Ok(())
    }
}
