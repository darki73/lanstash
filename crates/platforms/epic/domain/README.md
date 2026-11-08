# lanstash-epic

Epic Games: which requests are cached and under which game (`profile`), the
`.cmd` that sets up a Windows PC so that lanstash keeps the launcher's
downloads (`setup`: every word, path and setting in it a checked type),
signing in to an account (`auth`: Epic's login page, the pasted code, the
token requests and answers), an account's library (`library`: its assets,
catalog items, images and the catalog view), build manifests and their
chunk checks (`manifest`), and the platform the composition root registers
(`platform`, offering the accounts `lanstash-epic-app` keeps).

## Place in the workspace

- Layer: platform (epic).
- Depends on: `lanstash-domain`, `lanstash-ports`.
- Dev-depends on: none.
- `no_std` (+ `alloc`): pure logic, no I/O.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
