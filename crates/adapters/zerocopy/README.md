# lanstash-adapter-zerocopy

Sending a stored file's bytes to a client's socket without copying them
through lanstash (`sendfile`): on Linux the kernel moves them from the file
system's cache to the socket; elsewhere, or where the file system cannot,
`send_file` says so before sending anything and the caller sends them the
usual way (read into a buffer, write to the socket).

The only `unsafe` in lanstash is here: one system call in one function
(`sendfile::send_file`), with a safe signature. The crate's root denies
`unsafe`, that function alone allows it, and `crates/architecture` lets no
other file hold it.

## Place in the workspace

- Layer: adapter.
- Depends on: `libc` (MIT OR Apache-2.0; Linux only, already in the lock through `ring` and `getrandom`).
- Dev-depends on: none.
- Its lints are the workspace's but for `unsafe_code`, which is `deny` here (the workspace forbids it, which no item could lift).

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
