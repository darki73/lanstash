# Known limits

What lanstash does not do yet, or does only in part. Each is known; none is a surprise waiting for you.

## Platforms

| Platform | Cached | Prefill | Builds tracked | Notes |
| --- | --- | --- | --- | --- |
| Steam | yes | yes | yes | Prefill, library and builds need a Steam account signed in (QR code). |
| Battle.net | yes | yes | yes | |
| Epic Games | yes | yes | yes | Each PC needs a one-time setup: the launcher downloads over HTTPS by default. |
| Riot | yes | no | no | Not tried with a real Riot client yet. |

- **Phones are not cached.** Mobile games fetch from other CDNs over HTTPS, which a caching proxy cannot read.
- **Another platform's install** (a Mac's, a Linux PC's, another language's) can be prefilled for Battle.net and
  Steam, not for Epic Games.

## Systems

- `lanstash install` sets lanstash up as a service on **Linux with systemd**. Elsewhere on Linux, run
  `lanstash serve` under the service manager you have.
- **Windows** builds and runs as best effort; **macOS** is untested.
- Release archives are built on Ubuntu 24.04: they need glibc 2.39 or newer. Build from source for older systems.

## Web UI

- **Plain HTTP by default.** Sign-ins and their session cookie cross the LAN unencrypted unless
  `admin_listen_tls` is set; lanstash then makes a self-signed certificate for it.
- **Languages:** English and Russian. Numbers and sizes are written the English way in both.

## Setup

- **Host lists are not shipped.** lanstash reads the cache-domains lists (`<service>.txt`) from the directory
  `cache_domains` names; fetch them from the cache-domains project.
