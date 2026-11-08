//! The settings endpoints: `GET /api/v1/settings` (every setting the web UI
//! may change, its value and where it comes from), `PUT
//! /api/v1/settings/<key>?value=<value>` (a change, checked, applied at the
//! next start; 400 with the reason when refused), `DELETE
//! /api/v1/settings/<key>` (the configuration file's value again), and `POST
//! /api/v1/restart[?force=1]` (a clean stop that the service manager follows
//! with a start: 202; 409 `busy` while a prefill runs or a client downloads,
//! unless forced).

use std::sync::Arc;

use lanstash_ports::prefill::PrefillState;

use super::{AdminApi, Answer};
use crate::route::{Route, SETTINGS, rest};

/// `%XX` and `+` in a query value, decoded; `None` when it is not UTF-8.
#[must_use]
pub fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8(out).ok()
}

impl AdminApi {
    /// `GET`, `PUT` and `DELETE` of the settings.
    pub(super) fn settings(&self, route: Route, target: &str) -> Answer {
        let Some(s) = &self.settings else { return Answer::error(target, 404, "not-found", "not found") };
        let key = rest(target, SETTINGS).trim_start_matches('/');
        let done = match route {
            Route::SettingsSet => {
                let value = target
                    .split_once('?')
                    .and_then(|(_, q)| q.split('&').find_map(|kv| kv.strip_prefix("value=")))
                    .and_then(decode);
                match value {
                    Some(v) => s.set(key, &v),
                    None => Err("a value is needed: ?value=...".to_owned()),
                }
            }
            Route::SettingsReset => s.reset(key),
            _ => Ok(()),
        };
        match done {
            Ok(()) => Answer::json(200, s.view()),
            Err(e) => Answer::error(target, 400, "invalid", &e),
        }
    }

    /// `POST /api/v1/restart`: refused while lanstash is at work, unless forced.
    pub(super) fn restart(&self, target: &str) -> Answer {
        let Some(restart) = &self.restart else { return Answer::error(target, 404, "not-found", "not found") };
        let force = target.split_once('?').is_some_and(|(_, q)| q.split('&').any(|kv| kv == "force=1"));
        let prefilling = self.prefills().iter().any(|p| p.status().is_some_and(|s| s.state == PrefillState::Running));
        let downloading = self.status.cache().fetcher().is_some_and(|f| !f.downloads().is_empty());
        if !force && (prefilling || downloading) {
            let why = if prefilling { "a prefill is running" } else { "a client is downloading" };
            return Answer::error(target, 409, "busy", why);
        }
        let restart = Arc::clone(restart);
        // Answer first; the hook ends the process.
        std::thread::spawn(move || restart());
        Answer::json(202, "{\"restarting\":true}\n".to_owned())
    }
}
