//! Messages to and from Steam's connection managers (CMs). Each is
//! `u32 LE (EMsg | 0x8000_0000)`, `u32 LE` header length, a
//! `CMsgProtoBufHeader`, then the body (a protobuf of the EMsg's type).
//! A Multi (EMsg 1) carries several messages, maybe gzipped, each with a
//! `u32 LE` length in front.
//!
//! Message numbers and field numbers follow Steam's protobuf definitions.
//! An anonymous logon is answered with a Multi holding EMsg 5501 and the
//! logon response, with `steamid` and `session` set.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use lanstash_domain::inflate::{InflateError, gzip};

use super::proto::{Writer, fields};

/// Message numbers (EMsg).
pub mod emsg {
    pub const MULTI: u32 = 1;
    pub const SERVICE_METHOD_CALL_FROM_CLIENT: u32 = 151;
    pub const SERVICE_METHOD_RESPONSE: u32 = 147;
    pub const CLIENT_HEART_BEAT: u32 = 703;
    pub const CLIENT_LOG_ON_RESPONSE: u32 = 751;
    pub const CLIENT_LOGGED_OFF: u32 = 757;
    pub const CLIENT_LOGON: u32 = 5514;
    pub const SERVICE_METHOD_CALL_FROM_CLIENT_NON_AUTHED: u32 = 9804;
    pub const CLIENT_HELLO: u32 = 9805;
}

const PROTO: u32 = 0x8000_0000;

/// A job id that means "none".
pub const NO_JOB: u64 = u64::MAX;

/// Why a message could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CmError {
    /// Shorter than its own lengths say.
    Truncated,
    /// An old-style (not protobuf) message, of this EMsg.
    NotProtobuf(u32),
    /// A protobuf that does not parse.
    Malformed(&'static str),
    /// A Multi whose gzip does not decompress.
    Inflate(InflateError),
}

/// `CMsgProtoBufHeader`, the fields lanstash uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Field 1, fixed64.
    pub steamid: u64,
    /// Field 2, int32.
    pub session: i32,
    /// Field 10, fixed64: the sender's job, echoed back as `job_target`.
    pub job_source: u64,
    /// Field 11, fixed64.
    pub job_target: u64,
    /// Field 12: a service method, e.g. `Authentication.GetPasswordRSAPublicKey#1`.
    pub target_job_name: Option<String>,
    /// Field 13, int32 (EResult; 1 is OK, 2 the default "fail").
    pub eresult: i32,
    /// Field 14.
    pub error_message: Option<String>,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            steamid: 0,
            session: 0,
            job_source: NO_JOB,
            job_target: NO_JOB,
            target_job_name: None,
            eresult: 2,
            error_message: None,
        }
    }
}

impl Header {
    fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new().fixed64(1, self.steamid);
        if self.session != 0 {
            w = w.varint(2, i64::from(self.session) as u64);
        }
        if self.job_source != NO_JOB {
            w = w.fixed64(10, self.job_source);
        }
        if self.job_target != NO_JOB {
            w = w.fixed64(11, self.job_target);
        }
        if let Some(name) = &self.target_job_name {
            w = w.string(12, name);
        }
        if self.eresult != 2 {
            w = w.varint(13, i64::from(self.eresult) as u64);
        }
        if let Some(e) = &self.error_message {
            w = w.string(14, e);
        }
        w.finish()
    }

    fn decode(data: &[u8]) -> Result<Self, CmError> {
        let mut h = Self::default();
        for f in fields(data) {
            let (n, v) = f.map_err(|()| CmError::Malformed("header"))?;
            match n {
                1 => h.steamid = v.int().unwrap_or(0),
                2 => h.session = v.int().unwrap_or(0) as i32,
                10 => h.job_source = v.int().unwrap_or(NO_JOB),
                11 => h.job_target = v.int().unwrap_or(NO_JOB),
                12 => h.target_job_name = v.str().map(ToString::to_string),
                13 => h.eresult = v.int().unwrap_or(2) as i32,
                14 => h.error_message = v.str().map(ToString::to_string),
                _ => {}
            }
        }
        Ok(h)
    }
}

/// One message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub emsg: u32,
    pub header: Header,
    pub body: Vec<u8>,
}

impl Message {
    /// A message of `emsg` with `header` and `body`.
    #[must_use]
    pub fn new(emsg: u32, header: Header, body: Vec<u8>) -> Self {
        Self { emsg, header, body }
    }

    /// The bytes sent to a CM.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let header = self.header.encode();
        let mut m = Vec::with_capacity(8 + header.len() + self.body.len());
        m.extend_from_slice(&(self.emsg | PROTO).to_le_bytes());
        m.extend_from_slice(&(header.len() as u32).to_le_bytes());
        m.extend_from_slice(&header);
        m.extend_from_slice(&self.body);
        m
    }

    /// Reads one message.
    ///
    /// # Errors
    /// [`CmError`]; an old-style message gives `NotProtobuf` with its EMsg.
    pub fn decode(data: &[u8]) -> Result<Self, CmError> {
        let raw =
            u32::from_le_bytes(data.get(..4).ok_or(CmError::Truncated)?.try_into().map_err(|_| CmError::Truncated)?);
        let emsg = raw & !PROTO;
        if raw & PROTO == 0 {
            return Err(CmError::NotProtobuf(emsg));
        }
        let len =
            u32::from_le_bytes(data.get(4..8).ok_or(CmError::Truncated)?.try_into().map_err(|_| CmError::Truncated)?);
        let end = 8usize.checked_add(len as usize).ok_or(CmError::Truncated)?;
        let header = Header::decode(data.get(8..end).ok_or(CmError::Truncated)?)?;
        Ok(Self { emsg, header, body: data[end..].to_vec() })
    }
}

/// The messages inside a Multi's body (`CMsgMulti`: 1 `size_unzipped`,
/// 2 `message_body`, gzipped when the size is not 0), at most `max` bytes
/// unzipped.
///
/// # Errors
/// [`CmError`].
pub fn unpack_multi(body: &[u8], max: usize) -> Result<Vec<Vec<u8>>, CmError> {
    let (mut unzipped, mut packed) = (0u64, &[][..]);
    for f in fields(body) {
        match f.map_err(|()| CmError::Malformed("multi"))? {
            (1, v) => unzipped = v.int().unwrap_or(0),
            (2, v) => packed = v.bytes().ok_or(CmError::Malformed("multi body"))?,
            _ => {}
        }
    }
    let plain;
    let mut rest = if unzipped == 0 {
        packed
    } else {
        plain = gzip(packed, max).map_err(CmError::Inflate)?;
        if plain.len() as u64 != unzipped {
            return Err(CmError::Malformed("multi size"));
        }
        &plain[..]
    };
    let mut out = Vec::new();
    while !rest.is_empty() {
        let n = rest.get(..4).ok_or(CmError::Truncated)?;
        let n = u32::from_le_bytes([n[0], n[1], n[2], n[3]]) as usize;
        let m = rest.get(4..4 + n).ok_or(CmError::Truncated)?;
        out.push(m.to_vec());
        rest = &rest[4 + n..];
    }
    Ok(out)
}
