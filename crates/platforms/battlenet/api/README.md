# lanstash-battlenet-api

Battle.net's side of the platform contract: `platform::BattleNet` (id
`blizzard`), which the composition root registers, and the JSON of its views
(`json`).

## Place in the workspace

- Layer: platform (battlenet).
- Depends on: `lanstash-domain`, `lanstash-ports`, `lanstash-battlenet-domain`, `lanstash-battlenet-app`.
- Dev-depends on: `lanstash-store`, `lanstash-app`.
- Module bans: no `std::thread`, no `lanstash_store` (views render what the use cases found).

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
