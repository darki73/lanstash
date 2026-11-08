# Changelog

Notable changes to `lanstash-adapter-zerocopy`.

## 0.1.0 (unreleased)

First version.

- `sendfile::send_file`: a file's bytes from an offset to a socket by `sendfile(2)` on Linux, the file's position untouched.
- `Sent::Unsupported`, said before anything is sent, where the system or file system cannot.
- The only `unsafe` in lanstash, in that one function behind a safe signature.
