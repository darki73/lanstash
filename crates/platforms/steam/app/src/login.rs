//! Logging in to Steam: account name and password (the password RSA
//! encrypted with the key Steam hands out, never kept), a Steam Guard code or
//! an approval in the mobile app, then the refresh token kept in the secrets
//! so later logons need neither. Several accounts may be kept, each its own
//! token, listed in the order they signed in ([`accounts`], [`keep`], [`forget`]).

use lanstash_domain::checksum::sha1::base64;
use lanstash_domain::rsa::PublicKey;
use lanstash_ports::prompt::Prompt;
use lanstash_ports::random::Random;
use lanstash_ports::secrets::Secrets;
use lanstash_steam_domain::cm::auth::{
    Guard, RsaKey, Session, Tokens, begin_session, guard_code, method, poll, rsa_key_request,
};
use lanstash_steam_domain::cm::eresult::{INVALID_LOGIN_AUTH_CODE, TWO_FACTOR_CODE_MISMATCH};

use crate::connection::{CallError, Connection};

/// The single-account layout's refresh token ([`accounts`] moves it to the list).
pub const REFRESH_TOKEN: &str = "steam-refresh-token";
/// The single-account layout's account name.
pub const ACCOUNT: &str = "steam-account";
/// The secret listing the accounts signed in, one name a line, in the order they signed in.
pub const ACCOUNTS: &str = "steam-accounts";

/// The secret holding `account`'s refresh token: `steam-token-` and the name, lower case,
/// `_` written as `-` (Steam's names are `[A-Za-z0-9_]`, case-insensitive), else its bytes in hex.
#[must_use]
pub fn token_name(account: &str) -> String {
    let lower = account.to_ascii_lowercase();
    if !lower.is_empty() && lower.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
        format!("steam-token-{}", lower.replace('_', "-"))
    } else {
        let hex: String = account.bytes().map(|b| format!("{b:02x}")).collect();
        format!("steam-token-x{hex}")
    }
}

/// The accounts signed in, in the order they signed in. An account kept in the
/// single-account layout (its token under [`REFRESH_TOKEN`]) is moved to the list first.
///
/// # Errors
/// The secrets.
pub fn accounts(secrets: &dyn Secrets) -> Result<Vec<String>, String> {
    if let Some(token) = secrets.load(REFRESH_TOKEN)? {
        let name = secrets.load(ACCOUNT)?.map(|a| a.trim().to_string()).filter(|a| !a.is_empty());
        add(secrets, name.as_deref().unwrap_or("steam"), token.trim())?;
        secrets.remove(REFRESH_TOKEN)?;
        secrets.remove(ACCOUNT)?;
    }
    list(secrets)
}

fn list(secrets: &dyn Secrets) -> Result<Vec<String>, String> {
    let text = secrets.load(ACCOUNTS)?.unwrap_or_default();
    Ok(text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect())
}

/// Keeps `token` for `account`, adding it to the list when it is not there (by name, any case).
fn add(secrets: &dyn Secrets, account: &str, token: &str) -> Result<(), String> {
    secrets.save(&token_name(account), token)?;
    let mut names = list(secrets)?;
    if !names.iter().any(|n| n.eq_ignore_ascii_case(account)) {
        names.push(account.to_string());
        secrets.save(ACCOUNTS, &(names.join("\n") + "\n"))?;
    }
    Ok(())
}

/// `account`'s kept refresh token (a single-account layout moved to the list first).
///
/// # Errors
/// The secrets.
pub fn token(secrets: &dyn Secrets, account: &str) -> Result<Option<String>, String> {
    accounts(secrets)?;
    Ok(secrets.load(&token_name(account))?.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()))
}

/// How many Steam Guard codes may be typed before giving up.
const CODE_TRIES: u32 = 3;
/// How many times to look whether the sign-in was approved.
const POLLS: u32 = 10;

/// What a login needs besides the connection.
pub struct Login<'a> {
    pub prompt: &'a dyn Prompt,
    pub random: &'a dyn Random,
    pub secrets: &'a dyn Secrets,
    /// This machine's name in the account's list of devices.
    pub device: &'a str,
}

impl std::fmt::Debug for Login<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Login").field("device", &self.device).finish_non_exhaustive()
    }
}

fn unhex(s: &str) -> Result<Vec<u8>, String> {
    let bad = || String::from("the RSA key from Steam is not hex");
    if !s.len().is_multiple_of(2) {
        return Err(bad());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()).ok_or_else(bad))
        .collect()
}

fn said(e: &CallError) -> String {
    e.to_string()
}

fn shown(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}

