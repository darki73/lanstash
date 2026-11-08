//! Battle.net's domain: pure logic over plain values and byte slices.
//! `no_std` (+ `alloc`), so the compiler guarantees it does no I/O.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod blte;
pub mod catalog;
pub mod check;
pub mod download;
pub mod locate;
pub mod nginx_keys;
pub mod plan;
pub mod profile;
pub mod tact;
