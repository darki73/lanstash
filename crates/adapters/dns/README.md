# lanstash-adapter-dns

DNS over UDP to the configured upstream servers (`resolver`), never the
host's resolver (which points at lanstash); and what the LAN's own DNS
servers call a client's address (`reverse`, a PTR query).

## Place in the workspace

- Layer: adapter.
- Depends on: `lanstash-domain`, `lanstash-ports`.
- Dev-depends on: none.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
