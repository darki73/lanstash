# Map: operations

Code references are `path:line`; line numbers may have moved, so search for the named item. Claims are marked **[verified]** or **[hypothesis]**.
Battle.net's own views, plans and prefill are in [battlenet.md](battlenet.md); the DNS server link in [dns.md](dns.md).

## Commands

The full list is `USAGE` in `crates/server/src/cli.rs:7` (parsed by `parse` `:146`; dispatched by `main`
`crates/server/src/main.rs:20`). Exit codes: 0 ok, 1 problems found, 2 usage error (the usage is printed),
3 failure. Every command but `seg`, `uninstall`, `help` and `version` takes `--config FILE` and `--KEY VALUE`
flags ([configuration.md](configuration.md)). **[verified]**

| Command | What | Code |
|---|---|---|
| `lanstash serve` | serve and cache | `serve` `crates/server/src/run/serve.rs:52` |
| `lanstash stop [--wait SECS]` | clean stop of the running lanstash through `admin_listen`, waiting (default 60 s) until its admin port closes | `stop` `:379`; client `stop` `crates/adapters/admin-api/src/client.rs:98` |
| `lanstash import --nginx-cache DIR --dirs LIST [--threads N] [--slice SIZE]` | import lancache's nginx cache; `--dirs` level directories (`00,1a`, `30-3f`, `00-ff`); 16 threads and 1 MiB slices by default | `import` `crates/server/src/run/store.rs:55` |
| `lanstash store stats` | objects and bytes per service and game (read-only, safe while serving) | `store_stats` `:92` |
| `lanstash store verify [--threads N] [--rate MB/S] [--accept]` | every stored byte against its checksum (read-only), as fast as the disks go or at most `--rate` MB a second (`verify_paced`, `crates/app/src/verify.rs`); it says how much it reads and asks first; without a terminal it does not start unless `--accept`; 4 threads by default | `store_verify` `:126`, `confirm` `:25` |
| `lanstash prefill <platform>/<game> [--pre] [--from lancache\|cdn] [--detach]` | starts a prefill in the running lanstash and follows it every 2 s (Ctrl-C stops following, not the prefill); exit 1 unless it ends `done` | `prefill` `crates/server/src/remote.rs:33`; path `start_path` `crates/server/src/prefill_cli.rs:10`, progress line `progress` `:52` |
| `lanstash prefill stop` | stops every prefill | `prefill_stop` `crates/server/src/remote.rs:71` |
| `lanstash steam login [--password] \| status \| logout` | Steam sign-in by QR code (or account, password and Steam Guard), the refresh token kept in `secrets`; `status` logs on with each kept token; `logout` forgets them. Default config `/etc/lanstash/lanstash.conf` when it exists | `steam` `crates/server/src/run/steam.rs:75` |
| `sudo lanstash user add <name> [--admin] \| passwd <name> \| role <name> admin\|viewer \| remove <name> \| list` | lanstash's own accounts (secret `accounts`: name, role, PBKDF2-HMAC-SHA256 hash with 600,000 iterations and a 16-byte salt; the password never kept), the password asked twice with echo off; a new account is a viewer unless `--admin`; the last admin can be neither removed nor made a viewer; the running service reads the file afresh on each use | `user` `crates/server/src/run/users.rs`; rules `crates/app/src/accounts.rs`; hashing `crates/adapters/crypto/src/passwords.rs` **[verified: tests]** |
| `lanstash seg dump <file> [--verify] [--limit N]`, `seg verify <file>` | one segment's header, records and problems; every record body checked | `seg_dump` `crates/server/src/run/store.rs:100`, `seg_verify` `:113` |
| `sudo lanstash install [--no-start] [--force]`, `uninstall [--purge]`, `lanstash doctor` | [below](#install-uninstall-doctor) | `crates/server/src/setup.rs:90`, `:156`, `:198` |
| `lanstash help`, `version` | | `main` |

## Logs

- Access log (`access_log`): `ms ip METHOD host target range=... status=... cache=... bytes=...[ INCOMPLETE] ua="..."[ hdr="..."]`
  (`format_entry` `crates/adapters/logs/src/access_log.rs:65`). `cache` is the response's `X-LanStash`:
  `HIT`, `PARTIAL`, `MISS`, `PASS`, `UPSTREAM`, or a control answer (`HOST-NOT-ALLOWED`, `LOOP`, `RANGE`,
  `UPSTREAM-ERROR`, `ERROR`, `NOT-CACHED`; [requests.md](requests.md#marks-x-lanstash-and-the-lancache-headers)).
  `INCOMPLETE`: the response was cut off. `hdr` (`access_log_headers`): every request header,
  `name: value | ...`. Only the cache port logs; the admin port writes none. **[verified]**
- Event log (`event_log`, else standard error): one line per notable event, `ms kind key=value...`: `download-failed`,
  `checksum-mismatch`, `store-failed`, `range-ignored`, `key-mismatch`, `tls-failed`, `evicted`, `evict-failed`,
  `compacted`. Events: `Event` `crates/ports/src/events.rs:7`; formatting `format_event`
  `crates/adapters/logs/src/event_log.rs:16`. **[verified]**
- Both rotate by size (`log_max_size`, `log_keep`): `RotatingFile` `crates/adapters/logs/src/rotating_file.rs:27`.
- `serve` itself prints to standard output (the journal under systemd): the store load, the cache-domains
  counts, eviction passes, `auto-update:` and `store check:` lines, and every `stats_secs` the counters. **[verified]**

## Admin API

`admin_listen`; `AdminApi` `crates/adapters/admin-api/src/api/mod.rs`, `handle` in `api/access.rs`, dispatch
`answer` in `mod.rs`. Version 1, JSON. Every error on an `/api/` path is `{"error": "...", "kind": "..."}`
(`error_json` `crates/adapters/admin-api/src/render.rs:188`); elsewhere a line of text. Kinds: `not-found`,
`method` (405, with `Allow`), `forbidden`, `busy`, `invalid`, `bad-request`, `refused`, `unavailable`,
`upstream`, `internal`, `failed`, `untrusted`, `sign-in` (401), `admin-only` (403), `wrong`, `wait` (429).
**[verified]**

Who may do what (`api/access.rs`): reading is open to all, but for the DNS server's state (`GET /api/v1/dns`
needs `dns`: it names the server's address and admin user, and has lanstash ask it) and who the clients are
(`/api/v1/clients` and counters of one client, `client/<address>/...`, need a signed-in account; a guest's
answers and status events give each client's address as a name of no meaning, `client-1`, ..., and leave out
its label and host: `anon.rs`). A route that changes something needs the permission
`needs(route)` names (`prefill`, `settings`, `dns`, `delete`, `platforms`, `clients`, `hide`, `accounts`). A
role is a set of permissions (`domain::accounts`): admin all, player `prefill.own`, viewer none. A player may
start a prefill of a game its linked platform accounts (`steam/<account>`, set with `PUT
/api/v1/users/<name>?link=|unlink=`) own, and its catalog shows only those (`api/own.rs`). A request from this
machine (loopback) holds every permission, so the command line needs no sign-in; else the session cookie
(`lanstash_session`) says who, with the account's role as it is now; anyone else is a guest (401 on a route
that needs a permission; an account without it gets 403). Sign-in and accounts are `api/users.rs`: `GET|POST|DELETE
/api/v1/session` (name and password in `X-Lanstash-User` / `X-Lanstash-Password`, percent-encoded; the cookie
`HttpOnly; SameSite=Strict`, 30 days from the last use, kept as a SHA-256 in the secret `sessions`; wrong
sign-ins make the address wait 1 s, doubling to 30 s), `GET /api/v1/users`, `POST|PUT|DELETE
/api/v1/users/<name>[?role=]` (an account may change its own password with `X-Lanstash-Old-Password`; a changed
password or a removed account ends its sessions). Use case `crates/app/src/sessions.rs`. **[verified: tests in
`crates/adapters/admin-api/tests/api/access.rs`, `users.rs`]**

Deployment caveat: every request lanstash sees coming from loopback holds every permission. A reverse proxy, a
container's port forwarding or any other process on the box that relays requests would make everyone it relays
an admin: put nothing in front of the admin port on the box itself; to reach it through a proxy, run the proxy
elsewhere.

Invites (`api/invites.rs`, use case `crates/app/src/invites.rs`): an admin makes one with `POST
/api/v1/invites?role=[&link=]...[&name=]` and gets its token once (32 random bytes, hex; kept only as a
SHA-256 in the secret `invites`, with its role, links, end (7 days) and the account's name when the admin
chose it); `GET /api/v1/invites` lists them by id (a digest prefix, never the token), `DELETE
/api/v1/invites/<id>` takes one back. The link sent is `<page origin>/join/<token>`; that page reads `GET
/api/v1/invites/<token>` (role, links, name) and `POST`s it with a name and password in the sign-in headers,
which makes the account with the invite's role and links (a chosen name must be the one sent), uses the
invite up and signs the account in (the cookie). A wrong token makes the address wait as a wrong sign-in does. **[verified:
`crates/app/tests/invites.rs`, `crates/adapters/admin-api/tests/api/invites.rs`]**

For anyone without `hide`, the hidden games are masked in every JSON answer and every status event
(`crates/adapters/admin-api/src/mask.rs`, by the answer's shape, so a new endpoint is masked too): each hidden
game is a token (`hidden-1`, …) in place of every id of it (a Steam depot traced to its app through the
stash), named "A hidden game", marked `"hidden": true`, its build numbers and other words emptied, its sizes
kept; a platform's builds and catalog leave it out; its page and art are not found; `owners` (account names)
are left out everywhere. **[verified: `tests/mask.rs`, `tests/api/access.rs`]**

Routes are tables in `crates/adapters/admin-api/src/route.rs`: `EXACT` `:97` (exact paths, with the paths from
before the API), `PREFIXED` `:119` (prefixes), `BY_METHOD` `:132` (a method turning a route into another);
`route` `:158` ignores the query. A game or platform in a path is one segment of 1 to 64 letters, digits, `-`,
`_` (`valid_game` `:186`). Tests `crates/adapters/admin-api/tests/route.rs`, `tests/api/`. **[verified]**

Platform paths are answered by the registered `Platform` whose id the path names (`blizzard`, `steam`,
`epicgames`, `riot`; `crates/ports/src/platform.rs:99`); a platform without that view answers 404. A
part not wired (art, settings, DNS, history, stats, runs, health, clients) answers 404.

### The guard

Before routing, `refused` `crates/adapters/admin-api/src/guard.rs:18` answers `403 forbidden` (test
`crates/adapters/admin-api/tests/guard.rs`): **[verified]**

- any request whose `Host` is not the network's own (DNS rebinding): addresses, `localhost`, names of one label
  and names under `.local`, `.lan`, `.home.arpa`, `.internal`, `.localhost` pass (`LOCAL_SUFFIXES` `:14`,
  `local_host` `:42`);
- a request other than `GET`/`HEAD` that a browser marks as from another site: `Sec-Fetch-Site` other than
  `same-origin` or `none`, or an `Origin` whose authority is not the `Host`. Tools that send neither (the CLI,
  curl) pass.

With HTTPS on, the plain port sends every request not from this machine to the same path over HTTPS (308), only
to a name the same rule calls the network's own and only to a path (`https_redirect`), never to wherever a forged
`Host` points **[verified: `tests/guard.rs`]**.

### Secrets in headers

What signs in travels in a header, never in a path (paths end up in logs and browser histories):
`X-Lanstash-Password` for the DNS server's password (`PASSWORD_HEADER` `api/dns.rs:28`), `X-Lanstash-Secret` for
a pasted sign-in code or an account client's `<id>:<secret>` (`SECRET_HEADER` `api/account.rs:26`;
percent-encoded, a query value is taken when the header is empty). **[verified]**

### Event streams

`GET /api/v1/events` (and `/events`): `text/event-stream`, `retry: 2000`, then one `data: <snapshot>` a second
until the viewer goes away (`api/events.rs`). At most `MAX_STREAMS` (8) at once and `PER_ADDRESS` (3) from one
address other than this machine; beyond, `503` `busy`. Every `RECHECK_EVENTS` (10) events the stream looks
whether its viewer may still see as much (signed out, or its role changed elsewhere): if not it ends, and the
page reconnects with what it may see now. The admin port takes 32 connections, 24 per address (a dev server's
proxy is one address), so a stop still gets through. **[verified: `tests/api/events.rs`]**

### Routes

| Method | Path | Answer |
|---|---|---|
| GET | `/` | the built-in status page (`status_page.html`, compiled in), drawn from `api/v1/events` |
| GET | `/api/v1/version` | `{"version"}`, plus `"latest"` when a release newer than the running version is known (`check_updates`; `crates/adapters/admin-api/src/api/version.rs`); the web UI asks again after 15 s, then every 10 min, and its footer says "update available" |
| GET | `/api/v1/status` (`/status.json`) | one [status snapshot](#status) |
| GET | `/api/v1/events` (`/events`) | [event stream](#event-streams) |
| POST | `/api/v1/shutdown` (`/shutdown`) | 202 `{"stopping": true}`, then a [clean stop](#clean-stop); 403 unless from this machine (loopback) |
| GET | `/api/v1/art/<platform>/<id>[/icon\|/background\|/logo]` | an image (no kind: the header), `Cache-Control: public, max-age=86400`; 404 `not-found` without a source or image, 502 `upstream` when the CDN fails ([art](#art)) |
| GET | `/api/v1/builds/<platform>` | the platform's builds view |
| GET | `/api/v1/catalog/<platform>` | the platform's catalog view (every game it offers) |
| GET | `/api/v1/games/<platform>/<game>` | the platform's game view; 404 `not-found`, 503 `unavailable` when it cannot answer now |
| GET | `/api/v1/games/<platform>/<game>/nginx` | what lancache's nginx cache holds of it |
| GET | `/api/v1/games/<platform>/<game>/prune` | what a prune would delete (`game`, `files`, `bytes`, `recent_bytes`; segment platforms also `compact`); see [pruning](#pruning) |
| POST | `/api/v1/games/<platform>/<game>/prune` | starts that prune as a deletion job: 202 `{"pruning", "platform"}`; 409 `busy`; 503 when a build cannot be looked up; 404 for a platform that does not prune; 405 when the path does not end in `/prune` |
| DELETE | `/api/v1/games/<platform>/<game>` | starts [deleting](#deleting-a-game) the game: 202 `{"deleting", "platform"}`; 409 `busy` while the platform prefills it or another deletion runs |
| GET | `/api/v1/prefill` | the prefill started last, of any platform, or `null`; 404 when no platform prefills |
| POST | `/api/v1/prefill/<platform>/<game>[?build=pre][&from=lancache\|lancache-only][&queue=1]` | 202 `{"started"}`, or with `queue=1` on a busy platform 202 `{"queued", "position"}`; 409 `busy` (a prefill runs, or the game is being deleted); 404 no prefill, not a game, not set up, or (`from=lancache*`) no `nginx_cache`; 500 `internal` |
| POST | `/api/v1/prefill/stop[?platform=<id>]` | stops every prefill or one platform's: 202 `{"stopping": true}`; 404 for a platform without prefill |
| DELETE | `/api/v1/prefill/<platform>/<game>` | takes a game out of the queue: 200 `{"unqueued"}`; 404 when not queued |
| GET | `/api/v1/prefill/runs[?platform=<id>][&limit=N]` | the prefill jobs that ended, newest first, at most 200: `{"runs": [{"platform", "job"}]}` |
| GET | `/api/v1/settings` | the [settings the web UI may change](configuration.md#settings-the-web-ui-may-change) |
| PUT | `/api/v1/settings/<key>?value=<value>` | a change (percent-decoded): 200 with the settings; 400 `invalid` with the reason |
| DELETE | `/api/v1/settings/<key>` | drops the UI's change: 200 with the settings; 400 for a key the UI may not change |
| POST | `/api/v1/restart[?force=1]` | 202 `{"restarting": true}`, a clean stop with exit code 75; 409 `busy` while a prefill runs or a client downloads, unless forced |
| GET | `/api/v1/account/<platform>` | the platform's accounts view; 404 when the platform has none |
| GET | `/api/v1/account/<platform>/qr.svg` | the waiting sign-in's QR code (`image/svg+xml`); 404 when none waits |
| POST | `/api/v1/account/<platform>/sign-in` | with a code (`X-Lanstash-Secret`, or `?code=`): signs in with it, 200 the view, 400 `refused`; without: starts a QR sign-in, 202 the view |
| DELETE | `/api/v1/account/<platform>/sign-in` | cancels a waiting sign-in: 200 the view |
| POST | `/api/v1/account/<platform>/sign-out?account=<name>` | forgets that account's kept sign-in: 200; 400 `bad-request` without a name |
| POST / DELETE | `/api/v1/account/<platform>/client` | sets (`<id>:<secret>` in `X-Lanstash-Secret` or `?value=`) or drops the client the platform's sign-in names: 200; 400 `bad-request` |
| GET | `/api/v1/setup/<platform>/<windows\|macos\|linux>` | the file that sets up a PC for the platform, to save (`Content-Disposition: attachment`); 404 where there is none |
| GET | `/api/v1/dns` | the DNS server's setup and report ([dns.md](dns.md#admin-api)) |
| PUT / DELETE | `/api/v1/dns/setup?kind=&host=&port=&user=[...][&test=1]` | sets up (password in `X-Lanstash-Password`), tries, keeps unless testing; 400 `invalid`, or `untrusted` with the certificate's SHA-256 / forgets the setup |
| POST | `/api/v1/dns/apply` | adds the missing rewrites, merges duplicates: 200; 502 when the server cannot be asked |
| GET | `/api/v1/history[?limit=N]` | what was updated when, newest first, at most 200: `{"events": [{"at_ms", "platform", "game", "change", "version", "from"}]}` |
| GET | `/api/v1/stats?names=a,b&from=<ms>&to=<ms>[&step=<ms>]` | counters over time, how much each went up per bucket (1 to 32 names; default step 1 min): `{"step_ms", "names", "buckets": [{"at_ms", "values"}]}`; 400 `bad-request`, 503 `unavailable` |
| GET | `/api/v1/stats/top?prefix=&from=&to=[&limit=]` | the counters named `prefix...` that went up most (default 20, at most 100): `{"top": [{"name", "value"}]}` |
| GET | `/api/v1/health` | the checks the service makes of itself: `{"checks": [{"level", "what", "detail"}]}` (store load, open files, free space, background check; `crates/server/src/run/health.rs`) |
| GET | `/api/v1/clients?from=<ms>&to=<ms>` | what each client got over the range, per game, most first: `{"clients": [{"address", "label", "host", "hit", "miss", "games": [{"service", "game", "hit", "miss"}]}]}` |
| PUT | `/api/v1/clients/<address>?label=<name>` | names a client (empty: takes the name away): 200 `{"address", "label", "host"}`; 400 `bad-request` |
| DELETE | `/api/v1/clients/<address>` | takes its name away |

Handlers: `api/jobs.rs` (`prefill_start` `:17`, `prefill_unqueue` `:55`, `prefill` `:75`, `prune_game` `:98`,
`delete_game` `:126`), `api/settings.rs` (`settings` `:40`, `restart` `:64`), `api/account.rs` (`account` `:39`),
`api/clients.rs` (`clients` `:52`), `api/kept.rs` (`stats` `:43`, `stats_top` `:83`, `prefill_runs` `:104`),
`api/dns.rs` (`dns` `:155`, `dns_setup` `:170`), `api/history.rs`, `api/health.rs`, `api/setup.rs`, the rest in
`api/mod.rs`. Answers: `Answer` `api/answer.rs` (`no-store` unless said; `X-Content-Type-Options: nosniff` on a
cacheable 200 and on files to save). All **[verified]**.

The paths from before the API (`/status.json`, `/events`, `/shutdown`) still answer, so that a newer
`lanstash stop` or `doctor` and an older running lanstash understand each other during an upgrade: the client
tries the API path first and the older one on a 404 or 405 (`request_api` `crates/adapters/admin-api/src/client.rs:56`).
**[verified]**

## Status

`Status::snapshot` `crates/app/src/status.rs:214` (`Snapshot` `:41`), filled further by `AdminApi::snapshot`
and rendered by `render_json` `crates/adapters/admin-api/src/render.rs:44` (read by `web/src/contracts/status`).
Fields: **[verified]**

- `now_ms`, `started_ms`.
- `counters`: groups by name: `requests` (`requests`, `hits`, `misses`, `partial`, `hit_bytes`, `miss_bytes`,
  `refused`, `errors`, `retries`, `prefetch_used`, `prefetch_wasted`, `own_requests`, `own_bytes`), `downloads` (the fetcher's: `started`,
  `joined`, `completed`, `failed`, `fetched_bytes`, `stored_*`, `crc_mismatch`, `store_errors`,
  `range_ignored`), `pass-through`, `eviction`, `lifetime`, `verify` (with `verify_every`), `https` (with
  `listen_tls`) (`Status::counters` `:161`).
- `rates_mb_s`: MB/s since this viewer's previous snapshot of every counter named `*bytes*` (`group.name`; none
  under 0.2 s) (`rates` `render.rs:20`). Each event stream keeps its own; `GET /api/v1/status` callers share one.
- `downloads`: in flight, `{"key", "span": [start, end or null], "received", "total"}`.
- `usage_ms`, `usage`: per store directory `{"service", "game", "name", "group", "objects", "bytes",
  "last_used_ms"}` (`UsageRow` `:31`). `group` is the game the directory belongs to (a Steam depot's app id), so
  the web UI can group depots and ask for their art. The walk of the index is redone once the stash changed (at
  most every 5 s) and at least every 10 min (`USAGE_MAX_AGE_MS` `:73`); names are looked up again every 30 s.
- `prefill`: the prefill started last, any platform (`prefill_json` `render.rs:126`: `game`, `version`,
  `prerelease`, `from` (`cdn`, `lancache`, `lancache-only`), `state` (`running`, `done`, `stopped`, `failed`),
  file and byte counts (`files_total`, `files_done`, `files_skipped`, `bytes_skipped`, `files_missing`,
  `files_local`, `bytes_local`, `files_rejected`, `bytes_total`, `bytes`), `current`, `error`, `started_ms`,
  `finished_ms`, `planning` `{"done", "total"}`, `queued`); `prefills`: each platform's last, by platform id
  (after a restart the last one kept, `last_prefills` `api/kept.rs`).
- `deleting`: the last deletion ([below](#deleting-a-game)).
- `builds_changed`: per platform, when its builds view last changed (ms).
- `clients`: the clients downloading now: `address`, `label`, `host`, `service`, `game`, `per_sec`,
  `hit_bytes`, `miss_bytes`, `total_bytes`, `session_bytes`, `since_ms`, `last_ms` (`downloading_json` `render.rs:108`).

## Deleting a game

`GameDeleter` `crates/app/src/delete_game.rs:86` (`start` `:115`, `start_parts` `:124`, `start_items` `:143`,
`status` `:209`): one deletion at a time, on a thread of its own. Everything the game's directories hold goes the
way eviction deletes: the segment being written is closed first (`close_segment_of`
`crates/store/src/store.rs:285`) and sealed once its records are done, then every segment and object file, each
announced as evicted, then the journals are compacted. A platform may name the directories a game is made of
(`Platform::parts`: a Steam app's depots), and forgets what it keeps of the game (`Platform::forget`); a queued
prefill of the game is taken out of the queue (`delete_game` `api/jobs.rs:126`). A prefill is refused while
its game is being deleted. Progress is in every status snapshot as `deleting`: `{"platform", "game", "state"
(running, done), "items_total", "items_done", "bytes_total", "bytes_done", "failed" (a count), "started_ms",
"finished_ms", "partial", "bytes_kept"}` (`delete_json` `render.rs:156`). Games deleted whole are not followed by
automatic updates any more (`take_deleted`). Tests `crates/app/tests/delete_game.rs`. **[verified]**

### Pruning

A prune deletes, as a deletion job (`partial: true`), what no current build of a game needs, unused for 7 days
(`PRUNE_GRACE_MS` `crates/server/src/run/platforms.rs:171`). Battle.net (`Pruner`) prunes object files no current
or pre-release build of any product of the directory needs, for any platform or language. Steam (`SteamPruner`)
and Epic work on segments (`crates/app/src/prune_segments.rs`): a segment with none of its objects needed goes
whole; one at most half current is compacted (the current objects copied out, then it goes; `bytes_kept`); a
mostly current one stays. Steam looks at every current manifest of the app's depots the account's licences grant
(any OS, language or DLC); nothing is prunable when a manifest cannot be read. **[verified]**

## Settings and restart

The settings the web UI may change: [configuration.md](configuration.md#settings-the-web-ui-may-change). A
change applies at the next start. `POST /api/v1/restart` runs the restart hook: a clean stop that exits with
`RESTART_CODE` 75 (`crates/server/src/run/serve.rs:348`), which the service (`Restart=on-failure`) follows with a
start. **[verified]**

## Automatic updates

With `auto_update` set, lanstash keeps games current on its own (`AutoUpdate` `crates/app/src/auto_update.rs:52`,
a pass every 10 minutes): **[verified]**

- Every stashed game whose current build the stash holds whole is *followed* from then on. The list is kept in
  `state` (`auto-update`, one `<platform>/<game>` a line) across restarts; a game deleted is dropped.
- Inside the window, a followed game whose current build is known not to be held whole (a new build came out) is
  queued for a prefill from the CDN behind whatever runs, once per build. A prefill that fails is not retried
  until the next build or a restart. Each pass prints what it queued (`auto-update: queued ...`) and what it
  could not, with why (`auto-update: could not queue ...`).
- Games never held whole (imported in part from lancache, say) are not followed. Each platform says where its
  games stand through `Platform::build_status`.

## DNS server

The network's DNS server sends the game CDN names to lanstash. It is set up and checked on the web UI's DNS page
through `/api/v1/dns*` (`crates/adapters/admin-api/src/api/dns.rs`), not in the configuration file; its setup is
kept in `state` (`dns`) and its password, owner-only and never sent back, in `secrets` (`dns-password`; `dns_link`
`crates/server/src/run/serve.rs`). Everything else:
[dns.md](dns.md). **[verified]**

## Steam after a restart

Steam's builds view needs every stashed game's current manifests. Two things are kept in `state` so a restart
need not ask Steam for them again (`crates/platforms/steam/app/src/chunk_lists.rs`, `library.rs`):

- `steam-depot-<depot>`: the chunk list of the depot's latest manifest. A manifest never changes under its id, so
  a kept list is never stale; a new build's replaces it. **[verified]** Measured on a stash of 117 games: 339
  files, 141 MB. **[verified]**
- `steam-library`: Steam's answers the library was made of, and when they were read. The library is there at
  once after a start and is read again when it is 6 hours old (`REFRESH_MS` `library.rs:188`). **[verified]**

A game whose lists are all kept is worked out without a Steam session; measured on 117 stashed games, a restart
took seconds with the kept lists and 507 s without. **[verified]**

## Lifetime totals

The status snapshot's `lifetime` counters (`served_bytes`, `fetched_bytes`, `since_ms`) add this run's bytes to
those of every run before it (`Lifetime` `crates/app/src/lifetime.rs:45`): bytes clients got from the stash, and
bytes fetched from the internet (downloads, prefills, pass-through). They are kept in `state` (`lifetime`), saved
every minute and at a clean stop. **[verified]**

## Store check in the background

With `verify_every` set lanstash reads every stored byte against its checksum while it serves, as
`lanstash store verify` does (`Patrol` `crates/app/src/patrol.rs:81`): **[verified]**

- Every 10 minutes it looks whether a round is due (the period has passed since the last one finished, or one was
  cut short). A round checks one segment or object file at a time, in path order, and after each waits until the
  bytes read stay under `verify_rate`.
- Whatever looks wrong is checked once more against a fresh listing, so a file deleted or replaced during the
  round is not reported. Nothing is repaired.
- Its place, and the last round's finish and problems, are kept in `state` (`verify`) every 5 minutes and at the
  end, so a restart resumes the round. Each round prints `store check: N files, N bytes, N problems`, then each
  problem. The snapshot's `verify` counters carry where it stands; `doctor` and the health page report its problems.

## Art

`Art` `crates/app/src/art.rs:71`, `ArtOptions` `:30`; served at `/api/v1/art/...`. Each platform registers an
`ArtSource` (`Platform::art`): Steam `SteamArt` `crates/platforms/steam/domain/src/art.rs:38`, Battle.net its
catalog ([battlenet.md](battlenet.md#catalog-names-and-art)), Epic its library. A source names the address of an
image of a kind (`header`, `icon`, `background`, `logo`), fetched through the `Upstream` port over plain HTTP, with
a fallback where the first place has none; or it fetches the image itself where its CDN answers only over HTTPS
(Epic). Kept in memory: 128 MiB in all, least recently used out, at most 8 MiB an image; a miss remembered for
6 h, a failure for 1 min. The image type is taken from the bytes (PNG, JPEG, WebP, GIF; never SVG; `image_type`
`crates/domain/src/art.rs:6`). Tests `crates/app/tests/art.rs`. **[verified]**

## Game names

Through the `GameNames` port (`crates/ports/src/names.rs:7`): a name, and the game a directory belongs to
(`group`). The root builds the chain: first the platforms' own names (`Platform::names`: Steam's library,
Battle.net's catalog, Epic's library), then the names file over the grouping rules (`game_names`
`crates/server/src/run/platforms.rs:71`, `builtin_names` `:63`); the first name found wins (`ChainNames`
`crates/adapters/names/src/game_names.rs:66`; wiring `crates/server/src/run/serve.rs:128`). There is no built-in
name table. A Steam depot without its own entry is grouped under the nearest app id at most 100 below it
(`DEPOT_WINDOW` `crates/platforms/steam/domain/src/names.rs:7`, `NameTable::group_numbered`
`crates/domain/src/names.rs:26`). Once read, Steam's library names each depot's own app, and it comes first
(`impl GameNames for SteamLibrary` `crates/platforms/steam/app/src/library.rs:334`). **[verified]** That the
nearest app id below is the owner, for a depot the library does not list, is a **[hypothesis]**.

`store stats` and `store verify` use the names file chain only (no platforms). **[verified]**

## Web UI

`web/`: React, Vite and Tailwind, layered like the Rust crates (`adapters`, `app`, `contracts`, `features`,
`i18n`, `lib`, `state`, `styled`) and checked by tests in `web/tests/architecture`. It reads only the API. **[verified]**

- Words (`web/src/i18n`): every word the UI shows comes from a message catalogue, one JSON file per namespace
  (a feature, or `common` for the styled pieces) in `locales/<code>/`, read through i18next and react-i18next
  (MIT; only the `i18n` layer imports them). English is the source and the keys `useT(namespace)` takes are
  typed against it; `_one`/`_other` keys are plural forms. Another language is a folder of the same files and
  an entry in `languages` (`i18n/catalogue`); a test checks it holds every key English has. The page follows the
  browser's languages, a choice at the sidebar's foot (shown once there are two) overrides it in this browser;
  `<html lang dir>` follow. Settings' labels and help come from the catalogue for the settings it knows, else as
  lanstash sends them. Errors from the API are shown as lanstash words them (English) **[verified: web tests]**.
- The page's head: the tab's title names the page (a hidden game masked as everywhere) and, while lanstash
  fetches or serves at least 0.1 MB/s, how fast (`↓ 120 MB/s ↑ 40 MB/s · Steam · lanstash`); a description,
  theme colours for light and dark, icons (`web/public`: `favicon.svg`, `apple-touch-icon.png`, `icon-192.png`,
  `icon-512.png`, `icon.svg`) and a web app manifest (`manifest.webmanifest`, served as
  `application/manifest+json`), so the page can be pinned to a phone's home screen.
- Pages (`web/src/features/frame/route/index.ts`): `/overview[/<resource>]` (throughput, downloads, requests,
  storage, connections), `/platforms/<id>[/<part>]` (cached games, library, prefill, account, activity, setup),
  `/platforms/<id>/<game>`, `/settings`, `/dns`, `/history`, `/health`, `/clients`, `/sign-in`, `/join/<token>`,
  `/account`, `/accounts`; anything else leads to the overview. Live data comes from `/api/v1/events`
  (`web/src/adapters/api/live`).
- Overview (`web/src/features/status`): one range (day, week, month) for the whole page; a status line (what
  lanstash does now, the checks), a tile per resource, then the home (savings over the range, traffic, games
  served most, the stash) or a resource's page (live strip while something moves, the range's chart and
  figures, all counters folded). The counters over the range come from `/api/v1/stats`, asked in parts of at most
  32 names (the API's limit, `crates/adapters/admin-api/src/api/kept.rs`) **[verified]**.
- Hidden games (`/api/v1/hidden`, kept in `<state>/hidden`): the frame marks the stash's entries of a hidden
  game before any page draws them (`web/src/features/platforms/hidden`), so every page shows it as "A hidden
  game" without art, page link or builds; History, the account library, the palette and the prefill queue ask
  the same (`web/src/features/privacy`). The game is still cached, updated and prefilled. With accounts,
  lanstash masks it itself for anyone who may not hide games (entries marked `hidden`, which the page masks
  whatever it knows), and whoever may sees it by name, marked hidden; without accounts a browser can show
  hidden games by name (Settings, kept in its local storage) **[verified: web tests]**.
- Accounts (`web/src/features/access`): `/sign-in`, `/join/<token>` (an invite), `/account` (one's own: what
  it may do, its platform accounts, its password), `/accounts` (whoever may manage accounts: each a row with
  its role switched in place and a player's platform accounts as pills, the invites waiting, and adding an
  account by an invite link or with a password set now), and at the sidebar's foot who uses the page with
  Sign in or Sign out. Every control shows only with the permission its request needs (prefill, a player's own
  games only and no Stop, delete, clients, platforms, settings, dns, hide); the page asks for platform
  accounts and the hidden list only with `platforms` and `hide`. After a sign-in or sign-out the status stream
  starts again (lanstash masks per viewer). A lanstash without `/api/v1/session` reads as one without accounts
  **[verified: web tests]**.
- Development: `pnpm install`, `pnpm dev` (Vite), `pnpm test` (Vitest), `pnpm build` (`tsc -b`, then `dist/`),
  `pnpm lint` (Biome). `pnpm dev` proxies `/api` to the lanstash named in `LANSTASH_API`, from the environment or
  `web/.env.local` (not committed), else `http://localhost:8081` (`web/vite.config.ts`). The admin port's
  per-address limit (24) leaves room for the dev server's proxy, which is one address.
- Served by lanstash: `sudo lanstash install --web web/dist` copies the build where `web_ui` says (default
  `/usr/local/share/lanstash/web`; copied beside it, then swapped in whole, `install_web` in
  `crates/server/src/setup.rs`). The admin port then answers `/` and every path of the page's own with its
  `index.html` (`Cache-Control: no-cache`), a file of it as itself (a hashed `/assets/<name>-<hash>.js` kept by
  the browser for good, an icon or the manifest for an hour), and API paths as before; with no build there, `/` is the built-in status page
  (`crates/adapters/admin-api/src/api/web.rs`) **[verified: `crates/adapters/admin-api/tests/api/web.rs`]**.

## Clean stop

`lanstash stop` sends `POST /api/v1/shutdown` (then `/shutdown`) to `admin_listen`, reached through loopback when
it listens on a wildcard address, and waits until the port closes. The API accepts it from this machine only.
The stop hook (`stop_hook` `crates/server/src/run/serve.rs:351`) saves the lifetime totals, seals the open
segments (waiting up to 20 s, `STOP_WAIT` `:343`, for records being written; `Shutdown` `crates/app/src/shutdown.rs:23`,
`begin` `:38`, `finish` `:51`), saves the last-use times and exits (code 0; 75 for a restart). A hard kill is
still safe ([storage.md](storage.md#crash-safety)); it costs a scan of the open segments and up to 5 minutes of
recency (last-use times are saved every 5 minutes). **[verified]**

## Import from nginx

`lanstash import` reads the nginx cache (read-only): `import_nginx` `crates/app/src/import_nginx.rs:121`,
cache files `read_cache_file` `crates/app/src/nginx_file.rs:43`, format `parse` `crates/domain/src/nginx_cache.rs:110`.
Every file is handled on its own, each slice exactly once. Objects below `object_file_min` are reassembled by the
worker that finds their first slice: a complete one becomes one segment record (checked against the profile's body
check, Steam's `x-content-crc`); an incomplete one has its present slices written to an object file. Larger
objects: each slice becomes an extent of the object's file at its offset (the total from the stored
`Content-Range`). Exit 1 when files were bad or failed their check. **[verified]**

Games can also be found without walking the cache: lancache keys every slice `$cacheidentifier$uri$slice_range`
(**[verified]**: `blizzard/tpr/ovw/...bytes=0-1048575`, 1 MiB slices) and names the file `md5(key)` (`slice_key`
`crates/domain/src/nginx_cache.rs:208`, `cache_file_path` `:165`). A build's plan lists every file and archive
range an install downloads, so each slice it needs has a known file name: the nginx views and prefill from
lancache build on it ([battlenet.md](battlenet.md#prefill-from-lancache)).

## Install, uninstall, doctor

Steps that touch the system: `crates/server/src/setup.rs`; pure parts: `crates/server/src/install.rs` (`Layout`
`:12`, `render_unit` `:52`, `installed_config` `:85`, `render_checks` `:107`). Tests `crates/server/tests/install.rs`.
**[verified]**

- `sudo lanstash install [--config FILE] [--KEY VALUE]... [--no-start] [--force]` (`install` `setup.rs:90`): Linux
  with systemd only. The configuration is the given file (else the installed one), then the flags as lines, then
  `admin_listen = 127.0.0.1:8081` when nothing set it; it must parse, name `upstream_dns` and use absolute store
  paths. Refuses when a listen port is taken and the service is not running (lancache, a test lanstash), unless
  `--force`. Copies the binary to `/usr/local/bin/lanstash` (via `lanstash.new` and a rename, so a running copy
  keeps its file), writes `/etc/lanstash/lanstash.conf` (the previous one kept as `lanstash.conf.bak` when it
  differs), creates the log directories, writes `/etc/systemd/system/lanstash.service` (after
  `network-online.target` and `remote-fs.target`, `RequiresMountsFor=` each store root, `ExecStop=lanstash stop
  --wait 60`, `TimeoutStopSec=90`, `Restart=on-failure`, `RestartSec=5`, `LimitNOFILE=65536`), reloads systemd,
  then enables and starts it, or restarts it when it was running (an upgrade), and waits up to 60 s for the admin
  port. `--no-start` stops before starting. Output goes to the journal (`journalctl -u lanstash`).
- `sudo lanstash uninstall [--purge]` (`:156`): disables and stops the service (its `ExecStop` is a clean stop),
  removes the unit and the binary; `--purge` also `/etc/lanstash`. The cached data is never deleted; its roots
  are named.
- `lanstash doctor` (`:232`, read-only; default config `/etc/lanstash/lanstash.conf`): version (a newer release
  asked of GitHub now, unless `check_updates` is off: `version_check` `crates/app/src/releases.rs`), service state, configuration,
  `admin_listen` set, each listen port (answering when the service runs; else free, or held by a lanstash outside
  the service), the admin API, the service's open files against its limit, each store root writable and its free
  space against `keep_free`, the store's contents (from the running service's status, with its background check's
  problems; else the store loaded here read-only), upstream DNS (`us.patch.battle.net` resolved), cache-domain
  lists, log directories. One line per check `[ok  |WARN|FAIL] what detail`, then the counts; exit 1 on any
  `FAIL`.

## Tools

`lanstash seg dump|verify`, `lanstash store stats` (per service and game), `lanstash store verify` (all bodies and
extents against their checksums, [storage.md](storage.md#verification)).
