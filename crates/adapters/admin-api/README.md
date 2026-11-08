# lanstash-adapter-admin-api

The admin port: the HTTP API and the built-in status page (`api`), its
routes (`route`), generic documents (`render`) and the client the commands
use to reach a running lanstash (`client`). Platform views are served by
platform id from whatever the composition root registers.

## Place in the workspace

- Layer: adapter.
- Depends on: `lanstash-domain`, `lanstash-ports`, `lanstash-app`.
- Dev-depends on: `lanstash-store`, `lanstash-adapter-http`.
- Module ban: no `lanstash_store` (it reads the stash only through the status use case and the platforms).

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
