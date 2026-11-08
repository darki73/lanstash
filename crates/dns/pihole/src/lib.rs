//! lanstash's Pi-hole DNS server: the [`lanstash_ports::dns::DnsServer`] over
//! Pi-hole's v6 API ([`server`]), the reply rules it writes and reads back
//! ([`rules`]). It speaks to Pi-hole only through the
//! [`lanstash_ports::web::Web`] port, so it does no I/O of its own.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod rules;
pub mod server;
