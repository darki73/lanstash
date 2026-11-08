# Command line

Everything lanstash does starts from one binary, `lanstash`, with a command and its options. This page lists every
command, grouped by what it is for. `lanstash help` prints the same list.

## Configuration options

Most commands take the configuration the same way:

```text
--config FILE      a configuration file
--KEY VALUE        any configuration key, as a flag (later wins)
```

A flag wins over the file. In a key, `-` and `_` are the same (`--admin-listen` is `admin_listen`). Every key is in
the [configuration reference](configuration.md). The commands that take them are marked *(config)* below.

Sizes accept `K`, `M`, `G` and `T` suffixes (powers of 1024).

Exit codes: `0` ok, `1` problems found, `2` usage error (the usage is printed), `3` failure.

## Serving

### serve

```text
lanstash serve [--config FILE] [--KEY VALUE]...
```

Serves and caches. Under the installed service, systemd runs this for you. Common flags:

| Flag | What |
|---|---|
| `--store DIR[,DIR...]` | the store directory (extra entries: other mounts of the same directory, for reads) |
| `--store.<service> DIR` | a directory for one service |
| `--upstream-dns IP[,IP]` | the DNS for the real CDN names (not the LAN's DNS, which points at the cache) |
| `--cache-domains DIR` | the directory with the `<service>.txt` host lists |
| `--listen ADDR` | where clients connect; default `0.0.0.0:80` |
| `--access-log FILE`, `--access-log-headers`, `--event-log FILE`, `--stats-secs N` | logs and counters |

### stop

```text
lanstash stop [--wait SECS] [--config FILE] [--KEY VALUE]...
```

Stops the running lanstash cleanly, through `admin_listen`, from the same machine. It waits until the admin port
closes, 60 seconds by default.

## Store

### store stats

```text
lanstash store stats [--config FILE] [--KEY VALUE]...
```

Objects and bytes per service and game. Read-only, safe while serving.

### store verify

```text
lanstash store verify [--threads N] [--rate MB/S] [--accept] [--config FILE] [--KEY VALUE]...
```

Reads every stored byte and checks it against its checksum. Read-only and safe while serving. It goes as fast as
the disks allow, or at most `--rate` MB a second. It says how much it will read and asks first; `--accept` starts
without asking (for systemd and scripts). 4 threads by default. The `verify_every` setting does the same check
slowly in the background.

### import

```text
lanstash import --nginx-cache DIR --dirs LIST [--threads N] [--slice SIZE] [--config FILE] [--KEY VALUE]...
```

Imports an existing nginx or lancache cache into the stash, reading it only. `--dirs` names the cache's level
directories: `00,1a`, a range such as `30-3f`, or `00-ff` for all. 16 threads and 1 MiB slices by default.

### seg dump, seg verify

```text
lanstash seg dump <file> [--verify] [--limit N]
lanstash seg verify <file>
```

Tools for one segment file: `dump` shows its header, records and problems; `verify` checks every record's body.

## Prefill

### prefill

```text
lanstash prefill <platform>/<game> [--pre] [--from lancache|cdn] [--detach] [--config FILE] [--KEY VALUE]...
```

Prefills a game's current build in the running lanstash and follows its progress. `--pre` takes the pre-release
build instead. `--from lancache` takes what lancache's old cache (`nginx_cache`) holds, checked, and the rest from
the CDN. `--detach` starts it without following. Ctrl-C stops following, not the prefill. It exits with 1 unless
the prefill ends done.

```sh
lanstash prefill steam/<app id>
```

### prefill stop

```text
lanstash prefill stop [--config FILE] [--KEY VALUE]...
```

Stops the running prefill.

## Accounts and users

### user

```text
sudo lanstash user add <name> [--admin]
sudo lanstash user passwd <name>
sudo lanstash user role <name> admin|viewer
sudo lanstash user remove <name>
sudo lanstash user list
```

lanstash's own accounts for the web UI and the API, kept in `secrets`. Passwords are asked on the terminal and kept
only as hashes. `add` makes a viewer unless `--admin` is given. The last admin can be neither removed nor made a
viewer. These take `--config FILE` and `--KEY VALUE` too; the default configuration is
`/etc/lanstash/lanstash.conf`.

```sh
sudo lanstash user add admin --admin
sudo lanstash user add player.one
```

## Steam

### steam

```text
lanstash steam login [--password] [--config FILE] [--KEY VALUE]...
lanstash steam status [--config FILE] [--KEY VALUE]...
lanstash steam logout [--config FILE] [--KEY VALUE]...
```

- `login` signs in to Steam once by scanning a QR code with the Steam mobile app. With `--password` it asks for
  the account, password and Steam Guard code instead. Only the refresh token is kept, in `secrets`; no password is
  kept. The web UI signs in the same way.
- `status` logs on with the kept token.
- `logout` forgets it.

The default configuration is `/etc/lanstash/lanstash.conf`.

## Install and service

### install

```text
sudo lanstash install [--config FILE] [--KEY VALUE]... [--no-start] [--force] [--web DIR]
```

Installs, or upgrades, lanstash as the systemd service `lanstash`: the binary in `/usr/local/bin`, the
configuration in `/etc/lanstash/lanstash.conf` (from `--config` and the flags), started now and at every boot.
`--web DIR` puts the built web UI where `web_ui` says, served on `admin_listen`. `--no-start` stops before starting
the service; `--force` installs even when a listen port is taken. See [Install](../getting-started/install.md).

### uninstall

```text
sudo lanstash uninstall [--purge]
```

Stops and removes the service and the binary. `--purge` also removes `/etc/lanstash`. Cached data is kept.

## DNS and doctor

### doctor

```text
lanstash doctor [--config FILE] [--KEY VALUE]...
```

Checks the installation, configuration, store, ports and DNS, without changing anything. The default configuration
is `/etc/lanstash/lanstash.conf`. One line per check (`ok`, `WARN`, `FAIL`); exit code 1 when any check fails.

The network's DNS server is set up on the web UI's DNS page, not on the command line: see
[DNS](../getting-started/dns.md).

## Help and version

```text
lanstash help
lanstash version
```

`help` (also `--help`, `-h`) prints the usage; `version` (also `--version`) prints the version.
