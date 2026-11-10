# API

The web UI talks to lanstash through an HTTP API on `admin_listen`, and so can your own scripts. This page is a short
map of it: the main paths, what each is for, and who may use it. It does not list every field; the answers are JSON
and easy to look at with `curl`.

## Basics

- Every API path starts with `/api/v1/`. Answers are JSON.
- An error on an API path is `{"error": "...", "kind": "..."}`, with kinds such as `not-found`, `busy`, `invalid`,
  `sign-in` (401) and `admin-only` (403).
- Requests whose `Host` is not a local name or address, and changing requests a browser marks as coming from
  another site, are refused (403 `forbidden`). Tools such as `curl` pass.
- Passwords and sign-in codes travel in headers, never in the path.

```sh
curl http://192.0.2.10:8081/api/v1/version
```

## Who may do what

lanstash knows three kinds of caller:

- **This machine** (a request over loopback) holds every permission. That is why `lanstash stop`, `prefill` and
  `doctor` need no sign-in.
- **A signed-in account**, by its session cookie, holds its role's permissions as the accounts say now: an admin
  all of them, a player `prefill.own` (prefill the games its linked platform accounts own), a viewer none.
- **Anyone else** is a guest and holds none.

Reading is open to everyone, except the DNS server's state (needs `dns`), the hidden games list (needs `hide`), the
accounts and invites (need `accounts`), a platform's sign-ins (need `platforms`) and who the clients are (needs any
signed-in account; a guest sees clients as `client-1`, `client-2`, and so on). Anything that changes something
needs a permission. A guest asking for one gets 401 `sign-in`; an account without it gets 403 `admin-only`.

!!! warning "Nothing in front of the admin port on the same machine"
    Every request lanstash sees coming from loopback holds every permission. A reverse proxy or port forwarding
    on lanstash's own machine would make everyone it relays an admin. Run such a proxy on another machine.

## Session

| Method and path | What |
|---|---|
| `GET /api/v1/session` | who the caller is |
| `POST /api/v1/session` | sign in: name and password in the `X-Lanstash-User` and `X-Lanstash-Password` headers (percent-encoded) |
| `DELETE /api/v1/session` | sign out |

A sign-in sets the cookie `lanstash_session` (`HttpOnly`, `SameSite=Strict`), which lasts 30 days from its last
use. Wrong sign-ins make the address wait, from 1 second doubling up to 30. A changed password or a removed account
ends its sessions.

## Paths

| Path | What it is for | Needs |
|---|---|---|
| `GET /api/v1/version` | lanstash's version, and `latest` when a newer release is known (`check_updates`) | |
| `GET /api/v1/status` | one status snapshot: counters, downloads in flight, storage use, prefills, clients downloading now | |
| `GET /api/v1/events` | the same snapshot as a stream of server-sent events, one a second | |
| `GET /api/v1/games/<platform>/<game>` | one game's builds in detail | |
| `DELETE /api/v1/games/<platform>/<game>` | delete what the stash holds of a game | `delete` |
| `POST /api/v1/games/<platform>/<game>/prune` | delete what no current build of the game needs | `delete` |
| `GET /api/v1/builds/<platform>` | each game's current build and how much of it the stash holds | |
| `GET /api/v1/catalog/<platform>` | every game the platform offers (a player sees its own) | |
| `GET /api/v1/art/<platform>/<id>` | a game's image | |
| `GET /api/v1/prefill` | the prefill started last | |
| `POST /api/v1/prefill/<platform>/<game>` | start a prefill (`?build=pre` for pre-release, `&queue=1` to queue behind a running one) | `prefill` (a player: its own games) |
| `POST /api/v1/prefill/stop` | stop every prefill, or one platform's | `prefill` |
| `DELETE /api/v1/prefill/<platform>/<game>` | take a game out of the queue | `prefill` |
| `GET /api/v1/prefill/runs` | prefills that ended, newest first | |
| `GET /api/v1/auto-update` | the games kept up to date on their own | |
| `GET /api/v1/history` | what was updated when, newest first | |
| `GET /api/v1/stats` | counters over time; `/api/v1/stats/top` the ones that rose most | (a client's counters: signed in) |
| `GET /api/v1/clients` | what each client got over a time range | signed in |
| `PUT`/`DELETE /api/v1/clients/<address>` | name a client, or take its name away | `clients` |
| `GET`/`PUT`/`DELETE /api/v1/hidden[/<platform>/<game>]` | the hidden games, and hiding one | `hide` |
| `GET`/`PUT`/`DELETE /api/v1/private[/<platform>/<game>]` | a player's own games kept to itself | signed in (a player marks only its own games) |
| `GET /api/v1/account/<platform>` and below | a platform's signed-in accounts, its QR code and sign-in | `platforms` |
| `GET /api/v1/setup/<platform>/<os>` | the file that sets up a PC (`windows`, `macos`, `linux`) | |
| `GET /api/v1/users`, `POST`/`PUT`/`DELETE /api/v1/users/<name>` | lanstash's accounts (an account may change its own password) | `accounts` |
| `GET`/`POST /api/v1/invites`, `DELETE /api/v1/invites/<id>` | invite links: list, make, take back | `accounts` |
| `GET`/`POST /api/v1/invites/<token>` | read an invite and make the account with it | the token |
| `GET /api/v1/settings` | the settings the web UI may change | |
| `PUT`/`DELETE /api/v1/settings/<key>` | change one, or drop the change | `settings` |
| `POST /api/v1/restart` | restart to apply the changes (refused while at work unless `?force=1`) | `settings` |
| `GET /api/v1/dns` | the DNS server's setup and report | `dns` |
| `PUT`/`DELETE /api/v1/dns/setup`, `POST /api/v1/dns/apply` | set up or forget the DNS server; add the missing rewrites | `dns` |
| `GET /api/v1/health` | the checks the service makes of itself | |
| `POST /api/v1/shutdown` | stop cleanly | this machine only |

A platform in a path is its id: `steam`, `blizzard` (Battle.net), `epicgames` or `riot`. A platform without a
view answers 404.

For anyone who may not see hidden games, every JSON answer and every event has them masked: a hidden game shows as
"A hidden game", without its name or ids, and its own page and art are not found.

## Event stream

`GET /api/v1/events` is `text/event-stream`: one `data:` snapshot a second until the page goes away. At most 8
streams run at once and 3 from one address (other than this machine); beyond that the answer is 503 `busy`. A
stream ends when its viewer signs out or its role changes, and the page reconnects with what it may see now.

```sh
curl -N http://192.0.2.10:8081/api/v1/events
```
