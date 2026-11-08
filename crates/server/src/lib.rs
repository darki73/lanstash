//! The pure parts of the composition root (configuration, command line,
//! reports), as a library so `tests/` can reach them. The wiring itself is in
//! the binary (`main.rs`, `run/`).

#![forbid(unsafe_code)]

pub mod cli;
pub mod config;
pub mod dns;
pub mod install;
pub mod prefill_cli;
pub mod report;
pub mod settings;
