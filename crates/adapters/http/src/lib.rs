//! lanstash adapter for HTTP over TCP: the server that hands requests to a
//! [`lanstash_app::serve::RequestHandler`] over plain TCP or TLS ([`server`],
//! [`link`]), the client that
//! reaches the origins ([`upstream`]), the client of web APIs on the network
//! ([`web`]), and the HTTPS passthrough that relays TLS by its server name
//! ([`tls_passthrough`]).

#![forbid(unsafe_code)]

pub mod link;
pub mod server;
pub mod tls_passthrough;
pub mod upstream;
pub mod web;
pub mod web_tls;
