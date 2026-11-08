# Changelog

Notable changes to `lanstash-architecture`.

## 0.1.0 (unreleased)

First version.

- Dependency policy: acyclic, platform layering, the core never names an adapter or platform.
- A third-party allow-list, each crate approved only for the packages named.
- Module import bans and a 400-line ceiling per file.
- A README and a CHANGELOG per crate; tests mirror `src/`.
- No `pub use`; `unsafe` only in the files the policy names.
- No em dashes under `crates/`.
