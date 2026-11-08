# lanstash-adapter-sqlite

What lanstash keeps beyond a restart, in one SQLite database: `lanstash.db` in
the state directory.

| Port | What is kept |
|---|---|
| `HistoryLog` | The build history: events (a game added, a new build, a build held whole), the games as last seen, and the platforms taken in |
| `StatsLog` | Counters by the minute: how much each went up (a minute with no change has no row) |
| `PrefillLog` | The prefill jobs that ended: what, how much, how it ended |

## Schema

The schema is made and moved forward when the database is opened. SQLite's
`user_version` says which step it is at. A shipped step is never changed, and a
change is a new step. A database from a newer lanstash, at a step this one does
not know, is refused rather than guessed at.

The database runs in write-ahead-log mode with `synchronous = NORMAL`, so a
crash loses at most the last commits and never the database.

## Dependencies

- `rusqlite` 0.40.2 (MIT) with the `bundled` feature: SQLite 3.53.2 (public
  domain) is compiled in from source by `libsqlite3-sys` (MIT). Building needs a
  C compiler; no system SQLite is used.
- What it pulls in is on the workspace's allow-list
  (`crates/architecture/src/policy.rs`).
