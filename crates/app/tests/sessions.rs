//! Tests for `lanstash_app::sessions`: signing in, staying signed in, waiting after wrong
//! passwords, and sessions ending.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lanstash_app::accounts::Accounts;
use lanstash_app::sessions::{KEPT, LIFE_MS, MAX_WAIT_MS, Sessions, SignInError};
use lanstash_domain::accounts::Role;
use lanstash_ports::clock::Clock;
use lanstash_ports::digest::Digest;
use lanstash_ports::passwords::Passwords;
use lanstash_ports::random::Random;
use lanstash_ports::secrets::Secrets;

#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, String>>);

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

struct Plain;
impl Passwords for Plain {
    fn hash(&self, p: &str) -> Result<String, String> {
        Ok(format!("plain${}", p.replace(' ', "_")))
    }
    fn verify(&self, p: &str, kept: &str) -> bool {
        kept == format!("plain${}", p.replace(' ', "_"))
    }
}

/// Each draw fills with the next number.
#[derive(Default)]
struct Counting(AtomicU8);
impl Random for Counting {
    fn fill(&self, buf: &mut [u8]) -> Result<(), String> {
        buf.fill(self.0.fetch_add(1, Ordering::Relaxed) + 1);
        Ok(())
    }
}

/// Not SHA-256: each byte plus one, in its first 32 bytes.
struct Shift;
impl Digest for Shift {
    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (o, d) in out.iter_mut().zip(data) {
            *o = d.wrapping_add(1);
        }
        out
    }
}

struct Now(AtomicU64);
impl Clock for Now {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

struct Rig {
    secrets: Arc<Memory>,
    clock: Arc<Now>,
    accounts: Arc<Accounts>,
    sessions: Sessions,
}

fn rig() -> Rig {
    let secrets = Arc::new(Memory::default());
    let accounts = Arc::new(Accounts::new(secrets.clone(), Arc::new(Plain)).unwrap());
    accounts.add("boss", Role::Admin, "long enough pw").unwrap();
    accounts.add("player.one", Role::Viewer, "another long pw").unwrap();
    let clock = Arc::new(Now(AtomicU64::new(1_000_000)));
    let sessions =
        Sessions::new(accounts.clone(), secrets.clone(), Arc::new(Counting::default()), Arc::new(Shift), clock.clone())
            .unwrap();
    Rig { secrets, clock, accounts, sessions }
}

#[test]
fn a_sign_in_makes_a_session_kept_only_as_a_digest() {
    let r = rig();
    let (token, role) = r.sessions.sign_in("192.0.2.1", "boss", "long enough pw").unwrap();
    assert_eq!(role, Role::Admin);
    assert_eq!(token.len(), 64);
    assert_eq!(r.sessions.who(&token), Some(("boss".into(), Role::Admin)));
    let kept = r.secrets.load(KEPT).unwrap().unwrap();
    // The token is "0101…"; its fake digest shifts each character ('0' 0x30 to 0x31, '1' to 0x32).
    assert!(kept.starts_with("31323132") && !kept.contains(&token), "{kept}");
    assert_eq!(r.sessions.who("not a token"), None);
}

#[test]
fn a_session_outlives_a_restart_and_runs_out_unless_used() {
    let r = rig();
    let (token, _) = r.sessions.sign_in("a", "player.one", "another long pw").unwrap();
    let again = || {
        Sessions::new(
            r.accounts.clone(),
            r.secrets.clone(),
            Arc::new(Counting::default()),
            Arc::new(Shift),
            r.clock.clone(),
        )
        .unwrap()
    };
    assert_eq!(again().who(&token), Some(("player.one".into(), Role::Viewer)));
    r.clock.0.fetch_add(LIFE_MS - 1, Ordering::Relaxed);
    assert!(again().who(&token).is_some(), "used just in time: it lives on");
    r.clock.0.fetch_add(LIFE_MS - 1, Ordering::Relaxed);
    assert!(again().who(&token).is_some(), "the use above lengthened it");
    r.clock.0.fetch_add(LIFE_MS + 1, Ordering::Relaxed);
    assert_eq!(again().who(&token), None);
}

#[test]
fn wrong_sign_ins_make_an_address_wait_longer_each_time() {
    let r = rig();
    assert_eq!(r.sessions.sign_in("a", "boss", "wrong password"), Err(SignInError::Wrong));
    assert_eq!(r.sessions.sign_in("a", "boss", "long enough pw"), Err(SignInError::Wait(1_000)));
    assert!(r.sessions.sign_in("b", "boss", "long enough pw").is_ok(), "another address does not wait");
    r.clock.0.fetch_add(1_000, Ordering::Relaxed);
    assert_eq!(r.sessions.sign_in("a", "nobody", "long enough pw"), Err(SignInError::Wrong));
    assert_eq!(r.sessions.sign_in("a", "boss", "x"), Err(SignInError::Wait(2_000)));
    for _ in 0..10 {
        r.clock.0.fetch_add(MAX_WAIT_MS, Ordering::Relaxed);
        let _ = r.sessions.sign_in("a", "boss", "wrong password");
    }
    assert_eq!(r.sessions.sign_in("a", "boss", "x"), Err(SignInError::Wait(MAX_WAIT_MS)));
    r.clock.0.fetch_add(MAX_WAIT_MS, Ordering::Relaxed);
    assert!(r.sessions.sign_in("a", "boss", "long enough pw").is_ok());
    assert_eq!(r.sessions.sign_in("a", "boss", "wrong password"), Err(SignInError::Wrong), "a right one cleared it");
}

#[test]
fn a_session_starts_without_a_password_and_a_failed_try_counts_as_a_wrong_sign_in() {
    let r = rig();
    let token = r.sessions.start("player.one").unwrap();
    assert_eq!(r.sessions.who(&token), Some(("player.one".into(), Role::Viewer)));
    assert_eq!(r.sessions.waiting("a"), None);
    r.sessions.failed("a");
    assert_eq!(r.sessions.waiting("a"), Some(1_000));
    assert_eq!(r.sessions.sign_in("a", "boss", "long enough pw"), Err(SignInError::Wait(1_000)));
    r.clock.0.fetch_add(1_000, Ordering::Relaxed);
    assert_eq!(r.sessions.waiting("a"), None);
    r.sessions.failed("a");
    assert_eq!(r.sessions.waiting("a"), Some(2_000), "twice as long the second time");
    assert_eq!(r.sessions.waiting("b"), None, "another address does not wait");
}

#[test]
fn sessions_end_on_sign_out_on_forget_and_with_their_account() {
    let r = rig();
    let (one, _) = r.sessions.sign_in("a", "player.one", "another long pw").unwrap();
    let (two, _) = r.sessions.sign_in("b", "player.one", "another long pw").unwrap();
    let (boss, _) = r.sessions.sign_in("c", "boss", "long enough pw").unwrap();
    r.sessions.sign_out(&one);
    assert_eq!(r.sessions.who(&one), None);
    assert!(r.sessions.who(&two).is_some());
    r.sessions.forget("player.one");
    assert_eq!(r.sessions.who(&two), None);
    r.accounts.add("other", Role::Admin, "long enough pw").unwrap();
    r.accounts.set_role("boss", Role::Viewer).unwrap();
    assert_eq!(r.sessions.who(&boss), Some(("boss".into(), Role::Viewer)), "the role as it is now");
    r.accounts.remove("boss").unwrap();
    assert_eq!(r.sessions.who(&boss), None);
}
