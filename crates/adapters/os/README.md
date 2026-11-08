# lanstash-adapter-os

The operating system: running a program with a time limit (`command`),
asking systemd (`systemd`), a process's open files and their limit
(`process`), a real sleep for the `Pause` port (`pause`), and the terminal
prompt (`terminal`: secret answers with echo off).

## Place in the workspace

- Layer: adapter.
- Depends on: `lanstash-ports`.
- Dev-depends on: none.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
