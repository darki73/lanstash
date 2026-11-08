# lanstash-architecture

The workspace's architecture rules and the checks that enforce them, as
pure functions over the members loaded from disk (`workspace`, `manifest`,
`source`). `policy` holds the lanstash tables: allowed dependencies per
crate and layer, module import bans, the 400-line ceiling, `no_std` crates,
and the short allow-lists with their reasons. `cargo test -p
lanstash-architecture` runs every check against the real workspace.

## Place in the workspace

- Layer: tool.
- Depends on: none.
- Dev-depends on: none.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
