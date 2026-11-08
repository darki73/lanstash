//! Steam's connection-manager protocol: the protobuf wire format
//! ([`proto`]), message framing and Multi ([`message`]), hello and logon
//! ([`logon`]), the authentication service ([`auth`]), the server list
//! ([`servers`]), result codes ([`eresult`]). Pure: bytes in, bytes out; the WebSocket is an adapter's.

pub mod auth;
pub mod content;
pub mod eresult;
pub mod logon;
pub mod message;
pub mod pics;
pub mod proto;
pub mod servers;
