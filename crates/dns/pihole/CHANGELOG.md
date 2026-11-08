# Changelog

Notable changes to `lanstash-dns-pihole`.

## 0.1.0 (unreleased)

First version.

- Pi-hole as a `DnsServer`, through its v6 API, a session per operation.
- Rewrites listed, added and removed as reply rules, local DNS records and CNAMEs.
- The query log read as lookups; a reply rule's or a local record's answer counts as rewritten.
