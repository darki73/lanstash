# Changelog

Notable changes to `lanstash-adapter-fs`.

## 0.1.0 (unreleased)

First version.

- The storage ports over directories and files, with the OS file offered for zero-copy sending.
- Free space of the store's disk.
- lancache's nginx cache directory, read-only.
- Private files (directory 0700, files 0600, atomic replace) for the `Secrets` and `Keep` ports.
