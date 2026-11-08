//! lanstash adapter for logs: files that rotate by size ([`rotating_file`]),
//! the event log ([`event_log`]) and the HTTP server's access log
//! ([`access_log`]).

#![forbid(unsafe_code)]

pub mod access_log;
pub mod event_log;
pub mod rotating_file;
