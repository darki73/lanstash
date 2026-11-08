//! Steam's connection manager (CM) as lanstash reaches it: a WebSocket over
//! TLS ([`wss`]: rustls, Mozilla's roots) carrying binary messages
//! ([`frame`]: RFC 6455), served as the `Link`/`Dialer` ports ([`link`]),
//! and secure random bytes ([`random`]). The Steam messages themselves are
//! the Steam platform's.

#![forbid(unsafe_code)]

pub mod frame;
pub mod link;
pub mod random;
pub mod wss;
