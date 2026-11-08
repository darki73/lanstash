//! Epic Games: which requests are cached and under which game ([`profile`]),
//! the file that sets up a Windows PC so that lanstash keeps the launcher's
//! downloads ([`setup`]), signing in to an account ([`auth`]), and the
//! platform the composition root registers ([`platform`]). `no_std`
//! (+ `alloc`): pure logic, no I/O.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod auth;
pub mod library;
pub mod manifest;
pub mod platform;
pub mod profile;
pub mod setup;
