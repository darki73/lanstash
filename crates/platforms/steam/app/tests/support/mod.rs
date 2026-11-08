//! Doubles for the Steam app tests: a scripted Steam behind a fake link, a
//! prompt that answers from a list, fixed randomness, secrets in memory.

#![allow(dead_code)]

pub mod account;
pub mod two_accounts;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use lanstash_ports::link::{Dialer, Link};
use lanstash_ports::prompt::Prompt;
use lanstash_ports::random::Random;
use lanstash_ports::secrets::Secrets;
use lanstash_steam_domain::cm::message::{Header, Message, emsg};
use lanstash_steam_domain::cm::proto::Writer;

/// What the scripted Steam answers to each message it gets.
pub type Script = Box<dyn FnMut(&Message) -> Vec<Vec<u8>> + Send>;

/// Everything the client sent.
pub type Sent = Arc<Mutex<Vec<Message>>>;

pub struct FakeLink {
    inbox: VecDeque<Vec<u8>>,
    sent: Sent,
    script: Script,
}

impl Link for FakeLink {
    fn send(&mut self, raw: &[u8]) -> Result<(), String> {
        let m = Message::decode(raw).map_err(|e| format!("{e:?}"))?;
        self.inbox.extend((self.script)(&m));
        self.sent.lock().unwrap().push(m);
        Ok(())
    }

    fn recv(&mut self) -> Result<Option<Vec<u8>>, String> {
        Ok(self.inbox.pop_front())
    }
}

/// A link to a Steam that runs `script`, and what was sent over it.
pub fn steam(script: Script) -> (Box<dyn Link>, Sent) {
    let sent = Sent::default();
    (Box::new(FakeLink { inbox: VecDeque::new(), sent: sent.clone(), script }), sent)
}

/// The answer to service call `to`.
pub fn answer(to: &Message, eresult: i32, body: Vec<u8>) -> Vec<u8> {
    let header = Header { job_target: to.header.job_source, eresult, ..Header::default() };
    Message::new(emsg::SERVICE_METHOD_RESPONSE, header, body).encode()
}

/// A plain (not gzipped) Multi of `messages`.
pub fn multi(messages: &[Vec<u8>]) -> Vec<u8> {
    let mut packed = Vec::new();
    for m in messages {
        packed.extend_from_slice(&(m.len() as u32).to_le_bytes());
        packed.extend_from_slice(m);
    }
    Message::new(emsg::MULTI, Header::default(), Writer::new().bytes(2, &packed).finish()).encode()
}

/// Answers questions from a list, in order; keeps what it was told and asked.
#[derive(Default)]
pub struct Answers {
    pub answers: Mutex<VecDeque<String>>,
    pub asked: Mutex<Vec<(String, bool)>>,
    pub told: Mutex<Vec<String>>,
}

impl Answers {
    pub fn new(answers: &[&str]) -> Self {
        Self { answers: Mutex::new(answers.iter().map(|s| (*s).to_string()).collect()), ..Self::default() }
    }
}

impl Prompt for Answers {
    fn ask(&self, question: &str, secret: bool) -> Result<String, String> {
        self.asked.lock().unwrap().push((question.to_string(), secret));
        self.answers.lock().unwrap().pop_front().ok_or_else(|| format!("no answer left for {question:?}"))
    }

    fn tell(&self, message: &str) {
        self.told.lock().unwrap().push(message.to_string());
    }
}

/// Always the same bytes (never zero, so the padding takes them all).
pub struct Fixed;

impl Random for Fixed {
    fn fill(&self, buf: &mut [u8]) -> Result<(), String> {
        buf.fill(0x5A);
        Ok(())
    }
}

#[derive(Default)]
pub struct Memory(pub Mutex<BTreeMap<String, String>>);

impl Secrets for Memory {
    fn load(&self, name: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }

    fn save(&self, name: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), value.into());
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}

/// Lists `servers` as Steam's directory would and opens a scripted link to
/// any host but those in `down`.
pub struct FakeDialer {
    pub servers: Vec<&'static str>,
    pub down: Vec<&'static str>,
    pub link: Mutex<Option<Box<dyn Link>>>,
    pub dialed: Mutex<Vec<String>>,
}

impl Dialer for FakeDialer {
    fn dial(&self, host: &str, port: u16, path: &str) -> Result<Box<dyn Link>, String> {
        self.dialed.lock().unwrap().push(format!("{host}:{port}{path}"));
        if self.down.contains(&host) {
            return Err("refused".into());
        }
        self.link.lock().unwrap().take().ok_or_else(|| "only one link".to_string())
    }

    fn get(&self, _host: &str, _path: &str) -> Result<Vec<u8>, String> {
        let list: Vec<String> =
            self.servers.iter().map(|s| format!(r#"{{"endpoint":"{s}","type":"websockets"}}"#)).collect();
        Ok(format!(r#"{{"response":{{"serverlist":[{}]}}}}"#, list.join(",")).into_bytes())
    }
}
