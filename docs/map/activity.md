# Map: activity and history

What lanstash records about its use, how long it keeps it, and which admin paths read it: clients, counters over
time, lifetime totals, build history, prefill runs, automatic updates, health checks and the background store
check. Code references are `path:line`; line numbers may have moved, so search for the named item. Claims are marked
**[verified]** (read in the code or covered by a test) or **[hypothesis]**. Most of it is kept in one SQLite
database (`crates/adapters/sqlite`), the rest in small files in the `state` directory; wired in `serve`
`crates/server/src/run/serve.rs` (from `:118`) and `crates/server/src/run/kept.rs`.

## Clients

Who got what, counted as the bytes are sent.

- Counting: `ClientTraffic` `crates/app/src/serve/clients.rs:92`. Each body sent to a client of a known object is
  wrapped in `Counting` `:204` (`crates/app/src/serve/object.rs:181`), which calls `add` `:117` with the client's
  address, service, game (store directory) and whether it came from the stash (`hit`) or was sent while lanstash
  downloaded it (`miss`; a response partly stored counts as a miss). `HEAD` counts nothing.
- Since the start: `counters` `:150` gives `client/<address>/<service>/<game>/<hit|miss>` (`PREFIX` `:18`), which
  the counters kept by the minute file, so who got what outlives a restart ([Counters over time](#counters-over-time)).
  `ClientCounter::parse` `:71` reads such a name back.
- Now: `now` `:165` lists the clients that got something in the last `NOW_MS` (10 s,
  `crates/server/src/run/clients.rs:19`), fastest first (`Downloading` `:42`: `per_sec`, `hit_bytes`, `miss_bytes`
  of the window, `total_bytes` since the start, `session_bytes`, `since_ms`, `last_ms`).
- Sessions: a client that got nothing of a game for `SESSION_GAP_MS` (`:25`, 120 s) starts a new download of it
  when it asks again (`session` and `since_ms` restart).
- Over a range: `client_history::over` `crates/app/src/client_history.rs:71` reads the kept counters under
  `client/` (`top`, at most `MAX_COUNTERS` `:11`, 10,000) and `group` `:40` sums them per client and game, the
  client that got most first, each client's games most first (`ClientBytes` `:26`, `GameBytes` `:15`)
  **[verified: `crates/app/tests/client_history.rs`]**.

## Client names

`ClientNames` `crates/app/src/client_names.rs:45`: a name given in the web UI wins, else what the network calls
the address (reverse DNS), else none (the address stands for it) **[verified:
`crates/app/tests/client_names.rs`]**.

- Labels: `set_label` `:99` parses the address and keeps it canonical (`to_canonical`, so an IPv4-mapped IPv6
  address is its IPv4 form); `clean_label` `:64`: trimmed, control characters dropped, at most `MAX_LABEL` (`:23`,
  64) characters, empty takes the name away. Kept in `client_labels` through the `ClientLabels` port; read once at
  the start (`new` `:77`).
- Reverse names: `look_up` `:113` asks the `ReverseNames` port for the clients the cache has seen
  (`ClientTraffic::clients`), on a thread of its own every `LOOK_MS` (`:21`, 5 s), never while a request waits. An
  answer is kept as long as its TTL says, between `MIN_KEEP_MS` (`:15`, 5 min) and `MAX_KEEP_MS` (`:17`, 24 h); a
  failed lookup keeps what was known and is tried again after `RETRY_MS` (`:19`, 1 min).
- Adapter: `PtrResolver` `crates/adapters/dns/src/reverse.rs`, PTR queries to the LAN's DNS servers, 2 s per server
  (`LOOKUP_TIMEOUT` `crates/server/src/run/clients.rs:21`). Which servers: `client_names` (`system`: the
  `nameserver` lines of `/etc/resolv.conf`; a list of servers; `off`: no lookups) (`names` `:31`,
  `crates/server/src/config.rs:142`).

## Counters over time

`CountersKept` `crates/app/src/counters_kept.rs:57` **[verified: `crates/app/tests/counters_kept.rs`,
`crates/adapters/sqlite/tests/stats.rs`]**:

- Source (`counters` `crates/server/src/run/kept.rs:38`): every counter group of the status as `group.counter`,
  the HTTPS pass-through's (`https.*`), each game's bytes sent (`served/<service>/<game>`) and the per-client
  counters (`client/...`).
- `sample` `:88`, once a minute at the minute's start (`keep_going` `:125`): what each counter went up since the
  minute before is added to that minute; a counter that went down started again from 0 (a restart) and all of it
  counts. Left out by name (`KeptOptions::default` `:38`): `lifetime.*`, `verify.*`, `eviction.stored_bytes`,
  `eviction.disk_free_bytes`, `eviction.short_bytes`, `https.active` (gauges, not counters).
- Kept 180 days (`keep_ms`); older minutes are forgotten once an hour (`forget_before`).
- Read: `series` `:109` in buckets of `step` `:25` (whole minutes, at most `MAX_BUCKETS` `:20`, 2,000, so a long
  range gets larger buckets); `top` `:117`, the counters under a prefix that went up most.
- SQLite (`crates/adapters/sqlite/src/stats.rs`): `add` `:9` writes only deltas above 0, adding to a row that is
  there; `series` `:23` sums per bucket and name; `top` `:57` by `SUM(delta)`; `forget_before` `:76`.

## Lifetime totals

`Lifetime` `crates/app/src/lifetime.rs:45`: bytes clients got from the stash (`served_bytes`) and bytes fetched from
the internet (downloads, prefills, pass-through: `fetched_bytes`) over the cache's whole life, and `since_ms`, the
first start that counted them (`values` `:79`). The earlier runs' totals are read from `lifetime` (`KEPT` `:18`,
`served=`, `fetched=`, `since_ms=` lines) in the `state` directory and this run's counters added on top; saved every
minute (`LIFETIME_SAVE_EVERY_MS` `crates/server/src/run/serve.rs:288`) and on a clean stop or restart (`stop_hook`
`:351`), so a crash loses at most a minute **[verified: `the_totals_add_up_over_restarts_and_keep_when_they_began`,
`crates/app/tests/lifetime.rs`]**. Shown as the status's `lifetime` counter group (`crates/app/src/status.rs:173`),
not kept by the minute.

## Build history

`BuildHistory` `crates/app/src/build_history.rs:27` **[verified: `crates/app/tests/build_history.rs`]**:

- `record` `:55` is called each time a platform's build status is worked out (`KeptBuilds::work_out`
  `crates/app/src/kept_builds.rs:88`) and compares it, game by game, with what was seen before: `added` (a game
  seen for the first time), `new-build` (with the build before), `current` (the stash came to hold all of its
  build).
