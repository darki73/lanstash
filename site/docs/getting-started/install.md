# Install

This page puts lanstash on a Linux machine as a systemd service, with its web UI. You can install from a release
archive (the quickest way) or from a build of your own. Upgrading is the same command again, and `lanstash doctor`
tells you whether everything is in place.

## Requirements

- **Linux with systemd.** `lanstash install` sets lanstash up as the systemd service `lanstash`. On a Linux
  without systemd, run `lanstash serve` under the service manager you have. Windows builds and runs as best
  effort; macOS is untested.
- **Storage space.** Games are big. Give lanstash a directory on a disk (local or network storage) with room
  for the games your LAN plays. Eviction keeps some of it free (`keep_free`, 5% by default).
- **Free ports.** lanstash serves clients on port 80 (`listen`). The install refuses when a port it needs is
  already taken, for example by a lancache still running on the same machine (see `--force` below).
- **A DNS server on the LAN** that you can add rewrites to, so your PCs ask lanstash for game downloads. See
  [DNS](dns.md).
- **The cache-domains host lists**, which tell lanstash which host names belong to which platform. They are not
  shipped with lanstash; [Configure](configure.md) shows how to fetch them.

## From a release archive

A release is one archive, `lanstash-<version>-linux-x86_64.tar.gz`, with the binary, the built web UI, the
licence and the README, plus a `.sha256` file to check it with.

1. Download both files into the same directory and check the archive:

    ```sh
    sha256sum -c lanstash-<version>-linux-x86_64.tar.gz.sha256
    ```

2. Unpack it and go into its directory:

    ```sh
    tar -xzf lanstash-<version>-linux-x86_64.tar.gz
    cd lanstash-<version>-linux-x86_64
    ```

3. Write a configuration file, `lanstash.conf` (see [Configure](configure.md)), then install:

    ```sh
    sudo ./lanstash install --config lanstash.conf --web web
    ```

`--web web` copies the built web UI to where `web_ui` says (`/usr/local/share/lanstash/web` by default), and
lanstash serves it on `admin_listen`.

## What the install does

- Copies the binary to `/usr/local/bin/lanstash` (a running copy keeps its file until it restarts).
- Writes the configuration to `/etc/lanstash/lanstash.conf`, built from `--config` and any `--KEY VALUE` flags.
  When it differs from the one there, the previous file is kept as `lanstash.conf.bak`.
- Adds `admin_listen = 127.0.0.1:8081` when nothing sets `admin_listen`, so the commands that talk to the running
  lanstash work. Set it yourself to reach the web UI from other PCs.
- Checks that the configuration parses, names `upstream_dns` and uses absolute store paths.
- Writes the service `/etc/systemd/system/lanstash.service`, then enables and starts it (it starts at every boot
  too) and waits up to 60 seconds for the admin port.

Options:

| Option | What it does |
|---|---|
| `--config FILE` | the configuration to install (without it, the installed one is used) |
| `--KEY VALUE` | any configuration key, added to the installed file |
| `--web DIR` | the built web UI to install |
| `--no-start` | install everything, but do not start the service |
| `--force` | install even when a listen port is taken and the service is not running |

The service's output goes to the journal:

```sh
journalctl -u lanstash
```

## From source

You need Rust 1.95 or newer for the server and Node.js with pnpm for the web UI.

```sh
cargo build --release            # makes target/release/lanstash
cd web
pnpm install && pnpm build       # makes web/dist
cd ..
sudo target/release/lanstash install --config lanstash.conf --web web/dist
```

## Upgrading

Run the install again with the new binary. It replaces the binary and restarts the running service. Without
`--config` it keeps the configuration already installed:

```sh
sudo ./lanstash install --web web
```

Cached data is never touched by an upgrade.

## Uninstalling

```sh
sudo lanstash uninstall            # stop and remove the service and the binary
sudo lanstash uninstall --purge    # the same, and remove /etc/lanstash too
```

The service is stopped cleanly first. Cached data is always kept; the command names the store directories so you
can delete them yourself if you want the space back.

## Checking the installation

```sh
lanstash doctor
```

`doctor` reads `/etc/lanstash/lanstash.conf` by default and checks, without changing anything: the version, the
service state, the configuration, `admin_listen`, each listen port, the admin API, open files against the limit,
each store directory (writable, free space against `keep_free`), the store's contents, the upstream DNS, the
cache-domains lists and the log directories. It prints one line per check (`ok`, `WARN` or `FAIL`) and exits with
code 1 when any check fails.

Next: [Configure](configure.md).
