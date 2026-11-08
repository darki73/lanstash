# lanstash-battlenet-domain

Battle.net's pure logic: TACT configs and archives (`tact`), BLTE (`blte`),
install plans (`plan`, `download`, `locate`), the launcher catalog with each
product's installs and their names (`catalog`), the nginx keys of a plan (`nginx_keys`),
the checks of pieces read from lancache (`check`) and which requests are
cached (`profile`).

## Place in the workspace

- Layer: platform (battlenet).
- Depends on: `lanstash-domain`.
- Dev-depends on: none.
- `no_std` (+ `alloc`): the compiler guarantees no I/O.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
