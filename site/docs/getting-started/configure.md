# Configure

lanstash reads one configuration file of `key = value` lines. This page gives a small file to start with and
explains each line. Every key, with its default, is in the [configuration reference](../reference/configuration.md).

## A file to start with

```ini
# Where the stash lives (an absolute path)
store = /srv/lanstash

# Where clients' downloads come in
listen = 0.0.0.0:80

# DNS for the real CDN names: not your LAN's DNS server
upstream_dns = 198.51.100.1

# The cache-domains host lists (steam.txt, blizzard.txt, ...)
cache_domains = /etc/lanstash/cache-domains

# The web UI and the API
admin_listen = 0.0.0.0:8081

# Battle.net: the region and what an install is
blizzard_region = eu
blizzard_tags = Windows,x86_64,enUS

# Steam: who an install is for
steam_os = windows
steam_language = english

# Keep games held whole up to date, at night (UTC)
auto_update = 02:00-06:00

# Eviction
keep_free = 10%
# max_size = 4T
```

A line starting with `#` is a comment. In a key, `-` and `_` are the same. An unknown key is an error, reported
with its line number.

## What each line does

### Store

`store` is the directory the stash is kept in. It must be an absolute path. Extra entries after a comma are other
mounts of the same directory, used for reads. `store.<service>` gives one platform a directory of its own.

### Listen

`listen` is the address and port clients' HTTP downloads arrive on. The default, `0.0.0.0:80`, is what clients
expect.

### Upstream DNS

`upstream_dns` is the DNS server lanstash asks for the real addresses of the CDN names. It is required.

!!! warning "Not your LAN's DNS server"
    Your LAN's DNS server points the game CDN names at lanstash. If lanstash asked it too, it would get its own
    address back. Use a public resolver or your provider's; several are allowed, comma-separated.

### Host lists

`cache_domains` is the directory with the host lists, one `<service>.txt` per platform (`steam.txt`,
`blizzard.txt`, `epicgames.txt`, `riot.txt`). They tell lanstash which host names belong to which platform.
lanstash does not ship them: fetch them from the uklans cache-domains project (MIT licence), the same lists
lancache uses, and copy the `.txt` files into this directory. `services` chooses which lists are loaded (all four
by default).

### Web UI

`admin_listen` is where the web UI and the admin API listen. If you leave it out, `lanstash install` adds
`127.0.0.1:8081`, which only the machine itself can reach. `0.0.0.0:8081` lets the other PCs on your LAN open the
web UI. `lanstash stop`, `prefill` and `install` need it.

!!! note
    The web UI is plain HTTP by default, so sign-ins cross the LAN unencrypted. `admin_listen_tls` turns on HTTPS
    with a self-signed certificate (see the [reference](../reference/configuration.md)).

### Battle.net

`blizzard_region` is the region whose Battle.net builds count (`us` by default; the web UI offers `us`, `eu`,
`kr`, `tw`, `cn`). `blizzard_tags` describes an install: platform, architecture, locale
(`Windows,x86_64,enUS` by default). Prefill and the builds view follow them.

### Steam

`steam_os` is the operating system a Steam install is for (`windows`, `linux` or `macos`). `steam_language` lists
the languages your PCs play in, as Steam names them (`english`; comma-separated, at most 8): an install takes each
one's files. `steam_arch` (`64` or `32`) is there too.

### Automatic updates

With `auto_update`, games the stash once held whole are queued for a prefill when a new build comes out. It takes
`always`, a daily window in UTC (`HH:MM-HH:MM`, which may cross midnight), or `off` (the default).

### Eviction

`keep_free` keeps this much of each store's file system free: `1%` to `99%`, or a size such as `500G` (5% by
default). `max_size` caps what the stash holds, such as `2T` (no cap by default). When either is reached,
lanstash deletes what was used least recently.

Sizes take `K`, `M`, `G` and `T` (powers of 1024).

## Flags override the file

Every key can be given on the command line too, as `--KEY VALUE`, and a flag wins over the file:

```sh
lanstash serve --config lanstash.conf --admin-listen 0.0.0.0:8081
```

With `lanstash install`, flags are written into the installed configuration file.

## Changing settings in the web UI

The Settings page changes some keys while lanstash runs: automatic updates, prefill requests at once, Steam's
operating system, languages and architecture, Battle.net's region and install tags, the storage limits and the
background check. A change is checked before it is kept, and applies when lanstash restarts (the page has a
Restart button).

A value counts in this order, later winning: the built-in default, the configuration file, a change made in the
web UI, a command-line flag. A key a flag sets cannot be changed in the web UI. Where lanstash listens and where it
stores stay in the configuration file.

See the [configuration reference](../reference/configuration.md) for every key. Next: [DNS](dns.md).
