//! Saying hello to a CM, logging on, and what it answers.

use super::message::{CmError, Header, Message, emsg};
use super::proto::{Writer, fields};

/// The protocol version lanstash speaks (the Steam client's current one).
pub const PROTOCOL_VERSION: u32 = 65580;

/// Public universe, anonymous user account, instance 0: the SteamID of an anonymous logon.
pub const ANONYMOUS: u64 = 0x01A0_0000_0000_0000;

/// `CMsgClientHello` (1 `protocol_version`): the first message on a connection.
#[must_use]
pub fn hello() -> Message {
    let body = Writer::new().varint(1, u64::from(PROTOCOL_VERSION)).finish();
    Message::new(emsg::CLIENT_HELLO, Header::default(), body)
}

/// `CMsgClientLogon` for an anonymous user (1 `protocol_version`); enough
/// for public app info and depot manifests of free apps.
#[must_use]
pub fn anonymous_logon() -> Message {
    let body = Writer::new().varint(1, u64::from(PROTOCOL_VERSION)).finish();
    Message::new(emsg::CLIENT_LOGON, Header { steamid: ANONYMOUS, ..Header::default() }, body)
}

/// `CMsgClientHeartBeat`, sent every `heartbeat_seconds` once logged on.
#[must_use]
pub fn heartbeat(session: &Header) -> Message {
    let header = Header { steamid: session.steamid, session: session.session, ..Header::default() };
    Message::new(emsg::CLIENT_HEART_BEAT, header, alloc::vec::Vec::new())
}

/// `CMsgClientLogonResponse`, the fields lanstash uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogonResponse {
    /// Field 1 (EResult; 1 is OK).
    pub eresult: i32,
    /// Field 3: how often to send a heartbeat.
    pub heartbeat_seconds: i32,
    /// Field 7: the cell (region) Steam put this client in.
    pub cell_id: u32,
}

impl LogonResponse {
    /// Reads the body of a ClientLogOnResponse.
    ///
    /// # Errors
    /// The body is not a protobuf.
    pub fn decode(body: &[u8]) -> Result<Self, CmError> {
        let mut r = Self { eresult: 2, heartbeat_seconds: 0, cell_id: 0 };
        for f in fields(body) {
            match f.map_err(|()| CmError::Malformed("logon response"))? {
                (1, v) => r.eresult = v.int().unwrap_or(2) as i32,
                (3, v) => r.heartbeat_seconds = v.int().unwrap_or(0) as i32,
                (7, v) => r.cell_id = v.int().unwrap_or(0) as u32,
                _ => {}
            }
        }
        Ok(r)
    }
}
