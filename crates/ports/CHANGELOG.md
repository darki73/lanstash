# Changelog

Notable changes to `lanstash-ports`.

## 0.1.0 (unreleased)

First version.

- Ports for storage, origin servers (a failure says whether it was a timeout), the clock, events, names and
  lancache's nginx cache.
- The platform contract (`Platform`): id, names, art, builds and status, catalog, game parts, pruning, setup files, accounts.
- `Prefill`: starting, queueing and following prefill jobs, from the CDN or from lancache.
- Logs kept beyond a restart: history, counters over time and prefill runs; named state (`Keep`).
- DNS: the network's DNS server, its rewrites, lookups and setup; web API requests (`Web`, `Endpoint`).
- Clients: reverse names and the names given to them.
- Ports for Steam's login: `Link`, `Dialer`, secrets, randomness and prompts.
