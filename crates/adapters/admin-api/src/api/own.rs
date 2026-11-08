//! A player's own games: those a platform account linked to it owns, as the platform's catalog
//! says (`products[]`, each with its `game` and the `owners`: the accounts it is on). A player may
//! prefill them (`prefill.own`), and its catalog answer lists only them.

use std::collections::BTreeMap;

use lanstash_domain::json::{Value, parse, write};

use super::AdminApi;
use super::access::Caller;
use crate::route::{PREFILL, rest};

/// The account names `links` (`<platform>/<account>`) hold on `platform`.
fn on<'a>(links: &'a [String], platform: &str) -> Vec<&'a str> {
    links.iter().filter_map(|l| l.strip_prefix(platform)?.strip_prefix('/')).collect()
}

/// Whether a catalog product is owned by one of `accounts`.
fn owned(product: &Value, accounts: &[&str]) -> bool {
    product
        .get("owners")
        .is_some_and(|o| o.as_array().iter().any(|n| n.as_str().is_some_and(|n| accounts.contains(&n))))
}

/// `catalog` (a platform's catalog view) with only the products `accounts` own.
#[must_use]
pub fn own_catalog(catalog: &str, accounts: &[&str]) -> String {
    let Ok(Value::Object(members)) = parse(catalog) else { return catalog.to_string() };
    let members = members
        .into_iter()
        .map(|(k, v)| match (k.as_str(), v) {
            ("products", Value::Array(all)) => {
                (k, Value::Array(all.into_iter().filter(|p| owned(p, accounts)).collect()))
            }
            (_, v) => (k, v),
        })
        .collect();
    write(&Value::Object(members))
}

impl AdminApi {
    /// The platform accounts linked to the caller (none but for a signed-in account).
    pub(super) fn links_of(&self, caller: &Caller) -> Vec<String> {
        match (caller, &self.sessions) {
            (Caller::Signed { name, .. }, Some(s)) => s.accounts().links(name),
            _ => Vec::new(),
        }
    }

    /// Whether the game a prefill path names (`/api/v1/prefill/<platform>/<game>`) is the caller's own.
    pub(super) fn owns_prefill(&self, caller: &Caller, target: &str) -> bool {
        let mut parts = rest(target, PREFILL).split('/');
        let (Some(platform), Some(game)) = (parts.next(), parts.next()) else { return false };
        self.owns(caller, platform, game)
    }

    /// The platform accounts (`<platform>/<account>`) that own each game of `platform`'s catalog.
    pub(super) fn owners_on(&self, platform: &str) -> BTreeMap<String, Vec<String>> {
        let Some(Ok(catalog)) = self.platform(platform).and_then(|p| p.catalog()) else { return BTreeMap::new() };
        let Ok(doc) = parse(&catalog) else { return BTreeMap::new() };
        let products = doc.get("products").map_or(&[][..], Value::as_array);
        products
            .iter()
            .filter_map(|p| {
                let game = p.get("game").and_then(Value::as_str)?;
                let owners = p.get("owners")?.as_array();
                Some((
                    game.to_owned(),
                    owners.iter().filter_map(Value::as_str).map(|o| format!("{platform}/{o}")).collect(),
                ))
            })
            .collect()
    }

    /// Whether `platform`'s `game` is the caller's own: a platform account linked to it owns it.
    pub(super) fn owns(&self, caller: &Caller, platform: &str, game: &str) -> bool {
        let links = self.links_of(caller);
        let accounts = on(&links, platform);
        if accounts.is_empty() {
            return false;
        }
        let Some(Ok(catalog)) = self.platform(platform).and_then(|p| p.catalog()) else { return false };
        let Ok(doc) = parse(&catalog) else { return false };
        doc.get("products").is_some_and(|ps| {
            ps.as_array().iter().any(|p| p.get("game").and_then(Value::as_str) == Some(game) && owned(p, &accounts))
        })
    }

    /// A catalog answer for the caller: only its own games, for someone who prefills only those.
    pub(super) fn catalog_for(&self, caller: &Caller, platform: &str, body: &str) -> String {
        let links = self.links_of(caller);
        own_catalog(body, &on(&links, platform))
    }
}
