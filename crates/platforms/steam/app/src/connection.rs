//! A connection to a Steam CM: messages out, messages in (Multi unpacked),
//! service calls and requests matched to their answers by job id, and the
//! license list Steam sends after a logon kept for whoever asks.

use std::collections::VecDeque;

use lanstash_ports::link::{Dialer, Link};
use lanstash_steam_domain::cm::auth::call;
use lanstash_steam_domain::cm::eresult;
use lanstash_steam_domain::cm::logon::hello;
use lanstash_steam_domain::cm::message::{CmError, Header, Message, emsg, unpack_multi};
use lanstash_steam_domain::cm::pics::{self, License};
use lanstash_steam_domain::cm::servers::{DIRECTORY, PATH, websockets};

/// Largest unzipped Multi accepted.
const MAX_MULTI: usize = 64 << 20;

/// How many servers from the list are tried before giving up.
const TRIES: usize = 3;

/// An open connection, hello sent.
pub struct Connection {
    link: Box<dyn Link>,
    queue: VecDeque<Message>,
    next_job: u64,
    /// The logged-on session (steamid and session id), once there is one.
    pub session: Header,
    /// How many empty reads (each the link's timeout) a wait allows.
    pub patience: u32,
    /// The connection failed (link, logged off, no answer): it is of no more use.
    broken: bool,
    /// The license list, once Steam sent it.
    licenses: Option<Vec<License>>,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection").field("queued", &self.queue.len()).finish_non_exhaustive()
    }
}

impl Connection {
    /// Asks Steam's directory for CM servers and connects to the first of
    /// them that answers, then says hello.
    ///
    /// # Errors
    /// No list, or none of the first servers could be reached.
    pub fn open(dialer: &dyn Dialer) -> Result<Self, String> {
        let servers = websockets(&dialer.get(DIRECTORY.0, DIRECTORY.1)?)?;
        let mut last = String::from("Steam's directory listed no WebSocket servers");
        for (host, port) in servers.iter().take(TRIES) {
            match dialer.dial(host, *port, PATH) {
                Ok(link) => {
                    let mut c = Self::over(link);
                    c.send(&hello())?;
                    return Ok(c);
                }
                Err(e) => last = format!("{host}:{port}: {e}"),
            }
        }
        Err(last)
    }

    /// A connection over `link` (nothing sent yet).
    #[must_use]
    pub fn over(link: Box<dyn Link>) -> Self {
        Self {
            link,
            queue: VecDeque::new(),
            next_job: 1,
            session: Header::default(),
            patience: 3,
            licenses: None,
            broken: false,
        }
    }

    /// Sends `m`, as the logged-on session when it names none.
    ///
    /// # Errors
    /// The link failed.
    pub fn send(&mut self, m: &Message) -> Result<(), String> {
        let sent = if m.header.steamid == 0 && self.session.steamid != 0 {
            let mut m = m.clone();
            m.header.steamid = self.session.steamid;
            m.header.session = self.session.session;
            self.link.send(&m.encode())
        } else {
            self.link.send(&m.encode())
        };
        self.broken |= sent.is_err();
        sent
    }

    /// Whether the connection failed (as opposed to Steam refusing one request on it).
    #[must_use]
    pub fn broken(&self) -> bool {
        self.broken
    }

    /// The next message, waiting at most `patience` read timeouts.
    ///
    /// # Errors
    /// The link failed, nothing came in time, or Steam logged this connection off.
    pub fn receive(&mut self) -> Result<Message, String> {
        let got = self.receive_inner();
        self.broken |= got.is_err();
        got
    }

    fn receive_inner(&mut self) -> Result<Message, String> {
        let mut idle = 0;
        loop {
            if let Some(m) = self.queue.pop_front() {
                if m.emsg == emsg::CLIENT_LOGGED_OFF {
                    return Err(format!("Steam logged this connection off ({})", logged_off(&m)));
                }
                return Ok(m);
            }
            let Some(raw) = self.link.recv()? else {
                idle += 1;
                if idle >= self.patience {
                    return Err("Steam did not answer in time".into());
                }
                continue;
            };
            self.take(&raw)?;
        }
    }

