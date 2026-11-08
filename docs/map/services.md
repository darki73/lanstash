# Map: services, profiles and objects

Code references are `path:line`; line numbers may have moved, so search for the named item. Claims are marked **[verified]** or **[hypothesis]**.

## Services

A **service** is a CDN family (`steam`, `blizzard`, `epicgames`, `riot`, or any other cache-domains list). Its
identifier is the object key prefix, the store directory name and the platform id (`ID`
`crates/platforms/steam/domain/src/platform.rs:22`, `crates/platforms/battlenet/api/src/platform.rs:20`;
Epic and Riot answer `epicgames`, `riot`) **[verified]**.

Host names map to services through the uklans cache-domains lists (`<service>.txt`, MIT licence; the lists are
kept on the server and are not part of the repository), one per entry of `services`, read from the
`cache_domains` directory. In a list `*.x` matches subdomains only, `x` matches itself and its subdomains; `#`
starts a comment; a trailing dot and case are ignored **[verified]**.

- Host rules: `HostRules` `crates/domain/src/service/hosts.rs:18` (`add_host` `:25`, `add_list` `:38`,
  `classify` `:51`; the first matching rule wins).
- Built by the root: `host_lists` `crates/server/src/run/mod.rs:135`, `host_rules` `:147`.
- Built-in content hosts, added before the lists **[verified]**:
  - Steam, always: `CONTENT_HOSTS` `crates/platforms/steam/domain/src/profile.rs:33` (`steamcontent.com`,
    `steampipe.akamaized.net`, `steampipe-kr.akamaized.net`, `steampipe-partner.akamaized.net`, each with its
    subdomains). lancache's Steam list names only the discovery name `lancache.steamcontent.com`
    (`LANCACHE_HOST` `:27`); a client that found the cache asks it with the CDN's own name.
  - Epic, when `epicgames` is among `services`: `CONTENT_HOSTS` `crates/platforms/epic/domain/src/profile.rs:12`
    (`google.epicgamescdn.com`, a CDN the launcher's own list names and lancache's list lacks).
- A request for a host no rule names, or without a host, is refused with 403 and
  `X-LanStash: HOST-NOT-ALLOWED` (`crates/app/src/serve/mod.rs:326`): lanstash is never an open proxy. Refused
  host names are counted for the status page (`crates/app/src/serve/refused.rs`) **[verified]**.
- The cache's profiles are fixed per service when it is built: one for each service the host rules name
  (`Cache::new`, `crates/app/src/serve/mod.rs:248`).
- Segment headers carry a numeric service code (`service_code` `crates/domain/src/service/mod.rs:135`; steam 1,
  epicgames 2, blizzard 3, riot 4, others 0, `crates/domain/src/segment/header.rs:33`) **[verified]**.

## Profiles

A **profile** answers, without I/O, how a service's requests are cached. The trait is `Profile`
`crates/domain/src/service/mod.rs:56` (`service`, `route`, `fetch_mode`, `body_check`); each platform crate has
its own, registered by the root in `Profiles` (`profiles` `crates/server/src/run/platforms.rs:81`); every other
service gets `Generic` (`crates/domain/src/service/mod.rs:91`; `Profiles::get` `:124`) **[verified]**.

