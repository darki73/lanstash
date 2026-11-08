//! lanstash adapter for the admin port: the HTTP API and the built-in status
//! page ([`api`]), its routes ([`route`]) and generic documents
//! ([`render`]), the hidden games masked for anyone not an admin
//! ([`mask`]), and the client the commands use to reach a running
//! lanstash ([`client`]).

#![forbid(unsafe_code)]

pub mod anon;
pub mod api;
pub mod client;
pub mod guard;
pub mod mask;
pub mod render;
pub mod route;
