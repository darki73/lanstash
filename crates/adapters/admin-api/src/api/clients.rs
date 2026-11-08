//! Who downloads what, through the API:
//! - every status snapshot has `clients`: those downloading now, with their names;
//! - `GET /api/v1/clients?from=<ms>&to=<ms>`: what each client got over the range, per
//!   game (store directory), from the counters kept by the minute:
//!   `{"clients":[{"address","label","host","hit","miss","games":[{"service","game","hit","miss"}]}]}`,
//!   the client that got most first;
//! - `PUT /api/v1/clients/<address>?label=<name>`: names a client (an empty name takes it
//!   away), `DELETE /api/v1/clients/<address>`: takes its name away.

use std::sync::Arc;

use lanstash_app::client_history::over;
use lanstash_app::client_names::{ClientName, ClientNames};
use lanstash_app::serve::clients::Downloading;
use lanstash_domain::json::string;

use super::settings::decode;
use super::{AdminApi, Answer};
use crate::render::client_name_fields;
use crate::route::{CLIENTS, Route, rest};

/// The value of `key` in `target`'s query, as given.
fn param<'a>(target: &'a str, key: &str) -> Option<&'a str> {
    let (_, q) = target.split_once('?')?;
    q.split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))
}

impl AdminApi {
    /// Enables `/api/v1/clients` and the names of the clients in the status.
    #[must_use]
    pub fn with_clients(mut self, names: Arc<ClientNames>) -> Self {
        self.clients = Some(names);
        self
    }

    /// The clients downloading now, with their names (none without names or counting).
    pub(super) fn live_clients(&self) -> Vec<(Downloading, ClientName)> {
        let (Some(names), Some(traffic)) = (&self.clients, self.status.cache().clients()) else {
            return Vec::new();
        };
        traffic
            .now()
            .into_iter()
            .map(|d| {
                let n = names.name(&d.client);
                (d, n)
            })
            .collect()
    }

    /// `/api/v1/clients` and `/api/v1/clients/<address>`.
    pub(super) fn clients(&self, route: Route, target: &str) -> Answer {
        let Some(names) = &self.clients else {
            return Answer::error(target, 404, "not-found", "clients are not counted");
        };
        match route {
            Route::ClientLabel | Route::ClientUnlabel => {
                // Escaped as a path segment (an IPv6 address's colons).
                let address = decode(rest(target, CLIENTS)).unwrap_or_default();
                let address = address.as_str();
                let label = match route {
                    Route::ClientLabel => param(target, "label").and_then(decode).unwrap_or_default(),
                    _ => String::new(),
                };
                match names.set_label(address, &label) {
                    Ok(()) => {
                        let n = names.name(address);
                        Answer::json(200, format!("{{{}}}\n", client_name_fields(address, &n)))
                    }
                    Err(e) => Answer::error(target, 400, "bad-request", &e),
                }
            }
            _ => self.clients_over(names, target),
        }
    }

    /// `GET /api/v1/clients?from=&to=`.
    fn clients_over(&self, names: &ClientNames, target: &str) -> Answer {
        let Some(stats) = &self.stats else {
            return Answer::error(target, 404, "not-found", "no counters are kept");
        };
        let num = |k| param(target, k).and_then(|v| v.parse::<u64>().ok());
        let (Some(from), Some(to)) = (num("from"), num("to")) else {
            return Answer::error(target, 400, "bad-request", "from and to (ms) are needed");
        };
        if to <= from {
            return Answer::error(target, 400, "bad-request", "from before to");
        }
        match over(stats, from, to) {
            Ok(clients) => {
                let rows: Vec<String> = clients
                    .iter()
                    .map(|c| {
                        let games: Vec<String> = c
                            .games
                            .iter()
                            .map(|g| {
                                format!(
                                    "{{\"service\":{},\"game\":{},\"hit\":{},\"miss\":{}}}",
                                    string(&g.service),
                                    string(&g.game),
                                    g.hit,
                                    g.miss
                                )
                            })
                            .collect();
                        format!(
                            "{{{},\"hit\":{},\"miss\":{},\"games\":[{}]}}",
                            client_name_fields(&c.client, &names.name(&c.client)),
                            c.hit,
                            c.miss,
                            games.join(",")
                        )
                    })
                    .collect();
                Answer::json(200, format!("{{\"clients\":[{}]}}\n", rows.join(",")))
            }
            Err(e) => Answer::error(target, 503, "unavailable", &e),
        }
    }
}
