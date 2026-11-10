# Map: configuration

Code references are `path:line`; line numbers may have moved, so search for the named item. Claims are marked **[verified]** or **[hypothesis]**.

A configuration file (`--config FILE`, `key = value` lines, `#` starts a comment) and command-line flags for the
same keys (`--fetch-ahead 8`; `-` and `_` are the same in a key). All options are passed into the components that
use them; there is no global configuration. **[verified]**

- Every key with its default: `KEYS` `crates/server/src/config.rs:13` (this table follows it; the code wins).
  An unknown key is an error, with its line number in a file. `store.<service>` is the only key with a variable
  part (service names: letters, digits, `_`, `.`).
- Parsing: `Settings` `:154` (`set` `:178`, `add_file` `:191`), then `build` `:250` into `Config` `:67`.
  Command line: `parse` `crates/server/src/cli.rs:146`, `config_args` `:185`. Tests
  `crates/server/tests/config.rs`, `crates/server/tests/cli.rs`.
- Read by the root only: `config` `crates/server/src/run/mod.rs:52`.
- Switches (a flag without a value): `access_log_headers`, `prefetch`. Booleans take `true|yes|on|1` and
  `false|no|off|0`.
- Sizes: a number with an optional `K`, `M`, `G`, `T` (powers of 1024; `size` `:379`).

## Precedence

A value counts in this order, later winning: the built-in default, the configuration file, a change made in the
web UI (kept in `<state>/settings`), a command-line flag (`config` `crates/server/src/run/mod.rs:52`). The settings
file is found through `state` as the file (or the default) sets it, before the flags. **[verified]**

## Keys

