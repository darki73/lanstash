//! Signing in to Epic, as Epic's own launcher does: the person
//! signs in on Epic's own page ([`login_url`]), which then shows a small JSON
//! page with a one-time `authorizationCode`; that code ([`code_from`]: the code
//! itself, or the whole page pasted) is traded for tokens at Epic's account
//! service ([`code_body`]), the refresh token kept and traded for new tokens
//! as they run out ([`refresh_body`]). The client the requests name (an id and
//! a secret, sent as Basic authentication: [`basic`]) is the launcher's own
//! ([`LAUNCHER_CLIENT`]) unless another is set. Epic's answer ([`parse_tokens`]) gives the tokens, how long
//! each lasts, and the account's id and name; an error answer its message.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use lanstash_domain::checksum::sha1::base64;
use lanstash_domain::json::{Value, parse};

/// Epic's account service.
pub const ACCOUNT_HOST: &str = "account-public-service-prod03.ol.epicgames.com";
/// Where codes and refresh tokens are traded for tokens.
pub const TOKEN_PATH: &str = "/account/api/oauth/token";

/// The client the requests name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client<'a> {
    pub id: &'a str,
    pub secret: &'a str,
}

/// The Epic Games launcher's own client, public. The one lanstash names unless another
/// is set ([`parse_client`]).
pub const LAUNCHER_CLIENT: Client<'static> =
    Client { id: "34a02cf8f4414e29b15921876da36f9a", secret: "daafbccc737745039dffe53d94fc76cf" };

/// A client written `<id>:<secret>` (letters and digits, each at most 64), as kept or typed in.
#[must_use]
pub fn parse_client(text: &str) -> Option<(String, String)> {
    let word = |s: &str| !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_alphanumeric());
    let (id, secret) = text.trim().split_once(':')?;
    (word(id.trim()) && word(secret.trim())).then(|| (id.trim().to_string(), secret.trim().to_string()))
}

/// `s` with every byte but `[A-Za-z0-9-._~]` written `%XX`.
#[must_use]
pub fn percent(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The page a person signs in on: Epic's login, which then shows the code for `client`.
#[must_use]
pub fn login_url(client_id: &str) -> String {
    let redirect =
        format!("https://www.epicgames.com/id/api/redirect?clientId={}&responseType=code", percent(client_id));
    format!("https://www.epicgames.com/id/login?redirectUrl={}", percent(&redirect))
}

/// The one-time code in what was pasted: the code itself (letters and digits), or the JSON page
/// that carries it as `authorizationCode`.
#[must_use]
pub fn code_from(pasted: &str) -> Option<String> {
    let t = pasted.trim();
    let code = if t.starts_with('{') {
        parse(t).ok()?.get("authorizationCode")?.as_str()?.to_string()
    } else {
        t.trim_matches('"').to_string()
    };
    (!code.is_empty() && code.len() <= 128 && code.bytes().all(|b| b.is_ascii_alphanumeric())).then_some(code)
}

/// The form that trades a one-time code for tokens.
#[must_use]
pub fn code_body(code: &str) -> String {
    format!("grant_type=authorization_code&code={}&token_type=eg1", percent(code))
}

/// The form that trades a refresh token for new tokens.
#[must_use]
pub fn refresh_body(refresh_token: &str) -> String {
    format!("grant_type=refresh_token&refresh_token={}&token_type=eg1", percent(refresh_token))
}

/// The `Authorization` header's value naming `client`.
#[must_use]
pub fn basic(client: Client<'_>) -> String {
    format!("basic {}", base64(format!("{}:{}", client.id, client.secret).as_bytes()))
}

/// Epic's tokens for one account.
#[derive(Clone, PartialEq, Eq)]
pub struct Tokens {
    pub access_token: String,
    /// How long the access token lasts (seconds).
    pub expires_in: u64,
    pub refresh_token: String,
    /// How long the refresh token lasts (seconds).
    pub refresh_expires_in: u64,
    pub account_id: String,
    pub display_name: String,
}

impl core::fmt::Debug for Tokens {
    /// Never prints the tokens.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Tokens")
            .field("account_id", &self.account_id)
            .field("display_name", &self.display_name)
            .finish_non_exhaustive()
    }
}

/// Whether `id` is an account id as Epic writes them (32 hex digits), safe in a secret's name.
#[must_use]
pub fn is_account_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(ToString::to_string)
}

fn seconds(v: &Value, key: &str) -> Option<u64> {
    match v.get(key)? {
        Value::Number(n) if *n >= 0.0 => Some(*n as u64),
        _ => None,
    }
}

/// Epic's answer to a token request: the tokens, or the error it says (its message, else its code).
///
/// # Errors
/// An error answer, or one that is not tokens.
pub fn parse_tokens(body: &str) -> Result<Tokens, String> {
    let v = parse(body).map_err(|_| String::from("Epic's answer is not JSON"))?;
    if let Some(message) = text(&v, "errorMessage").or_else(|| text(&v, "errorCode")) {
        return Err(message);
    }
    let missing = |what: &str| format!("Epic's answer has no {what}");
    let account_id = text(&v, "account_id").ok_or_else(|| missing("account id"))?.to_ascii_lowercase();
    if !is_account_id(&account_id) {
        return Err(format!("Epic's account id {account_id:?} is not 32 hex digits"));
    }
    let display_name = text(&v, "displayName").unwrap_or_else(|| account_id.clone());
    Ok(Tokens {
        access_token: text(&v, "access_token").ok_or_else(|| missing("access token"))?,
        expires_in: seconds(&v, "expires_in").ok_or_else(|| missing("lifetime"))?,
        refresh_token: text(&v, "refresh_token").ok_or_else(|| missing("refresh token"))?,
        refresh_expires_in: seconds(&v, "refresh_expires").unwrap_or(0),
        account_id,
        display_name,
    })
}

/// What is kept of an account between runs: its refresh token, when that runs out (ms since
/// the epoch), and its name. One line each.
#[must_use]
pub fn kept(refresh_token: &str, refresh_until_ms: u64, display_name: &str) -> String {
    let name: String = display_name.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    format!("{refresh_token}\n{refresh_until_ms}\n{name}\n")
}

/// What [`kept`] wrote: the refresh token, when it runs out, the name; none when it is not that.
#[must_use]
pub fn unkept(text: &str) -> Option<(String, u64, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let token = lines.first()?.trim();
    let until = lines.get(1)?.trim().parse().ok()?;
    let name = lines.get(2).map_or("", |n| n.trim());
    (!token.is_empty()).then(|| (token.to_string(), until, name.to_string()))
}
