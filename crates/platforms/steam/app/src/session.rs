//! What Steam's use cases need from outside (the CM link, the kept sign-ins,
//! HTTP to the content servers), and the Steam sessions lanstash keeps: one
//! per account signed in.
//!
//! Steam lets an account have one such session: a second logon replaces the
//! first (EResult 34, "logon session replaced"). So every use goes through
//! [`Services::with_session_as`]: the account's one logged-on connection,
//! taken in turn, opened again when it was dropped (and the work retried once).
//! Different accounts' sessions are independent.
//!
//! While a session is open a heartbeat keeps it so ([`Services::keep_alive`]),
//! as Steam's own client does: without it Steam may drop an idle connection,
//! and every use after a pause would cost a new logon. Logons are counted.
//!
//! A kept token Steam refuses is remembered, per account: it is tried again
//! only after [`RETRY_REFUSED_MS`], or at once with a new sign-in, so lanstash
//! does not knock on Steam's door every minute. After many logons Steam may
//! refuse even a fresh token ("wrong password").

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::{Clock, Pause};
use lanstash_ports::keep::Keep;
use lanstash_ports::link::Dialer;
use lanstash_ports::secrets::Secrets;
use lanstash_ports::upstream::Upstream;

use crate::chunk_lists::ChunkLists;
use crate::connection::Connection;
use crate::login::{accounts, token};
use crate::logon::{LoggedOn, beat, logon};

type Slot = Arc<Mutex<Option<(Connection, LoggedOn)>>>;

/// The ports, and the shared sessions.
#[derive(Clone)]
pub struct Services {
    pub dialer: Arc<dyn Dialer>,
    pub secrets: Arc<dyn Secrets>,
    /// Content servers, through upstream DNS like every other origin.
    pub upstream: Arc<dyn Upstream>,
    /// Manifests' chunk lists kept between runs (none kept until [`Services::with_chunk_lists`]).
    pub lists: ChunkLists,
    /// Each account's session (by its name, lower case).
    slots: Arc<Mutex<BTreeMap<String, Slot>>>,
    /// Each account's kept token Steam last refused, and when.
    refused: Arc<Mutex<BTreeMap<String, (String, u64)>>>,
    clock: Arc<dyn Clock>,
    /// Logons since start.
    logons: Arc<std::sync::atomic::AtomicU64>,
}

/// How long a refused token rests before it is tried again.
pub const RETRY_REFUSED_MS: u64 = 30 * 60 * 1000;

