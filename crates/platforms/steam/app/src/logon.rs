//! Logging on with the kept refresh token: the session every later Steam
//! request (app info, manifests) runs in.

use lanstash_ports::secrets::Secrets;
use lanstash_steam_domain::cm::auth::token_logon;
use lanstash_steam_domain::cm::eresult::{OK, describe, refuses_token};
use lanstash_steam_domain::cm::logon::{LogonResponse, heartbeat};
use lanstash_steam_domain::cm::message::{Header, emsg};

use crate::connection::Connection;
use crate::login::token;

/// A logged-on session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedOn {
    pub account: String,
    pub steamid: u64,
    pub cell_id: u32,
    pub heartbeat_seconds: i32,
}

/// Logs `conn` on as `account`, with its kept token.
///
/// # Errors
/// No token kept for it (sign in first), the connection, or Steam refusing the token.
pub fn logon(conn: &mut Connection, secrets: &dyn Secrets, account: &str) -> Result<LoggedOn, String> {
    let token = token(secrets, account)?
        .ok_or_else(|| format!("{account} is not signed in to Steam: sign in from the web UI"))?;
    conn.send(&token_logon(account.trim(), token.trim()))?;
    let m = conn.wait_for(emsg::CLIENT_LOG_ON_RESPONSE)?;
    let r = LogonResponse::decode(&m.body).map_err(|e| format!("{e:?}"))?;
    if refuses_token(r.eresult) {
        return Err(format!("Steam refused the kept token: {} ({}); sign in again", describe(r.eresult), r.eresult));
    }
    if r.eresult != OK {
        return Err(format!("Steam could not log on now: {} ({})", describe(r.eresult), r.eresult));
    }
    conn.session = Header { steamid: m.header.steamid, session: m.header.session, ..Header::default() };
    Ok(LoggedOn {
        account: account.trim().to_string(),
        steamid: m.header.steamid,
        cell_id: r.cell_id,
        heartbeat_seconds: r.heartbeat_seconds,
    })
}

/// Sends one heartbeat for the logged-on session (every `heartbeat_seconds`).
///
/// # Errors
/// The link.
pub fn beat(conn: &mut Connection) -> Result<(), String> {
    let h = heartbeat(&conn.session);
    conn.send(&h)
}
