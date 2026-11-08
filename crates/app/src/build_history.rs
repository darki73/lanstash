//! Use case: what was updated when. Each time a platform's build status is
//! worked out, it is compared with the one seen before, game by game, and what
//! changed is noted: a game seen for the first time (`added`), a new build of
//! it (`new-build`, with the build before), and the stash coming to hold all of
//! its build (`current`).
//!
//! The events and what was seen are kept in a [`HistoryLog`], so an update
//! that came while lanstash was stopped is noted at the next start. The first
//! status a platform ever gives is only taken in (no `added` for every game
//! already there). A game that leaves the status is not taken to be gone:
//! right after a start a platform may list only some of its games; it stays seen.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::history::{Change, Event, EventQuery, HistoryLog, Seen};
use lanstash_ports::platform::BuildStatus;

/// What was seen, as kept: (platform, game) to (build, held whole if known), and the platforms taken in.
#[derive(Debug, Default)]
struct State {
    games: BTreeMap<(String, String), (String, Option<bool>)>,
    platforms: BTreeSet<String>,
}

/// The history of builds.
pub struct BuildHistory {
    log: Arc<dyn HistoryLog>,
    state: Mutex<State>,
}

impl std::fmt::Debug for BuildHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildHistory").finish_non_exhaustive()
    }
}

impl BuildHistory {
    /// The history kept in `log` (as if nothing was seen when it cannot be read).
    #[must_use]
    pub fn new(log: Arc<dyn HistoryLog>) -> Self {
        let mut state = State::default();
        match log.seen() {
            Ok(all) => {
                state.platforms = all.platforms.into_iter().collect();
                state.games = all.games.into_iter().map(|s| ((s.platform, s.game), (s.version, s.held))).collect();
            }
            Err(e) => println!("build history not read: {e}"),
        }
        Self { log, state: Mutex::new(state) }
    }

    /// Takes in `platform`'s build status as worked out at `now_ms`: notes what changed, keeps it.
    /// When it cannot be kept, nothing of it is taken in (it is noted again next time).
    pub fn record(&self, platform: &str, status: &[BuildStatus], now_ms: u64) {
        let mut st = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let first = !st.platforms.contains(platform);
        if first && status.is_empty() {
            return; // nothing to take in yet (a library not read)
        }
        let (mut events, mut seen) = (Vec::new(), Vec::new());
        for s in status {
            let current = s.known && s.current;
            let event = |change, from| Event {
                at_ms: now_ms,
                platform: platform.to_owned(),
                game: s.game.clone(),
                change,
                version: s.version.clone(),
                from,
            };
            // What the stash holds of it now, as far as is known.
            let now_held = s.known.then_some(current);
            let before = st.games.get(&(platform.to_owned(), s.game.clone()));
            // What is kept of it now (None: nothing changed), and whether it became current.
            let (held, became_current) = match before {
                // Taken in as it is: no news (not known yet stays unknown, so its first answer is none either).
                None if first => (Some(now_held), false),
                None => {
                    events.push(event(Change::Added, None));
                    (Some(Some(current)), current)
                }
                Some((version, _)) if *version != s.version => {
                    events.push(event(Change::NewBuild, Some(version.clone())));
                    (Some(Some(current)), current)
                }
                // How much is held is not known yet: the game stays as it was.
                Some(_) if !s.known => (None, false),
                // Not known when taken in: its first known answer is taken in, no news.
                Some((_, None)) => (Some(now_held), false),
                Some((_, Some(was))) if *was != current => (Some(now_held), current),
                Some(_) => (None, false),
            };
            let Some(held) = held else { continue };
            if became_current {
                events.push(event(Change::Current, None));
            }
            seen.push(Seen { platform: platform.to_owned(), game: s.game.clone(), version: s.version.clone(), held });
        }
        if !first && seen.is_empty() {
            return;
        }
        if let Err(e) = self.log.note(platform, &events, &seen) {
            println!("build history not kept: {e}");
            return;
        }
        st.platforms.insert(platform.to_owned());
        for s in seen {
            st.games.insert((s.platform, s.game), (s.version, s.held));
        }
    }

    /// The last `limit` events, newest first: of `platform` (and of its `games`, when any are given), or of all.
    #[must_use]
    pub fn events(&self, platform: Option<&str>, games: &[String], limit: usize) -> Vec<Event> {
        self.log.events(&EventQuery { platform, games, limit }).unwrap_or_else(|e| {
            println!("build history not read: {e}");
            Vec::new()
        })
    }
}