    /// Queues the message(s) in `raw`; old-style messages are skipped.
    fn take(&mut self, raw: &[u8]) -> Result<(), String> {
        let m = match Message::decode(raw) {
            Ok(m) => m,
            Err(CmError::NotProtobuf(_)) => return Ok(()),
            Err(e) => return Err(format!("a message from Steam: {e:?}")),
        };
        if m.emsg == pics::emsg::CLIENT_LICENSE_LIST {
            self.licenses = Some(pics::licenses(&m.body).map_err(|e| format!("the license list: {e:?}"))?);
            return Ok(());
        }
        if m.emsg != emsg::MULTI {
            self.queue.push_back(m);
            return Ok(());
        }
        for inner in unpack_multi(&m.body, MAX_MULTI).map_err(|e| format!("a Multi from Steam: {e:?}"))? {
            self.take(&inner)?;
        }
        Ok(())
    }

    /// The next message of `wanted`, others passed over.
    ///
    /// # Errors
    /// As [`Self::receive`].
    pub fn wait_for(&mut self, wanted: u32) -> Result<Message, String> {
        loop {
            let m = self.receive()?;
            if m.emsg == wanted {
                return Ok(m);
            }
        }
    }

    /// Calls service method `name` with `body` and returns the answer; an
    /// answer whose result is not OK is an error naming it.
    ///
    /// # Errors
    /// As [`Self::receive`], or Steam's result.
    pub fn call(&mut self, name: &str, body: Vec<u8>) -> Result<Message, CallError> {
        let job = self.job();
        let mut m = call(name, job, body);
        if self.session.steamid != 0 {
            m.emsg = emsg::SERVICE_METHOD_CALL_FROM_CLIENT;
        }
        self.send(&m).map_err(CallError::Link)?;
        loop {
            let m = self.receive().map_err(CallError::Link)?;
            if m.emsg == emsg::SERVICE_METHOD_RESPONSE && m.header.job_target == job {
                if m.header.eresult != eresult::OK {
                    return Err(CallError::Steam(m.header.eresult));
                }
                return Ok(m);
            }
        }
    }
}

impl Connection {
    fn job(&mut self) -> u64 {
        let job = self.next_job;
        self.next_job += 1;
        job
    }

    /// Sends `body` as an `emsg` request with a new job id; returns the id for [`Self::reply`].
    ///
    /// # Errors
    /// The link failed.
    pub fn request(&mut self, emsg: u32, body: Vec<u8>) -> Result<u64, String> {
        let job = self.job();
        self.send(&Message::new(emsg, Header { job_source: job, ..Header::default() }, body))?;
        Ok(job)
    }

    /// The next `emsg` answer to job `job` (an answer may come in several).
    ///
    /// # Errors
    /// As [`Self::receive`].
    pub fn reply(&mut self, job: u64, emsg: u32) -> Result<Message, String> {
        loop {
            let m = self.receive()?;
            if m.emsg == emsg && m.header.job_target == job {
                return Ok(m);
            }
        }
    }

    /// The account's licenses (sent by Steam after a logon), waiting for them if need be.
    ///
    /// # Errors
    /// As [`Self::receive`].
    pub fn licenses(&mut self) -> Result<Vec<License>, String> {
        loop {
            if let Some(l) = &self.licenses {
                return Ok(l.clone());
            }
            self.receive()?;
        }
    }
}

/// Why a service call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// The connection.
    Link(String),
    /// Steam answered with this result.
    Steam(i32),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Link(e) => f.write_str(e),
            Self::Steam(code) => write!(f, "Steam: {} ({code})", eresult::describe(*code)),
        }
    }
}

/// `CMsgClientLoggedOff` (1 `eresult`), as words.
fn logged_off(m: &Message) -> String {
    let code = lanstash_steam_domain::cm::proto::fields(&m.body)
        .filter_map(Result::ok)
        .find(|(n, _)| *n == 1)
        .and_then(|(_, v)| v.int())
        .map_or(2, |v| v as i32);
    format!("{} ({code})", eresult::describe(code))
}
