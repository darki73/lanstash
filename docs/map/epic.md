# Map: Epic Games

Code references are `path:line`; line numbers may move, so search for the named item. Claims are marked **[verified]**
(read in the code or a test) or **[hypothesis]**. Two crates: `crates/platforms/epic/domain` (pure, `no_std` +
`alloc`: caching rules, sign-in forms, library and manifest parsing, the setup file, the `Platform`) and `.../app`
(use cases over ports: Epic's web APIs through the `Web` port, the secrets, the kept state, the clock, the pause).
Wired by `platform` `crates/server/src/run/epic.rs:36` (registered in `crates/server/src/run/serve.rs:115`). The
`Platform` is `EpicPlatform` `crates/platforms/epic/domain/src/platform.rs:25` (id `epicgames`, `:124`), then
wrapped in `KeptBuilds` ([battlenet.md](battlenet.md#kept-builds)).

## The platform

`EpicPlatform` answers `names` and `art` (the library), `builds`, `build_status`, `builds_mark`, `catalog`,
`prefill`, `prune`, `parts`, `account` and `setup` (Windows only, `:167`) **[verified]**. No `game` or `nginx` view;
no prefill from lancache (the root gives `with_checked_cdn` only).

| Path | Document |
|---|---|
| `GET /api/v1/builds/epicgames` | Steam's shape (`EpicBuilds::view` `crates/platforms/epic/app/src/builds.rs:151`): per stashed app its build, `have`, `want`, `current` |
| `GET /api/v1/catalog/epicgames` | every game an account owns, once, with its owners and download size (`render_catalog` `crates/platforms/epic/domain/src/library.rs:271`); before a library, `render_none` `:293` |
| `GET`/`POST /api/v1/games/epicgames/<game>/prune` | what a prune deletes / delete it ([Prune](#prune)) |
| `/api/v1/account/epicgames[/sign-in, /sign-out, /client]` | the accounts ([Sign-in](#sign-in-and-the-client)) |
| `GET /api/v1/setup/epicgames/windows` | the `.cmd` ([Windows setup file](#windows-setup-file)) |

## Sign-in and the client

By a code pasted from Epic's own page; no QR code, no password **[verified]**:

- `login_url` `crates/platforms/epic/domain/src/auth.rs:59`: Epic's login page, redirecting to a small JSON page
  that shows a one-time `authorizationCode` for the client the requests name.
- The person pastes it (the code itself, or the whole page: `code_from` `:68`) into the account page, which sends
  it to `POST /api/v1/account/epicgames/sign-in` in the `X-Lanstash-Secret` header (`SECRET_HEADER`
  `crates/adapters/admin-api/src/api/account.rs:27`, percent-decoded; `?code=` also works), not in the path,
  which logs keep. `sign_in_with_code` calls `EpicAccounts::add` `crates/platforms/epic/app/src/accounts.rs:180`:
  the code traded at `account-public-service-prod03.ol.epicgames.com` `/account/api/oauth/token` (`code_body`
  `auth.rs:80`, Basic authentication naming the client: `basic` `:92`), over HTTPS. 200 with the view, 400 when
  Epic refuses (a code is good once, for a few minutes). `start_sign_in` says Epic signs in by code.
- The client: Epic's launcher's own public client (`LAUNCHER_CLIENT` `auth.rs:32`) unless another is set:
  `POST .../client` with `<id>:<secret>` (letters and digits, each at most 64: `parse_client` `:37`) in the same
  `X-Lanstash-Secret` header (or `?value=`), `DELETE .../client` back to the built-in one (`set_client`
  `accounts.rs:256`). Kept as the secret `epic-client` (`CLIENT` `:38`). Setting it clears the access tokens of the
  other client; signed-in accounts keep their refresh tokens.
- Account view (`view` `:234`): `{"accounts": [{"name", "id", "refused"}], "account", "sign_in": {"state": "idle" |
  "failed", "scanned": false, "error"}, "method": "code", "login_url", "client": "built-in" | "own"}`.
- Tests: `crates/platforms/epic/app/tests/accounts.rs`, `crates/platforms/epic/domain/tests/auth.rs`.

## Accounts and token refresh

`EpicAccounts` `crates/platforms/epic/app/src/accounts.rs:69` **[verified]**:

- Kept per account: `epic-token-<account id>` (`token_name` `:33`), three lines: the refresh token, when it runs out
  (ms; 0: not said), the display name (`kept`/`unkept` `auth.rs:164`/`:171`). The list of ids in `epic-accounts`
  (`ACCOUNTS` `:29`), in sign-in order. An account id is 32 hex digits (`is_account_id`). Several accounts may be
  signed in; signing in again renews one.
- `access_token` `:193`: the kept access token while it lasts beyond `MARGIN_MS` `:41` (5 min), else a refresh
  (`refresh_body` `auth.rs:86`). One refresh at a time (`refreshing` `:79`, a lock); a caller that waited takes the
  token the first one got, so two callers near expiry do not both spend the same refresh token (test
  `callers_at_once_near_expiry_spend_the_refresh_token_once`). Epic hands out a new refresh token with each; it
  replaces the kept one (`keep` `:161`).
- A refresh token past its time, or one Epic refused (an `Epic: ` error, its message), marks the account `refused`
  (`refused` `:227`): it needs signing in again. `sign_out` `:281` takes an id or a name.

## Library

`EpicLibrary` `crates/platforms/epic/app/src/library.rs:54` **[verified]**; the answers' shapes in
`crates/platforms/epic/domain/src/library.rs` **[verified on Epic's answers: a library read whole, every game
named]**:

- `refresh` `:131`, per signed-in account and with its access token (`get` `:92`, HTTPS): its assets from the
  launcher service (`ASSETS_PATH` `domain/library.rs:26`, Windows, label Live), plus the library service's
  records (`records` `:111`, paged, at most `MAX_PAGES` 50) for apps the assets leave out (their builds unknown).
  Then each catalog item not known yet, asked once (`items_path`, `parse_items`): title, whether it is a game (a
  game or an application, not a DLC nor an engine asset), images.
- An account whose read fails keeps its last assets; the failure shows in the view. Read again every 6 h
  (`REFRESH_MS` `:29`) or when the accounts signed in changed (`due` `:235`; thread `epic-library`, looking every
  minute).
- Download directories: Epic files an app's downloads under another id than its name. `find_dirs` `:215` asks the
  launcher service for each app's manifests (`manifest_path` `domain/library.rs:122`) and files each manifest URL's
  directory under the app (`parse_manifest_dirs` `:134`), once per app per run.
- Kept as `epic-library-2` in the state directory (`KEPT` `:34`; `Saved`, `crates/platforms/epic/app/src/saved.rs`:
  assets, items, directories, read time), so names and art are there at once after a start.
- `app` `:268`: a game (app name or download directory) to its asset and an account that owns it. `GameNames`
  `:336`: an app's title; a download directory is grouped under its app.

## Manifests and chunks

`crates/platforms/epic/domain/src/manifest.rs` **[verified on Epic's manifests: a build's chunks listed and every
one fetched with the size and hash listed; the parsing by `crates/platforms/epic/domain/tests/manifest.rs`]**:

- `EpicPlanner::current` `crates/platforms/epic/app/src/prefill.rs:90`: the launcher service's answer about the
  app's current build (`EpicLibrary::build` `library.rs:280`, as an account that owns it) lists a manifest URL per
  CDN, with a query (a signature) where the CDN wants one (`parse_manifest_urls` `domain/library.rs:163`). The
  manifest is fetched over HTTPS (`fetch` `:112`) from the first CDN that gives one.
- `parse` `manifest.rs:85`: header (magic `0x44BEC00C`), data zlib-compressed when the header says so (at most
  256 MiB inflated), the meta skipped, then the chunk list column by column: GUID, rolling hash, SHA-1, group, file
  size. `Chunk::path` `:40`: `<ChunksV4|V3|V2|Chunks by feature level>/<group, 2 digits>/<hash, 16 hex>_<GUID, 32
  hex>.chunk` under the manifest's directory.
- `check_chunk` `:140`: a chunk file (magic `0xB1FE3AA2`, zlib when stored so) whose data's SHA-1 is the
  manifest's.
- `Current::chunk_keys` `prefill.rs:56`: the build's chunks as object keys and sizes (what builds, sizes and prune
  use).

## Builds

`EpicBuilds` `crates/platforms/epic/app/src/builds.rs:44` **[verified]**:

- The stashed apps: Epic's store directories mapped to their app through the library (`apps` `:81`).
- `refresh` `:95`: an app whose build is new (or whose chunks are not known) has its manifest read
  (`EpicPlanner::current`); an app no longer stashed leaves the view. `held`: bytes of the build's chunks the stash
  holds whole; current when all are (`current` `:175`, `status` `:181`).
- `keep_fresh` `:197`: looks every 30 s (`LOOK_MS` `:28`) at Epic's part of the stash and the library's builds
  (`mark` `:210`), refreshing when they move and at least every 10 min (`EVERY_MS` `:30`). `builds_mark` `:76`.
- Test: `crates/platforms/epic/app/tests/builds.rs`.

## Sizes

`EpicSizes` `crates/platforms/epic/app/src/sizes.rs:28` **[verified]**: a game's download size is the sum of its
current manifest's chunk sizes. Worked out on a thread of its own (`keep_going` `:90`), one game at a time
(`one` `:68`), 5 s apart, looking every minute once all are known. Kept as `epic-sizes` (`KEPT` `:18`, a line
`app<TAB>build<TAB>bytes`; a size that could not be worked out is not kept). `size` `:63` counts only for the
current build. The catalog view draws it (`crates/server/src/run/epic.rs:96`).

## Prefill

`EpicPlanner` `crates/platforms/epic/app/src/prefill.rs:67` (`admit` `:128`, `work` `:135`), run by the core's
`Prefiller` ([battlenet.md](battlenet.md#prefill)) **[verified: `crates/platforms/epic/app/tests/prefill.rs`]**:

- `admit`: the game is in the library with a build. `work`: no pre-release; the current manifest; one job per
  chunk at `<dir><Chunk::path>`; the hosts are the CDNs the manifest URLs name.
- `check_cdn: true` with `with_checked_cdn` (`crates/server/src/run/epic.rs:80`): each chunk is fetched over plain
  HTTP from the CDN straight into the stash and committed only once its SHA-1 is the manifest's (`ChunkCheck`
  `:24` holds the chunk, at most `MAX_CHUNK` 64 MiB, then `check_chunk`); a failure is asked once more, a second
  ends the job ([battlenet.md](battlenet.md#checked-from-the-cdn)). Filed under the directory the cache files the
  launcher's downloads under (`profile` `crates/platforms/epic/domain/src/profile.rs`: `/Builds/Org/<org>/<app>/...`
  or `/Builds/<app>/...` **[hypothesis, from URL samples]**).

## Prune

`EpicPruner` `crates/platforms/epic/app/src/prune.rs:18` (`find` `:48`, `dirs` `:59`) over the core's segment rules
(`crates/app/src/prune_segments.rs`, as Steam's: [steam.md](steam.md#prune)) **[verified]**: the current build's
chunk keys are needed; only the directories the library files under the game are looked at; a manifest that cannot
be read makes nothing prunable. `dirs` is also the game's `parts`. Hourly on its own (`auto_prune`
`crates/server/src/run/epic.rs:101`, thread `epic-prune`): the first game whose current build is stashed whole,
unless a deletion runs or it is being prefilled. Test: `crates/platforms/epic/app/tests/prune.rs`.

## Windows setup file

`windows_cmd` `crates/platforms/epic/domain/src/setup.rs:279`, served as `Epic Games (lanstash).cmd` (`FILE_NAME`
`:131`) **[verified: `crates/platforms/epic/domain/tests/setup.rs`]**.

- Why: the launcher downloads over HTTPS, which lanstash only passes through and cannot keep. Four settings
  (`HTTP_SETTINGS` `:113`: `[Launcher] ForceNonSslCdn=true`, `[HTTP] ForceNonSslCdn=true`, `[HTTP]
  bUseSslCdn=false`, `[Portal.BuildPatch] bForceNonSSL=true`) in the settings file Epic's servers deliver
  (`EMS_ENGINE_INI` `:121`, under `C:\ProgramData\Epic\EpicGamesLauncher\Data\EMS\...\Engine.ini`) make it download
  over plain HTTP. The launcher reads that file at its start and then replaces it, so the lines go back in before
  every start: the user starts Epic with this file in place of Epic's own shortcut.
- Each start: (1) as the user, copies itself to `%LOCALAPPDATA%\lanstash\epic-games.cmd` and makes a desktop and a
  Start menu shortcut to it (`shortcut_script` `:245`), when started from elsewhere; (2) closes the launcher if it
  runs (up to 20 s); (3) adds the lines (`edit_script` `:186`: each key set in its section, else added; line ends,
  final line end and BOM kept; checked after writing), as the user, else elevated once Windows asks (exit codes
  `EXIT_*` `:171`); (4) starts the launcher by its `com.epicgames.launcher://` link. Without the file yet (first
  start) or when the change is declined, Epic starts as usual and says so.
- Nothing in it is typed in: every word, path and label is a `Word` `:37`, `WinPath` `:69` or `Label` `:136` whose
  `const` constructor refuses characters `cmd` or PowerShell would read as more than text (a bad constant does not
  compile). The PowerShell parts travel as `-EncodedCommand` (UTF-16LE base64, `encoded` `:272`). CRLF line ends,
  ASCII.

## Art

`ArtSource for EpicLibrary` `crates/platforms/epic/app/src/library.rs:354` **[verified]**: `image`
`crates/platforms/epic/domain/src/library.rs:235` picks the item's first image of its kind (header and background:
`DieselGameBox`, `OfferImageWide`, `DieselStoreFrontWide`; logo: `DieselGameBoxLogo`; icon: `Thumbnail`,
`DieselGameBoxTall`); `image_source` `:247` accepts only hosts under `.epicgames.com`. Epic's image CDN answers
only over HTTPS (403 over HTTP), so `fetch` `:361` gets the image itself through the `Web` port, over TLS.
