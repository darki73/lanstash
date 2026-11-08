//! lanstash's own accounts: who may sign in to its web UI and API, and with what role. A role is a
//! set of permissions: an admin may do everything and sees games kept private; a player prefills
//! the games its linked platform accounts own; a viewer reads (as does someone not signed in), with
//! hidden games masked for both. The accounts are kept as text, one a line:
//! `<name> <role> <password hash> [<links>]`, the hash as the password hasher wrote it (never the
//! password), the links the platform accounts linked to it (`steam/<account>`, comma-separated).

use alloc::string::String;
use alloc::vec::Vec;

/// Something an account may be allowed to do; reading is never one (everyone reads).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Permission {
    /// Prefill any game, and stop or reorder prefills.
    Prefill,
    /// Prefill the games a platform account linked to the account owns.
    PrefillOwn,
    /// Change the settings, and restart lanstash.
    Settings,
    /// Set up and change the network's DNS server.
    Dns,
    /// Delete games from the stash, and prune old builds.
    Delete,
    /// Hide and show games; see hidden games by name.
    Hide,
    /// Name the clients.
    Clients,
    /// Sign lanstash in to a platform's accounts (Steam's, Epic's).
    Platforms,
    /// Manage lanstash's own accounts.
    Accounts,
}

impl Permission {
    /// Every permission, in order.
    pub const ALL: [Self; 9] = [
        Self::Prefill,
        Self::PrefillOwn,
        Self::Settings,
        Self::Dns,
        Self::Delete,
        Self::Hide,
        Self::Clients,
        Self::Platforms,
        Self::Accounts,
    ];

    /// The permission's name in the API.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Prefill => "prefill",
            Self::PrefillOwn => "prefill.own",
            Self::Settings => "settings",
            Self::Dns => "dns",
            Self::Delete => "delete",
            Self::Hide => "hide",
            Self::Clients => "clients",
            Self::Platforms => "platforms",
            Self::Accounts => "accounts",
        }
    }
}

/// What an account may do: a named set of permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// Reads: what anyone not signed in reads too.
    Viewer,
    /// Reads, and prefills the games its linked platform accounts own.
    Player,
    /// Everything.
    Admin,
}

impl Role {
    /// The role's name in the file and in the API.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Player => "player",
            Self::Admin => "admin",
        }
    }

    /// The role called `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "viewer" => Some(Self::Viewer),
            "player" => Some(Self::Player),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }

    /// The role's permissions.
    #[must_use]
    pub fn permissions(self) -> &'static [Permission] {
        match self {
            Self::Viewer => &[],
            Self::Player => &[Permission::PrefillOwn],
            Self::Admin => &Permission::ALL,
        }
    }

    /// Whether the role holds `p`.
    #[must_use]
    pub fn can(self, p: Permission) -> bool {
        self.permissions().contains(&p)
    }
}

/// One account: its name, role and password hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub name: String,
    pub role: Role,
    /// What the password hasher made of the password.
    pub hash: String,
    /// The platform accounts linked to it (`steam/<account>`): their games are its own.
    pub links: Vec<String>,
}

/// Whether `link` names a platform's account: `<platform>/<account>`, a platform of lower-case
/// letters, digits and `-`, an account of 1 to 64 characters without spaces or commas.
#[must_use]
pub fn is_link(link: &str) -> bool {
    let Some((platform, account)) = link.split_once('/') else { return false };
    !platform.is_empty()
        && platform.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && (1..=64).contains(&account.chars().count())
        && !account.chars().any(|c| c.is_whitespace() || c == ',' || c.is_control())
}

/// The longest account name.
pub const MAX_NAME: usize = 32;
/// The shortest and longest password, in characters.
pub const MIN_PASSWORD: usize = 10;
pub const MAX_PASSWORD: usize = 128;

/// Why `name` cannot name an account: 1 to [`MAX_NAME`] of `a-z`, `0-9`, `.`, `_`, `-`.
#[must_use]
pub fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.len() > MAX_NAME {
        return Some("a name has 1 to 32 characters");
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')) {
        return Some("a name is lower-case letters, digits, '.', '_' and '-'");
    }
    None
}

/// Why `password` cannot be a password: [`MIN_PASSWORD`] to [`MAX_PASSWORD`] characters.
#[must_use]
pub fn password_problem(password: &str) -> Option<&'static str> {
    let n = password.chars().count();
    if n < MIN_PASSWORD {
        return Some("a password has at least 10 characters");
    }
    if n > MAX_PASSWORD {
        return Some("a password has at most 128 characters");
    }
    None
}

/// The accounts in `text`; a line that is not an account (a bad name or role, no hash) is skipped.
#[must_use]
pub fn parse(text: &str) -> Vec<Account> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let (name, role, hash) = (parts.next()?, parts.next()?, parts.next()?);
            let links: Vec<String> = parts.next().map_or_else(Vec::new, |l| l.split(',').map(String::from).collect());
            if parts.next().is_some() || name_problem(name).is_some() || !links.iter().all(|l| is_link(l)) {
                return None;
            }
            Some(Account { name: name.into(), role: Role::parse(role)?, hash: hash.into(), links })
        })
        .collect()
}

/// `accounts` as text, one a line.
#[must_use]
pub fn encode(accounts: &[Account]) -> String {
    let mut out = String::new();
    for a in accounts {
        for part in [a.name.as_str(), " ", a.role.name(), " ", a.hash.as_str()] {
            out.push_str(part);
        }
        if !a.links.is_empty() {
            out.push(' ');
            out.push_str(&a.links.join(","));
        }
        out.push('\n');
    }
    out
}

/// `bytes` as lower-case hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 15)]));
    }
    out
}

/// The bytes `text` holds as hex (either case); none when it is not hex.
#[must_use]
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let digit = |c: u8| char::from(c).to_digit(16).and_then(|d| u8::try_from(d).ok());
    text.as_bytes().chunks(2).map(|p| Some(digit(p[0])? << 4 | digit(p[1])?)).collect()
}
