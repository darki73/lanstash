//! Riot: which requests are cached ([`profile`]), and the platform the
//! composition root registers ([`platform`]). `no_std`: pure logic, no I/O.

#![no_std]
#![forbid(unsafe_code)]

pub mod platform;
pub mod profile;
