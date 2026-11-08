//! Use case: signing in to lanstash and staying signed in. A sign-in with a right name and password
//! makes a session: a token of 32 random bytes the browser keeps (as a cookie), of which lanstash
//! keeps only the SHA-256 (the secret `sessions`: `<digest> <name> <until ms>` a line), so what is
//! kept cannot be used to sign in. A session lasts [`LIFE_MS`] from its last use (it is kept again
//! when it has a day more to live) and outlives a restart. Its role is the account's as it is now:
//! a session of an account taken away is none, and taking an account away or changing its password
//! ends its sessions. After a wrong sign-in an address waits before it may try again: 1 s, twice as
//! long each time, at most [`MAX_WAIT_MS`]; a right one clears that.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::accounts::{Role, hex};
use lanstash_ports::clock::Clock;
use lanstash_ports::digest::Digest;
use lanstash_ports::random::Random;
use lanstash_ports::secrets::Secrets;

use crate::accounts::Accounts;

/// The secret the sessions are kept under.
pub const KEPT: &str = "sessions";
/// How long a session lasts after its last use.
pub const LIFE_MS: u64 = 30 * DAY_MS;
/// The longest wait after wrong sign-ins.
pub const MAX_WAIT_MS: u64 = 30_000;

const DAY_MS: u64 = 24 * 3_600_000;
const FIRST_WAIT_MS: u64 = 1_000;

/// Why a sign-in did not make a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInError {
    /// The address must wait this long before it tries again.
    Wait(u64),
    /// The name or the password is wrong (which, it does not say).
    Wrong,
    /// The session could not be kept.
    NotKept(String),
}

#[derive(Debug, Clone)]
struct Session {
    digest: String,
    name: String,
    until_ms: u64,
}

/// Sign-ins and the sessions they made.
pub struct Sessions {
    accounts: Arc<Accounts>,
    secrets: Arc<dyn Secrets>,
    random: Arc<dyn Random>,
    digest: Arc<dyn Digest>,
    clock: Arc<dyn Clock>,
    live: Mutex<Vec<Session>>,
    /// Per address: wrong sign-ins in a row, and until when it waits.
    waits: Mutex<BTreeMap<String, (u32, u64)>>,
}

impl std::fmt::Debug for Sessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.live.lock().unwrap_or_else(PoisonError::into_inner).len();
        f.debug_struct("Sessions").field("sessions", &n).finish_non_exhaustive()
    }
}

fn parse(text: &str, now_ms: u64) -> Vec<Session> {
    text.lines()
        .filter_map(|l| {
            let mut p = l.split_whitespace();
            let (digest, name, until) = (p.next()?, p.next()?, p.next()?.parse().ok()?);
            (until > now_ms).then(|| Session { digest: digest.into(), name: name.into(), until_ms: until })
        })
        .collect()
}

