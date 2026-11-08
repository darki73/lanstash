//! lanstash storage layer.
//!
//! Per service and game: small whole objects in segments, large or partial
//! objects in sparse object files described by a journal, and one in-memory
//! index over all of it. All file access goes through
//! the storage ports of `lanstash-ports`; this crate does no I/O of its own.

#![forbid(unsafe_code)]

pub mod compact;
pub mod evict;
pub mod game;
pub mod index;
pub mod journal;
pub mod layout;
pub mod segment;
pub mod store;
pub mod verify;

#[cfg(feature = "test-support")]
pub mod test_support;
