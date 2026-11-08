//! The Epic accounts lanstash signs in to ([`EpicAccounts`]), as the service
//! shows them ([`Account`]). A person signs in on Epic's own page and pastes
//! the one-time code it shows (or the whole page); lanstash trades it for
//! tokens and keeps the account's refresh token among the secrets
//! (`epic-token-<account id>`, its refresh token, when that runs out, and the
//! account's name; the list in `epic-accounts`). Several may be signed in;
//! signing in again with one renews it. An access token is asked for when one
//! is needed ([`EpicAccounts::access_token`]) and kept until shortly before it
//! runs out; Epic hands out a new refresh token with each, which replaces the
//! kept one. An account whose refresh token ran out or was refused needs
//! signing in again, and says so. The requests name the Epic launcher's own
//! client unless another is set from the account page ([`CLIENT`], kept among
//! the secrets).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::json::string;
use lanstash_epic::auth::{
    ACCOUNT_HOST, Client, TOKEN_PATH, Tokens, basic, code_body, code_from, is_account_id, kept, login_url,
    parse_client, parse_tokens, refresh_body, unkept,
};
use lanstash_ports::account::Account;
use lanstash_ports::clock::Clock;
use lanstash_ports::secrets::Secrets;
use lanstash_ports::web::{Tls, Web, WebRequest};

/// The secret listing the accounts signed in (their ids), one a line, in the order they signed in.
pub const ACCOUNTS: &str = "epic-accounts";

/// The secret holding `account`'s refresh token, when it runs out, and its name.
#[must_use]
pub fn token_name(account: &str) -> String {
    format!("epic-token-{}", account.to_ascii_lowercase())
}

/// The secret holding the client set in place of the built-in one (`<id>:<secret>`).
pub const CLIENT: &str = "epic-client";

/// An access token is renewed this long before it runs out.
const MARGIN_MS: u64 = 5 * 60 * 1000;

/// What is wired in from outside.
pub struct Wiring {
    pub web: Arc<dyn Web>,
    pub secrets: Arc<dyn Secrets>,
    pub clock: Arc<dyn Clock>,
    /// The client the requests name unless another is set ([`CLIENT`]): the launcher's own.
    pub client_id: String,
    pub client_secret: String,
}

impl std::fmt::Debug for Wiring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wiring").field("client_id", &self.client_id).finish_non_exhaustive()
    }
}

/// One signed-in account, as kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub id: String,
    pub name: String,
    /// When its refresh token runs out (ms since the epoch; 0: not said).
    pub refresh_until_ms: u64,
}

/// The Epic accounts.
pub struct EpicAccounts {
    wiring: Wiring,
    /// Each account's access token and when it runs out.
    access: Mutex<BTreeMap<String, (String, u64)>>,
    /// Accounts whose refresh Epic refused.
    refused: Mutex<BTreeSet<String>>,
    /// Why the last sign-in failed.
    error: Mutex<Option<String>>,
    /// Held while a refresh token is used: one refresh at a time, so two callers near
    /// expiry do not both spend the same refresh token.
    refreshing: Mutex<()>,
}

