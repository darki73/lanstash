//! lanstash adapter for the operating system: running a program with a time
//! limit ([`command`]), sleeping ([`pause`]), asking systemd ([`systemd`]), and asking the person
//! at the terminal ([`terminal`]).

#![forbid(unsafe_code)]

pub mod command;
pub mod pause;
pub mod process;
pub mod systemd;
pub mod terminal;
