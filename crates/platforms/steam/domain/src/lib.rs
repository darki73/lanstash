//! Steam: which requests are cached and how ([`profile`]), where a game's
//! cover image is ([`art`]), its connection-manager protocol ([`cm`]), and the
//! platform the composition root registers ([`platform`]). `no_std` (+ `alloc`): pure logic, no I/O.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod apps;
pub mod art;
pub mod builds;
pub mod catalog;
pub mod check;
pub mod cm;
pub mod kv;
pub mod manifest;
pub mod names;
pub mod platform;
pub mod profile;
pub mod share;
