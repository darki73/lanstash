# Map: Steam

Code references are `path:line`; line numbers may move, so search for the named item. Claims are marked **[verified]**
(read in the code or a test) or **[hypothesis]**. Three crates: `crates/platforms/steam/domain` (pure, `no_std` +
`alloc`: the CM protocol, PICS app info, manifests, the views), `.../app` (use cases over ports) and
`crates/adapters/steam` (the CM link: WebSocket over TLS, `rustls` with Mozilla's roots, and `ring` randomness).
Wired by `platform` `crates/server/src/run/steam.rs:128` (registered in `crates/server/src/run/serve.rs:113`); the
CLI `lanstash steam login|status|logout` is `steam` `:75`. The `Platform` is `SteamPlatform`
`crates/platforms/steam/domain/src/platform.rs:26` (id `steam`, `:22`), then wrapped in `KeptBuilds`
([battlenet.md](battlenet.md#kept-builds)).

## The platform

`SteamPlatform` answers `names` (the library), `art`, `builds`, `build_status`, `builds_mark`, `catalog`,
`prefill`, `prune`, `parts`, `forget`, `account` and `nginx` (the inspection), each given by the root as a closure
or an `Arc` (`with_*` `:76` to `:153`) **[verified]**. No `game` view and no setup file.

| Path | Document |
|---|---|
| `GET /api/v1/builds/steam` | Battle.net's shape: `{"region": <os>, "lancache", "games": [{"game": <app id>, "product": null, "name", "version": <build id>, "build_config": null, "have", "want", "plan_error", "current", "prerelease": null, "error": null, "lancache_share"}]}` (`render` `crates/platforms/steam/domain/src/builds.rs:49`) |
| `GET /api/v1/catalog/steam` | `{"products": [{"code", "name", "game", "version", "known": true, "download"?, "lancache"?, "owners"?}]}`, games only, by name (`render` `crates/platforms/steam/domain/src/catalog.rs:34`); before a library: `{"products": [], "error"}` (`render_none` `:65`) |
| `GET /api/v1/games/steam/<app>/nginx` | the lancache inspection ([Lancache probe and inspect](#lancache-probe-and-inspect)) |
| `GET`/`POST /api/v1/games/steam/<app>/prune` | what a prune deletes / delete it ([Prune](#prune)) |
| `/api/v1/account/steam[/qr.svg, /sign-in, /sign-out]` | the accounts ([Sign-in](#sign-in)) |

## Sign-in

Two ways, both ending with a refresh token kept in the secrets directory (`PrivateDir`, the `Secrets` port), never a
password **[verified]**:

- QR code (web UI and CLI): `QrLogin` `crates/platforms/steam/app/src/qr_login.rs:15` (`start` `:37`, `poll`
  `:56`, `interval_ms` `:46`): Steam hands out an `https://s.team/q/...` link, the mobile app scans and approves;
  polling follows the link as Steam rotates it. In the service, `SteamAccount`
  `crates/platforms/steam/app/src/account.rs:65` runs it on a thread (`steam-sign-in`, `sign_in` `:128`): up to
  `MAX_POLLS` 60 polls, a code whose connection Steam closed unscanned is replaced up to `MAX_FRESH` 3 times;
  starting again or cancelling moves to a new generation and the old thread stops at its next look. `qr_svg`
  `:222` draws the waiting code.
- Account name and password (CLI only, `lanstash steam login --password`): `login`
  `crates/platforms/steam/app/src/login.rs:124`: the password RSA-encrypted with Steam's key
  (`GetPasswordRSAPublicKey`), then a Steam Guard code (device or email, `CODE_TRIES` 3) or an approval in the
  app, then polling for the tokens (`POLLS` 10).
- Kept tokens: `steam-token-<name>` (lower case, `_` as `-`, else hex; `token_name` `:29`), the list of accounts in
  `steam-accounts` in sign-in order (`accounts` `:44`, which also moves an older single-account layout,
  `steam-refresh-token`/`steam-account`, into the list). `keep` `:207` adds or renews, `forget` `:188` removes one.
- Account view (`view` `account.rs:174`): `{"accounts": [{"name", "refused"}], "account", "sign_in": {"state",
  "scanned", "error"}, "logons"}`. Sign-out (`sign_out` `:216`) forgets the token and drops that account's session.
- Tests: `crates/platforms/steam/app/tests/{login,qr_login,account}.rs`.

## Sessions

`Services` `crates/platforms/steam/app/src/session.rs:38`: the ports (CM `Dialer`, `Secrets`, `Upstream` for the
content servers), the kept chunk lists, and one session per signed-in account **[verified]**:

- Steam lets an account have one session (a second logon replaces the first, EResult 34). So every use goes through
  `with_session_as` `:169`: the account's one logged-on connection, held under a lock while in use, opened when
  missing. When the connection broke under the work, the session is dropped and the work tried once more; when
  Steam only refused the work (no manifest, access denied) the error is returned and the session kept.
  `with_session` `:199` takes the first account signed in. Logons are counted (`logons` `:209`).
- Heartbeat: `keep_alive` `:242` (thread `steam-heartbeat`, every 9 s, `HEARTBEAT_MS`
  `crates/server/src/run/steam.rs:57`) calls `beat` `:216`, which `try_lock`s each slot: a session in use is
  skipped, not waited for; one whose heartbeat fails is dropped (test
  `a_session_in_use_is_skipped_by_the_heartbeat_not_waited_for`).
- Refused tokens: `logon` `crates/platforms/steam/app/src/logon.rs:26` says "Steam refused the kept token" only
  when `refuses_token` `crates/platforms/steam/domain/src/cm/eresult.rs:21` holds (InvalidPassword, AccessDenied,
  Revoked, Expired, AccountLogonDenied, InvalidLoginAuthCode, RateLimitExceeded, TwoFactorCodeMismatch); any other
  failure (busy, try another CM, no connection) passes. `session` `:134` then remembers that token for the account,
  and `resting` `:117` refuses to try it again for `RETRY_REFUSED_MS` `:55` (30 min) unless a new sign-in replaced
  the token (tests `a_refused_token_rests_until_a_new_sign_in_or_half_an_hour`,
  `a_passing_logon_failure_does_not_rest_the_token`, `only_what_says_the_token_will_not_do_refuses_it`).
- The CM itself: `Connection` `crates/platforms/steam/app/src/connection.rs:22` (messages out and in, Multi unpacked,
  service calls matched by job id, the license list kept after logon); the protocol in
  `crates/platforms/steam/domain/src/cm/` (`proto`, `message`, `logon`, `auth`, `content`, `pics`, `servers`).

## Library and app info

`SteamLibrary` `crates/platforms/steam/app/src/library.rs:193` keeps the latest `Library` `:28` **[verified]**:

- Read per account (`read_raw` `crates/platforms/steam/app/src/read.rs:43`, on that account's session): its
  licenses, the packages' info (binary KeyValues: which apps and depots each grants), then each app's access token
  and info (text KeyValues), in batches. `from_raws` `:52` merges the accounts in sign-in order: each app once, and
  who owns each app (`app_owners`) and depot (`depot_owners`).
- `refresh` `:261`: an account whose read fails keeps its last answers (the failure is kept for the view); one no
  longer signed in is dropped. `keep_fresh` `:321` (thread `steam-library`) looks every minute and reads again
  when the library is 6 h old (`REFRESH_MS` `:188`) or the accounts signed in changed (`due` `:301`).
- Kept between runs as `steam-library` in the state directory (`KEPT` `:206`, `with_keep` `:231`; blob `LSL2`,
  an older `LSL1` read as the first account's: `crates/platforms/steam/app/src/raw.rs`), so names are there at once
  after a start ([operations.md](operations.md#steam-after-a-restart)).
- App info (`app` `crates/platforms/steam/domain/src/apps.rs:162`): name, type, public build, depots with their
  public manifest gid, sizes, `oslist`, language, architecture, `depotfromapp`, DLC; art file names (`Assets` `:61`).
- `Library::install` `:118`: an install for the configured `Target` (`steam_os`, `steam_language`, `steam_arch`;
  `Target` `apps.rs:90`) is the app's own depots, the depots of DLCs the accounts own (listed with the game or with
  the DLC), and the shared depots (fetched as their provider), each once, and only depots the licenses grant
  (`grants` `:139`: a package lists the depot, or a granted app has its id) **[verified: `tests/library.rs`]**.
- Names (`GameNames` `:334`): a depot or app is its app's name; a depot is grouped under its app exactly (a DLC's
  own depot under its game).

### Which account reads a depot

`account_for` `library.rs:157`: the first signed-in account whose licenses grant the depot, else one that owns the
app, else the first one. `by_account` `:169` groups an install's depots by that account, in sign-in order; every
manifest read (builds, prefill, probe, inspect, prune) goes group by group on that account's session
(`install_chunks` `crates/platforms/steam/app/src/manifests.rs:84`; the prune groups the same way with
`account_for`) **[verified]**.

## Manifests and kept chunk lists

`crates/platforms/steam/app/src/manifests.rs` **[verified]**:

- `servers` `:27`: `GetServersForSteamPipe` for the session's cell (20 asked), those for every app only, plain HTTP
  first. `Servers` `:110` asks at most once per run, and only when a manifest must be downloaded.
- `fetch` `:47`: a request code (`GetManifestRequestCode`, branch `public`), then
  `/depot/<depot>/manifest/<gid>/5/<code>` from up to 3 servers through the `Upstream` port, as Steam's own user
  agent, at most `MAX_MANIFEST` 512 MiB (`crates/platforms/steam/domain/src/manifest.rs:24`). The parsed manifest
  must be the depot and gid asked for, else that server's answer is an error ("got manifest ... not ...", `:65`):
  only then is it kept as that manifest's chunk list **[verified: `a_manifest_of_another_depot_or_build_is_not_taken_for_the_one_asked`]**.
- `chunks` `:134`: the kept list when it is that gid's, else the manifest downloaded and (with `keep`) its list
  kept. `parse` `manifest.rs:183` (ZIP of sections, protobuf payload); `Manifest::chunks` gives the unique chunks;
  `chunk_path` `:207` is `/depot/<depot>/chunk/<sha1 hex>`.
- `ChunkLists` `crates/platforms/steam/app/src/chunk_lists.rs:70`: one blob per depot, `steam-depot-<depot>`
  (`name` `:20`): magic `LSD1`, the gid, the count, 32 bytes a chunk (SHA-1, CRC, size unpacked, size on the
  wire). A depot keeps only its latest gid's list (`kept` `:88`, `put` `:94`); `forget` `:101` drops it when the
  game is deleted (`with_forget`, `crates/server/src/run/steam.rs:254`). Losing a blob costs a download, never
  correctness. Tests: `crates/platforms/steam/app/tests/chunk_lists.rs`.

## Builds and sizes

`SteamBuilds` `crates/platforms/steam/app/src/builds.rs:51` **[verified]**:

- The stashed apps: Steam's store directories (depots) mapped to their app through the library, kept when the app
  installs something for the target (`stashed` `:155`).
- `refresh` `:171`: each app whose public build is new; those whose chunk lists are all kept first, without a Steam
  session (`kept_install` `:81`), then the others with one list of content servers (`install` `:91`).
- Held: bytes of the install's unique chunks the stash holds whole (`held` `:206`, `lookup_quietly`: no use
  counted). `want` is the sum of the chunks' sizes on the wire; current when `have == want`.
- `keep_fresh` `:292` (thread `steam-builds`): within 30 s of Steam's part of the stash or the library changing
  (`CHANGE_LOOK_MS` `:36`), at least every 5 min (`LOOK_MS` `:34`); `ready` is raised after the first whole
  refresh, which the lancache probe waits for. `builds_mark` `:132` moves with each app worked out.
- The view and status share one walk (`apps` `:248`, `OncePer` on the stash's changes, the mark and the library's
  read time), except while lancache is probed.
- Sizes in the catalog: an install's download from the app info, per depot `estimate` `apps.rs:125` (the download,
  never more than the size; a 0 download beside a real size counts the size), summed over `Library::install`
  (`crates/server/src/run/steam.rs:270`); no size is drawn when it is 0.

## Prefill

`SteamPlanner` `crates/platforms/steam/app/src/prefill.rs:25` (`admit` `:61`, `work` `:76`), run by the core's
`Prefiller` ([battlenet.md](battlenet.md#prefill)) **[verified]**:

- `admit`: signed in (else `NotSetUp`), an app id, and with a library the app must install something.
- `work`: no pre-release (public branch only). The install's depots by account (`by_account`), each depot's
  manifest read on that account's session; one job per unique chunk, asked of the cache as
  `lancache.steamcontent.com` (`LANCACHE_HOST` `crates/platforms/steam/domain/src/profile.rs:27`), so it is filed
  as a client's download. `planning` counts depots read.
- `check_cdn: false`: CDN chunks go through the cache, which checks `x-content-crc` as it stores them. From lancache
  (`with_lancache`, and `with_tails` for a chunk lancache holds all but the end of), a chunk counts once its
  CRC-32 matches the stored `x-content-crc` and its length the manifest's (`ChunkCheck`
  `crates/platforms/steam/domain/src/check.rs:12`); a chunk without the header is fetched from the CDN.

## Prune

`SteamPruner` `crates/platforms/steam/app/src/prune.rs:30` (`find` `:60`) over the core's segment rules
(`prunable`, `crates/app/src/prune_segments.rs:33`) **[verified]**:

- Needed: every chunk of every current manifest of the depots the accounts are granted of the app and of its DLCs
  with depots of their own, any OS or language (shared depots are left out); only those depots' directories are
  looked at.
- A segment goes when none of its chunks is needed and it was not used within the grace period (7 days); a segment
  mixing old and current chunks is compacted (current chunks copied out) once at most half is current; else it
  stays. A manifest that cannot be read makes nothing prunable.
- `prune_once` `:106`: hourly (thread `steam-prune`), the first app whose current build is stashed whole, unless a
  deletion runs or it is being prefilled. Tests: `crates/platforms/steam/app/tests/prune.rs`.

## Lancache probe and inspect

Only with `nginx_cache` set **[verified]**:

- Probe: `SteamLancache` `crates/platforms/steam/app/src/lancache.rs:46` (`pass` `:174`, `probe` `:155`): per
  offered app and build, the install's chunks (kept lists serve; the probe keeps none of its own), a sample of 256
  (`SAMPLE` `:29`) looked up by their first slice's cache file (`first_slice`
  `crates/platforms/steam/domain/src/share.rs:32`: key `steam/depot/<d>/chunk/<sha>` plus `bytes=0-1048575`), the
  found bytes scaled up (`estimate` `:39`). One app at a time on the shared sessions, 500 ms apart, a pass every
  10 min; waits for the builds' `ready` after a start. Results kept as `steam-lancache` (`KEPT` `:58`, a line
  `app build have want`). `share` `:141` counts only for the current build and an install of the same size. Shown
  as `lancache_share` in builds and `lancache` in the catalog: an estimate.
- Inspect: `LancacheInspect` `crates/platforms/steam/app/src/inspect.rs:116` (`inspect` `:146`): 64 chunks
  (`SAMPLE` `:24`) read the way an import reads them (`classify` `:62`), tallied by `Outcome` `:28` (`absent`,
  `unusable`, `error`, `no_crc`, `size`, `crc`, `ok`): `{"game", "sampled", "outcomes": {<outcome>: {"chunks",
  "bytes", "example"}}}` (`view` `:105`). A diagnostic: the probe only looks whether a first slice exists.

## Catalog and art

- Catalog: the library's apps of type Game with something to install for the target (`offered`
  `crates/platforms/steam/domain/src/catalog.rs:18`), with download size, lancache share and owners
  (`crates/server/src/run/steam.rs:261`) **[verified]**.
- Art: `SteamArt` `crates/platforms/steam/domain/src/art.rs:38` (`locate` `:69`), plain HTTP only (both CDNs answer it). Where the app info names a file (newer apps keep art under a hash), it is asked of
  `shared.akamai.steamstatic.com/store_item_assets/steam/apps/<app>/<name>`; else
  `cdn.akamai.steamstatic.com/steam/apps/<app>/{header.jpg, library_hero.jpg, logo.png}`. The icon is the 32x32
  `<sha1>.jpg` the app info names, among the community images; none named, no icon. A missing background falls back
  to the store page's `page_bg_generated_v6b.jpg` (`fallback` `:94`) **[verified]**.

## Setup file

None: `SteamPlatform` does not implement `setup`, so `GET /api/v1/setup/steam/<os>` answers 404 (the trait default,
`crates/ports/src/platform.rs:185`; `crates/adapters/admin-api/src/api/setup.rs`) **[verified]**. Steam clients
reach lanstash through DNS alone ([dns.md](dns.md)) **[hypothesis]**.