impl std::fmt::Debug for EpicAccounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpicAccounts").field("wiring", &self.wiring).finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl EpicAccounts {
    #[must_use]
    pub fn new(wiring: Wiring) -> Self {
        Self {
            wiring,
            access: Mutex::default(),
            refused: Mutex::default(),
            error: Mutex::default(),
            refreshing: Mutex::default(),
        }
    }

    /// The client set in place of the built-in one, if one is.
    fn own_client(&self) -> Option<(String, String)> {
        parse_client(&self.wiring.secrets.load(CLIENT).ok().flatten()?)
    }

    /// The client the requests name: the one set, else the built-in one.
    fn client(&self) -> (String, String) {
        self.own_client().unwrap_or_else(|| (self.wiring.client_id.clone(), self.wiring.client_secret.clone()))
    }

    /// The page a person signs in on (for the client the requests name).
    #[must_use]
    pub fn login_url(&self) -> String {
        login_url(&self.client().0)
    }

    fn ids(&self) -> Vec<String> {
        let text = self.wiring.secrets.load(ACCOUNTS).ok().flatten().unwrap_or_default();
        text.lines().map(str::trim).filter(|l| is_account_id(l)).map(str::to_string).collect()
    }

    /// The accounts signed in, in the order they signed in.
    #[must_use]
    pub fn accounts(&self) -> Vec<Kept> {
        self.ids()
            .into_iter()
            .filter_map(|id| {
                let (_, until, name) = unkept(&self.wiring.secrets.load(&token_name(&id)).ok().flatten()?)?;
                Some(Kept { name: if name.is_empty() { id.clone() } else { name }, id, refresh_until_ms: until })
            })
            .collect()
    }

    /// Sends a token request with `body`; Epic's tokens.
    fn tokens(&self, body: &str) -> Result<Tokens, String> {
        let (id, secret) = self.client();
        let auth = basic(Client { id: &id, secret: &secret });
        let headers = [("Authorization", auth.as_str()), ("Content-Type", "application/x-www-form-urlencoded")];
        let req = WebRequest {
            method: "POST",
            host: ACCOUNT_HOST,
            port: 443,
            target: TOKEN_PATH,
            headers: &headers,
            body: body.as_bytes(),
            tls: Some(Tls { pin: None, unchecked: false }),
        };
        let answer = self.wiring.web.send(&req).map_err(|e| format!("Epic's account service: {e}"))?;
        let text = String::from_utf8_lossy(&answer.body);
        match parse_tokens(&text) {
            Ok(t) if answer.status == 200 => Ok(t),
            Ok(_) => Err(format!("Epic's account service answered {}", answer.status)),
            Err(e) => Err(format!("Epic: {e}")),
        }
    }

    /// Keeps `t`'s refresh token (adding the account to the list), and its access token.
    fn keep(&self, t: &Tokens) -> Result<(), String> {
        let now = self.wiring.clock.now_ms();
        let until = if t.refresh_expires_in == 0 { 0 } else { now + t.refresh_expires_in * 1000 };
        let s = &self.wiring.secrets;
        s.save(&token_name(&t.account_id), &kept(&t.refresh_token, until, &t.display_name))?;
        let mut ids = self.ids();
        if !ids.contains(&t.account_id) {
            ids.push(t.account_id.clone());
            s.save(ACCOUNTS, &(ids.join("\n") + "\n"))?;
        }
        lock(&self.access).insert(t.account_id.clone(), (t.access_token.clone(), now + t.expires_in * 1000));
        lock(&self.refused).remove(&t.account_id);
        Ok(())
    }

    /// Signs in with what was pasted from Epic's page (the code, or the whole page); the account's name.
    ///
    /// # Errors
    /// No code in it, Epic refusing it (a code is good once, for a few minutes), or the secrets.
    pub fn add(&self, pasted: &str) -> Result<String, String> {
        let result = code_from(pasted)
            .ok_or_else(|| String::from("that is not a code from Epic's page: paste its authorizationCode"))
            .and_then(|code| self.tokens(&code_body(&code)))
            .and_then(|t| self.keep(&t).map(|()| t.display_name));
        *lock(&self.error) = result.as_ref().err().cloned();
        result
    }

    /// An access token for `account` (an id): the one kept while it lasts, else a new one.
    ///
    /// # Errors
    /// The account is not signed in, or Epic refuses its refresh token (it then needs signing in again).
    pub fn access_token(&self, account: &str) -> Result<String, String> {
        let now = self.wiring.clock.now_ms();
        let fresh =
            || lock(&self.access).get(account).filter(|(_, until)| now + MARGIN_MS < *until).map(|(t, _)| t.clone());
        if let Some(token) = fresh() {
            return Ok(token);
        }
        let _one = lock(&self.refreshing);
        // Another caller may have refreshed it while this one waited.
        if let Some(token) = fresh() {
            return Ok(token);
        }
        let text =
            self.wiring.secrets.load(&token_name(account))?.ok_or_else(|| format!("{account} is not signed in"))?;
        let (refresh, until, _) = unkept(&text).ok_or_else(|| format!("{account}'s kept sign-in is unreadable"))?;
        if until != 0 && now >= until {
            lock(&self.refused).insert(account.to_string());
            return Err(format!("{account}'s sign-in ran out: sign in again"));
        }
        match self.tokens(&refresh_body(&refresh)) {
            Ok(t) => {
                self.keep(&t)?;
                Ok(t.access_token)
            }
            Err(e) => {
                if e.starts_with("Epic: ") {
                    lock(&self.refused).insert(account.to_string());
                }
                Err(e)
            }
        }
    }

    /// Whether `account` needs signing in again.
    fn refused(&self, k: &Kept) -> bool {
        lock(&self.refused).contains(&k.id)
            || (k.refresh_until_ms != 0 && self.wiring.clock.now_ms() >= k.refresh_until_ms)
    }
}

