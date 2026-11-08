//! Steam's authentication service, called as service methods over the CM
//! connection (EMsg 9804 out, 147 back, matched by job id): the password's
//! RSA key, a session from account name and encrypted password, a Steam
//! Guard code, polling for the tokens, and a logon with the refresh token.
//!
//! Field numbers follow Steam's `steammessages_auth.steamclient.proto` and
//! `steammessages_clientserver_login.proto`. The password's RSA key is a
//! 2048-bit modulus with exponent 010001; a wrong password gets eresult 5
//! (InvalidPassword), a malformed request 8 (InvalidParam).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::logon::PROTOCOL_VERSION;
use super::message::{CmError, Header, Message, emsg};
use super::proto::{Wire, Writer, fields};

/// Service method names.
pub mod method {
    pub const GET_PASSWORD_RSA_PUBLIC_KEY: &str = "Authentication.GetPasswordRSAPublicKey#1";
    pub const BEGIN_AUTH_SESSION_VIA_CREDENTIALS: &str = "Authentication.BeginAuthSessionViaCredentials#1";
    pub const UPDATE_AUTH_SESSION_WITH_STEAM_GUARD_CODE: &str = "Authentication.UpdateAuthSessionWithSteamGuardCode#1";
    pub const POLL_AUTH_SESSION_STATUS: &str = "Authentication.PollAuthSessionStatus#1";
    pub const BEGIN_AUTH_SESSION_VIA_QR: &str = "Authentication.BeginAuthSessionViaQR#1";
}

/// `EAuthTokenPlatformType::SteamClient`.
const PLATFORM_STEAM_CLIENT: u64 = 1;
/// `ESessionPersistence::Persistent`.
const PERSISTENT: u64 = 1;
/// `EOSType::LinuxUnknown` (-203), sent sign-extended as an int32 varint.
const OS_LINUX: i64 = -203;

/// A public individual account with no account id yet (universe 1, type 1, instance 1).
pub const INDIVIDUAL: u64 = 0x0110_0001_0000_0000;

/// A service call with job id `job`.
#[must_use]
pub fn call(name: &str, job: u64, body: Vec<u8>) -> Message {
    let header = Header { job_source: job, target_job_name: Some(name.to_string()), ..Header::default() };
    Message::new(emsg::SERVICE_METHOD_CALL_FROM_CLIENT_NON_AUTHED, header, body)
}

/// `CAuthentication_GetPasswordRSAPublicKey_Request` (1 `account_name`).
#[must_use]
pub fn rsa_key_request(account: &str) -> Vec<u8> {
    Writer::new().string(1, account).finish()
}

fn each<'a>(body: &'a [u8], what: &'static str, mut f: impl FnMut(u32, Wire<'a>)) -> Result<(), CmError> {
    for field in fields(body) {
        let (n, v) = field.map_err(|()| CmError::Malformed(what))?;
        f(n, v);
    }
    Ok(())
}

fn text(v: Wire<'_>) -> String {
    v.str().unwrap_or_default().to_string()
}

/// The password's key: hex modulus (1), hex exponent (2), and the timestamp (3) to send back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RsaKey {
    pub modulus_hex: String,
    pub exponent_hex: String,
    pub timestamp: u64,
}

impl RsaKey {
    /// # Errors
    /// Not a protobuf, or no modulus.
    pub fn decode(body: &[u8]) -> Result<Self, CmError> {
        let mut k = Self { modulus_hex: String::new(), exponent_hex: String::new(), timestamp: 0 };
        each(body, "rsa key", |n, v| match n {
            1 => k.modulus_hex = text(v),
            2 => k.exponent_hex = text(v),
            3 => k.timestamp = v.int().unwrap_or(0),
            _ => {}
        })?;
        if k.modulus_hex.is_empty() {
            return Err(CmError::Malformed("rsa key without a modulus"));
        }
        Ok(k)
    }
}

