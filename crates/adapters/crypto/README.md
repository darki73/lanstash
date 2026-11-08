# lanstash-adapter-crypto

Cryptography through ring: password hashing for lanstash's own accounts (`passwords`: the
`Passwords` port as PBKDF2-HMAC-SHA256, 600,000 iterations, a 16-byte salt) and secure random
bytes (`random`: the `Random` port on the system's generator).

## Place in the workspace

- Layer: adapter.
- Depends on: `lanstash-domain`, `lanstash-ports`, and ring (Apache-2.0 AND ISC; already in the
  lock file through rustls).
- Dev-depends on: none.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file per module.
