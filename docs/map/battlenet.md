# Map: Battle.net

Code references are `path:line`; line numbers may move, so search for the named item. Claims are marked **[verified]**
(read in the code or a test) or **[hypothesis]**. Three crates: `crates/platforms/battlenet/domain` (pure),
`.../app` (use cases), `.../api` (the `Platform` the root registers). Wired by `battle_net`
`crates/server/src/run/platforms.rs:87`; like every platform it is then wrapped in `KeptBuilds` ([Kept builds](#kept-builds)).

## The platform port

`Platform` `crates/ports/src/platform.rs:99`: every capability optional (`names`, `art`, `builds`,
`build_status`, `builds_and_status`, `builds_mark`, `catalog`, `game`, `nginx`, `prefill`, `prune`, `parts`,
`forget`, `account`, `setup`), a view being a JSON string or a `ViewError` (`NotFound`, `Unavailable`).
`BattleNet` `crates/platforms/battlenet/api/src/platform.rs:25` (id `blizzard`, `:20`) answers `names` and `art`
(both its catalog), `builds`, `build_status`, `builds_and_status`, `catalog`, `game`, `prune`, `nginx`, `prefill`;
each is `None` until its use case is given (`with_products`, `with_builds`, `with_pruner`, `with_nginx`,
`with_prefill`) **[verified]**. No account, no setup file. `status_of` `:84` gives one `BuildStatus` per product:
the game's own by its directory, each flavour by its product code, so automatic updates keep every flavour current
**[verified]**.

## Views

The views behind the generic admin paths ([operations.md](operations.md#admin-api)); JSON in
`crates/platforms/battlenet/api/src/json.rs`.

| Path | Document |
|---|---|
| `GET /api/v1/builds/blizzard` | `{"region", "lancache", "games": [{"game", "product", "version", "build_config", "have", "want", "complete", "plan_error", "current", "prerelease", "error", "flavours", "lancache_share"}]}`; `prerelease` is `null` or `{"version", "build_config", "have", "want", "complete", "plan_error"}`; `lancache`: whether prefill from lancache can run; `flavours`: the other products of the directory, `[{"product", "name", "build", "prerelease"}]` (`BuildCheck::flavours`); `lancache_share`: `{"have", "want"}` bytes of the current build lancache's cache holds, asked only for a build not stashed whole and probed in the background once per build (`NginxProbe::share`), `null` until known (`json::builds_of` `:48`) |
| `GET /api/v1/catalog/blizzard` | every Battle.net install there is, one entry per product code: `{"products": [{"code", "name", "family", "game", "version", "known"}]}` (`family` the catalog product's name; `game` its CDN directory; `json::catalog` `:85`) |
| `GET /api/v1/games/blizzard/<game>` | one game: `{"game", "product", "region", "build", "prerelease", "error", "lancache", "flavours"}`; each build adds `"detail"`: `{"files_have", "files_want", "archived_have", "archived_want", "file_count", "part_count", "archive_count", "requests", "absent"}` or `null` until planned (`json::game` `:128`, `json::detail` `:105`) |
| `GET /api/v1/games/blizzard/<game>/nginx` | what lancache's nginx cache (`nginx_cache`) holds of the current build, from its plan (read-only, synchronous): `{"game", "product", "version", "whole", "archived", "slices", "slices_found", "missing"}`; `whole`/`archived` are `{"wanted", "found", "bytes", "bytes_found"}`; 404 without `nginx_cache` (`json::probe` `:169`) |
| `GET /api/v1/games/blizzard/<game>/prune` | `{"game", "files", "bytes", "recent_bytes"}` (`json::prune` `:194`); `POST` the same path deletes `files` as a deletion job ([Prune](#prune)) |

## Kept builds

`KeptBuilds` `crates/app/src/kept_builds.rs:55` wraps every platform (`kept_builds`
`crates/server/src/run/platforms.rs:44`) **[verified]**:

- `builds`, `build_status` and `builds_and_status` answer what a thread of its own (`keep_fresh` `:131`, named
  `<id>-builds`) last worked out, so a request costs nothing and a slow platform holds back no other. Before the
  first run a request works them out itself (`kept` `:164`).
- One run is one `builds_and_status` of the inner platform: the view and the status from one walk over the stash.
- The thread runs again when `Store::changes(<id>)` or the platform's own `builds_mark` moves (looked at every
  `look_ms`, 2 s), rests at least `rest_ms` (5 s) and twice the last run's time, and runs anyway every
  `every_ms` (1 min, doubling while runs change nothing) while a build is not known, every `settled_ms` (5 min)
  once all are (`KeepOptions` `:33`).
- When the result differs, `changed_ms` is noted (the status stream tells the web) and `BuildHistory` records it.
- A platform with no builds ends its thread (`Next::Stop`). Every other call passes through to the inner platform.

## Catalog, names and art

`BlizzardCatalog` `crates/platforms/battlenet/app/src/catalog.rs:61`; pure parts
`crates/platforms/battlenet/domain/src/catalog.rs` (`current_build` `:29`, `base_fragments` `:43`,
`global_types` `:134`, `product_with` `:174`). All plain HTTP **[verified]**: `/catalogs/versions` and
`/catalogs/cdns` from `us.patch.battle.net:1119` (`PATCH_HOST` `crates/platforms/battlenet/domain/src/tact.rs:20`),
then the build config, the root and the base fragments from the CDN, built on first use and checked again every
6 h (`CatalogOptions` `:29`: `refresh_ms`, `retry_ms` 1 min, `max_document` 8 MiB). The current catalog is the row
with the highest `VersionsName`; `BuildId` is not monotonic.

The catalog is the only source of Battle.net names, codes and art; there is no built-in table.

- Installs (flavours): a product lists every install players get (`Install` `:60`, code and name). An install's
  type is its key among the fragment's own types, else its key without the main install's prefix among the
  root's shared types (`wow_ptr` is `ptr`); the main install (shortest key) is retail. Left out: types in an
  internal category (`INTERNAL` `:100`: in development, vendor, event, professional, tournament, unknown),
  `deprecated` installs, and, where a fragment gives installs an `auto_update_policy`, those without one.
- An install's name: its type's own name (`name_style` `default`: "WoW: Forever"), else the product whose
  `default_product_type` is its type, else the main product's name with the type's name after it, else the main
  product's name. The main product (no `real_product_id`) names the product and gives its art
  **[verified: `crates/platforms/battlenet/domain/tests/catalog.rs`]**.
- A game (CDN path) finds its product by the path as a product code, else by the code the patch service names
  for the path: `CodeOf` `crates/platforms/battlenet/app/src/catalog.rs:192`, which the root sets once the
  products are built (`set_codes` `:199`, held weakly; `BlizzardProducts` implements it).
- Art (`ArtSource for BlizzardCatalog` `:215`, `locate` `:87`): header is the key art, icon the largest square
  icon. Names (`GameNames` `:222`): the catalog names Battle.net games; the names file still overrides.

## Products

`BlizzardProducts` `crates/platforms/battlenet/app/src/products.rs:97`: the launcher catalog's installs, one
entry per product code with its `family`, each code's CDN directory (`/<code>/cdns`, `tpr/<game>`) and version,
looked up on a thread of their own, 8 at a time (`LOOKUPS` `:108`), kept 10 min (`ProductOptions` `:46`).
`list` `:132` never waits: it answers with what is known. A directory maps back to its codes (`codes_of` `:180`:
every code under it, as several flavours share one, the main install first; `code_of` `:171` the first), so a
game not in the stash can be checked and prefilled **[verified]**.

## Builds

`BlizzardBuilds` `crates/platforms/battlenet/app/src/builds.rs:123` (`check` `:195`, `check_all` `:175`):
a game's product code is the products' `code_of` for its directory, else the directory itself;
`/<code>/versions` of `blizzard_region` (kept 10 min, `BuildOptions` `:35`) is a game's current build,
`/<code>/bgdl` a pre-release build (when it names another build). Coverage (`have` of `want`) is the plan's bytes
the stash holds, archive ranges and whole files, without counting as a use; a game is *current* only when all of
it is (`crates/platforms/battlenet/app/src/plans/held.rs`, one sweep per archive). `bnt*`, `catalogs` and
`configs` are the launcher's own data (`skipped` `:186`) **[verified]**.

## Plans

`BlizzardPlans` `crates/platforms/battlenet/app/src/plans/mod.rs:112` (`plan` `:155`, `ready` `:164`,
`progress` `:181`, `coverage` `:189`, `detail` `:195`); made in `crates/platforms/battlenet/app/src/plans/make.rs`;
assembled by `assemble` `crates/platforms/battlenet/domain/src/plan.rs:63` into `Plan` `:15`.

What an install of a build downloads, read through the cache (so stashed like a client's) when the stash holds the
game's directory already, else from the CDN alone, so looking at a game puts nothing in the stash; a prefill fetches
these documents anyway, as the plan's first files: the build and CDN configs, every archive index and the file
index (`parse_index` `crates/platforms/battlenet/domain/src/tact.rs:156`), the download manifest (`decode`
`crates/platforms/battlenet/domain/src/blte.rs:41` with the in-house `crates/domain/src/inflate.rs`, then `parse`
`crates/platforms/battlenet/domain/src/download.rs:69`), its entries for `blizzard_tags` plus the region (`select`
`:113`: per tag type, an entry carries one of the chosen tags; a type with none chosen does not filter), and the
files the build config names; each placed (`locate` `crates/platforms/battlenet/domain/src/locate.rs:49`) in an
archive (offset, size), loose (file index) or absent **[verified]**.

- `PlanOptions` `:44`: `tags` (default Windows, x86_64, enUS, US), `gap` 64 KiB (archive files this close share a
  request), `max_file` 64 MiB, `keep` 32 plans (the prune's tagless plans: 4), `parallel` 8 **[verified]**.
- The CDN config's archive and file-index lists are cumulative over many builds, so they are never fetched whole.
- The selection reproduces a real install within a fraction of a per cent **[hypothesis: measured once, not
  recorded in `docs/measurements.md`]**.
- Plans are made on a thread of their own, so the builds page never waits. A build is planned once at a time: a
  second caller waits for the first and takes its plan. The configs and indexes are read `parallel` at a time over
  the region's CDN hosts, and `progress` says how many are in (Diablo IV, 915 indexes: `docs/measurements.md`,
  "Import from lancache") **[verified]**.

## Prefill

The runner is generic, in the core: `Prefiller` `crates/app/src/prefill/mod.rs:178` (the `Prefill` port,
`crates/ports/src/prefill.rs:161`), its jobs in `crates/app/src/prefill/run.rs`. A platform supplies a
`Planner` (`crates/app/src/prefill/mod.rs:103`: `admit`, `work`, `planning`) whose `work` returns a `Work` `:82`
(hosts, jobs, bytes, a `Check` per piece, `parts`, `check_cdn`). Battle.net's is `BlizzardJobs`
`crates/platforms/battlenet/app/src/prefill.rs:21` (`admit` `:64`, `work` `:74`, `planning` `:125`) **[verified]**.

- One job at a time, the current or the pre-release build: the plan's whole files (its documents first), then its
  archive ranges. `prefill_parallel` requests run at once, the workers taking the plan's CDN hosts in turn; what is
  stashed is skipped without reading, 404s counted (`files_missing`).
- `admit` refuses the launcher's own directories (`NotAGame`) and a build without plans (`NotSetUp`).
- While the build is still being planned, the status carries `planning: {"done", "total"}` (`Planning`
  `crates/ports/src/prefill.rs:110`).
- For another platform (a Mac in a LAN of Windows PCs): `POST /api/v1/prefill/blizzard/<game>?install=OSX[,arm64]`
  (`Prefill::queue_for`, `Planner::work_for`; a platform that cannot answers `NoInstall`, 400). Its plan
  (`BlizzardPlans::plan_for`, kept by `<build>+<tags>`) takes the configured tags with each named tag in place of
  those of its kind (`retarget` `crates/platforms/battlenet/domain/src/download.rs`): platform and architecture
  change, locale and region stay. The game view's `installs` lists the platforms and architectures the build has
  installs for (the tags of the kinds of `Windows` and `x86_64`, `install_tags`); the job's status names its
  `install`; the web game page offers them under "Download for another platform" **[verified: tests in
  `crates/platforms/battlenet/{domain,app}/tests`, `crates/app/tests/prefill/asked.rs`]**.

### Checked from the CDN

Battle.net's `Work` has `check_cdn: true` (every file's name is a hash of its content), and the root adds
`with_checked_cdn` (`crates/app/src/prefill/mod.rs:248`) **[verified]**:

- A request goes to the CDN directly through the `Upstream` port, not through the cache:
  `Shared::fetch_checked` `crates/app/src/prefill/checked.rs:267`. Bytes are written as they arrive
  (`StashPut::begin` `crates/app/src/stash_put.rs:87`), checked as they stream, and committed
  (`Pending::commit` `:165`) only once the check passes. A short body, a wrong `Content-Range` or a failed
  check stores nothing; the request is asked once more.
- Failing the check twice ends the job (`files_rejected`, "failed its check twice; nothing of it was stored",
  `crates/app/src/prefill/run.rs:233`). A piece the check cannot judge (`PieceCheck::checks` false) goes through
  the cache as a client's GET (`via_cache::get` `crates/app/src/via_cache.rs:69`).
- The check (`Check` `crates/platforms/battlenet/domain/src/check.rs:60`, `whole` `:77`, `range` `:95`): a
  config is the MD5 of its content, an index the MD5 of its 28-byte footer, a loose file or each file inside an
  archive range BLTE checked against its encoding key; a path no rule covers is `Never`.
- BLTE: `Verifier` `crates/platforms/battlenet/domain/src/blte.rs:111` keeps no more than the header: it collects
  the header, checks its MD5 against the encoding key, then hashes each chunk against its table entry. A header
  longer than `MAX_HEADER` `:106` (12 + 24 x 2^20 bytes: a table of 2^20 chunks) or not of the `12 + 24 n` shape
  fails at once, before it is collected (test `a_header_too_long_to_be_real_fails_without_being_collected`,
  `crates/platforms/battlenet/domain/tests/blte.rs:140`) **[verified]**.

## Prefill from lancache

`?from=lancache` (CLI `--from lancache`): each request is first read out of lancache's nginx cache by its slice
keys ([operations.md](operations.md#import-from-nginx)), checked against the hash in its path, and stored where a
download would have put it; the CDN for the rest (`?from=lancache-only`: nothing from the CDN, what lancache lacks counts as missing)
**[verified]**.

- Reading: `LancacheRead` `crates/app/src/lancache_read.rs:50` (1 MiB slices, pieces up to 512 MiB, `lancache_read`
  `crates/server/src/run/platforms.rs:177`): `open` `:128` checks that every slice the range needs exists before
  the first byte; `Slices::next_piece` `:226` hands over one slice at a time, so a file of any size takes one
  slice of memory.
- When lancache holds only some of an archive range, each file inside it whose slices are all there (`Work::parts`,
  from the plan) is taken and checked on its own, and the range then goes to the CDN for the rest.
- Storing: `StashPut::begin` writes the parts as they come; they count only once `Pending::commit` runs after the
  check passed (a failed check stores nothing, counts `files_rejected`, and the request goes to the CDN).
- Status: `files_local`, `bytes_local` (counted as they stream, taken back on a failed check), `files_rejected`.
- Measured (Overwatch 2: 83.42 GB from lancache in 245 s; Diablo Immortal 89.73 GB in 227 s; no rejections)
  **[verified]**: `docs/measurements.md`, "Import from lancache".

## Prune

`Pruner` `crates/platforms/battlenet/app/src/prune.rs:325` (`find` `:357`) **[verified]**:

- What of a game's directory no build players get any more needs: object files (archives and large loose files)
  that no current or pre-release build of any product installed from the directory uses (the game's own product
  and every flavour), for any platform or language: the plans come from a second `BlizzardPlans` made with no
  tags (`full_plans` `crates/server/src/run/platforms.rs:128`, `keep: 4`).
- If any of those builds cannot be looked up or planned, nothing is prunable (`Err`): an unknown build might need
  anything.
- Files used within the grace period (`PRUNE_GRACE_MS` `crates/server/src/run/platforms.rs:171`, 7 days) stay,
  counted as `recent_bytes`. Segments (small whole objects) are never pruned: they mix objects of several builds.
- `prune_once` `:403`: hourly (`AUTO_PRUNE_EVERY` `:174`, thread `lanstash-prune`), the first game whose live build
  is stashed whole and has something prunable, unless a deletion runs or the game is being prefilled; one deletion
  job (`GameDeleter::start_files`) at a time.
- Tests: `crates/platforms/battlenet/app/tests/prune.rs`.

## The nginx probe

`NginxProbe` `crates/platforms/battlenet/app/src/nginx.rs:61` (`probe` `:131`, `share` `:88`): the slice keys a
plan needs (`wanted` `crates/platforms/battlenet/domain/src/nginx_keys.rs:34`), which exist
(`NginxCacheSource::exists` on `cache_file_path`, `nginx.rs:171`), and the tally (`tally` `nginx_keys.rs:72`).
It reads nothing else and writes nothing. `share` answers what is known and queues the game: one probe at a time,
on a thread of its own (`lanstash-probe`), once per build config; a failed probe gives no share **[verified]**.
