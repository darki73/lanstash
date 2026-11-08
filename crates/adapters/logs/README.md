# lanstash-adapter-logs

Log files that rotate by size (`rotating_file`), the event log
(`event_log`) and the HTTP server's access log (`access_log`).

## Place in the workspace

- Layer: adapter.
- Depends on: `lanstash-domain`, `lanstash-ports`, `lanstash-adapter-http`.
- Dev-depends on: none.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