/// What a logon with a refused token says (and what [`Services::session`] says without trying again).
pub const REFUSED: &str =
    "Steam refuses the kept sign-in; lanstash tries again every 30 minutes (signing in again may help)";

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Services")
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Services {
    #[must_use]
    pub fn new(
        dialer: Arc<dyn Dialer>,
        secrets: Arc<dyn Secrets>,
        upstream: Arc<dyn Upstream>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let lists = ChunkLists::default();
        Self {
            dialer,
            secrets,
            upstream,
            lists,
            slots: Arc::default(),
            refused: Arc::default(),
            clock,
            logons: Arc::default(),
        }
    }

    /// Keeps manifests' chunk lists in `keep` between runs.
    #[must_use]
    pub fn with_chunk_lists(mut self, keep: Arc<dyn Keep>) -> Self {
        self.lists = ChunkLists::new(keep);
        self
    }

    /// The accounts signed in, in the order they signed in (none when the secrets cannot be read).
    #[must_use]
    pub fn accounts(&self) -> Vec<String> {
        accounts(self.secrets.as_ref()).unwrap_or_default()
    }

    fn token(&self, account: &str) -> Option<String> {
        token(self.secrets.as_ref(), account).ok().flatten()
    }

    /// Whether Steam refused `account`'s token kept now.
    #[must_use]
    pub fn token_refused(&self, account: &str) -> bool {
        let refused = lock(&self.refused).get(&account.to_ascii_lowercase()).cloned();
        refused.is_some_and(|(t, _)| Some(t) == self.token(account))
    }

    /// Whether `account`'s refused token is still resting (not to be tried yet).
    fn resting(&self, account: &str) -> bool {
        let refused = lock(&self.refused).get(&account.to_ascii_lowercase()).cloned();
        refused.is_some_and(|(t, at)| {
            Some(t) == self.token(account) && self.clock.now_ms().saturating_sub(at) < RETRY_REFUSED_MS
        })
    }

    /// Whether any account is signed in.
    #[must_use]
    pub fn signed_in(&self) -> bool {
        !self.accounts().is_empty()
    }

    /// A new connection logged on as `account` (prefer [`Self::with_session_as`]).
    ///
    /// # Errors
    /// No sign-in kept for it, the connection, or Steam refusing the token.
    pub fn session(&self, account: &str) -> Result<(Connection, LoggedOn), String> {
        if self.resting(account) {
            return Err(REFUSED.into());
        }
        let mut c = Connection::open(self.dialer.as_ref())?;
        self.logons.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let key = account.to_ascii_lowercase();
        match logon(&mut c, self.secrets.as_ref(), account) {
            Ok(on) => {
                lock(&self.refused).remove(&key);
                Ok((c, on))
            }
            Err(e) if e.starts_with("Steam refused the kept token") => {
                let now = self.clock.now_ms();
                if let Some(t) = self.token(account) {
                    lock(&self.refused).insert(key, (t, now));
                }
                Err(REFUSED.into())
            }
            Err(e) => Err(e),
        }
    }

    fn slot(&self, account: &str) -> Slot {
        Arc::clone(lock(&self.slots).entry(account.to_ascii_lowercase()).or_default())
    }

    /// Runs `work` on `account`'s shared session (opened if need be). When the connection
    /// broke under it the session is dropped and `work` is tried once more on a new
    /// one; when Steam only refused the work (a manifest it will not give, say) the
    /// error is returned and the session kept (dropping it would cost two logons per
    /// such refusal).
    ///
    /// # Errors
    /// No sign-in kept for it, the connection, or `work`'s second failure.
    pub fn with_session_as<T>(
        &self,
        account: &str,
        mut work: impl FnMut(&mut Connection, &LoggedOn) -> Result<T, String>,
    ) -> Result<T, String> {
        let slot = self.slot(account);
        let mut slot = lock(&slot);
        let mut last = String::new();
        for _ in 0..2 {
            if slot.is_none() {
                *slot = Some(self.session(account)?);
            }
            let Some((conn, on)) = slot.as_mut() else { continue };
            match work(conn, on) {
                Ok(v) => return Ok(v),
                // Steam refused this work (no manifest, access denied): the session is fine.
                Err(e) if !conn.broken() => return Err(e),
                Err(e) => {
                    *slot = None;
                    last = e;
                }
            }
        }
        Err(last)
    }

    /// Runs `work` on the session of the first account signed in (for what any account may do).
    ///
    /// # Errors
    /// No account signed in, or as [`Self::with_session_as`].
    pub fn with_session<T>(
        &self,
        work: impl FnMut(&mut Connection, &LoggedOn) -> Result<T, String>,
    ) -> Result<T, String> {
        let first = self.accounts().into_iter().next().ok_or("not signed in to Steam: sign in from the web UI")?;
        self.with_session_as(&first, work)
    }

    /// Logons since start.
    #[must_use]
    pub fn logons(&self) -> u64 {
        self.logons.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Sends one heartbeat on each open session; drops one that fails. A session in use
    /// is skipped: the work on it keeps it busy, and another account's heartbeat does not
    /// wait for it. Returns whether a session was open.
    pub fn beat(&self) -> bool {
        let slots: Vec<Slot> = lock(&self.slots).values().cloned().collect();
        let mut any = false;
        for slot in slots {
            let mut slot = match slot.try_lock() {
                Ok(s) => s,
                Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    any = true;
                    continue;
                }
            };
            let Some((conn, _)) = slot.as_mut() else { continue };
            any = true;
            if beat(conn).is_err() {
                *slot = None;
            }
        }
        any
    }

    /// Keeps the shared sessions alive on a thread of their own: a heartbeat every
    /// `every_ms` (Steam asks for one every 9 s).
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_alive(&self, pause: Arc<dyn Pause>, every_ms: u64) -> Result<(), String> {
        let me = self.clone();
        lanstash_app::rounds::forever("steam-heartbeat", Arc::clone(&pause), move || {
            pause.pause_ms(every_ms);
            me.beat();
            lanstash_app::rounds::Next::Again
        })
    }

    /// Drops `account`'s shared session (after its sign-out, say).
    pub fn forget_session(&self, account: &str) {
        lock(&self.slots).remove(&account.to_ascii_lowercase());
    }
}
