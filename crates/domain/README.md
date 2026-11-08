# lanstash-domain

Pure logic over plain values and byte slices: HTTP heads, byte ranges and
chunked bodies (`http`), extents (`extent`), the segment and journal formats
(`segment`, `journal`), checksums, inflate, DNS and TLS wire formats, the
nginx cache file format (`nginx_cache`), request routing per service
(`service`: the `Profile` trait and the `Profiles` registry), game names,
JSON, image types, eviction choices, QR codes (`qr`), ZIP entries (`zip`)
and RSA encryption (`rsa`).

## Place in the workspace

- Layer: domain.
- Depends on: none.
- Dev-depends on: none.
- `no_std` (+ `alloc`): the compiler guarantees no I/O.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
