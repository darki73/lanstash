//! Tests for `lanstash_app::invites`: invites made, accepted once, taken back, and run out.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lanstash_app::accounts::Accounts;
use lanstash_app::invites::{AcceptError, Invites, KEPT, LIFE_MS};
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

#[derive(Default)]
struct Counting(AtomicU8);
impl Random for Counting {
    fn fill(&self, buf: &mut [u8]) -> Result<(), String> {
        buf.fill(self.0.fetch_add(1, Ordering::Relaxed) + 1);
        Ok(())
    }
}

/// Not SHA-256: each byte plus one.
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

fn rig() -> (Arc<Memory>, Arc<Now>, Arc<Accounts>, Invites) {
    let secrets = Arc::new(Memory::default());
    let accounts = Arc::new(Accounts::new(secrets.clone(), Arc::new(Plain)).unwrap());
    let clock = Arc::new(Now(AtomicU64::new(1_000)));
    let invites =
        Invites::new(accounts.clone(), secrets.clone(), Arc::new(Counting::default()), Arc::new(Shift), clock.clone());
    (secrets, clock, accounts, invites)
}

#[test]
fn an_invite_makes_one_account_with_its_role_and_links() {
    let (secrets, _, accounts, invites) = rig();
    let (token, until) = invites.make(Role::Player, &["steam/someone".into()], None).unwrap();
    assert_eq!(until, 1_000 + LIFE_MS);
    assert!(!secrets.load(KEPT).unwrap().unwrap().contains(&token), "only its digest is kept");
    assert_eq!(invites.list().len(), 1);
    assert_eq!(invites.accept(&token, "player.one", "long enough pw"), Ok(Role::Player));
    assert_eq!(accounts.check("player.one", "long enough pw"), Some(Role::Player));
    assert_eq!(accounts.links("player.one"), ["steam/someone"]);
    assert_eq!(invites.accept(&token, "second", "long enough pw"), Err(AcceptError::Unknown), "used once");
    assert!(invites.list().is_empty());
}

#[test]
fn a_refused_name_keeps_the_invite() {
    let (_, _, _, invites) = rig();
    let (token, _) = invites.make(Role::Viewer, &[], None).unwrap();
    assert!(matches!(invites.accept(&token, "Bad Name", "long enough pw"), Err(AcceptError::Refused(_))));
    assert!(matches!(invites.accept(&token, "good", "short"), Err(AcceptError::Refused(_))));
    assert_eq!(invites.accept(&token, "good", "long enough pw"), Ok(Role::Viewer));
}

#[test]
fn an_invite_is_taken_back_or_runs_out() {
    let (_, clock, _, invites) = rig();
    let (a, _) = invites.make(Role::Viewer, &[], None).unwrap();
    let (b, _) = invites.make(Role::Admin, &[], None).unwrap();
    let id = invites.list()[0].id.clone();
    invites.revoke(&id).unwrap();
    assert!(invites.accept(&a, "x1", "long enough pw").is_err());
    assert!(invites.revoke("short").is_err());
    clock.0.fetch_add(LIFE_MS, Ordering::Relaxed);
    assert!(invites.accept(&b, "x2", "long enough pw").is_err(), "ran out");
    assert!(invites.make(Role::Viewer, &["nonsense".into()], None).is_err());
}

#[test]
fn an_invite_with_a_name_makes_only_that_account_and_shows_it() {
    let (_, _, accounts, invites) = rig();
    accounts.add("taken", Role::Viewer, "long enough pw").unwrap();
    assert!(invites.make(Role::Player, &[], Some("taken")).is_err(), "the name is taken");
    assert!(invites.make(Role::Player, &[], Some("Bad Name")).is_err());
    let links = ["steam/a".to_string(), "steam/b".to_string()];
    let (token, _) = invites.make(Role::Player, &links, Some("player.one")).unwrap();
    let shown = invites.show(&token).unwrap();
    assert_eq!((shown.name.as_deref(), shown.links.len()), (Some("player.one"), 2));
    assert!(matches!(invites.accept(&token, "other", "long enough pw"), Err(AcceptError::Refused(_))));
    assert_eq!(invites.accept(&token, "player.one", "long enough pw"), Ok(Role::Player));
    assert_eq!(accounts.links("player.one"), links);
    assert_eq!(invites.show(&token), None, "used up");
}
