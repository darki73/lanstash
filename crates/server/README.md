# lanstash

The composition root and the `lanstash` binary. The library holds the parts
`tests/` can reach (command line `cli`, configuration `config`, the web UI's
`settings`, the DNS server's setup `dns`, `install`, `report`,
`prefill_cli`). The binary wires everything: `run/mod.rs` (configuration,
store), `run/platforms.rs` (the platforms and their use cases),
`run/steam.rs` and `run/epic.rs` (their accounts and libraries),
`run/serve.rs` (`serve`, `stop`), `run/store.rs` (import, statistics,
verification), `run/clients.rs`, `run/health.rs`, `run/kept.rs` (the SQLite
database), plus `setup` and `remote`.

## Place in the workspace

- Layer: root.
- Depends on: every workspace crate.
- Dev-depends on: none.
- Module bans: `cli`, `config`, `install`, `report` and `prefill_cli` use no `std::fs`, `std::process`, `std::thread` or adapter crate.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
