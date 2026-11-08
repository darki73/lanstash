//! Tests for `lanstash_app::accounts`: adding, checking, changing and taking away accounts, and
//! always keeping an admin.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lanstash_app::accounts::{Accounts, KEPT};
use lanstash_domain::accounts::Role;
use lanstash_ports::passwords::Passwords;
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

/// Keeps a password reversed, spaces as `_` (a kept hash is one word): the test can see what was
/// kept is not the password.
struct Reversed(Mutex<usize>);

fn reversed(password: &str) -> String {
    format!("rev${}", password.chars().rev().map(|c| if c == ' ' { '_' } else { c }).collect::<String>())
}

impl Passwords for Reversed {
    fn hash(&self, password: &str) -> Result<String, String> {
        Ok(reversed(password))
    }
    fn verify(&self, password: &str, kept: &str) -> bool {
        *self.0.lock().unwrap() += 1;
        kept == reversed(password)
    }
}

fn accounts() -> (Arc<Memory>, Arc<Reversed>, Accounts) {
    let secrets = Arc::new(Memory::default());
    let passwords = Arc::new(Reversed(Mutex::new(0)));
    let a = Accounts::new(secrets.clone(), passwords.clone()).unwrap();
    (secrets, passwords, a)
}

#[test]
fn an_account_signs_in_with_its_password_only_and_is_kept_without_it() {
    let (secrets, _, a) = accounts();
    assert!(!a.has_admin());
    a.add("boss", Role::Admin, "long enough pw").unwrap();
    assert!(a.has_admin());
    assert_eq!(a.check("boss", "long enough pw"), Some(Role::Admin));
    assert_eq!(a.check("boss", "long enough pW"), None);
    let kept = secrets.load(KEPT).unwrap().unwrap();
    assert!(kept.starts_with("boss admin rev$") && !kept.contains("long enough pw"), "{kept}");
    let again = Accounts::new(secrets, Arc::new(Reversed(Mutex::new(0)))).unwrap();
    assert_eq!(again.list(), [("boss".to_string(), Role::Admin)]);
}

#[test]
fn an_unknown_name_still_checks_a_password() {
    let (_, passwords, a) = accounts();
    assert_eq!(a.check("nobody", "whatever pw"), None);
    assert_eq!(*passwords.0.lock().unwrap(), 1, "as long as a wrong password");
}

#[test]
fn bad_names_bad_passwords_and_taken_names_are_refused() {
    let (_, _, a) = accounts();
    assert!(a.add("Bad Name", Role::Viewer, "long enough pw").is_err());
    assert!(a.add("viewer", Role::Viewer, "short").is_err());
    a.add("viewer", Role::Viewer, "long enough pw").unwrap();
    assert!(a.add("viewer", Role::Admin, "another long pw").unwrap_err().contains("already"));
    assert!(a.set_password("viewer", "tiny").is_err());
    assert!(a.set_password("ghost", "long enough pw").unwrap_err().contains("no account"));
}

#[test]
fn the_last_admin_is_never_taken_away_nor_made_a_viewer() {
    let (_, _, a) = accounts();
    a.add("boss", Role::Admin, "long enough pw").unwrap();
    a.add("player.one", Role::Viewer, "long enough pw").unwrap();
    assert!(a.remove("boss").unwrap_err().contains("last admin"));
    assert!(a.set_role("boss", Role::Viewer).unwrap_err().contains("last admin"));
    a.set_role("player.one", Role::Admin).unwrap();
    a.set_role("boss", Role::Viewer).unwrap();
    a.remove("boss").unwrap();
    assert_eq!(a.list(), [("player.one".to_string(), Role::Admin)]);
}

#[test]
fn a_change_made_elsewhere_is_seen_at_once() {
    let (secrets, _, a) = accounts();
    let other = Accounts::new(secrets, Arc::new(Reversed(Mutex::new(0)))).unwrap();
    other.add("boss", Role::Admin, "long enough pw").unwrap();
    assert_eq!(a.check("boss", "long enough pw"), Some(Role::Admin));
}

#[test]
fn platform_accounts_are_linked_and_unlinked() {
    let (_, _, a) = accounts();
    a.add("player.one", Role::Player, "long enough pw").unwrap();
    a.set_link("player.one", "steam/someone", true).unwrap();
    a.set_link("player.one", "steam/someone", true).unwrap();
    a.set_link("player.one", "epicgames/other", true).unwrap();
    assert_eq!(a.links("player.one"), ["steam/someone", "epicgames/other"]);
    a.set_link("player.one", "steam/someone", false).unwrap();
    assert_eq!(a.links("player.one"), ["epicgames/other"]);
    assert!(a.set_link("player.one", "nonsense", true).is_err());
    assert!(a.set_link("ghost", "steam/x", true).is_err());
    assert!(a.links("ghost").is_empty());
}

#[test]
fn a_new_password_replaces_the_old() {
    let (_, _, a) = accounts();
    a.add("boss", Role::Admin, "first password").unwrap();
    a.set_password("boss", "second password").unwrap();
    assert_eq!(a.check("boss", "first password"), None);
    assert_eq!(a.check("boss", "second password"), Some(Role::Admin));
}
