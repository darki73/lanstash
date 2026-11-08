//! lanstash ports: traits only, plus the plain values they carry.
//!
//! What the use cases need from the outside world (files, origin servers,
//! web APIs, the network's DNS server, message links, secrets, randomness, a prompt,
//! the clock, logs, names) and what the core asks of a platform. Adapters
//! and platforms implement them; the composition root wires them. `no_std`
//! (+ `alloc`), so nothing here can do I/O.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod account;
pub mod art;
pub mod clients;
pub mod clock;
pub mod digest;
pub mod dns;
pub mod events;
pub mod history;
pub mod keep;
pub mod link;
pub mod names;
pub mod nginx;
pub mod passwords;
pub mod platform;
pub mod prefill;
pub mod prompt;
pub mod random;
pub mod releases;
pub mod secrets;
pub mod settings;
pub mod stats;
pub mod storage;
pub mod upstream;
pub mod web;
