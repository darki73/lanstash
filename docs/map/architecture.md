# Map: architecture

Code references are `path:line`; line numbers may have moved, so search for the named item. Measurements behind
the decisions are in `docs/measurements.md`; claims there and here are marked **[verified]** or **[hypothesis]**.

lanstash is a caching HTTP proxy for game CDNs (Steam, Battle.net, Epic, Riot, ...) that replaces
`lancachenet/monolithic` (nginx). Clients reach it through DNS that points the CDN host names at the cache.

## Decisions

| Topic | Decision |
|---|---|
| Platform | Linux is the supported and benchmarked target. Builds and runs on Windows as best effort (development): positional I/O, directory fsync and `sendfile` have fallbacks there (`pos_io` in `crates/adapters/fs/src/fs_file.rs`, `sync_dir` in `crates/adapters/fs/src/dir_fs.rs`, `crates/adapters/zerocopy/src/sendfile.rs:63`) **[verified]**. |
| Licence | Apache-2.0 (`Cargo.toml`, `[workspace.package]`) **[verified]**. |
| Dependencies | std, plus an explicit allow-list of third-party crates, each approved for named packages only ([rules](#third-party-crates)) **[verified]**. |
| Concurrency | One OS thread per client connection (`crates/adapters/http/src/server.rs:126`) and per download (`crates/app/src/fetch/mod.rs:374`), blocking I/O, bounded counts (`max_connections`, `max_per_client`). Background work runs in [loops](#background-loops) on threads of their own **[verified]**. |
| Crash model | Crash-only: every write is ordered so that a kill at any point leaves a consistent store ([storage.md, crash safety](storage.md#crash-safety)). A clean stop exists but only shortens recovery (it seals open segments). |
| Unit of caching | An **object** (one CDN file) stored as **extents** (byte ranges actually downloaded). No fixed slice size. |
| Misses | Fetch exactly the missing part of the requested range, in pieces of at most `fetch_piece` bytes. |
| Storage | Per service and game: small whole objects in 1 GiB segments, large or partial objects in sparse files at their CDN path ([storage.md](storage.md#layout)). State beyond the stash is in one SQLite database and private files ([storage.md](storage.md#kept-outside-the-store)). |
| Zero copy | Stored bytes are sent from their file with `sendfile(2)` on Linux (`send_file` in `crates/adapters/http/src/server.rs:206`, `zero_copy` in the server settings); the only `unsafe` in the workspace, in one file ([rules](#rules)). Falls back to read and write where the system or file system cannot (`Sent::Unsupported`, `crates/adapters/zerocopy/src/sendfile.rs:45`) **[verified]**. |

## Goals and non-goals

Goals:
1. Saturate a 10 GbE client from cache on SMB-backed storage (at least 1 GB/s with one Steam client).
2. First downloads as fast as the origin allows, for every service (no per-slice round trips).
3. Never serve or store bad data: truncated, mismatching or failed bodies are never stored, and a client never
   receives the final byte of a body that failed a check.
4. Startup in seconds, independent of the number of cached objects.
5. Know games: per-service, per-game storage that can be listed, sized, moved and deleted.
6. Import the existing nginx cache instead of downloading it again.
7. Clean hexagonal architecture, enforced by tests; third-party crates only where std has no answer (TLS,
   SQLite, one system call), each named with its licence.

Non-goals: TLS interception (HTTPS is passed through by host name, [requests.md](requests.md#https-listen_tls)),
running a DNS server (lanstash only configures an existing one through its admin API: AdGuard Home, Pi-hole),
HTTP/2 and HTTP/3.

## Crates

Cargo workspace (`Cargo.toml` members, 26 crates). Dependencies point inward only; platforms are
self-contained. The "may depend on" column is the `internal` list of each crate's row in `LANSTASH`
(`crates/architecture/src/policy.rs:136`) and matches the manifests **[verified]**.

| Crate (directory) | Layer | May depend on | Holds |
|---|---|---|---|
| `lanstash-domain` (`crates/domain`) | domain | nothing | pure logic, `#![no_std]` + alloc: HTTP/DNS/TLS codecs, segment and journal formats, extents, object keys and storage names, the `Profile` trait and `Profiles`, host rules, eviction policy, nginx cache format, names, JSON, checksums, inflate, zip, RSA, QR codes, art |
| `lanstash-ports` (`crates/ports`) | ports | domain | traits and the plain values they carry, `no_std` ([ports](#ports-and-the-composition-root)) |
| `lanstash-store` (`crates/store`) | core | domain, ports | layout, segments, object files, journal, index, eviction support, compaction, verification; no I/O of its own (`no_io`) |
| `lanstash-app` (`crates/app`) | core | domain, ports, store | generic use cases: serve, fetch, pass-through, evict, status, health, art, nginx import and lancache reads, prefill runner, delete and prune a game, patrol (background verify), build history, counters kept, client names, DNS link, auto update, background loops; no I/O of its own |
| `lanstash-adapter-os` (`crates/adapters/os`) | adapter | ports | running programs with a time limit, sleeping (`Pause`), systemd, the terminal prompt |
| `lanstash-adapter-fs` (`crates/adapters/fs`) | adapter | domain, ports, adapter-os | the storage ports over directories (symbolic links refused), free space (`df`), lancache's nginx cache directory, private files (`Keep`, `Secrets`) |
| `lanstash-adapter-dns` (`crates/adapters/dns`) | adapter | domain, ports | the upstream DNS resolver (UDP), reverse lookups of client addresses (`ReverseNames`) |
| `lanstash-adapter-zerocopy` (`crates/adapters/zerocopy`) | adapter | nothing (libc on Linux) | `send_file` over `sendfile(2)`, behind a safe signature |
| `lanstash-adapter-http` (`crates/adapters/http`) | adapter | domain, ports, app, adapter-dns, adapter-zerocopy | HTTP server, upstream HTTP client (`Upstream`), web API client over HTTP(S) (`Web`, rustls), HTTPS passthrough |
| `lanstash-adapter-logs` (`crates/adapters/logs`) | adapter | domain, ports, adapter-http | rotating files, event log (`Events`), access log |
| `lanstash-adapter-names` (`crates/adapters/names`) | adapter | domain, ports | name tables, the names file, chains of name sources (`GameNames`) |
| `lanstash-adapter-admin-api` (`crates/adapters/admin-api`) | adapter | domain, ports, app | admin HTTP API, status page, client of a running lanstash |
| `lanstash-adapter-steam` (`crates/adapters/steam`) | adapter | domain, ports | Steam's CM over WebSocket and TLS (`Link`, `Dialer`), secure random bytes (`Random`) |
| `lanstash-adapter-sqlite` (`crates/adapters/sqlite`) | adapter | ports | the SQLite database: `HistoryLog`, `StatsLog`, `PrefillLog`, `ClientLabels` ([storage.md](storage.md#the-database)) |
| `lanstash-battlenet-domain` (`crates/platforms/battlenet/domain`) | platform | domain | TACT, BLTE, download manifests, plans, catalog, piece checks, nginx keys, profile; `no_std` |
| `lanstash-battlenet-app` (`crates/platforms/battlenet/app`) | platform | domain, ports, store, app, battlenet-domain | products, builds, plans, catalog (names and art), prefill, nginx probe, prune |
| `lanstash-battlenet-api` (`crates/platforms/battlenet/api`) | platform | domain, ports, battlenet-domain, battlenet-app | the `Platform` the root registers, JSON views |
| `lanstash-steam-domain` (`crates/platforms/steam/domain`) | platform | domain, ports | profile, art, depot names, builds, manifests, the CM protocol, the `Platform`; `no_std` |
| `lanstash-steam-app` (`crates/platforms/steam/app`) | platform | domain, ports, store, app, steam-domain | CM connection, log-in and QR sign-in, accounts, library, builds, manifests, chunk lists, prefill, prune, what lancache holds |
| `lanstash-epic` (`crates/platforms/epic/domain`) | platform | domain, ports | profile, the PC setup file, signing in, library answers, manifests, the `Platform`; `no_std` |
| `lanstash-epic-app` (`crates/platforms/epic/app`) | platform | domain, ports, store, app, epic | accounts (a pasted code traded for tokens, tokens kept and renewed), library, builds, sizes, prefill, prune, through the `Web` port |
| `lanstash-riot` (`crates/platforms/riot`) | platform | domain, ports | profile and `Platform`; `no_std` |
| `lanstash-dns-adguard` (`crates/dns/adguard`) | DNS server, built like a platform | domain, ports | AdGuard Home as a `DnsServer` (rewrites, how it answers a name, query log) over its admin API, through the `Web` port; `no_std` |
| `lanstash-dns-pihole` (`crates/dns/pihole`) | DNS server, built like a platform | domain, ports | Pi-hole as a `DnsServer` (reply rules, local records and CNAMEs, query log) over its v6 API, through the `Web` port; `no_std` |
| `lanstash` (`crates/server`) | root | every crate but adapter-zerocopy (reached through adapter-http) | the binary and the composition root |
| `lanstash-architecture` (`crates/architecture`) | tool | nothing | the rules below, as tests |

Every platform crate and both DNS server crates are `no_io`: their sources name none of `std::{fs, net, process,
env, io, time}` (`NO_IO_STD`, `crates/architecture/src/policy.rs:52`) **[verified]**.

Dev-dependencies (tests only, the `dev_internal` lists): store and app on themselves with `test-support`;
adapter-fs on store; admin-api on store and adapter-http; battlenet-api on store and app. The platform apps
turn on `test-support` of store (and app, for Battle.net and Steam), which their `internal` lists already allow
**[verified]**.

## Rules

All in `crates/architecture`; `cargo test -p lanstash-architecture` runs them against the real workspace
(`the_workspace_keeps_every_rule`, `crates/architecture/tests/audit.rs:15`; `audit`,
`crates/architecture/src/audit.rs:16`). The tables are `LANSTASH` (`crates/architecture/src/policy.rs:136`).
The workspace is loaded from disk by `load` (`crates/architecture/src/workspace.rs:128`); the checks are pure
functions over it **[verified]**.

| Rule | Check |
|---|---|
| Every member has a policy row at its directory; every manifest under `crates/` is a member; every row is a member | `check_policy_coverage` `crates/architecture/src/deps.rs:15` |
| Workspace dependencies only as the row allows (`internal`; `dev_internal` for dev-dependencies) | `check_internal_dependencies` `crates/architecture/src/deps.rs:44` |
| Layers: core never names an adapter or a platform; an adapter never names a platform; a platform names only core and its own platform's crates | `check_layers` `crates/architecture/src/deps.rs:81` |
| No dependency cycles (a crate's dev-dependency on itself is not an edge) | `check_acyclic` `crates/architecture/src/deps.rs:114` |
| Third-party crates: a direct dependency must be on the allow-list and approved for that package; `Cargo.lock` holds only members and allow-listed crates | `crates/architecture/src/deps.rs:49`, `check_lock` `:163` |
| Crate roots carry `#![forbid(unsafe_code)]` (`#![deny(unsafe_code)]` in the one crate allowed `unsafe`), and `#![no_std]` where the row says | `check_sources` `crates/architecture/src/sources.rs:200` |
| `unsafe` (or `allow(unsafe_code)`) only in `crates/adapters/zerocopy/src/sendfile.rs` | `crates/architecture/src/sources.rs:217`, policy `unsafe_code` `crates/architecture/src/policy.rs:379` |
| No `pub use` / `pub(crate) use` re-exports (the exempt list is empty) | `crates/architecture/src/sources.rs:223` |
| No inline tests (`#[cfg(test)]`, `#[test]` in `src/`) | `crates/architecture/src/sources.rs:228` |
| No global mutable or lazy state (`static mut`, `OnceLock`, `LazyLock`, `lazy_static`, `thread_local!`, `OnceCell`) | `GLOBAL_STATE` `crates/architecture/src/policy.rs:58` |
| Only the root (and the tool) reads arguments, environment and clocks: `env::args`, `env::var`, `SystemTime::now`, `Instant::now` | `ROOT_ONLY_CALLS` `crates/architecture/src/policy.rs:55`, `crates/architecture/src/sources.rs:238` |
| `no_io` crates name no `std::{fs, net, process, env, io, time}` | `crates/architecture/src/sources.rs:246` |
| Per-module import bans: admin-api never names the store; battlenet-api starts no threads and never names the store; the server's pure modules (`cli`, `config`, `install`, `report`, `prefill_cli`, `dns`) name no `std::fs`/`process`/`thread` and no adapter | `module_bans` `crates/architecture/src/policy.rs:347`, `check_module_imports` `crates/architecture/src/module_imports.rs:30` |
| At most 400 lines per `.rs` file under `src/` and `tests/` (the exempt list is empty) | `check_file_sizes` `crates/architecture/src/file_size.rs:9` |
| `tests/` mirrors `src/`: a test file per module, no test file without a module | `check_test_mirror` `crates/architecture/src/mirror.rs:17` |
| README.md, CHANGELOG.md and `tests/` per crate (the exempt list is empty); `[lints] workspace = true`, except the crate allowed `unsafe`, which keeps the same lints with `unsafe_code = "deny"` | `crates/architecture/src/files.rs:8`, `:23` |
| No em dashes in `.rs`, `.toml` and `.md` files under `crates/` | `check_em_dashes` `crates/architecture/src/comments.rs:9` |

The modules allowed to have no test file are listed with a reason in `untested`
(`crates/architecture/src/policy.rs:333`): `domain::segment/bytes` (crate-private, run by every segment round trip)
and the binary's own modules (`run`, `run/platforms`, `run/serve`, `run/store`, `run/steam`, `run/epic`,
`run/kept`, `run/clients`, `run/health`, `setup`, `remote`), which `tests/` cannot reach. A test asserts that every
allow-list entry names something that exists (`the_allow_lists_name_existing_things`,
`crates/architecture/tests/audit.rs:30`) **[verified]**.

### Third-party crates

`ALLOWED` (`crates/architecture/src/external.rs:13`) lists what may be in `Cargo.lock`; `direct_external`
(`crates/architecture/src/policy.rs:322`) says which package may name which directly **[verified]**:

| Package | Crates (pinned versions in its manifest) | Licence | Why |
|---|---|---|---|
| adapter-http | `rustls`, `webpki-roots`, `ring` | rustls: Apache-2.0 OR ISC OR MIT; webpki-roots: CDLA-Permissive-2.0; ring: Apache-2.0 AND ISC | HTTPS to web APIs on the network (a DNS server's admin API), a pinned certificate's SHA-256 |
| adapter-steam | `rustls`, `webpki-roots` | as above | TLS for Steam's CM WebSocket |
| adapter-sqlite | `rusqlite` (feature `bundled`) | MIT; SQLite itself public domain, compiled in | what is kept beyond a restart |
| adapter-zerocopy | `libc` (Linux only) | MIT OR Apache-2.0 | `sendfile(2)` |

The rest of `ALLOWED` is what these pull in (`cc`, `cfg-if`, `getrandom`, `once_cell`, `rustls-pki-types`,
`rustls-webpki`, `shlex`, `subtle`, `untrusted`, `zeroize`, `libsqlite3-sys`, `bitflags`, `fallible-iterator`,
`fallible-streaming-iterator`, `smallvec`, `pkg-config`, `vcpkg`, `find-msvc-tools`, the `windows-*` and `wasi`
targets); licences per `crates/architecture/src/external.rs:1`.

## Ports and the composition root

Ports (`crates/ports/src`, traits only) and their implementations **[verified]**:

| Port | Trait | Implemented by |
|---|---|---|
| Storage | `FileRead` `crates/ports/src/storage.rs:51`, `FileWrite` `:74`, `StoreFs` `:102` | `FsFile`, `DirFs` (adapter-fs); `MemFs` (store `test-support`) |
| Origins | `Upstream` `crates/ports/src/upstream.rs:55` | `HttpUpstream` `crates/adapters/http/src/upstream.rs:160` |
| Web APIs | `Web` `crates/ports/src/web.rs:104` | `HttpWeb` `crates/adapters/http/src/web.rs:160` |
| Clock, pause | `Clock` `crates/ports/src/clock.rs:5`, `Pause` `:21` | `SystemClock` `crates/server/src/main.rs:68`, `Sleep` `crates/adapters/os/src/pause.rs:11`; `FixedClock`, `NoPause` for tests |
| Events | `Events` `crates/ports/src/events.rs:30` | `EventLog` `crates/adapters/logs/src/event_log.rs:66` |
| Names | `GameNames` `crates/ports/src/names.rs:7` | `TableNames`, `ChainNames` (adapter-names); Steam's and Epic's libraries |
| Art | `ArtSource` `crates/ports/src/art.rs:47` | Steam (`SteamArt`), Battle.net (`BlizzardCatalog`), Epic (`EpicLibrary`) |
| lancache's cache | `NginxCacheSource` `crates/ports/src/nginx.rs:10` | `NginxDir` `crates/adapters/fs/src/nginx_dir.rs:33` |
| Kept state, secrets | `Keep` `crates/ports/src/keep.rs:9`, `Secrets` `crates/ports/src/secrets.rs:7` | `PrivateDir` `crates/adapters/fs/src/private_dir.rs:96`, `:110` |
| Database | `HistoryLog` `crates/ports/src/history.rs:76`, `StatsLog` `crates/ports/src/stats.rs:11`, `PrefillLog` `crates/ports/src/prefill.rs:202`, `ClientLabels` `crates/ports/src/clients.rs:19` | `Database` (adapter-sqlite) |
| Client names | `ReverseNames` `crates/ports/src/clients.rs:9` | `PtrResolver` `crates/adapters/dns/src/reverse.rs:54` |
| Message links | `Link` `crates/ports/src/link.rs:9`, `Dialer` `:25`, `Random` `crates/ports/src/random.rs:6` | `Wss`, `WssDialer`, `RingRandom` (adapter-steam) |
| Prompt | `Prompt` `crates/ports/src/prompt.rs:7` | `Terminal` `crates/adapters/os/src/terminal.rs:31` |
| DNS server | `DnsSetup` `crates/ports/src/dns.rs:49`, `DnsServer` `:109` | `LiveDns` `crates/server/src/dns.rs:107`; `AdGuard`, `PiHole` (dns crates) |
| Live settings | `Tunables` `crates/ports/src/settings.rs:8` | `LiveSettings` `crates/server/src/settings.rs:268` |
| Releases | `Releases` `crates/ports/src/releases.rs:6` | `NoReleases` only |
| Accounts | `Account` `crates/ports/src/account.rs:10` | `SteamAccount`, `EpicAccounts` (platform apps) |

- The platform contract: `Platform` `crates/ports/src/platform.rs:99` (id, names, art, builds and build status,
  catalog, game, nginx, prefill, prune, parts, forget, account, setup file; views are `Result<String, ViewError>`,
  `:20`) and `Prefill` `crates/ports/src/prefill.rs:161` (start, queue, unqueue, stop, status, ended jobs).
  Implemented by `SteamPlatform`, `BattleNet`, `EpicPlatform`, `RiotPlatform`, and wrapped by `KeptBuilds`
  (`crates/app/src/kept_builds.rs:178`), which works builds out on a thread and keeps them.
- Request routing stays a domain concern: `Profile` `crates/domain/src/service/mod.rs:56`, registered in
  `Profiles` (`:112`), with `Generic` (`:91`) for every other service ([services.md](services.md#profiles)).
- One composition root, `crates/server` **[verified]**:
  - `main` `crates/server/src/main.rs:20`: parses the command line, makes the `SystemClock`, dispatches.
  - `crates/server/src/run/mod.rs`: configuration (file, then settings saved from the web UI, then flags)
    `config` `:52`; store roots `store_roots` `:76`, `open_store` `:84` (prints the [startup
    report](storage.md#startup)); host rules `host_rules` `:147`; HTTP server settings `server_config` `:37`.
  - `serve` `crates/server/src/run/serve.rs:52`: resolver, upstream, fetcher, passthrough, HTTPS passthrough,
    eviction, the cache; with `admin_listen`, the platforms, the database, build history, art, names, lifetime
    totals, status, patrol, auto update, DNS link and the admin API; then the cache port. Clean stop
    `stop_hook` `:351` (exit code 0, or `RESTART_CODE` 75 `:348` to be restarted after a settings change).
  - `crates/server/src/run/platforms.rs`: `profiles` `:81`, `battle_net` `:87`, `kept_builds` `:44`,
    grouping rules `builtin_names` `:63`, `game_names` `:71`; `run/steam.rs` `platform` `:128`; `run/epic.rs`
    `platform` `:36`; `run/kept.rs` (database `:23`, counters `:38`, prefill runs `:66`); `run/clients.rs`;
    `run/health.rs` `checks` `:19`; `run/store.rs` (import, `store stats`, `store verify`, segment dump).
  - The pure parts (`cli`, `config`, `dns`, `install`, `prefill_cli`, `report`, `settings`) are a library
    (`crates/server/src/lib.rs`) so `tests/` can reach them.
  - No global configuration: every option is passed in.

### Background loops

`forever` `crates/app/src/rounds.rs:25` runs a round again and again on a named thread until it answers
`Next::Stop`; a round that panics is reported and followed by a pause of `AFTER_PANIC_MS` (60 s, `:10`) instead
of ending the thread **[verified]**. Users (thread names): `lanstash-evict` (`crates/server/src/run/serve.rs:313`),
`auto-update`, `client-names`, `counters-kept`, `<platform>-builds` (`KeptBuilds`), `lifetime`, `verify`
(patrol), `prefill-runs`, `epic-builds`, `epic-library`, `epic-sizes`, `steam-builds`, `steam-lancache`,
`steam-library`, `steam-heartbeat`. The HTTPS and admin accept loops (`lanstash-tls-accept`, `lanstash-admin`) and
the console statistics (`lanstash-stats`) are plain threads started by `serve`; the cache port's accept loop runs
on the main thread (`crates/server/src/run/serve.rs:198`).

## Tests

Tests live in each crate's `tests/` directory (public API only), never as `#[cfg(test)]` modules in `src/`;
`tests/` mirrors `src/` ([rules](#rules)); helpers sit in `tests/support/` or `support.rs` and are not test
modules (`crates/architecture/src/mirror.rs:1`) **[verified]**. Test doubles shared across crates sit behind a
`test-support` feature: `lanstash-store` (`MemFs`, `MemFile`, `ManualClock`, an nginx cache;
`crates/store/src/test_support/mod.rs`) and `lanstash-app` (`FakeUpstream`, `Capture`, `MemoryLogs`;
`crates/app/src/test_support.rs`). The SQLite adapter has `Database::in_memory`
(`crates/adapters/sqlite/src/db.rs:96`).
