//! The job endpoints: starting, stopping and reporting prefills, and
//! deleting a game from the stash.

use lanstash_app::delete_game::DeleteError;
use lanstash_domain::json::string;
use lanstash_ports::platform::ViewError;
use lanstash_ports::prefill::{PrefillFrom, PrefillState, StartError};

use super::{AdminApi, Answer};
use crate::render::prefill_on;
use crate::route::{GAMES, PREFILL, rest, valid_game};

impl AdminApi {
    /// `POST /api/v1/prefill/<platform>/<game>[?build=pre][&from=lancache|lancache-only][&queue=1][&install=<tags>]`;
    /// with `queue=1` a busy platform queues the job instead of refusing it; `install` names another
    /// install than the platform is set up for (Battle.net: `OSX,arm64`), queued like `queue=1`.
    /// `from=lancache-only` takes only what lancache holds, nothing from the CDN.
    pub(super) fn prefill_start(&self, target: &str) -> Answer {
        let query = target.split_once('?').map_or("", |(_, q)| q);
        let pre = query.split('&').any(|q| q == "build=pre");
        let has = |want: &str| query.split('&').any(|q| q == want);
        let queued = has("queue=1");
        let from = if has("from=lancache") {
            PrefillFrom::Lancache
        } else if has("from=lancache-only") {
            PrefillFrom::LancacheOnly
        } else {
            PrefillFrom::Cdn
        };
        let found = rest(target, PREFILL)
            .split_once('/')
            .filter(|(_, game)| valid_game(game))
            .and_then(|(id, game)| Some((id, self.platform(id)?.prefill()?, game)));
        let Some((platform, prefill, game)) = found else {
            return Answer::error(target, 404, "not-found", "no prefill for this game");
        };
        if self.deleter.as_ref().is_some_and(|d| d.deleting(platform, game)) {
            return Answer::error(target, 409, "busy", "it is being deleted; wait for that first");
        }
        // Another install than the platform is set up for: its tags, comma-separated (`OSX,arm64`).
        let install = query.split('&').find_map(|q| q.strip_prefix("install=")).map(|v| v.replace("%2C", ","));
        if install.as_deref().is_some_and(|i| {
            i.is_empty() || i.len() > 64 || !i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b',' || b == b'_')
        }) {
            return Answer::error(target, 400, "bad-request", "install: tag names, comma-separated");
        }
        let started = match install.as_deref() {
            // Started now when nothing runs, else queued: the job is asked for by its install.
            Some(i) => prefill.queue_for(game, pre, from, Some(i)),
            None if queued => prefill.queue(game, pre, from),
            None => prefill.start(game, pre, from).map(|()| 0),
        };
        match started {
            Ok(0) => Answer::json(202, format!("{{\"started\":{}}}\n", string(game))),
            Ok(n) => Answer::json(202, format!("{{\"queued\":{},\"position\":{n}}}\n", string(game))),
            Err(StartError::Busy) => Answer::error(target, 409, "busy", "a prefill is running"),
            Err(StartError::NotAGame) => Answer::error(target, 404, "not-found", "the launcher's own data, not a game"),
            Err(StartError::NotSetUp) => Answer::error(target, 404, "not-found", "prefill is not set up"),
            Err(StartError::NoLancache) => {
                Answer::error(target, 404, "not-found", "no lancache cache is set up (nginx_cache)")
            }
            Err(StartError::NoInstall) => {
                Answer::error(target, 400, "bad-request", "this platform prefills only the install it is set up for")
            }
            Err(StartError::Spawn(e)) => Answer::error(target, 500, "internal", &e),
        }
    }

    /// `DELETE /api/v1/prefill/<platform>/<game>`: takes a game out of the
    /// platform's queue (200 `{"unqueued"}`; 404 when it is not queued).
    pub(super) fn prefill_unqueue(&self, target: &str) -> Answer {
        let found = rest(target, PREFILL)
            .split_once('/')
            .filter(|(_, game)| valid_game(game))
            .and_then(|(id, game)| Some((self.platform(id)?.prefill()?, game)));
        match found {
            Some((prefill, game)) if prefill.unqueue(game) => Answer::json(
                200,
                format!(
                    "{{\"unqueued\":{}}}
",
                    string(game)
                ),
            ),
            _ => Answer::error(target, 404, "not-found", "that game is not queued"),
        }
    }

    /// `GET /api/v1/prefill` and `POST /api/v1/prefill/stop[?platform=<id>]`
    /// (one platform's prefill, or every one).
    pub(super) fn prefill(&self, target: &str, stop: bool) -> Answer {
        let prefills = self.prefills();
        if prefills.is_empty() {
            return Answer::error(target, 404, "not-found", "not found");
        }
        if stop {
            let only =
                target.split_once('?').and_then(|(_, q)| q.split('&').find_map(|kv| kv.strip_prefix("platform=")));
            match only {
                Some(id) => match self.platform(id).and_then(|p| p.prefill()) {
                    Some(p) => p.stop(),
                    None => return Answer::error(target, 404, "not-found", "no prefill for this platform"),
                },
                None => prefills.iter().for_each(|p| p.stop()),
            }
            return Answer::json(202, "{\"stopping\":true}\n".to_owned());
        }
        let latest = self.latest_prefill();
        Answer::json(
            200,
            format!("{}\n", latest.as_ref().map_or_else(|| "null".to_owned(), |(id, p)| prefill_on(id, p))),
        )
    }

    /// `POST /api/v1/games/<platform>/<game>/prune`: deletes, as a deletion job,
    /// the object files no current build of the game needs.
    pub(super) fn prune_game(&self, target: &str) -> Answer {
        let path = rest(target, GAMES);
        let Some((platform, game)) = path.strip_suffix("/prune").and_then(|p| p.split_once('/')) else {
            return Answer::error(target, 405, "method", "POST only to /prune");
        };
        let (Some(d), Some(p)) = (&self.deleter, self.platform(platform)) else {
            return Answer::error(target, 404, "not-found", "not found");
        };
        if !valid_game(game) {
            return Answer::error(target, 404, "not-found", "no such game");
        }
        let items = match p.prune(game) {
            Some(Ok((_, items))) => items,
            Some(Err(ViewError::NotFound(e) | ViewError::Unavailable(e))) => {
                return Answer::error(target, 503, "unavailable", &e);
            }
            None => return Answer::error(target, 404, "not-found", "this platform does not prune"),
        };
        match d.start_items(platform, game, items) {
            Ok(()) => {
                Answer::json(202, format!("{{\"pruning\":{},\"platform\":{}}}\n", string(game), string(platform)))
            }
            Err(DeleteError::Busy) => Answer::error(target, 409, "busy", "a deletion is running"),
            Err(DeleteError::Spawn(e)) => Answer::error(target, 500, "internal", &e),
        }
    }

    /// `DELETE /api/v1/games/<platform>/<game>`: refused while the platform prefills that game.
    pub(super) fn delete_game(&self, target: &str) -> Answer {
        let Some((d, (platform, game))) = self.deleter.as_ref().zip(rest(target, GAMES).split_once('/')) else {
            return Answer::error(target, 404, "not-found", "not found");
        };
        if !valid_game(platform) || !valid_game(game) {
            return Answer::error(target, 404, "not-found", "no such game");
        }
        let prefilling = self.platform(platform).and_then(|p| p.prefill()).and_then(|p| p.status());
        if prefilling.is_some_and(|s| s.game == game && s.state == PrefillState::Running) {
            return Answer::error(target, 409, "busy", "it is being prefilled; stop that first");
        }
        let parts = self.platform(platform).and_then(|p| p.parts(game));
        let started = match parts {
            Some(parts) => d.start_parts(platform, game, parts),
            None => d.start(platform, game),
        };
        match started {
            Ok(()) => {
                if let Some(p) = self.platform(platform) {
                    p.forget(game);
                    // A queued prefill would fetch it all back.
                    if let Some(q) = p.prefill() {
                        q.unqueue(game);
                    }
                }
                Answer::json(202, format!("{{\"deleting\":{},\"platform\":{}}}\n", string(game), string(platform)))
            }
            Err(DeleteError::Busy) => Answer::error(target, 409, "busy", "another game is being deleted"),
            Err(DeleteError::Spawn(e)) => Answer::error(target, 500, "internal", &e),
        }
    }
}
