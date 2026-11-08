# Changelog

Notable changes to `lanstash-adapter-sqlite`.

## 0.1.0 (unreleased)

First version.

- `db::Database`: one SQLite database, its schema made and moved forward on open (`user_version`), write-ahead log.
- `HistoryLog`: the build history.
- `StatsLog`: counters by the minute, and the counters under a prefix that went up most in a range.
- `PrefillLog`: the prefill jobs that ended.
- `ClientLabels`: the names given to clients.
