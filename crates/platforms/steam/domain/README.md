# lanstash-steam-domain

Steam: which requests are cached and how (`profile`), where a game's cover
image is (`art`), depot naming (`names`), the connection-manager protocol
(`cm`: protobuf, message framing, logon, authentication, product info,
content servers), KeyValues (`kv`), apps and their depots (`apps`), depot
manifests (`manifest`), the builds and catalog views (`builds`, `catalog`),
chunk checks (`check`), sampling lancache's cache (`share`) and the platform
the composition root registers (`platform`).

## Place in the workspace

- Layer: platform (steam).
- Depends on: `lanstash-domain`, `lanstash-ports`.
- Dev-depends on: none.
- `no_std` (+ `alloc`): pure logic, no I/O.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
