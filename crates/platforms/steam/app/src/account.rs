//! The Steam accounts as the service shows them ([`Account`]): who is signed
//! in (from the kept secrets; several may be, each with its own token), and a
//! QR sign-in that runs on its own thread (connect, ask for a code, poll at
//! Steam's interval) while the web UI shows the code; whoever approves it is
//! added. Starting again or cancelling moves on to a new generation; a
//! thread of an older one stops at its next look.

use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::json::string;
use lanstash_domain::qr::encode;
use lanstash_domain::qr::render::svg;
use lanstash_ports::account::Account;
use lanstash_ports::clock::Pause;
use lanstash_ports::link::Dialer;
use lanstash_ports::secrets::Secrets;

use crate::connection::Connection;
use crate::login::{accounts, forget};
use crate::qr_login::{QrLogin, Step};

/// Polls before a code is given up on (about five minutes at Steam's 5 s).
const MAX_POLLS: u32 = 60;
/// How many times a code is replaced when Steam closed the connection waiting on it unscanned.
const MAX_FRESH: u32 = 3;
/// Pauses are cut into pieces this long, so a cancel is noticed soon.
const STEP_MS: u64 = 250;

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Idle,
    Starting,
    Waiting { url: String, scanned: bool },
    Failed(String),
}

#[derive(Debug)]
struct Inner {
    generation: u64,
    state: State,
}

/// What a sign-in needs from outside.
pub struct Wiring {
    pub dialer: Arc<dyn Dialer>,
    pub secrets: Arc<dyn Secrets>,
    pub pause: Arc<dyn Pause>,
    /// This machine's name in the account's device list.
    pub device: String,
    /// Whether Steam refused an account's kept sign-in (it then needs a new one).
    pub refused: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// An account was signed out: drop what is open for it (its session).
    pub signed_out: Arc<dyn Fn(&str) + Send + Sync>,
    /// How many times lanstash logged on to Steam since it started.
    pub logons: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl std::fmt::Debug for Wiring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wiring").field("device", &self.device).finish_non_exhaustive()
    }
}

/// The Steam account.
pub struct SteamAccount {
    wiring: Arc<Wiring>,
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for SteamAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamAccount").field("device", &self.wiring.device).finish_non_exhaustive()
    }
}

impl SteamAccount {
    #[must_use]
    pub fn new(wiring: Wiring) -> Self {
        Self { wiring: Arc::new(wiring), inner: Arc::new(Mutex::new(Inner { generation: 0, state: State::Idle })) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The accounts signed in, in the order they signed in.
    fn signed_in(&self) -> Vec<String> {
        accounts(self.wiring.secrets.as_ref()).unwrap_or_default()
    }
}

/// One sign-in thread: its generation, and how it reports.
struct Run {
    wiring: Arc<Wiring>,
    inner: Arc<Mutex<Inner>>,
    generation: u64,
}

impl Run {
    /// Sets the state while this run is still the current one; `false` once it is not.
    fn set(&self, state: State) -> bool {
        let mut i = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if i.generation != self.generation {
            return false;
        }
        i.state = state;
        true
    }

    fn current(&self) -> bool {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).generation == self.generation
    }

    /// Waits `ms`; `false` when this run was replaced meanwhile.
    fn wait(&self, ms: u64) -> bool {
        let mut left = ms;
        while left > 0 {
            if !self.current() {
                return false;
            }
            let step = left.min(STEP_MS);
            self.wiring.pause.pause_ms(step);
            left -= step;
        }
        self.current()
    }

    fn sign_in(&self) -> Result<(), String> {
        let w = &self.wiring;
        let mut fresh = 0;
        let mut polls = 0;
        // Steam may close a connection that only waits on a code nobody scanned yet ("the server
        // closed the WebSocket"): then a new connection, with a new code.
        'code: loop {
            let mut conn = Connection::open(w.dialer.as_ref())?;
            let mut q = QrLogin::start(&mut conn, &w.device)?;
            if !self.set(State::Waiting { url: q.url.clone(), scanned: false }) {
                return Ok(());
            }
            while polls < MAX_POLLS {
                polls += 1;
                if !self.wait(q.interval_ms()) {
                    return Ok(());
                }
                match q.poll(&mut conn, w.secrets.as_ref()) {
                    Ok(Step::Done(_)) => {
                        self.set(State::Idle);
                        return Ok(());
                    }
                    Ok(Step::Waiting) => {
                        if !self.set(State::Waiting { url: q.url.clone(), scanned: q.scanned }) {
                            return Ok(());
                        }
                    }
                    Err(_) if conn.broken() && !q.scanned && fresh < MAX_FRESH => {
                        fresh += 1;
                        continue 'code;
                    }
                    Err(e) => return Err(e),
                }
            }
            return Err("the QR code was not approved in time; start again".into());
        }
    }

    fn run(self) {
        if let Err(e) = self.sign_in() {
            self.set(State::Failed(e));
        }
    }
}

impl Account for SteamAccount {
    fn view(&self) -> String {
        let names = self.signed_in();
        let account = names.first().map_or_else(|| "null".to_string(), |a| string(a));
        let list: Vec<String> = names
            .iter()
            .map(|n| format!("{{\"name\":{},\"refused\":{}}}", string(n), (self.wiring.refused)(n)))
            .collect();
        let (state, scanned, error) = match &self.lock().state {
            State::Idle => ("idle", false, None),
            State::Starting => ("starting", false, None),
            State::Waiting { scanned, .. } => ("waiting", *scanned, None),
            State::Failed(e) => ("failed", false, Some(e.clone())),
        };
        let error = error.map_or_else(|| "null".to_string(), |e| string(&e));
        format!(
            "{{\"accounts\":[{}],\"account\":{account},\"sign_in\":{{\"state\":{},\"scanned\":{scanned},\"error\":{error}}},\"logons\":{}}}\n",
            list.join(","),
            string(state),
            (self.wiring.logons)()
        )
    }

    fn start_sign_in(&self) -> Result<(), String> {
        let generation = {
            let mut i = self.lock();
            i.generation += 1;
            i.state = State::Starting;
            i.generation
        };
        let run = Run { wiring: Arc::clone(&self.wiring), inner: Arc::clone(&self.inner), generation };
        std::thread::Builder::new().name("steam-sign-in".into()).spawn(move || run.run()).map(drop).map_err(|e| {
            self.lock().state = State::Failed(format!("starting the sign-in: {e}"));
            e.to_string()
        })
    }

    fn cancel_sign_in(&self) {
        let mut i = self.lock();
        i.generation += 1;
        i.state = State::Idle;
    }

    fn sign_out(&self, account: &str) -> Result<(), String> {
        forget(self.wiring.secrets.as_ref(), account)?;
        (self.wiring.signed_out)(account);
        Ok(())
    }

    fn qr_svg(&self) -> Option<String> {
        match &self.lock().state {
            State::Waiting { url, .. } => encode(url.as_bytes(), None).map(|q| svg(&q)),
            _ => None,
        }
    }
}