- A platform's first status ever is only taken in, with no `added` for every game already there; an empty first
  status is not taken in. A game whose coverage is not known yet changes nothing. A game that leaves the status is
  not taken to be gone (right after a start a platform may list only some games).
- What was seen and the events are kept together (`HistoryLog::note`, one transaction); when that fails nothing is
  taken in and it is noted again next time. So an update that came while lanstash was stopped is noted at the next
  start.
- SQLite (`crates/adapters/sqlite/src/history.rs`): `note` `:28`; `history_seen.current` is 1 held whole, 0 not,
  2 not known when taken in (`held_code` `:102`); `events` `:60` newest first, by platform and games; a change this
  lanstash does not know is left out.

## Prefill runs

`PrefillRuns` `crates/app/src/prefill_runs.rs:16` **[verified: `crates/app/tests/prefill_runs.rs`,
`crates/adapters/sqlite/tests/runs.rs`]**:

- Every `LOOK_MS` (`:13`, 5 s) `look` `:48` takes each platform's ended jobs, `Prefill::take_ended`
  (`crates/ports/src/prefill.rs:196`, default none; `Prefiller` `crates/app/src/prefill/mod.rs:355` drains the
  jobs that ended since, the last `ENDED_KEPT` 64, `crates/app/src/prefill/line.rs:12`), plus the last job when it
  is not running, so every job of a queue is kept even when several end between two looks
  (`every_queued_job_is_kept_though_they_end_between_two_looks`).
- Each job is kept once per (platform, game, start) (`keep` `:57`).
- After a restart, a platform's last job in the status is the last one kept until a new one starts (`last` `:40`;
  `last_prefills` `crates/adapters/admin-api/src/api/kept.rs:118`).
