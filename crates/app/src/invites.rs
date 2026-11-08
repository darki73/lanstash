//! Use case: invites. An admin makes an invite for a role (and platform accounts to link); its token
//! (32 random bytes, hex) goes into the link the admin sends, and lanstash keeps only the token's
//! SHA-256 (the secret `invites`: `<digest> <role> <until ms> <links or -> [<name>]` a line). Whoever
//! opens the link picks a password, and a name unless the admin chose one: the account is made with
//! the invite's role and links, and the invite is used up. An invite lasts [`LIFE_MS`]; one past it
//! is forgotten.

use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::accounts::{Role, hex, is_link, name_problem};
use lanstash_ports::clock::Clock;
use lanstash_ports::digest::Digest;
use lanstash_ports::random::Random;
use lanstash_ports::secrets::Secrets;

use crate::accounts::Accounts;

/// The secret the invites are kept under.
pub const KEPT: &str = "invites";
/// How long an invite lasts.
pub const LIFE_MS: u64 = 7 * 24 * 3_600_000;

/// An invite as an admin sees it (never its token).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    /// The first 12 hex digits of the token's digest: enough to name it.
    pub id: String,
    pub role: Role,
    pub links: Vec<String>,
    pub until_ms: u64,
    /// The name the account will have, when the admin chose it.
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
struct Kept {
    digest: String,
    role: Role,
    until_ms: u64,
    links: Vec<String>,
    name: Option<String>,
}

/// A kept invite as it is shown.
fn shown(k: Kept) -> Invite {
    Invite {
        id: k.digest[..12.min(k.digest.len())].to_string(),
        role: k.role,
        links: k.links,
        until_ms: k.until_ms,
        name: k.name,
    }
}

/// The invites.
pub struct Invites {
    accounts: Arc<Accounts>,
    secrets: Arc<dyn Secrets>,
    random: Arc<dyn Random>,
    digest: Arc<dyn Digest>,
    clock: Arc<dyn Clock>,
    changing: Mutex<()>,
}

impl std::fmt::Debug for Invites {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invites").finish_non_exhaustive()
    }
}

impl Invites {
    #[must_use]
    pub fn new(
        accounts: Arc<Accounts>,
        secrets: Arc<dyn Secrets>,
        random: Arc<dyn Random>,
        digest: Arc<dyn Digest>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self { accounts, secrets, random, digest, clock, changing: Mutex::new(()) }
    }

    fn read(&self) -> Vec<Kept> {
        let now = self.clock.now_ms();
        let text = self.secrets.load(KEPT).ok().flatten().unwrap_or_default();
        text.lines()
            .filter_map(|l| {
                let mut p = l.split_whitespace();
                let (digest, role, until) = (p.next()?, p.next()?, p.next()?.parse().ok()?);
                let links = match p.next() {
                    None | Some("-") => Vec::new(),
                    Some(l) => l.split(',').map(String::from).collect(),
                };
                let name = p.next().map(String::from);
                let role = Role::parse(role)?;
                (until > now).then(|| Kept { digest: digest.into(), role, until_ms: until, links, name })
            })
            .collect()
    }

    fn keep(&self, all: &[Kept]) -> Result<(), String> {
        let text: String = all
            .iter()
            .map(|k| {
                let links = if k.links.is_empty() { "-".to_string() } else { k.links.join(",") };
                let name = k.name.as_ref().map_or_else(String::new, |n| format!(" {n}"));
                format!("{} {} {} {links}{name}\n", k.digest, k.role.name(), k.until_ms)
            })
            .collect();
        self.secrets.save(KEPT, &text)
    }

    /// A new invite for `role` and `links`, and the account's `name` when the admin chooses it: its
    /// token (for the link) and until when it lasts.
    ///
    /// # Errors
    /// A link names no platform account, the name will not do or is taken, no token could be drawn,
    /// or the invite could not be kept.
    pub fn make(&self, role: Role, links: &[String], name: Option<&str>) -> Result<(String, u64), String> {
        if let Some(bad) = links.iter().find(|l| !is_link(l)) {
            return Err(format!("not a platform account: {bad:?}"));
        }
        if let Some(n) = name {
            if let Some(why) = name_problem(n) {
                return Err(why.into());
            }
            if self.accounts.list().iter().any(|(a, _)| a == n) {
                return Err(format!("there is an account {n} already"));
            }
        }
        let mut bytes = [0u8; 32];
        self.random.fill(&mut bytes)?;
        let token = hex(&bytes);
        let until_ms = self.clock.now_ms() + LIFE_MS;
        let _one = self.changing.lock().unwrap_or_else(PoisonError::into_inner);
        let mut all = self.read();
        let digest = hex(&self.digest.sha256(token.as_bytes()));
        all.push(Kept { digest, role, until_ms, links: links.to_vec(), name: name.map(String::from) });
        self.keep(&all)?;
        Ok((token, until_ms))
    }

    /// The invites not yet used nor run out.
    #[must_use]
    pub fn list(&self) -> Vec<Invite> {
        self.read().into_iter().map(shown).collect()
    }

    /// The invite `token` holds, while it may be used (the page it opens shows its role and name).
    #[must_use]
    pub fn show(&self, token: &str) -> Option<Invite> {
        let digest = hex(&self.digest.sha256(token.as_bytes()));
        self.read().into_iter().find(|k| k.digest == digest).map(shown)
    }

    /// Takes the invite `id` back.
    ///
    /// # Errors
    /// There is no such invite, or the change could not be kept.
    pub fn revoke(&self, id: &str) -> Result<(), String> {
        let _one = self.changing.lock().unwrap_or_else(PoisonError::into_inner);
        let mut all = self.read();
        let before = all.len();
        all.retain(|k| id.len() < 12 || !k.digest.starts_with(id));
        if all.len() == before {
            return Err(format!("there is no invite {id}"));
        }
        self.keep(&all)
    }

    /// Makes the account `name` with `password` from the invite `token`, which is then used up:
    /// the role the account got.
    ///
    /// # Errors
    /// The invite is unknown or ran out ([`AcceptError::Unknown`]), or the name or password will not
    /// do, the name is taken, or the account could not be kept ([`AcceptError::Refused`]).
    pub fn accept(&self, token: &str, name: &str, password: &str) -> Result<Role, AcceptError> {
        let digest = hex(&self.digest.sha256(token.as_bytes()));
        let _one = self.changing.lock().unwrap_or_else(PoisonError::into_inner);
        let mut all = self.read();
        let at = all.iter().position(|k| k.digest == digest).ok_or(AcceptError::Unknown)?;
        if let Some(chosen) = all[at].name.as_ref().filter(|n| *n != name) {
            return Err(AcceptError::Refused(format!("this invite is for the account {chosen}")));
        }
        let invite = all.remove(at);
        self.accounts.add(name, invite.role, password).map_err(AcceptError::Refused)?;
        for l in &invite.links {
            self.accounts.set_link(name, l, true).map_err(AcceptError::Refused)?;
        }
        self.keep(&all).map_err(AcceptError::Refused)?;
        Ok(invite.role)
    }
}

/// Why an invite made no account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptError {
    /// No such invite: never made, used, taken back, or run out.
    Unknown,
    /// The invite holds, but the account will not do (the invite is kept): why.
    Refused(String),
}

impl std::fmt::Display for AcceptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => f.write_str("this invite is not valid (used, taken back, or ran out)"),
            Self::Refused(why) => f.write_str(why),
        }
    }
}
