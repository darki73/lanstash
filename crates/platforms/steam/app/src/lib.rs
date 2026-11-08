//! Steam's use cases over the ports: a connection to a CM that sends,
//! receives and calls services ([`connection`]), logging in once with a
//! password and Steam Guard and keeping the refresh token ([`login`]), and
//! logging on with that token ([`logon`]), signing in by QR code
//! ([`qr_login`]) and the account the service shows, its QR sign-in on a
//! thread ([`account`]). No I/O of its own: the link, the pause,
//! the prompt, the randomness and the secrets are ports.

#![forbid(unsafe_code)]

pub mod account;
pub mod builds;
pub mod chunk_lists;
pub mod connection;
pub mod inspect;
pub mod lancache;
pub mod library;
pub mod login;
pub mod logon;
pub mod manifests;
pub mod prefill;
pub mod prune;
pub mod qr_login;
pub mod raw;
pub mod read;
pub mod session;
