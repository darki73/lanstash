# lanstash-dns-pihole

Pi-hole as the network's DNS server (`server`): lanstash's rewrites (list, add,
remove) and the query log, through Pi-hole's v6 API, signing in with its
password (a session per operation, signed out after it). How a rewrite is
written (`rules`):

- to an address: a regex rule with a reply (`^name$;reply=ADDRESS`,
  `[.]name$;reply=ADDRESS` for `*.name`), which answers A with the address and
  AAAA with `::`; a local DNS record answers A only and lets AAAA go to the
  internet, so lanstash writes none for game names;
- to a name: a local CNAME record; its target gets a local DNS record (Pi-hole
  follows a CNAME only to one, and not to a name with a deny rule), and a
  `*.name` pointed at a name takes that name's address (Pi-hole has no
  wildcard CNAMEs).

Tested against Pi-hole 2026.10.0. Pi-hole is EUPL-1.2 and is not a
dependency: lanstash only speaks its HTTP API. It cannot say how it answers a name with every list counted, so
`DnsServer::check` is left at its default (only the rewrites count).

## Place in the workspace

- Layer: DNS server (pihole), built like a platform.
- Depends on: `lanstash-domain`, `lanstash-ports`.
- Dev-depends on: none.
- `no_std` (+ `alloc`): no I/O; requests go through `lanstash_ports::web::Web`.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file per module, against a Pi-hole in memory
that answers the way Pi-hole 2026.10.0 does.
