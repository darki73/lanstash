# lanstash-adapter-http

HTTP over TCP: the server that hands requests to a `RequestHandler`
(`server`), the client that reaches the origins (`upstream`) and the HTTPS
passthrough that relays TLS by its server name (`tls_passthrough`).

## Place in the workspace

- Layer: adapter.
- Depends on: `lanstash-domain`, `lanstash-ports`, `lanstash-app`, `lanstash-adapter-dns`.
- Dev-depends on: none.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
