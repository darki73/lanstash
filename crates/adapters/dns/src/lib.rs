//! lanstash adapter for DNS: names resolved through the configured upstream
//! servers, never through the host's resolver (which points at lanstash); and
//! what the LAN calls a client's address, asked of the LAN's own servers.

#![forbid(unsafe_code)]

pub mod resolver;
pub mod reverse;
pub mod udp;
