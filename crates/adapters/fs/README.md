# lanstash-adapter-fs

The file system: the storage ports over directories and files (`dir_fs`,
`fs_file`), free space (`disk_space`), lancache's nginx cache directory,
read-only (`nginx_dir`), and secrets as private files (`private_dir`).

## Place in the workspace

- Layer: adapter.
- Depends on: `lanstash-domain`, `lanstash-ports`, `lanstash-adapter-os`.
- Dev-depends on: `lanstash-store`.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
