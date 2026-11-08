//! Use case: what each client got over a range, per game, from the counters kept
//! by the minute (`client/<address>/<service>/<game>/<hit|miss>`): so it outlives a
//! restart and covers any day, week or month kept.

use std::collections::BTreeMap;

use crate::counters_kept::CountersKept;
use crate::serve::clients::{ClientCounter, PREFIX};

/// The most counters read for a range (two per client and game).
pub const MAX_COUNTERS: usize = 10_000;

/// What a client got of one game (a store directory: a Steam depot, a Battle.net CDN folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameBytes {
    pub service: String,
    pub game: String,
    /// From the stash.
    pub hit: u64,
    /// Sent while lanstash downloaded it.
    pub miss: u64,
}

/// What a client got over a range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientBytes {
    pub client: String,
    pub hit: u64,
    pub miss: u64,
    /// Most first.
    pub games: Vec<GameBytes>,
}

/// A client's (from the stash, while downloading) bytes per (service, game).
type PerGame = BTreeMap<(String, String), (u64, u64)>;

/// The counters `counters` (name, how much it went up) as each client's bytes per game, the
/// client that got most first; names that are not clients' counters are left out.
#[must_use]
pub fn group(counters: &[(String, u64)]) -> Vec<ClientBytes> {
    let mut by: BTreeMap<String, PerGame> = BTreeMap::new();
    for (name, value) in counters {
        let Some(c) = ClientCounter::parse(name) else { continue };
        let g = by.entry(c.client).or_default().entry((c.service, c.game)).or_default();
        if c.hit {
            g.0 += value;
        } else {
            g.1 += value;
        }
    }
    let mut out: Vec<ClientBytes> = by
        .into_iter()
        .map(|(client, games)| {
            let mut games: Vec<GameBytes> = games
                .into_iter()
                .map(|((service, game), (hit, miss))| GameBytes { service, game, hit, miss })
                .collect();
            games.sort_by(|a, b| (b.hit + b.miss).cmp(&(a.hit + a.miss)).then_with(|| a.game.cmp(&b.game)));
            let (hit, miss) = games.iter().fold((0, 0), |(h, m), g| (h + g.hit, m + g.miss));
            ClientBytes { client, hit, miss, games }
        })
        .collect();
    out.sort_by(|a, b| (b.hit + b.miss).cmp(&(a.hit + a.miss)).then_with(|| a.client.cmp(&b.client)));
    out
}

/// What each client got from `from_ms` to before `to_ms`.
///
/// # Errors
/// The kept counters could not be read.
pub fn over(kept: &CountersKept, from_ms: u64, to_ms: u64) -> Result<Vec<ClientBytes>, String> {
    kept.top(PREFIX, from_ms, to_ms, MAX_COUNTERS).map(|c| group(&c))
}
