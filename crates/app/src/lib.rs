//! lanstash application layer.
//!
//! Use cases orchestrate domain logic and the store through ports (traits).
//! This crate does no I/O itself: sockets and clocks are reached only through
//! ports, implemented in `crates/adapters/*` and wired in the composition root.

#![forbid(unsafe_code)]

pub mod accounts;
pub mod art;
pub mod auto_update;
pub mod build_history;
pub mod client_history;
pub mod client_names;
#[macro_use]
mod counters;
pub mod counters_kept;
pub mod delete_game;
pub mod dns;
pub mod download;
pub mod evict;
pub mod fetch;
pub mod health;
pub mod hidden;
pub mod import_nginx;
pub mod invites;
pub mod kept_builds;
pub mod lancache_read;
pub mod lifetime;
pub mod look;
pub mod nginx_file;
pub mod once_per;
pub mod origin;
pub mod passthrough;
pub mod patrol;
pub mod prefill;
pub mod prefill_runs;
pub mod private_games;
pub mod prune_segments;
pub mod releases;
pub mod rounds;
pub mod serve;
pub mod sessions;
pub mod shutdown;
pub mod stash_put;
pub mod status;
pub mod verify;
pub mod via_cache;

#[cfg(feature = "test-support")]
pub mod test_support;
