//! Display names of games: a table from
//! `(service, game directory)` to a name, filled from `names.conf` lines
//! (`steam/1422450 = Deadlock`), with the grouping rules the platforms give.
//! Battle.net games are named by Blizzard's own catalog, not by a table here.

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;

/// Names by `(service, game)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameTable {
    names: BTreeMap<(String, String), String>,
    /// Services whose numbered game directories group under a nearby lower number, and how far below.
    numbered: BTreeMap<String, u32>,
}

impl NameTable {
    pub fn insert(&mut self, service: &str, game: &str, name: &str) {
        self.names.insert((service.to_owned(), game.to_owned()), name.to_owned());
    }

    /// Numbered game directories of `service` without an entry of their own
    /// take the entry of the nearest number at most `window` below them.
    pub fn group_numbered(&mut self, service: &str, window: u32) {
        self.numbered.insert(service.to_owned(), window);
    }

    /// Adds `service/game = Name` lines (`#` comments); later lines win.
    ///
    /// # Errors
    /// `(line number, message)` of the first malformed line.
    pub fn add_text(&mut self, text: &str) -> Result<(), (usize, String)> {
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let parsed = line.split_once('=').and_then(|(k, name)| {
                let (service, game) = k.trim().split_once('/')?;
                let (service, game, name) = (service.trim(), game.trim(), name.trim());
                (!service.is_empty() && !game.is_empty() && !name.is_empty()).then_some((service, game, name))
            });
            let Some((service, game, name)) = parsed else {
                return Err((n + 1, format!("expected `service/game = Name`, got {line:?}")));
            };
            self.insert(service, game, name);
        }
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The name of a game directory: its own entry, or for a numbered
    /// service the nearest entry below it within its window.
    #[must_use]
    pub fn get(&self, service: &str, game: &str) -> Option<&str> {
        self.entry(service, game).map(|(_, name)| name)
    }

    /// The game a directory belongs to: the key of the entry [`Self::get`]
    /// found, so a Steam depot gives its app id. `None` when no entry matches.
    #[must_use]
    pub fn group(&self, service: &str, game: &str) -> Option<&str> {
        self.entry(service, game).map(|(key, _)| key)
    }

    fn entry(&self, service: &str, game: &str) -> Option<(&str, &str)> {
        let found = |g: &str| {
            self.names.get_key_value(&(service.to_owned(), g.to_owned())).map(|((_, k), n)| (k.as_str(), n.as_str()))
        };
        if let Some(e) = found(game) {
            return Some(e);
        }
        let window = *self.numbered.get(service)?;
        let number: u32 = game.parse().ok()?;
        (number.saturating_sub(window)..number).rev().find_map(|below| found(&format!("{below}")))
    }
}