/// Asks for the account and password, confirms with Steam Guard, and keeps
/// the refresh token; returns the account name.
///
/// # Errors
/// The connection, Steam's answer (wrong password, rate limit), the prompt,
/// or the secrets.
pub fn login(conn: &mut Connection, with: &Login<'_>) -> Result<String, String> {
    let account = with.prompt.ask("Steam account name", false)?.trim().to_string();
    if account.is_empty() {
        return Err("no account name".into());
    }
    let password = with.prompt.ask("Password", true)?;
    let answer = conn.call(method::GET_PASSWORD_RSA_PUBLIC_KEY, rsa_key_request(&account)).map_err(|e| said(&e))?;
    let key = RsaKey::decode(&answer.body).map_err(shown)?;
    let public = PublicKey::new(&unhex(&key.modulus_hex)?, &unhex(&key.exponent_hex)?)
        .ok_or("the RSA key from Steam is not usable")?;
    let mut random = vec![0u8; 2 * public.size()];
    with.random.fill(&mut random)?;
    let encrypted = base64(&public.encrypt_pkcs1(password.as_bytes(), &random).ok_or("the password is too long")?);
    drop(password);
    let body = begin_session(with.device, &account, &encrypted, key.timestamp);
    let answer = conn.call(method::BEGIN_AUTH_SESSION_VIA_CREDENTIALS, body).map_err(|e| said(&e))?;
    let session = Session::decode(&answer.body).map_err(shown)?;
    let tokens = confirm(conn, with, &session)?;
    keep(with.secrets, &tokens, &account)
}

/// Gets the session approved (a code, or a tap in the app) and returns its tokens.
fn confirm(conn: &mut Connection, with: &Login<'_>, session: &Session) -> Result<Tokens, String> {
    let has = |g: Guard| session.confirmations.contains(&g);
    let tap = has(Guard::DeviceConfirmation) || has(Guard::EmailConfirmation);
    let code = [Guard::DeviceCode, Guard::EmailCode].into_iter().find(|g| has(*g));
    if let Some(kind) = code {
        let mut question = match kind {
            Guard::DeviceCode => String::from("Steam Guard code from the Steam mobile app"),
            _ => String::from("Steam Guard code from the email Steam just sent"),
        };
        if tap {
            question.push_str(" (or approve the sign-in there and press Enter)");
        }
        for attempt in 1..=CODE_TRIES {
            let typed = with.prompt.ask(&question, false)?.trim().to_uppercase();
            if typed.is_empty() && tap {
                break;
            }
            match conn.call(method::UPDATE_AUTH_SESSION_WITH_STEAM_GUARD_CODE, guard_code(session, &typed, kind)) {
                Ok(_) => break,
                Err(CallError::Steam(INVALID_LOGIN_AUTH_CODE | TWO_FACTOR_CODE_MISMATCH)) if attempt < CODE_TRIES => {
                    with.prompt.tell("Steam did not accept that code; try again.");
                }
                Err(e) => return Err(said(&e)),
            }
        }
    } else if tap {
        with.prompt.tell("Approve this sign-in in the Steam mobile app (or the email from Steam).");
    }
    for _ in 0..POLLS {
        let answer = conn.call(method::POLL_AUTH_SESSION_STATUS, poll(session)).map_err(|e| said(&e))?;
        if let Some(t) = Tokens::decode(&answer.body).map_err(shown)? {
            return Ok(t);
        }
        with.prompt.ask("Not approved yet; press Enter once it is", false)?;
    }
    Err("the sign-in was not approved".into())
}

/// Forgets `account`'s kept token and takes it off the list (its next use needs a sign-in).
///
/// # Errors
/// The secrets.
pub fn forget(secrets: &dyn Secrets, account: &str) -> Result<(), String> {
    let names = accounts(secrets)?;
    let Some(name) = names.iter().find(|n| n.eq_ignore_ascii_case(account)).cloned() else {
        return Err(format!("{account} is not signed in"));
    };
    secrets.remove(&token_name(&name))?;
    let rest: Vec<&String> = names.iter().filter(|n| **n != name).collect();
    if rest.is_empty() {
        secrets.remove(ACCOUNTS)
    } else {
        secrets.save(ACCOUNTS, &(rest.iter().map(|n| n.as_str()).collect::<Vec<_>>().join("\n") + "\n"))
    }
}

/// Keeps the refresh token of the account it is for (`fallback` when Steam named none),
/// beside the others kept; returns the account.
///
/// # Errors
/// The secrets, or no name at all.
pub fn keep(secrets: &dyn Secrets, tokens: &Tokens, fallback: &str) -> Result<String, String> {
    let name = if tokens.account_name.is_empty() { fallback.to_string() } else { tokens.account_name.clone() };
    if name.trim().is_empty() {
        return Err("Steam named no account for this sign-in".into());
    }
    accounts(secrets)?;
    add(secrets, name.trim(), &tokens.refresh_token)?;
    Ok(name.trim().to_string())
}
