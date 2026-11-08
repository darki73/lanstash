//! lanstash adapter: what lanstash keeps beyond a restart, in one SQLite
//! database (`lanstash.db` in the state directory): the build history
//! ([`lanstash_ports::history::HistoryLog`]), counters over time
//! ([`lanstash_ports::stats::StatsLog`]), the prefill jobs that ran
//! ([`lanstash_ports::prefill::PrefillLog`]) and the names given to clients
//! ([`lanstash_ports::clients::ClientLabels`]).
//!
//! One connection, behind a lock: what is kept is small and written seldom
//! (a minute's counters, an event, a finished job). The schema is made and
//! moved forward by [`db::Database::open`] (SQLite's `user_version` says how far).

#![forbid(unsafe_code)]

pub mod db;
pub mod history;
pub mod labels;
pub mod runs;
pub mod stats;
