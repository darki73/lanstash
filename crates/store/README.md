# lanstash-store

Per service and game: small whole objects in segments, large or partial
objects in sparse object files described by a journal, one in-memory index
over all of it, eviction candidates and a verifier. Feature `test-support`
adds in-memory storage doubles (`MemFs`, `MemFile`, `ManualClock`) and an
nginx cache double (`test_support::nginx`).

## Place in the workspace

- Layer: core.
- Depends on: `lanstash-domain`, `lanstash-ports`.
- Dev-depends on: itself with `test-support`.
- No I/O of its own: every file goes through the storage ports.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
