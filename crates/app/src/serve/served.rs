//! Bytes sent to clients per game since the start (from the stash or while
//! downloading), by service and store directory: what the counters kept by the
//! minute file as `served/<service>/<game>`, so how much each game served
//! outlives a restart.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

/// Bytes sent per game.
#[derive(Debug, Default)]
pub struct ServedByGame(Mutex<BTreeMap<(String, String), u64>>);

impl ServedByGame {
    /// Counts `bytes` sent of `service`'s `game`.
    pub fn add(&self, service: &str, game: &str, bytes: u64) {
        if bytes == 0 {
            return;
        }
        let mut m = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match m.get_mut(&(service.to_owned(), game.to_owned())) {
            Some(n) => *n += bytes,
            None => {
                m.insert((service.to_owned(), game.to_owned()), bytes);
            }
        }
    }

    /// Every game sent anything since the start: its counter name (`served/<service>/<game>`)
    /// and its bytes.
    #[must_use]
    pub fn counters(&self) -> Vec<(String, u64)> {
        let m = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        m.iter().map(|((s, g), n)| (format!("{PREFIX}{s}/{g}"), *n)).collect()
    }
}

/// What the counters of games' bytes sent are named after.
pub const PREFIX: &str = "served/";