impl Account for EpicAccounts {
    fn view(&self) -> String {
        let accounts = self.accounts();
        let list: Vec<String> = accounts
            .iter()
            .map(|k| {
                format!("{{\"name\":{},\"id\":{},\"refused\":{}}}", string(&k.name), string(&k.id), self.refused(k))
            })
            .collect();
        let first = accounts.first().map_or_else(|| "null".to_string(), |k| string(&k.name));
        let error = lock(&self.error).clone();
        let (state, error) = match error {
            Some(e) => ("failed", string(&e)),
            None => ("idle", "null".to_string()),
        };
        let client = if self.own_client().is_some() { "own" } else { "built-in" };
        format!(
            "{{\"accounts\":[{}],\"account\":{first},\"sign_in\":{{\"state\":\"{state}\",\"scanned\":false,\"error\":{error}}},\"method\":\"code\",\"login_url\":{},\"client\":\"{client}\"}}\n",
            list.join(","),
            string(&self.login_url())
        )
    }

    fn set_client(&self, client: Option<&str>) -> Result<(), String> {
        match client {
            None => self.wiring.secrets.remove(CLIENT)?,
            Some(text) => {
                let (id, secret) = parse_client(text).ok_or("a client is <id>:<secret>, letters and digits")?;
                self.wiring.secrets.save(CLIENT, &format!("{id}:{secret}"))?;
            }
        }
        // Access tokens of the other client are not used again.
        lock(&self.access).clear();
        Ok(())
    }

    fn start_sign_in(&self) -> Result<(), String> {
        Err("Epic signs in with the code its own page shows: paste it".into())
    }

    fn cancel_sign_in(&self) {
        *lock(&self.error) = None;
    }

    fn sign_in_with_code(&self, pasted: &str) -> Result<(), String> {
        self.add(pasted).map(drop)
    }

    fn sign_out(&self, account: &str) -> Result<(), String> {
        let ids = self.ids();
        let id = ids
            .iter()
            .find(|id| id.eq_ignore_ascii_case(account))
            .cloned()
            .or_else(|| self.accounts().into_iter().find(|k| k.name == account).map(|k| k.id))
            .ok_or_else(|| format!("{account} is not signed in"))?;
        let s = &self.wiring.secrets;
        s.remove(&token_name(&id))?;
        let rest: Vec<&String> = ids.iter().filter(|x| **x != id).collect();
        if rest.is_empty() {
            s.remove(ACCOUNTS)?;
        } else {
            s.save(ACCOUNTS, &(rest.iter().map(|x| x.as_str()).collect::<Vec<_>>().join("\n") + "\n"))?;
        }
        lock(&self.access).remove(&id);
        lock(&self.refused).remove(&id);
        Ok(())
    }

    fn qr_svg(&self) -> Option<String> {
        None
    }
}
