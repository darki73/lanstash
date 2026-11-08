# DNS

Your PCs find lanstash through DNS. The LAN's DNS server answers the game CDN host names with lanstash's address,
so a launcher that wants a download asks lanstash instead of the internet. This page explains what is needed, how
lanstash's DNS page sets it up on AdGuard Home or Pi-hole for you, and how to do it by hand on any other DNS server.

The examples use `192.0.2.10` for lanstash and `192.0.2.1` for the LAN's DNS server.

## Why DNS

A launcher downloads from host names such as `google.epicgamescdn.com`. When the LAN's DNS server answers those
names with `192.0.2.10`, the request reaches lanstash, which serves it from the stash or fetches it once from the
real CDN (looked up through `upstream_dns`) and keeps it. Nothing needs changing on the PCs themselves, apart from
Epic's launcher (see [First steps](first-steps.md#set-up-an-epic-games-pc)).

The names come from the cache-domains lists in your `cache_domains` directory (see [Configure](configure.md)). Each
line is written one of two ways:

- `name`: that one name;
- `*.name`: every name under it, but not the name itself.

When Epic Games is among the `services`, lanstash also wants `google.epicgamescdn.com` and `*.google.epicgamescdn.com`,
which its list lacks.

!!! warning "Make the LAN use this DNS server"
    The PCs must use the DNS server you change, usually by having the router hand it out. A PC with its own DNS
    set goes around it.

## The DNS page

The web UI's DNS page links lanstash to your DNS server, adds the rewrites lanstash wants and checks how the names
stand. It works with:

| Server | Usual port | Signs in with |
|---|---|---|
| AdGuard Home | 3000 (HTTPS 443) | an admin user and its password |
| Pi-hole (v6) | 80 (HTTPS 443) | its web password (or an app password) |

Changing the DNS setup, and even reading it, needs an account with the `dns` permission (an admin).

### Set up the server

Under **Server**, choose **Change** and fill in:

- **Kind**: AdGuard Home or Pi-hole.
- **Address**: its host name or IP address, without `http://` (for example `192.0.2.1`).
- **Connection** and **Port**: how its admin page is reached, plain HTTP or HTTPS, and on which port.
- **Certificate** (HTTPS only): **Check** wants a certificate from a public authority for that address; **Trust
  one** accepts only the certificate with a given SHA-256, for a self-signed one; **Don't check** accepts any.
- **User** (AdGuard Home) and **Password**. lanstash keeps the password where only it can read it and never shows
  it again; leave it empty later to keep it.
- **lanstash's address**: the IP address your PCs reach lanstash at, `192.0.2.10`. Left empty, lanstash uses the
  address your existing rewrites of its names already lead to.
- **Point the names at**: **The address**, or **A name** (every game name points at that name, and that name at
  lanstash's address, so only one rewrite changes if the address ever does).

**Test** tries the setup without keeping it; **Save** tries it, then keeps it. When an HTTPS certificate is not
trusted, the page shows its SHA-256 and offers to trust exactly that certificate. **Forget** removes the setup and
its password.

### Add the rewrites

**Rewrites** shows how many of the names lanstash wants are rewritten to its address, per platform, with the ones
missing, the ones there twice, and the ones the server already sends to lanstash by a rule of its own or a wider
rewrite. **Add N rewrites** adds the missing ones and merges duplicates. It never removes or changes a rewrite that
is not lanstash's: one pointing elsewhere, or one pointing at lanstash for a name on none of its lists, is left
alone.

On Pi-hole, a rewrite is a reply rule (marked with the comment `lanstash`) that answers IPv4 queries with
lanstash's address and IPv6 queries with `::`, so a PC with IPv6 cannot go around the cache.

### Check what went around

- **Went to the internet** lists, from the latest lookups the server logged, the names of a cached platform that no
  rewrite answered, with how often and by which clients.
- **Refused by lanstash** lists the names sent to lanstash since it started that none of its lists name.

## By hand, on any other DNS server

On any DNS server that can answer a name with a fixed address, add one entry per line of the lists you use:

| Line in the list | Entry on the DNS server |
|---|---|
| `name` | `name` answers `192.0.2.10` |
| `*.name` | every name under `name` answers `192.0.2.10` |

Add `google.epicgamescdn.com` and its subdomains for Epic Games. Make sure IPv6 (AAAA) queries for these names do
not lead to the internet, or a PC with IPv6 may download around the cache.

Then check from a PC on the LAN:

```sh
nslookup google.epicgamescdn.com 192.0.2.1
```

The answer should be `192.0.2.10`. `lanstash doctor` checks the upstream DNS; the DNS page's checks need AdGuard
Home or Pi-hole.

Next: [First steps](first-steps.md).
