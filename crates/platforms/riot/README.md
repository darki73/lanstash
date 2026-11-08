# lanstash-riot

Riot: which requests are cached (`profile`) and the platform the
composition root registers (`platform`).

## Place in the workspace

- Layer: platform (riot).
- Depends on: `lanstash-domain`, `lanstash-ports`.
- Dev-depends on: none.
- `no_std`: pure logic, no I/O.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
