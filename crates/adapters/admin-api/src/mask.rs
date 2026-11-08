//! The games kept private, masked in what the admin API answers anyone who is not an admin. Each
//! hidden game gets a token (`hidden-1`, ... in the order of the list) that stands for it in every
//! answer, so sizes still add up per game but neither its name nor any of its ids show (a Steam
//! depot or app id, a Battle.net code would each name it). A Steam depot is traced to its app
//! through the stash's own entries.
//!
//! The masking works on any JSON answer by its shape, so an answer added later is masked too:
//! - an object with a `game`, `group`, `code` or `product` of a hidden game (its platform from its
//!   `service` or `platform`, or from where it sits: under a platform's key, or the page's
//!   platform) is the game's: in a list that leaves hidden games out (a platform's builds, its
//!   catalog) it goes; elsewhere its ids become the token, its `name` "A hidden game", what tells
//!   of the game's kind (`service`, `platform`, `change`, `state`, `kind`, `region`), its numbers
//!   and its yes-or-no stay, every other string is emptied (builds, paths, errors), every object
//!   in it goes, and it is marked `"hidden": true`;
//! - a string naming a hidden game's directory (`served/<p>/<dir>`, a client's
//!   `client/<address>/<p>/<dir>/<hit|miss>`, `<p>/<dir>/...`), or a game
//!   (`<p>/<game>`, or a bare id in a list under a platform) becomes its token in the same place;
//! - `owners` (the accounts a game is on) are left out everywhere.

use std::collections::{BTreeMap, BTreeSet};

use lanstash_domain::json::{Value, parse, write};

/// What a masked game is called.
pub const HIDDEN_NAME: &str = "A hidden game";

const IDS: &[&str] = &["game", "group", "code", "product"];
const KINDS: &[&str] = &["service", "platform", "change", "state", "kind", "region"];

/// The hidden games and how to find them.
#[derive(Debug, Clone, Default)]
pub struct Mask {
    /// `<platform>/<game>` to its token.
    tokens: BTreeMap<String, String>,
    /// (platform, store directory) to the game it belongs to (a Steam depot's app).
    apps: BTreeMap<(String, String), String>,
    platforms: BTreeSet<String>,
}

impl Mask {
    /// The mask of the `hidden` games (`<platform>/<game>`), with the stash's directories:
    /// (platform, directory, the game it belongs to).
    #[must_use]
    pub fn new(hidden: &[String], dirs: &[(String, String, Option<String>)]) -> Self {
        let tokens: BTreeMap<String, String> =
            hidden.iter().enumerate().map(|(i, k)| (k.clone(), format!("hidden-{}", i + 1))).collect();
        let platforms = hidden.iter().filter_map(|k| k.split_once('/').map(|(p, _)| p.to_string())).collect();
        let apps = dirs.iter().filter_map(|(p, d, g)| Some(((p.clone(), d.clone()), g.clone()?))).collect();
        Self { tokens, apps, platforms }
    }

    /// Whether nothing is hidden.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// The token of the game `id` (or of the game the directory `id` belongs to) on `platform`.
    #[must_use]
    pub fn token(&self, platform: &str, id: &str) -> Option<&str> {
        let direct = self.tokens.get(&format!("{platform}/{id}"));
        direct
            .or_else(|| {
                let app = self.apps.get(&(platform.to_string(), id.to_string()))?;
                self.tokens.get(&format!("{platform}/{app}"))
            })
            .map(String::as_str)
    }

    /// `text` masked (a platform's page gives its `platform`; `drop`: hidden games leave lists);
    /// what is not JSON comes back as it was.
    #[must_use]
    pub fn json(&self, text: &str, platform: Option<&str>, drop: bool) -> String {
        match parse(text) {
            Ok(v) => {
                let out = self.walk(v, platform, drop, false).map_or_else(|| "null".into(), |v| write(&v));
                if text.ends_with('\n') { out + "\n" } else { out }
            }
            Err(_) => text.to_string(),
        }
    }

    /// A string masked: a directory's counter or key, a game's key, or (in a list) a bare id.
    fn string(&self, s: &str, platform: Option<&str>, listed: bool) -> Option<String> {
        if let Some(rest) = s.strip_prefix("served/") {
            let (p, dir) = rest.split_once('/')?;
            return self.token(p, dir).map(|t| format!("served/{p}/{t}"));
        }
        if let Some(rest) = s.strip_prefix("client/") {
            let mut parts = rest.splitn(4, '/');
            let (address, p, dir) = (parts.next()?, parts.next()?, parts.next()?);
            let tail = parts.next().map_or_else(String::new, |t| format!("/{t}"));
            return self.token(p, dir).map(|t| format!("client/{address}/{p}/{t}{tail}"));
        }
        if let Some((p, rest)) = s.split_once('/')
            && self.platforms.contains(p)
        {
            let id = rest.split('/').next().unwrap_or(rest);
            return self.token(p, id).map(|t| format!("{p}/{t}"));
        }
        if listed {
            return self.token(platform?, s).map(String::from);
        }
        None
    }

    fn walk(&self, v: Value, platform: Option<&str>, drop: bool, listed: bool) -> Option<Value> {
        match v {
            Value::Str(s) => Some(Value::Str(self.string(&s, platform, listed).unwrap_or(s))),
            Value::Array(items) => {
                Some(Value::Array(items.into_iter().filter_map(|x| self.walk(x, platform, drop, true)).collect()))
            }
            Value::Object(members) => self.object(members, platform, drop, listed),
            other => Some(other),
        }
    }

    fn object(&self, members: Vec<(String, Value)>, ctx: Option<&str>, drop: bool, listed: bool) -> Option<Value> {
        let own = ["service", "platform"]
            .iter()
            .find_map(|k| members.iter().find(|(m, _)| m == k).and_then(|(_, v)| v.as_str().map(String::from)));
        let platform = own.as_deref().or(ctx);
        let token = platform.and_then(|p| {
            members
                .iter()
                .filter(|(k, _)| IDS.contains(&k.as_str()))
                .find_map(|(_, v)| self.token(p, v.as_str()?))
                .map(String::from)
        });
        let Some(token) = token else {
            let out = members
                .into_iter()
                .filter(|(k, _)| k != "owners")
                .filter_map(|(k, v)| {
                    let under = if self.platforms.contains(&k) { Some(k.as_str()) } else { platform };
                    let v = self.walk(v, under, drop, false)?;
                    Some((k, v))
                })
                .collect();
            return Some(Value::Object(out));
        };
        if drop && listed {
            return None;
        }
        let mut out: Vec<(String, Value)> = members
            .into_iter()
            .filter(|(k, _)| k != "owners" && k != "hidden")
            .filter_map(|(k, v)| {
                let masked = match v {
                    Value::Str(_) if IDS.contains(&k.as_str()) => Value::Str(token.clone()),
                    Value::Str(_) if k == "name" => Value::Str(HIDDEN_NAME.into()),
                    Value::Str(s) if KINDS.contains(&k.as_str()) => Value::Str(s),
                    Value::Str(_) => Value::Str(String::new()),
                    Value::Object(_) => Value::Null,
                    Value::Array(a) => self.walk(Value::Array(a), platform, drop, false)?,
                    other => other,
                };
                Some((k, masked))
            })
            .collect();
        if !out.iter().any(|(k, _)| k == "name") {
            out.push(("name".into(), Value::Str(HIDDEN_NAME.into())));
        }
        out.push(("hidden".into(), Value::Bool(true)));
        Some(Value::Object(out))
    }
}
