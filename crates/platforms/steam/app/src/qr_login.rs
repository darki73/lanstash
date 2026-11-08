//! Signing in by QR code: Steam hands out a link (`https://s.team/q/...`),
//! the Steam mobile app scans it and the person approves; polling meanwhile
//! follows the link as Steam rotates it, and ends with the refresh token,
//! kept like a password login's. No password is involved. The caller paces
//! the polls ([`QrLogin::interval_ms`]).

use lanstash_ports::secrets::Secrets;
use lanstash_steam_domain::cm::auth::{PollStatus, QrSession, Session, begin_qr, method, poll};

use crate::connection::Connection;
use crate::login::keep;

/// A sign-in waiting for its QR code to be scanned and approved.
#[derive(Debug, Clone, PartialEq)]
pub struct QrLogin {
    session: Session,
    /// The link the QR code shows (it changes as Steam rotates it).
    pub url: String,
    /// Whether a phone has scanned it.
    pub scanned: bool,
}

/// Where a sign-in stands after a poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Not approved yet.
    Waiting,
    /// Approved; the token is kept; the account name.
    Done(String),
}

impl QrLogin {
    /// Asks Steam for a QR sign-in for `device` (this machine's name in the account's device list).
    ///
    /// # Errors
    /// The connection, or Steam's answer.
    pub fn start(conn: &mut Connection, device: &str) -> Result<Self, String> {
        let answer = conn.call(method::BEGIN_AUTH_SESSION_VIA_QR, begin_qr(device)).map_err(|e| e.to_string())?;
        let q = QrSession::decode(&answer.body).map_err(|e| format!("{e:?}"))?;
        Ok(Self { session: q.session, url: q.challenge_url, scanned: false })
    }

    /// How long to wait between polls (what Steam asked for, 1 to 30 s; 5 s for an answer
    /// that is not a number).
    #[must_use]
    pub fn interval_ms(&self) -> u64 {
        let asked = self.session.interval;
        let s = if asked.is_finite() { asked.clamp(1.0, 30.0) } else { 5.0 };
        (f64::from(s) * 1000.0) as u64
    }

    /// Looks once whether the sign-in was approved; keeps the token when it was.
    ///
    /// # Errors
    /// The connection, Steam's answer (an expired code), or the secrets.
    pub fn poll(&mut self, conn: &mut Connection, secrets: &dyn Secrets) -> Result<Step, String> {
        let answer = conn.call(method::POLL_AUTH_SESSION_STATUS, poll(&self.session)).map_err(|e| e.to_string())?;
        let p = PollStatus::decode(&answer.body).map_err(|e| format!("{e:?}"))?;
        if let Some(id) = p.new_client_id {
            self.session.client_id = id;
        }
        if let Some(url) = p.new_challenge_url {
            self.url = url;
        }
        self.scanned |= p.had_remote_interaction;
        match p.tokens {
            Some(t) => Ok(Step::Done(keep(secrets, &t, "")?)),
            None => Ok(Step::Waiting),
        }
    }
}