/// `CAuthentication_BeginAuthSessionViaCredentials_Request`. `device` names
/// this machine in the account's device list; `encrypted_password` is the
/// base64 of the RSA ciphertext.
#[must_use]
pub fn begin_session(device: &str, account: &str, encrypted_password: &str, key_timestamp: u64) -> Vec<u8> {
    let details = Writer::new().string(1, device).varint(2, PLATFORM_STEAM_CLIENT).varint(3, OS_LINUX as u64).finish();
    Writer::new()
        .string(1, device)
        .string(2, account)
        .string(3, encrypted_password)
        .varint(4, key_timestamp)
        .bool(5, true)
        .varint(6, PLATFORM_STEAM_CLIENT)
        .varint(7, PERSISTENT)
        .string(8, "Client")
        .bytes(9, &details)
        .finish()
}

/// How Steam wants the session confirmed (`EAuthSessionGuardType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// 1: nothing more is needed.
    None,
    /// 2: a code sent by email.
    EmailCode,
    /// 3: a code from the Steam Guard mobile authenticator.
    DeviceCode,
    /// 4: a tap in the mobile app.
    DeviceConfirmation,
    /// 5: a link in an email.
    EmailConfirmation,
    Other(u64),
}

impl Guard {
    fn of(v: u64) -> Self {
        match v {
            1 => Self::None,
            2 => Self::EmailCode,
            3 => Self::DeviceCode,
            4 => Self::DeviceConfirmation,
            5 => Self::EmailConfirmation,
            o => Self::Other(o),
        }
    }

    /// The number sent with a code (`code_type`).
    #[must_use]
    pub fn number(self) -> u64 {
        match self {
            Self::None => 1,
            Self::EmailCode => 2,
            Self::DeviceCode => 3,
            Self::DeviceConfirmation => 4,
            Self::EmailConfirmation => 5,
            Self::Other(o) => o,
        }
    }
}

/// `CAuthentication_BeginAuthSessionViaCredentials_Response`.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// 1.
    pub client_id: u64,
    /// 2.
    pub request_id: Vec<u8>,
    /// 3, seconds between polls.
    pub interval: f32,
    /// 4, each a `CAuthentication_AllowedConfirmation` (1 `confirmation_type`).
    pub confirmations: Vec<Guard>,
    /// 5.
    pub steamid: u64,
}

impl Session {
    /// # Errors
    /// Not a protobuf, or a confirmation without its type.
    pub fn decode(body: &[u8]) -> Result<Self, CmError> {
        let mut s = Self { client_id: 0, request_id: Vec::new(), interval: 5.0, confirmations: Vec::new(), steamid: 0 };
        let mut bad = false;
        each(body, "auth session", |n, v| match (n, v) {
            (1, v) => s.client_id = v.int().unwrap_or(0),
            (2, v) => s.request_id = v.bytes().unwrap_or_default().to_vec(),
            (3, Wire::Fixed32(f)) => s.interval = f32::from_bits(f),
            (4, v) => {
                let inner = v.bytes().unwrap_or_default();
                let kind = fields(inner).filter_map(Result::ok).find(|(n, _)| *n == 1).and_then(|(_, v)| v.int());
                match kind {
                    Some(k) => s.confirmations.push(Guard::of(k)),
                    None => bad = true,
                }
            }
            (5, v) => s.steamid = v.int().unwrap_or(0),
            _ => {}
        })?;
        if bad {
            return Err(CmError::Malformed("allowed confirmation"));
        }
        Ok(s)
    }
}

/// `CAuthentication_UpdateAuthSessionWithSteamGuardCode_Request`.
#[must_use]
pub fn guard_code(session: &Session, code: &str, kind: Guard) -> Vec<u8> {
    Writer::new()
        .varint(1, session.client_id)
        .fixed64(2, session.steamid)
        .string(3, code)
        .varint(4, kind.number())
        .finish()
}

/// `CAuthentication_PollAuthSessionStatus_Request`.
#[must_use]
pub fn poll(session: &Session) -> Vec<u8> {
    Writer::new().varint(1, session.client_id).bytes(2, &session.request_id).finish()
}

/// `CAuthentication_PollAuthSessionStatus_Response`: the tokens, once the session is confirmed.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Tokens {
    /// 3; kept (root-only) to log on again without the password.
    pub refresh_token: String,
    /// 4.
    pub access_token: String,
    /// 6.
    pub account_name: String,
}

impl core::fmt::Debug for Tokens {
    /// Never prints the tokens.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Tokens").field("account_name", &self.account_name).finish_non_exhaustive()
    }
}