`route` answers `Route::Pass` (not cached, relayed to the origin, [requests.md](requests.md#http)) or
`Route::Object { game, path }` (`crates/domain/src/service/mod.rs:21`). The cache then applies
`Route::filter_plain` (`:31`): an object whose path is not [plain](#plain-paths) is passed, not cached
(`crates/app/src/serve/mod.rs:332`) **[verified]**.

| Question | Steam | Blizzard | Epic | Riot | Generic |
|---|---|---|---|---|---|
| Is this request cached? | chunks `/depot/<d>/chunk/<40 hex>` and patches `/depot/<d>/patch/<from>/<to>` (all decimal); everything else (manifests, ...) passes | everything except `releaselisting_*` and `*.version` | the same | the same | the same |
| Game (storage group) | depot `<d>` | product: `/tpr/<product>/...`, else `_` | app in `/Builds/Org/<o>/<app>/...` or `/Builds/<app>/...`, else `_` **[hypothesis]**, from URL samples | `_` | `_` |
| Fetch mode | `WholeObject`: chunks are fetched whole, without `Range` | `Ranges` | `Ranges` | `Ranges` | `Ranges`, with a fallback ([requests.md](requests.md#origins-that-ignore-range)) |
| Origin honours `Range`? | not used | yes **[verified]** | yes **[hypothesis]** | yes **[hypothesis]** | depends on the origin |
| Body check | `x-content-crc` = CRC-32/IEEE of the whole body, when the header is there (chunks) **[verified]**; patches carry none | none | none | none | none |
| Code | `Steam` `crates/platforms/steam/domain/src/profile.rs:92` | `Blizzard` `crates/platforms/battlenet/domain/src/profile.rs:9` | `Epic` `crates/platforms/epic/domain/src/profile.rs:16` | `Riot` `crates/platforms/riot/src/profile.rs:9` | `Generic` `crates/domain/src/service/mod.rs:91` |

Requests that a profile cannot place in a game go to the game `_` (`UNKNOWN_GAME`
`crates/domain/src/object.rs:19`). A path that does not start with `/` is passed **[verified]**.

Helpers the profiles share: `route_all` `crates/domain/src/service/mod.rs:74` (everything but
`uncacheable_path` `:68`), `component` `:85`. Steam's paths: `chunk_from_path`
`crates/platforms/steam/domain/src/profile.rs:54`, `chunk_from_key` `:61`, `patch_depot` `:80`; a chunk's key
`SteamChunk::cache_key` `:47`.

For names and the web UI, Steam's depots are grouped under the nearest app id below them (`group_numbered`
with `DEPOT_WINDOW`, `builtin_names` `crates/server/src/run/platforms.rs:63`); the store keeps one directory per
depot **[verified]**.

## Object keys

The **object key** is `<service><path>`, the path without query string or fragment (as lancache's
`$cacheidentifier$uri`), e.g. `blizzard/tpr/hs/data/8e/fc/8efccb85...` or `steam/depot/1422450/chunk/9de1...`. It
does not include the host: the same archive from two Blizzard CDN hosts is one object **[verified]** identical.
The upstream request uses the client's full target, including the query string.

- `object_key` `crates/domain/src/object.rs:40`, `path_of` `:23`; built per request in
  `crates/app/src/serve/mod.rs:343`.
- The index hashes the key with FNV-1a 64 (`key_hash` `crates/domain/src/segment/footer.rs:31`) and checks the
  full key on every read ([storage.md](storage.md#index)).

### Plain paths

`plain_path` `crates/domain/src/object.rs:31`: a path is plain when it names one file one way only: no `.` or
`..` segment, no backslash, no encoded dot, slash or backslash (`%2e`, `%2f`, `%5c`, any case). An origin reads
`/a/../b` as `/b`; kept as a key of its own, every such spelling would be fetched and stored again. Requests with
a path that is not plain are passed through, never cached **[verified]**.

### Storage names

Names on disk derived from a CDN path are restricted to `[A-Za-z0-9._-]` so a path can never escape its
directory **[verified]**:

- `safe_component` `crates/domain/src/object.rs:58`: allowed bytes kept, others percent-encoded (`%XX`); an
  empty, `.`, `..` or over-long (more than `MAX_COMPONENT` = 200 bytes, `:15`) result becomes `_h_<md5>`.
- `game_dir` `:75`: the game's directory name (empty: `_`).
- `safe_rel_path` `:82`: an object file's path below `obj/`, every component made safe; more than `MAX_DEPTH`
  = 32 components (`:17`) or none: stored under the hash.
- `fallback_rel_path` `:94`: the name used when the normal one is taken by another object (on a
  case-insensitive file system, say), `_h_<md5 of the key>`, with `-<n>` added if that is taken too
  (`Game::object_file` `crates/store/src/game/write.rs:171`).

## Objects and extents

An object has a total length (known from `Content-Length` of a 200 or the `/total` of a `Content-Range`) and a
set of stored **extents**: non-overlapping byte ranges, each stored either as a whole segment record or as a
range of an object file. A request for bytes `a..=b` is split against that set into stored parts (read from disk)
and missing parts (downloaded, [requests.md](requests.md#downloads)). The pure range arithmetic has no notion of
storage: `Span` `crates/domain/src/extent.rs:11` (half-open `start..end`), `ExtentMap` `:67` (`insert` `:83`
refuses overlaps, `gaps` `:151`, `covers` `:145`), `uncovered` `:188` **[verified]**.

Where an object goes ([storage.md](storage.md#layout)): a whole object smaller than `object_file_min` (default
16 MiB) downloaded in one piece goes to a segment record; anything else (large objects, objects downloaded in
parts) to an object file (`open_target` `crates/app/src/fetch/transfer.rs:288`). A body whose length was unknown
until it ended is stored only when it is smaller than `object_file_min`, as a record (`store_unknown_length`
`:325`) **[verified]**.

Response headers that are replayed on hits (`content-type`, `content-encoding`, `cache-control`, `expires`,
`last-modified`, `etag`, `x-content-crc`, `content-md5`) are stored once per object as `name=value` lines (the
segment record's metadata, or the journal's `OBJECT` entry): `STORED_RESPONSE_HEADERS`
`crates/domain/src/http/mod.rs:204`. `x-content-crc` and `content-md5` describe the whole body and are not sent
on 206 responses (`WHOLE_BODY_HEADERS` `:216`) **[verified]**.