- SQLite (`crates/adapters/sqlite/src/runs.rs`): `record` `:16`, `INSERT OR REPLACE` (the same job again replaces
  it) after keeping only the newest `RUNS_KEPT - 1`; `runs` `:55` newest first; a row whose source or state this
  lanstash does not know is left out (`run` `:72`).

## Automatic updates

`AutoUpdate` `crates/app/src/auto_update.rs:52`, only with `auto_update` (a daily window in UTC, `Window` `crates/domain/src/window.rs:9`) set; a pass every 10 min
(`AUTO_UPDATE_EVERY_MS` `crates/server/src/run/serve.rs:268`) **[verified: `crates/app/tests/auto_update.rs`]**:

- `pass` `:108` reads each platform's `build_status`. A game whose current build the stash holds whole is
  followed from then on; inside the window, a followed game whose current build is known not to be held whole is
  queued for a prefill from the CDN, once per build (`tried`: `<platform>/<game>@<version>`). A build that could not
  be queued is printed with why and not tried again until its next build or a restart.
- Games never held whole (imported in part, say) are not followed.
- Deleted games are not followed again: the games `GameDeleter` deleted whole (`take_deleted`) leave `followed`
  and go to `deleted`; while a status from before the deletion still calls one current it is not followed again;
  it leaves `deleted` once a status shows it not held whole, or not at all
  (`a_status_from_before_the_deletion_does_not_follow_the_game_again`,
  `a_deleted_game_held_whole_again_is_followed_again`).
- Kept: the followed games, one `<platform>/<game>` per line, under `auto-update` (`KEPT` `:27`) in the `state`
  directory, saved when the set changes (`save` `:159`); losing it only means following games again as they are
  seen current.
- `GET /api/v1/auto-update` answers the followed games, `{"followed":["<platform>/<game>",..]}` (404 when
  `auto_update` is off; `crates/adapters/admin-api/src/api/updates.rs`), and a game's page says "updates: automatic"
  or "by hand"; what a pass does is printed to the journal (`keep_going`) **[verified:
  `the_followed_games_are_listed_and_the_path_is_404_when_updates_are_off`]**.

## Health checks

`crates/app/src/health.rs`: one model, `Check` `:34` (`level` `ok`/`warn`/`fail`, `what`, `detail`), shared by
`lanstash doctor` and the web UI **[verified: `crates/app/tests/health.rs`]**. `checks`
`crates/server/src/run/health.rs:19` makes them each time it is asked:

- `store contents` (`store_load` `:79`): games and bytes from the status's snapshot (walked again only once the
  stash changed); files that could not be loaded and damaged records as problems (warn). A file not opened because
  the process may open no more (`is_open_file_limit` `:71`: EMFILE, OS error 24) is not damage but a limit to raise.
- `open files` (`open_files` `:53`): this process's open files of its limit; warn from 80 %, fail from 95 %
  (`OPEN_FILES_WARN` `:47`, `OPEN_FILES_FAIL` `:48`).
- `free space` (`free_space` `:100`): the store disk against `keep_free`; warn below it.
- `background check` (`background_check` `:114`), only with the patrol: from the status's `verify` group, warn
  when its last round found problems.

The service's state and its ports are for `lanstash doctor` to say, from outside.

## Background store check

`Patrol` `crates/app/src/patrol.rs:81`, only with `verify_every` (days; default `off`), paced by `verify_rate`
(bytes a second, default `50M`, at least 1 MiB) (`start_patrol` `crates/server/src/run/serve.rs:206`) **[verified:
`crates/app/tests/patrol.rs`]**:

- Every 10 min (`PATROL_LOOK_EVERY_MS` `:203`) it looks whether a round is due (`due` `:127`: one was cut short, or
  `every_ms` has passed since the last finished).
- A round (`round` `:150`) checks every stored byte against its checksum (`lanstash_store::verify`), items in path
  order, paced so the bytes read stay under the rate. What looked wrong is checked again against a fresh listing
  before it is reported (`confirm` `:205`: a file deleted meanwhile is no damage). A store that cannot be listed is a
  problem and the round stays due.
