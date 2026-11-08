# lanstash-dns-adguard

AdGuard Home as the network's DNS server (`server`): its rewrites (list, add,
remove), how it answers a name with its rules counted (`check`), and its query
log, through AdGuard Home's admin API with basic authentication, and the times
its log writes (`time`). Tested against AdGuard Home v0.107.62. AdGuard Home
is GPL-3.0 and is not a dependency: lanstash only speaks its HTTP API, over
HTTP or HTTPS.

Other DNS servers get a crate of their own beside this one (Pi-hole:
`crates/dns/pihole`), implementing the same `lanstash_ports::dns::DnsServer`.

## Place in the workspace

- Layer: DNS server (adguard), built like a platform.
- Depends on: `lanstash-domain`, `lanstash-ports`.
- Dev-depends on: none.
- `no_std` (+ `alloc`): no I/O; requests go through `lanstash_ports::web::Web`.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
