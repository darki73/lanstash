//! Epic Games' use cases over the ports: the accounts lanstash signs in to
//! ([`accounts`]: a code from Epic's own page traded for tokens, each
//! account's refresh token kept and its access token renewed), and their
//! library ([`library`]: each account's games, their titles and images, kept
//! between runs). No I/O of its own: Epic's web APIs, the secrets, the kept
//! state, the clock and the pause are ports.

#![forbid(unsafe_code)]

pub mod accounts;
pub mod builds;
pub mod library;
pub mod prefill;
pub mod prune;
pub mod saved;
pub mod sizes;
