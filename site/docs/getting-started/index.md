# Getting started

From nothing to a LAN whose game downloads come from lanstash, in four steps:

1. **[Install](install.md)** lanstash on a Linux machine with storage to spare, as a service.
2. **[Configure](configure.md)** where it stores games, which DNS it asks, and where its web UI listens.
3. **[Point your DNS](dns.md)** at it, so the PCs on your network fetch games through it.
4. **[First steps](first-steps.md)**: make an admin, sign in, connect Steam, set up Epic PCs, prefill a game.

!!! tip "Before you start"
    lanstash needs the machines on your LAN to use a DNS server you control (AdGuard Home, Pi-hole, or any
    server where you can add records). That is how game downloads find their way to it.
