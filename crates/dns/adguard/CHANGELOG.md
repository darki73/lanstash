# Changelog

Notable changes to `lanstash-dns-adguard`.

## 0.1.0 (unreleased)

First version.

- AdGuard Home as a `DnsServer`, through its admin API with basic authentication.
- Rewrites listed, added and removed.
- How AdGuard Home answers a name: rewrites, `$dnsrewrite` rules and hosts-file rules counted.
- The query log read as lookups: who, which name, what was answered, whether a rewrite answered, when.
- HTTPS admin pages: Mozilla's roots, a pinned certificate or unchecked.
