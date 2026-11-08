# Changelog

Notable changes to `lanstash`.

## 0.1.0 (unreleased)

First version.

- The composition root: the only crate that names adapters and platforms; it registers each platform's profile, names and `Platform`.
- `lanstash serve`: the cache, the admin API and web UI, stored bodies sent by `sendfile` on Linux.
- Importing lancache's cache, store statistics and verification, also in the background (`verify_every`, `verify_rate`).
- `lanstash doctor` and `GET /api/v1/health`: the checks the service makes of itself.
- `lanstash steam login|status|logout`, by QR code or password; the Steam library read and prefilled once signed in.
- Clients counted per game and named by reverse DNS or a name given in the web UI.
- The DNS server's setup (AdGuard Home, Pi-hole) and the names lanstash wants rewritten.
- `lanstash.db` (SQLite) in the state directory: build history, counters by the minute and prefill runs.