| Key | Default | Meaning and limits |
|---|---|---|
| `listen` | `0.0.0.0:80` | HTTP listen address |
| `listen_tls` | off | HTTPS pass-through listen address, e.g. `0.0.0.0:443` ([requests.md](requests.md#https-listen_tls)) |
| `admin_listen` | off | admin API and status page ([operations.md](operations.md#admin-api)); `lanstash stop`, `prefill` and `install` need it |
| `admin_listen_tls` | off | the admin API and web UI over HTTPS (e.g. `0.0.0.0:8443`), with a self-signed ECDSA P-256 certificate made once and kept in `secrets` (made again when its names change); with it, `admin_listen` answers this machine and sends every other request there (308) |
| `admin_tls_names` | this machine's name, `localhost` (and the HTTPS address when it is one) | the names and addresses the certificate is for, comma-separated |
| `web_ui` | `/usr/local/share/lanstash/web` | the built web UI served on `admin_listen` (`install --web web/dist` puts it there); none there: the built-in page ([operations.md](operations.md#web-ui)) |
| `store` | (required) | default root; further `,`-separated entries are other mounts of the same directory, used for reads |
| `store.<service>` | none | root(s) for one service, same form |
| `upstream_dns` | none | DNS server(s) for origin names, `IP` or `IP:port` (port 53), comma-separated; required to serve and to install (not the local DNS, which points at the cache) |
| `client_names` | `system` | where a client's name is asked (a reverse DNS query): `system` (the servers in `/etc/resolv.conf`, which know the LAN's names), DNS server(s) (`192.0.2.1`, comma-separated), or `off`. A name given in the web UI wins |
| `cache_domains` | none | directory with `<service>.txt` host lists |
| `services` | `steam,blizzard,epicgames,riot` | lists to load (lower-cased) |
| `names` | none | names file: `service/game = Name` lines; asked after the platforms' own names ([operations.md](operations.md#game-names)) |
| `blizzard_region` | `us` | Battle.net region whose current builds count: 2 or 3 lower-case letters (the UI offers `us`, `eu`, `kr`, `tw`, `cn`) ([battlenet.md](battlenet.md#builds)) |
| `blizzard_tags` | `Windows,x86_64,enUS` | download-manifest tags of a Battle.net install (platform, architecture, locale, optional content): 1 or more, each 1 to 32 letters, digits or `_`; the region's tag is added |
| `steam_os` | `windows` | the operating system a Steam install is for: `windows`, `linux` or `macos` (prefill, the builds view, the library and lancache estimates) |
| `steam_language` | `english` | the languages clients play in, as Steam names them, comma-separated, at most 8 (each 1 to 32 lower-case letters or `_`): an install takes each one's depots |
| `steam_arch` | `64` | `64` or `32` |
| `prefill_parallel` | `8` | prefill requests in flight at once, 1 to 64 ([battlenet.md](battlenet.md#prefill)) |
| `auto_update` | off | `always` (same as `00:00-24:00`), a daily window in UTC (`HH:MM-HH:MM`, may cross midnight; start and end differ), or `off`: when games once held whole are queued for their new builds ([operations.md](operations.md#automatic-updates)) |
| `nginx_cache` | off | lancache's nginx cache directory, read-only: the nginx views and prefill from lancache ([battlenet.md](battlenet.md#prefill-from-lancache)) |
| `check_updates` | `on` | whether lanstash asks GitHub for a newer release of itself: `GitHubReleases` `crates/adapters/http/src/github.rs` (`releases/latest`, `tag_name`) watched by `ReleaseWatch` `crates/app/src/releases.rs` (every 6 h, a failure again after 1 h, never while a caller waits), wired in `crates/server/src/run/releases.rs`; `off`: `NoReleases`, nothing asked ([operations.md](operations.md#admin-api)) |
| `secrets` | `/var/lib/lanstash/secrets` | private files (directory 0700, files 0600) for secrets: Steam refresh tokens, the Epic sign-ins (`epic-accounts`) and client (`epic-client`), lanstash's own accounts (`accounts`: name, role, PBKDF2 hash) |
| `state` | `/var/lib/lanstash/state` | private files kept between runs so work is not redone after a restart: `settings`, `lanstash.db` (history, counters, prefill runs), `lifetime`, `auto-update`, `verify`, `dns` (the DNS server's setup; its password is in `secrets`), `hidden` (the games kept private), `steam-depot-*`, `steam-library`, `epic-library-2`. Losing it costs time and history, never stored bytes |
| `access_log` | off | one line per request ([operations.md](operations.md#logs)) |
| `access_log_headers` | `false` | switch: also log all request headers |
| `event_log` | standard error | failed downloads, storage errors, eviction, recovery |
| `segment_size` | `1G` | segment size target |
| `object_file_min` | `16M` | whole objects at least this large go to object files |
| `fetch_piece` | `16M` | largest upstream request; at least `64K` |
| `fetch_ahead` | `4` | pieces started ahead of a client, 1 to 64 (`MAX_AHEAD` `:58`: each one a download thread a client can start) |
| `download_window` | `4M` | bytes of a download kept in memory; at least `64K` ([requests.md](requests.md#memory)) |
| `send_piece` | `1M` | read and send granularity towards clients where a stored body is not handed to the kernel whole (`sendfile`, Linux); at least `4K` |
| `prefetch` | `false` | switch: read the next segment record ahead on sequential access |
| `retries` | `3` | new downloads for one response after a download failed |
| `max_connections` | `4096` | client connections in all (one address holds at most a quarter, at least 64); also the HTTPS relays' limit |
| `stats_secs` | `0` | print counters every N seconds (`0`: never) |
| `max_size` | off | eviction: at most this many bytes stored (`2T`; `0` or empty: no limit) |
| `keep_free` | `5%` | eviction: keep this much of each root's file system free: `1%` to `99%`, or a size (`500G`); `0` or empty: no limit |
| `evict_secs` | `30` | seconds between eviction checks, more than 0 |
| `verify_every` | `off` | `weekly` (7 days), `monthly` (30 days), `<n>d` (1 to 365) or `off`: how often every stored byte is checked in the background ([operations.md](operations.md#store-check-in-the-background)) |
| `verify_rate` | `50M` | the most that check reads a second; at least `1M` |
| `log_max_size` | `100M` | `access_log` and `event_log` rotate before passing this (`0`: never) |
| `log_keep` | `5` | rotated log files kept (`log.1` ... `log.N`) |

All of the above **[verified]** against `KEYS` and `build`.

## Settings the web UI may change

`LIVE` `crates/server/src/settings.rs:108`; read and written by `LiveSettings` `:215` behind the `Tunables` port
(`crates/ports/src/settings.rs`); served at `/api/v1/settings` ([operations.md](operations.md#settings-and-restart)).
Tests `crates/server/tests/settings.rs`. **[verified]**

| Key | Group | Choices offered | Typed value (`custom` `:31`) |
|---|---|---|---|
| `auto_update` | Updates | `always`, `off` | window `HH:MM-HH:MM, UTC` |
| `prefill_parallel` | Updates | `4`, `8`, `16`, `32` | other, 1 to 64 |
| `check_updates` | Updates | `on`, `off` | |
| `steam_os` | Steam | `windows`, `linux`, `macos` | |
| `steam_language` | Steam | Steam's 30 language names, several at once (`multi`) | |
| `steam_arch` | Steam | `64`, `32` | |
| `blizzard_region` | Battle.net | `us`, `eu`, `kr`, `tw`, `cn` | |
| `blizzard_tags` | Battle.net | in parts (`parts`): platform `Windows`/`OSX`, bitness `x86_64`/`x86_32`/`arm64`, one of 15 languages | |
| `max_size` | Storage | free text (`8000G`, `2T`; empty: no limit) | |
| `keep_free` | Storage | free text (`30%`, `500G`) | |
| `verify_every` | Storage | `off`, `weekly`, `monthly` | every `<n>d`, 1 to 365 |
| `verify_rate` | Storage | `20M`, `50M`, `100M` | other, `1M` or more |

- A change is checked by building the whole configuration with it (file, changes, flags) and refused with the
  reason when that fails; it is kept as `key = value` lines in `<state>/settings` (written to `settings.tmp`,
  then renamed) and applies at the next start (the UI's Restart).
- A key outside `LIVE` is refused ("set in the configuration file only"), as is a key a command-line flag sets.
  Lines of other keys in the settings file are ignored when it is read (`read` `:192`).
- The view per setting: key, group, label, help, choices, `multi`, `parts`, `custom`, the value in force, the
  file's, the built-in default, the UI's change, and whether a flag sets it (`view` `:269`).
