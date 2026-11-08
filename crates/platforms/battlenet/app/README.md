# lanstash-battlenet-app

Battle.net's use cases: the product list (`products`), each game's live and
pre-release builds (`builds`), install plans (`plans`), the launcher catalog
(`catalog`), what lancache's nginx cache holds of a plan (`nginx`) and the
prefill planner (`prefill`, run by `lanstash_app::prefill`).

## Place in the workspace

- Layer: platform (battlenet).
- Depends on: `lanstash-domain`, `lanstash-ports`, `lanstash-store`, `lanstash-app`, `lanstash-battlenet-domain`.
- Dev-depends on: none.
- No I/O of its own beyond spawning background threads.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
