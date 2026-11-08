# lanstash-adapter-names

Game names: a table (started by the composition root with the platforms'
grouping rules) plus a names file, and a chain of sources (`game_names`).
Battle.net's names come from its catalog, chained after the file.

## Place in the workspace

- Layer: adapter.
- Depends on: `lanstash-domain`, `lanstash-ports`.
- Dev-depends on: none.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