impl Tokens {
    /// `Ok(None)` while the session still waits for confirmation.
    ///
    /// # Errors
    /// Not a protobuf.
    pub fn decode(body: &[u8]) -> Result<Option<Self>, CmError> {
        let mut t = Self::default();
        each(body, "poll status", |n, v| match n {
            3 => t.refresh_token = text(v),
            4 => t.access_token = text(v),
            6 => t.account_name = text(v),
            _ => {}
        })?;
        Ok((!t.refresh_token.is_empty()).then_some(t))
    }
}

/// `CAuthentication_BeginAuthSessionViaQR_Request` (1 `device_friendly_name`,
/// 2 `platform_type`, 3 `device_details`, 4 `website_id`).
#[must_use]
pub fn begin_qr(device: &str) -> Vec<u8> {
    let details = Writer::new().string(1, device).varint(2, PLATFORM_STEAM_CLIENT).varint(3, OS_LINUX as u64).finish();
    Writer::new().string(1, device).varint(2, PLATFORM_STEAM_CLIENT).bytes(3, &details).string(4, "Client").finish()
}

/// `CAuthentication_BeginAuthSessionViaQR_Response`: the session and the
/// link the QR code carries: 1 `client_id`, 2
/// `challenge_url` (`https://s.team/q/1/...`), 3 `request_id` (16 bytes), 4
/// `interval` (5.0), 5 confirmations (4 and 3), 6 `version`.
#[derive(Debug, Clone, PartialEq)]
pub struct QrSession {
    pub session: Session,
    pub challenge_url: String,
}

impl QrSession {
    /// # Errors
    /// Not a protobuf, or no link.
    pub fn decode(body: &[u8]) -> Result<Self, CmError> {
        let mut s =
            Session { client_id: 0, request_id: Vec::new(), interval: 5.0, confirmations: Vec::new(), steamid: 0 };
        let mut url = String::new();
        each(body, "qr session", |n, v| match (n, v) {
            (1, v) => s.client_id = v.int().unwrap_or(0),
            (2, v) => url = text(v),
            (3, v) => s.request_id = v.bytes().unwrap_or_default().to_vec(),
            (4, Wire::Fixed32(f)) => s.interval = f32::from_bits(f),
            (5, v) => {
                let inner = v.bytes().unwrap_or_default();
                if let Some(k) = fields(inner).filter_map(Result::ok).find(|(n, _)| *n == 1).and_then(|(_, v)| v.int())
                {
                    s.confirmations.push(Guard::of(k));
                }
            }
            _ => {}
        })?;
        if url.is_empty() {
            return Err(CmError::Malformed("qr session without a link"));
        }
        Ok(Self { session: s, challenge_url: url })
    }
}

/// `CAuthentication_PollAuthSessionStatus_Response`, all of it lanstash uses:
/// a new QR link (2) and client id (1) when Steam rotates the code, whether
/// a phone has scanned it (5), and the tokens once approved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PollStatus {
    pub new_client_id: Option<u64>,
    pub new_challenge_url: Option<String>,
    pub had_remote_interaction: bool,
    pub tokens: Option<Tokens>,
}

impl PollStatus {
    /// # Errors
    /// Not a protobuf.
    pub fn decode(body: &[u8]) -> Result<Self, CmError> {
        let mut p = Self::default();
        each(body, "poll status", |n, v| match n {
            1 => p.new_client_id = v.int(),
            2 => p.new_challenge_url = v.str().filter(|s| !s.is_empty()).map(ToString::to_string),
            5 => p.had_remote_interaction = v.int() == Some(1),
            _ => {}
        })?;
        p.tokens = Tokens::decode(body)?;
        Ok(p)
    }
}

/// `CMsgClientLogon` with a refresh token (1 `protocol_version`, 6
/// `client_language`, 7 `client_os_type`, 8 `should_remember_password`, 50
/// `account_name`, 108 `access_token`), as an individual account.
#[must_use]
pub fn token_logon(account: &str, refresh_token: &str) -> Message {
    let body = Writer::new()
        .varint(1, u64::from(PROTOCOL_VERSION))
        .string(6, "english")
        .varint(7, OS_LINUX as u64)
        .bool(8, true)
        .string(50, account)
        .string(108, refresh_token)
        .finish();
    Message::new(emsg::CLIENT_LOGON, Header { steamid: INDIVIDUAL, ..Header::default() }, body)
}
