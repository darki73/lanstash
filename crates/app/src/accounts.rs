//! Use case: lanstash's own accounts, kept as a secret (`accounts`, readable by the service's user
//! only): adding one, taking one away, changing a password or a role, listing them, and checking a
//! sign-in. The last admin can be neither taken away nor made a viewer, so there is always one.
//! A sign-in with an unknown name takes as long as one with a wrong password. The accounts are read
//! afresh each time (they are few, and asked for seldom), so what the command line changes, the
//! running service sees at once; a file that cannot be read is no account.

use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::accounts::{Account, Role, encode, is_link, name_problem, parse, password_problem};
use lanstash_ports::passwords::Passwords;
use lanstash_ports::secrets::Secrets;

/// The secret the accounts are kept under.
pub const KEPT: &str = "accounts";

/// lanstash's accounts.
pub struct Accounts {
    secrets: Arc<dyn Secrets>,
    passwords: Arc<dyn Passwords>,
    /// Held while the accounts are changed, so two changes never lose one another.
    changing: Mutex<()>,
}

impl std::fmt::Debug for Accounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Accounts").field("accounts", &self.list().len()).finish_non_exhaustive()
    }
}

impl Accounts {
    /// The accounts `secrets` keeps.
    ///
    /// # Errors
    /// They are kept but could not be read.
    pub fn new(secrets: Arc<dyn Secrets>, passwords: Arc<dyn Passwords>) -> Result<Self, String> {
        secrets.load(KEPT)?;
        Ok(Self { secrets, passwords, changing: Mutex::new(()) })
    }

    /// The accounts as kept now.
    fn read(&self) -> Result<Vec<Account>, String> {
        Ok(parse(&self.secrets.load(KEPT)?.unwrap_or_default()))
    }

    /// Every account's name and role, by name.
    #[must_use]
    pub fn list(&self) -> Vec<(String, Role)> {
        let mut all: Vec<_> = self.read().unwrap_or_default().into_iter().map(|a| (a.name, a.role)).collect();
        all.sort();
        all
    }

    /// Whether there is an admin.
    #[must_use]
    pub fn has_admin(&self) -> bool {
        self.read().unwrap_or_default().iter().any(|a| a.role == Role::Admin)
    }

    /// The role of `name` when `password` is its password; none otherwise (an unknown name too).
    #[must_use]
    pub fn check(&self, name: &str, password: &str) -> Option<Role> {
        let found = self.read().unwrap_or_default().into_iter().find(|a| a.name == name);
        match found {
            Some(a) => self.passwords.verify(password, &a.hash).then_some(a.role),
            None => {
                let _ = self.passwords.verify(password, "");
                None
            }
        }
    }

    /// Adds `name` with `role` and `password`.
    ///
    /// # Errors
    /// The name or password will not do, the name is taken, or the accounts could not be kept.
    pub fn add(&self, name: &str, role: Role, password: &str) -> Result<(), String> {
        if let Some(p) = name_problem(name).or_else(|| password_problem(password)) {
            return Err(p.into());
        }
        let hash = self.passwords.hash(password)?;
        self.change(|list| {
            if list.iter().any(|a| a.name == name) {
                return Err(format!("there is already an account called {name}"));
            }
            list.push(Account { name: name.into(), role, hash, links: Vec::new() });
            Ok(())
        })
    }

    /// Gives `name` a new password.
    ///
    /// # Errors
    /// The password will not do, there is no such account, or the accounts could not be kept.
    pub fn set_password(&self, name: &str, password: &str) -> Result<(), String> {
        if let Some(p) = password_problem(password) {
            return Err(p.into());
        }
        let hash = self.passwords.hash(password)?;
        self.change(|list| {
            let a = list.iter_mut().find(|a| a.name == name).ok_or_else(|| no_such(name))?;
            a.hash = hash;
            Ok(())
        })
    }

    /// The platform accounts linked to `name` (none for no such account).
    #[must_use]
    pub fn links(&self, name: &str) -> Vec<String> {
        self.read().unwrap_or_default().into_iter().find(|a| a.name == name).map(|a| a.links).unwrap_or_default()
    }

    /// Links the platform account `link` (`steam/<account>`) to `name`, or takes the link away.
    ///
    /// # Errors
    /// The link names no platform account, there is no such account, or it could not be kept.
    pub fn set_link(&self, name: &str, link: &str, on: bool) -> Result<(), String> {
        if !is_link(link) {
            return Err(format!("not a platform account: {link:?} (<platform>/<account>)"));
        }
        self.change(|list| {
            let a = list.iter_mut().find(|a| a.name == name).ok_or_else(|| no_such(name))?;
            a.links.retain(|l| l != link);
            if on {
                a.links.push(link.to_string());
            }
            Ok(())
        })
    }

    /// Gives `name` the role `role`.
    ///
    /// # Errors
    /// There is no such account, it is the last admin (made a viewer), or the accounts could not
    /// be kept.
    pub fn set_role(&self, name: &str, role: Role) -> Result<(), String> {
        self.change(|list| {
            let i = list.iter().position(|a| a.name == name).ok_or_else(|| no_such(name))?;
            if role != Role::Admin && last_admin(list, i) {
                return Err(format!("{name} is the last admin"));
            }
            list[i].role = role;
            Ok(())
        })
    }

    /// Takes `name` away.
    ///
    /// # Errors
    /// There is no such account, it is the last admin, or the accounts could not be kept.
    pub fn remove(&self, name: &str) -> Result<(), String> {
        self.change(|list| {
            let i = list.iter().position(|a| a.name == name).ok_or_else(|| no_such(name))?;
            if last_admin(list, i) {
                return Err(format!("{name} is the last admin"));
            }
            list.remove(i);
            Ok(())
        })
    }

    /// Reads the accounts, changes them and keeps them, one change at a time.
    fn change(&self, edit: impl FnOnce(&mut Vec<Account>) -> Result<(), String>) -> Result<(), String> {
        let _one = self.changing.lock().unwrap_or_else(PoisonError::into_inner);
        let mut list = self.read()?;
        edit(&mut list)?;
        self.secrets.save(KEPT, &encode(&list))
    }
}

fn no_such(name: &str) -> String {
    format!("there is no account called {name}")
}

/// Whether the account at `i` is the only admin.
fn last_admin(list: &[Account], i: usize) -> bool {
    list[i].role == Role::Admin && list.iter().filter(|a| a.role == Role::Admin).count() == 1
}
