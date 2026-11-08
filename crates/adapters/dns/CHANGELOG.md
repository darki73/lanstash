# Changelog

Notable changes to `lanstash-adapter-dns`.

## 0.1.0 (unreleased)

First version.

- `resolver`: DNS over UDP to the configured upstream servers, never the host's resolver.
- `reverse::PtrResolver`: what the LAN's DNS servers call a client's address (a PTR query), the servers read from a `resolv.conf`.
- `udp::exchange`: one query and its answer over UDP.
