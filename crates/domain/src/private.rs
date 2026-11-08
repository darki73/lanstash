//! Private games: a player keeps to themselves that they have a game. A mark is per account; it
//! hides that account's having the game, never another owner's. So a game is masked for a reader
//! only when every account that owns it marked it private, and the reader is none of them: one
//! owner who did not mark it keeps it in view (and who owns a game is never shown to a reader in
//! any case). Admins see everything; that is decided before this.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

/// The games marked private (`<platform>/<game>`), each with the accounts that marked it.
pub type Marks = BTreeMap<String, BTreeSet<String>>;

/// Of the marked games, those `reader` (an account; none: signed out) sees masked. `owners` gives
/// the platform accounts that own a game (`steam/<account>`; none known: only its markers do),
/// `links` the platform accounts an account is linked to.
#[must_use]
pub fn masked_for(
    reader: Option<&str>,
    marks: &Marks,
    owners: &dyn Fn(&str) -> Vec<String>,
    links: &dyn Fn(&str) -> Vec<String>,
) -> Vec<String> {
    marks
        .iter()
        .filter(|(_, by)| !by.is_empty() && reader.is_none_or(|r| !by.contains(r)))
        .filter(|(game, by)| {
            // Every owner is one of the markers' own platform accounts.
            let theirs: BTreeSet<String> = by.iter().flat_map(|a| links(a)).collect();
            owners(game).iter().all(|o| theirs.contains(o))
        })
        .map(|(game, _)| game.clone())
        .collect()
}
