# lanstash-app

The use cases: answering a client request (`serve`), coordinating
downloads (`fetch`, `download`), pass-through, eviction, verification,
status, cover art, shutdown, importing lancache's cache (`import_nginx`,
`nginx_file`), reading pieces from it (`lancache_read`), storing them
(`stash_put`) and the generic prefill runner (`prefill`). Feature
`test-support` adds a scripted origin and a response sink.

## Place in the workspace

- Layer: core.
- Depends on: `lanstash-domain`, `lanstash-ports`, `lanstash-store`.
- Dev-depends on: itself with `test-support`.
- No I/O of its own: sockets, files and clocks only through ports.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
