//! What of a game no current build needs, for a platform whose files are small
//! and sit in segments (many each): Steam's chunks, Epic's. A platform names the
//! game's store directories and the keys its current builds need; a segment
//! goes only when none of its objects is needed and it was not used within the
//! grace period (old builds' files were written together, so their segments go
//! whole). A segment mixing old and current objects is compacted (its current
//! ones copied out, then it goes) once at most half of it is current and it was
//! not used within the grace period; otherwise it stays.

use std::collections::{BTreeMap, BTreeSet};

use lanstash_ports::platform::PruneItem;
use lanstash_store::evict::EvictItem;
use lanstash_store::index::{Lookup, SegId, Source};
use lanstash_store::store::Store;

use crate::delete_game::{DeleteState, GameDeleter};

/// What a prune of a game would delete, and what it keeps for now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prunable {
    pub items: Vec<PruneItem>,
    pub bytes: u64,
    /// Bytes of segments no current build needs, kept as used within the grace period.
    pub recent_bytes: u64,
    /// Of `items`, the segments compacted rather than deleted whole.
    pub compact: usize,
}

/// What of `service`'s directories `dirs` the objects `needed` (their keys) do not keep, at
/// `now_ms`, with a grace period of `grace_ms`.
#[must_use]
pub fn prunable(
    store: &Store,
    service: &str,
    dirs: &BTreeSet<String>,
    needed: &[String],
    now_ms: u64,
    grace_ms: u64,
) -> Prunable {
    // The segments holding any needed object: their id, needed bytes and keys.
    let index = store.index();
    let mut live: BTreeMap<String, (SegId, u64, Vec<String>)> = BTreeMap::new();
    for k in needed {
        let Lookup::Found(o) = store.lookup_quietly(k) else { continue };
        for (span, src) in o.pieces {
            let Source::Record { seg, .. } = src else { continue };
            let Some(s) = index.segment(seg) else { continue };
            let e = live.entry(s.path).or_insert((seg, 0, Vec::new()));
            e.1 += span.len();
            e.2.push(k.clone());
        }
    }
    let mut out = Prunable { items: Vec::new(), bytes: 0, recent_bytes: 0, compact: 0 };
    for v in store.eviction_candidates() {
        let EvictItem::Segment(seq) = v.item else { continue };
        if v.game.service != service || !dirs.contains(&v.game.game) {
            continue;
        }
        let (kept, keep) = match live.remove(&v.path) {
            None => (0, Vec::new()),
            Some((seg, kept, keep)) => {
                // Bytes of the objects served from it that no current build needs.
                let served: u64 = index.records_in(seg).iter().filter(|r| index.serves(r)).map(|r| r.body_len).sum();
                if served.saturating_sub(kept) < kept.max(1) {
                    continue; // mostly current objects: stays
                }
                (kept, keep)
            }
        };
        let gain = v.bytes.saturating_sub(kept);
        if now_ms.saturating_sub(v.last_used_ms) < grace_ms {
            out.recent_bytes += gain;
            continue;
        }
        out.bytes += gain;
        let dir = v.game.game.clone();
        if keep.is_empty() {
            out.items.push(PruneItem::Segment { dir, seq });
        } else {
            out.compact += 1;
            out.items.push(PruneItem::Compact { dir, seq, keep });
        }
    }
    out
}

/// One step of the automatic prune: the first of `current` (games whose current build is
/// stashed whole) that has something no current build needs starts deleting it, unless a
/// deletion runs or `busy` says the game is being prefilled. Returns the game and the bytes going.
pub fn prune_once(
    service: &str,
    current: &[String],
    find: &dyn Fn(&str) -> Result<Prunable, String>,
    deleter: &GameDeleter,
    busy: &dyn Fn(&str) -> bool,
) -> Option<(String, u64)> {
    if deleter.status().is_some_and(|s| s.state == DeleteState::Running) {
        return None;
    }
    for game in current {
        if busy(game) {
            continue;
        }
        let Ok(p) = find(game) else { continue };
        if p.items.is_empty() {
            continue;
        }
        let bytes = p.bytes;
        return deleter.start_items(service, game, p.items).ok().map(|()| (game.clone(), bytes));
    }
    None
}

/// The prune view, in Battle.net's shape (`files`: the segments; `compact`: how many of them
/// are compacted; `bytes`: the space given back; `recent_bytes`: kept as used lately).
#[must_use]
pub fn view(game: &str, p: &Prunable) -> String {
    format!(
        "{{\"game\":{},\"files\":{},\"compact\":{},\"bytes\":{},\"recent_bytes\":{}}}\n",
        lanstash_domain::json::string(game),
        p.items.len(),
        p.compact,
        p.bytes,
        p.recent_bytes
    )
}
