# Configuration

Every configuration key lanstash knows, grouped by topic, with its default and what it does. Keys marked
**Settings** can also be changed in the web UI's Settings page. For a short file to start with, see
[Configure](../getting-started/configure.md).

## How values are given

- **The file** (`--config FILE`): `key = value` lines; `#` starts a comment. An unknown key is an error, with its
  line number.
- **Flags**: every key works as a command-line flag, `--KEY VALUE` (`--fetch-ahead 8`). In a key, `-` and `_` are
  the same.
- **Switches** (`access_log_headers`, `prefetch`) are flags without a value. Booleans take `true`, `yes`, `on`, `1`
  and `false`, `no`, `off`, `0`.
- **Sizes** are a number with an optional `K`, `M`, `G` or `T` (powers of 1024).

A value counts in this order, later winning:

1. the built-in default,
2. the configuration file,
3. a change made in the web UI's Settings page (kept in `<state>/settings`),
4. a command-line flag.

A key set by a flag cannot be changed in the web UI, and the web UI refuses keys it does not offer ("set in the
configuration file only"). A change made there is checked against the whole configuration first and applies when
lanstash restarts.

## Network and web UI

| Key | Default | Meaning |
|---|---|---|
| `listen` | `0.0.0.0:80` | where clients' HTTP downloads arrive |
| `listen_tls` | off | an HTTPS address (such as `0.0.0.0:443`) for pass-through: hosts on the lists asked for over HTTPS are relayed, not cached |
| `admin_listen` | off | the web UI and the admin API. `lanstash stop`, `prefill` and `install` need it; `install` sets `127.0.0.1:8081` when nothing sets it |
| `admin_listen_tls` | off | the web UI and the admin API over HTTPS (such as `0.0.0.0:8443`), with a self-signed certificate made once and kept in `secrets`. With it, `admin_listen` answers this machine and sends everyone else to HTTPS |
| `admin_tls_names` | this machine's name, `localhost` | the names and addresses the certificate is for, comma-separated |
| `web_ui` | `/usr/local/share/lanstash/web` | the built web UI served on `admin_listen` (`install --web` puts it there); with none there, a built-in status page |
| `upstream_dns` | none (required) | DNS server(s) for the real CDN names, `IP` or `IP:port`, comma-separated. Not the LAN's DNS, which points at the cache |
| `client_names` | `system` | where a client's name is looked up (reverse DNS): `system` (the servers in `/etc/resolv.conf`), DNS server address(es), or `off`. A name given in the web UI wins |
| `max_connections` | `4096` | client connections in all (one address may hold at most a quarter, at least 64); also the HTTPS relays' limit |

## Storage

| Key | Default | Meaning |
|---|---|---|
| `store` | none (required) | the store directory. Further `,`-separated entries are other mounts of the same directory, used for reads |
| `store.<service>` | none | directory (or directories, same form) for one service |
| `secrets` | `/var/lib/lanstash/secrets` | private files: Steam sign-ins, Epic sign-ins and client, lanstash's own accounts (as hashes), sessions, invites, the DNS server's password |
| `state` | `/var/lib/lanstash/state` | files kept between runs so work is not redone after a restart: Settings page changes, history and counters, the DNS setup, hidden games and more. Losing it costs time and history, never stored bytes |
| `segment_size` | `1G` | the size a segment file aims for |
| `object_file_min` | `16M` | whole objects at least this large go to object files of their own |

## Host lists and names

| Key | Default | Meaning |
|---|---|---|
| `cache_domains` | none | the directory with the `<service>.txt` host lists (the cache-domains lists) |
| `services` | `steam,blizzard,epicgames,riot` | the lists to load |
| `names` | none | a names file of `service/game = Name` lines, asked after the platforms' own names |

## Battle.net

| Key | Default | Meaning | Settings |
|---|---|---|---|
| `blizzard_region` | `us` | the region whose current builds count: 2 or 3 lower-case letters (the web UI offers `us`, `eu`, `kr`, `tw`, `cn`) | yes |
| `blizzard_tags` | `Windows,x86_64,enUS` | the download tags of an install (platform, architecture, locale, optional content): 1 or more, each 1 to 32 letters, digits or `_`; the region's tag is added | yes |

## Steam

| Key | Default | Meaning | Settings |
|---|---|---|---|
| `steam_os` | `windows` | the operating system a Steam install is for: `windows`, `linux` or `macos` (prefill, builds, library, size estimates) | yes |
| `steam_language` | `english` | the languages clients play in, as Steam names them, comma-separated, at most 8: an install takes each one's files | yes |
| `steam_arch` | `64` | `64` or `32` | yes |

## Prefill and updates

| Key | Default | Meaning | Settings |
|---|---|---|---|
| `prefill_parallel` | `8` | prefill requests in flight at once, 1 to 64 | yes |
| `auto_update` | off | when games once held whole are queued for their new builds: `always`, a daily window in UTC (`HH:MM-HH:MM`, may cross midnight), or `off` | yes |
| `nginx_cache` | off | lancache's nginx cache directory, read only: lets you see what it holds of a game and prefill from it | |
| `check_updates` | `on` | lanstash asks GitHub every few hours whether a newer lanstash is released (`api.github.com`, the repository's latest release); when one is, the foot of every page and `lanstash doctor` say so. `off`: nothing is asked | yes |

## Downloads

These tune how lanstash fetches and sends. The defaults suit most setups.

| Key | Default | Meaning |
|---|---|---|
| `fetch_piece` | `16M` | the largest single request to the CDN; at least `64K` |
| `fetch_ahead` | `4` | pieces started ahead of a client, 1 to 64 |
| `download_window` | `4M` | bytes of a download kept in memory; at least `64K` |
| `send_piece` | `1M` | read and send size towards clients where a stored body is not handed to the kernel whole; at least `4K` |
| `prefetch` | `false` | switch: read the next segment record ahead on sequential access |
| `retries` | `3` | new downloads for one response after a download failed |

## Eviction and checks

| Key | Default | Meaning | Settings |
|---|---|---|---|
| `max_size` | off | the most the stash may hold (`2T`); `0` or empty: no limit | yes |
| `keep_free` | `5%` | keep this much of each store's file system free: `1%` to `99%`, or a size (`500G`); `0` or empty: no limit | yes |
| `evict_secs` | `30` | seconds between eviction checks, more than 0 | |
| `verify_every` | `off` | how often every stored byte is checked in the background: `weekly` (7 days), `monthly` (30 days), `<n>d` (1 to 365) or `off` | yes |
| `verify_rate` | `50M` | the most that background check reads a second; at least `1M` | yes |

When the stash passes `max_size` or a file system's free space drops under `keep_free`, the least recently used
files go first.

## Logs and counters

| Key | Default | Meaning |
|---|---|---|
| `access_log` | off | a file with one line per request |
| `access_log_headers` | `false` | switch: also log every request header |
| `event_log` | standard error | failed downloads, storage errors, eviction, recovery |
| `log_max_size` | `100M` | `access_log` and `event_log` rotate before they pass this (`0`: never) |
| `log_keep` | `5` | rotated log files kept |
| `stats_secs` | `0` | print the counters every N seconds (`0`: never) |

## Settings page summary

The Settings page offers these keys, in groups:

| Group | Keys | Choices offered |
|---|---|---|
| Updates | `auto_update`, `prefill_parallel`, `check_updates` | always, off or a window; 4, 8, 16, 32 or another number from 1 to 64; on, off |
| Steam | `steam_os`, `steam_language`, `steam_arch` | windows, linux, macos; Steam's languages, several at once; 64, 32 |
| Battle.net | `blizzard_region`, `blizzard_tags` | us, eu, kr, tw, cn; platform (Windows, OSX), bitness and language |
| Storage | `max_size`, `keep_free`, `verify_every`, `verify_rate` | free text sizes; off, weekly, monthly or every `<n>d`; 20M, 50M, 100M or another rate |

Each row shows where its value comes from (default, config file, changed here, or command-line flag). **Reset**
drops the web UI's change and goes back to the file's value. Changes apply after a restart (the page's
**Restart**), which cuts off running prefills and downloads.
