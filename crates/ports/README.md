# lanstash-ports

What the use cases need from the outside world (storage, origin servers,
the clock, events, names, lancache's nginx cache) and what the core asks of
a platform (`platform::Platform`, `prefill::Prefill`). Adapters and
platforms implement them; the composition root wires them.

## Place in the workspace

- Layer: ports.
- Depends on: `lanstash-domain`.
- Dev-depends on: none.
- `no_std` (+ `alloc`): traits and the plain values they carry.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