impl Sessions {
    /// The sessions `secrets` keeps (those run out are dropped), for the accounts of `accounts`.
    ///
    /// # Errors
    /// They are kept but could not be read.
    pub fn new(
        accounts: Arc<Accounts>,
        secrets: Arc<dyn Secrets>,
        random: Arc<dyn Random>,
        digest: Arc<dyn Digest>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, String> {
        let live = parse(&secrets.load(KEPT)?.unwrap_or_default(), clock.now_ms());
        Ok(Self {
            accounts,
            secrets,
            random,
            digest,
            clock,
            live: Mutex::new(live),
            waits: Mutex::new(BTreeMap::new()),
        })
    }

    /// The accounts sessions are of.
    #[must_use]
    pub fn accounts(&self) -> &Accounts {
        &self.accounts
    }

    fn digest_of(&self, token: &str) -> String {
        hex(&self.digest.sha256(token.as_bytes()))
    }

    fn keep(&self, live: &[Session]) -> Result<(), String> {
        let text: String = live.iter().map(|s| format!("{} {} {}\n", s.digest, s.name, s.until_ms)).collect();
        self.secrets.save(KEPT, &text)
    }

    /// Signs `name` in from `from` (an address): a new session's token and the account's role.
    ///
    /// # Errors
    /// The address must wait, the name or password is wrong, or the session could not be kept.
    pub fn sign_in(&self, from: &str, name: &str, password: &str) -> Result<(String, Role), SignInError> {
        let now = self.clock.now_ms();
        let waited = self.waits.lock().unwrap_or_else(PoisonError::into_inner).get(from).copied();
        if let Some((_, until)) = waited
            && until > now
        {
            return Err(SignInError::Wait(until - now));
        }
        let Some(role) = self.accounts.check(name, password) else {
            let mut waits = self.waits.lock().unwrap_or_else(PoisonError::into_inner);
            let fails = waits.get(from).map_or(0, |w| w.0) + 1;
            let wait = (FIRST_WAIT_MS << (fails - 1).min(15)).min(MAX_WAIT_MS);
            waits.insert(from.into(), (fails, now + wait));
            return Err(SignInError::Wrong);
        };
        self.waits.lock().unwrap_or_else(PoisonError::into_inner).remove(from);
        Ok((self.start(name).map_err(SignInError::NotKept)?, role))
    }

    /// A new session of `name`, which has just shown who it is (a password, an invite): its token.
    ///
    /// # Errors
    /// No token could be drawn, or the session could not be kept.
    pub fn start(&self, name: &str) -> Result<String, String> {
        let now = self.clock.now_ms();
        let mut bytes = [0u8; 32];
        self.random.fill(&mut bytes)?;
        let token = hex(&bytes);
        let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        live.retain(|s| s.until_ms > now);
        live.push(Session { digest: self.digest_of(&token), name: name.into(), until_ms: now + LIFE_MS });
        self.keep(&live)?;
        Ok(token)
    }

    /// Whether `from` must wait before it tries again (a wrong sign-in or invite); how long (ms).
    #[must_use]
    pub fn waiting(&self, from: &str) -> Option<u64> {
        let now = self.clock.now_ms();
        let w = self.waits.lock().unwrap_or_else(PoisonError::into_inner).get(from).copied()?;
        (w.1 > now).then(|| w.1 - now)
    }

    /// Counts a wrong try from `from` (a bad invite): it waits longer before the next.
    pub fn failed(&self, from: &str) {
        let now = self.clock.now_ms();
        let mut waits = self.waits.lock().unwrap_or_else(PoisonError::into_inner);
        let fails = waits.get(from).map_or(0, |w| w.0) + 1;
        waits.insert(from.into(), (fails, now + (FIRST_WAIT_MS << (fails - 1).min(15)).min(MAX_WAIT_MS)));
    }

    /// The account a token's session is of, and its role now; none for no live session (or an
    /// account taken away). A use lengthens the session.
    #[must_use]
    pub fn who(&self, token: &str) -> Option<(String, Role)> {
        let now = self.clock.now_ms();
        let digest = self.digest_of(token);
        let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        let s = live.iter_mut().find(|s| s.digest == digest && s.until_ms > now)?;
        let name = s.name.clone();
        if s.until_ms < now + LIFE_MS - DAY_MS {
            s.until_ms = now + LIFE_MS;
            let _ = self.keep(&live);
        }
        drop(live);
        let role = self.accounts.list().into_iter().find(|(n, _)| *n == name)?.1;
        Some((name, role))
    }

    /// Ends a token's session.
    pub fn sign_out(&self, token: &str) {
        let digest = self.digest_of(token);
        let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        let before = live.len();
        live.retain(|s| s.digest != digest);
        if live.len() != before {
            let _ = self.keep(&live);
        }
    }

    /// Ends every session of `name` (its password changed, or it was taken away).
    pub fn forget(&self, name: &str) {
        let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        live.retain(|s| s.name != name);
        let _ = self.keep(&live);
    }
}
