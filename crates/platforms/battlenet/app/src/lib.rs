//! Battle.net's use cases: the product list, each game's live and
//! pre-release builds, install plans, the launcher catalog (names and art),
//! prefill, what lancache's nginx cache holds of a plan, and what of a game no
//! current build needs (prune).
//!
//! Over the ports and the core's use cases (`lanstash-app`); no I/O of its
//! own beyond spawning background threads.

#![forbid(unsafe_code)]

pub mod builds;
pub mod catalog;
pub mod nginx;
pub mod plans;
pub mod prefill;
pub mod products;
pub mod prune;
