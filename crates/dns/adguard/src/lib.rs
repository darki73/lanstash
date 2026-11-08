//! lanstash's AdGuard Home DNS server: the [`lanstash_ports::dns::DnsServer`]
//! over AdGuard Home's admin API ([`server`]), how it answers a name
//! ([`check`]), and the times its query log writes ([`time`]). It speaks to AdGuard only through the
//! [`lanstash_ports::web::Web`] port, so it does no I/O of its own.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod check;
pub mod server;
pub mod time;
