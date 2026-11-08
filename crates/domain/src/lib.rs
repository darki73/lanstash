//! lanstash domain layer.
//!
//! Pure logic only. This crate is `no_std` (+ `alloc`), so the compiler itself
//! guarantees it cannot touch files, sockets, threads, clocks or the environment.
//! Everything here works on plain values and byte slices.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod accounts;
pub mod art;
pub mod checksum;
pub mod dns;
pub mod evict;
pub mod extent;
pub mod http;
pub mod inflate;
pub mod journal;
pub mod json;
pub mod names;
pub mod nginx_cache;
pub mod object;
pub mod private;
pub mod qr;
pub mod rsa;
pub mod segment;
pub mod service;
pub mod tls;
pub mod window;
pub mod x509;
pub mod zip;