- Kept under `verify` (`KEPT` `:18`) in the `state` directory: `started_ms`, `finished_ms`, `cursor`, `problem`
  lines (`text` `:67`), every 5 min during a round (`SAVE_EVERY_MS` `:78`) and at its end, so a restart resumes
  after the last item checked.
- Shown as the status's `verify` counter group (`values` `:134`, `crates/app/src/status.rs:176`); what a round
  finds is printed to the journal.

## SQLite tables

`Database` `crates/adapters/sqlite/src/db.rs:68`, one file `lanstash.db` in the `state` directory
(`crates/server/src/run/kept.rs:20`), one connection behind a lock, WAL with `synchronous = NORMAL` (`open` `:83`).
The schema moves forward in `STEPS` `:11` (SQLite `user_version`); a database from a newer lanstash is refused
(`ready` `:106`) **[verified: `crates/adapters/sqlite/tests/db.rs`]**. Retention **[verified: tests named]**:

| Table | Step | Holds | Kept |
|---|---|---|---|
| `history_events` | 1 | `at_ms`, `platform`, `game`, `change`, `version`, `from_version` | the newest 50,000 (`EVENTS_KEPT` `history.rs:10`, `only_the_newest_events_are_kept`) |
| `history_seen` | 1 | per (platform, game): `version`, `current` | one row per game, replaced (`what_was_seen_is_kept_and_replaced`) |
| `history_platforms` | 1 | the platforms whose status was taken in | always |
| `stats_minutes` | 1 | per (minute, name): `delta` | 180 days, forgotten hourly (`old_minutes_are_forgotten`) |
| `prefill_runs` | 1 | per (platform, game, start): version, source, state, file and byte counts, error | the newest 2,000 (`RUNS_KEPT` `runs.rs:13`, `only_the_newest_jobs_are_kept`) |
| `client_labels` | 2 | `address`, `label` | until taken away (`a_name_is_given_changed_and_taken_away`) |

Kept apart, as files in the `state` directory: `lifetime`, `auto-update`, `verify`, and the DNS setup
([dns.md](dns.md#where-the-setup-is-kept)).

## Admin API

Routes in `crates/adapters/admin-api/src/route.rs` (`:56` to `:70`; paths `:105` to `:110`, `CLIENTS` `:124`). Each
answers 404 when its part is not wired.

| Path | What |
|---|---|
| `GET /api/v1/status` | the snapshot's `clients` (downloading now, with `address`, `label`, `host`; `crates/adapters/admin-api/src/api/mod.rs:230`), the `lifetime` and `verify` counter groups, each platform's last prefill (kept one after a restart) |
| `GET /api/v1/clients?from=<ms>&to=<ms>` | `{"clients":[{"address","label","host","hit","miss","games":[{"service","game","hit","miss"}]}]}`, most first; 400 without `from` before `to` (`clients_over` `api/clients.rs:78`) |
| `PUT /api/v1/clients/<address>?label=` | names a client (empty: takes the name away); `{"address","label","host"}`; 400 for a bad address or name (`clients` `:52`) |
| `DELETE /api/v1/clients/<address>` | takes the name away |
| `GET /api/v1/stats?names=a,b&from=&to=[&step=]` | `{"step_ms","names","buckets":[{"at_ms","values"}]}`, 1 to 32 names (`stats` `api/kept.rs:43`) |
| `GET /api/v1/stats/top?prefix=&from=&to=[&limit=]` | `{"top":[{"name","value"}]}`, limit 20 by default, at most 100 (`stats_top` `:83`) |
| `GET /api/v1/history[?limit=N]` | `{"events":[{"at_ms","platform","game","change","version","from"}]}`, newest first, at most 200 (`history` `api/history.rs:25`) |
| `GET /api/v1/prefill/runs[?platform=][&limit=]` | `{"runs":[{"platform","job":{..}}]}`, newest first, at most 200 (`prefill_runs` `api/kept.rs:104`) |
| `GET /api/v1/health` | `{"checks":[{"level","what","detail"}]}` (`health` `api/health.rs:23`, `json` `crates/app/src/health.rs:126`) |

`GET /api/v1/auto-update`: the followed games of automatic updates; see [Automatic updates](#automatic-updates).
