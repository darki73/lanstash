# Map of lanstash

What the code does and where it does it. Every file gives `path:line` references; line numbers may have moved, so
search for the named item in the same file. Measurements are in [../measurements.md](../measurements.md).

lanstash is a caching HTTP proxy for game CDNs (Steam, Battle.net, Epic, Riot) that replaces
`lancachenet/monolithic` (nginx). Clients reach it through DNS that points the CDN host names at the cache.

## Find it in two steps

Find the question below, then open the section it names.

| Question | Where |
|---|---|
| Why std only (and which crates not), threads, crash-only, extents? | [architecture.md#decisions](architecture.md#decisions) |
| What is lanstash for, and what not? | [architecture.md#goals-and-non-goals](architecture.md#goals-and-non-goals) |
| Which crates exist and what may each depend on? | [architecture.md#crates](architecture.md#crates) |
| Which rules do the architecture tests enforce? | [architecture.md#rules](architecture.md#rules) |
| Which third-party crates, and their licences? | [architecture.md#third-party-crates](architecture.md#third-party-crates) |
| Which ports exist; where is everything wired? | [architecture.md#ports-and-the-composition-root](architecture.md#ports-and-the-composition-root) |
| Which loops run in the background? | [architecture.md#background-loops](architecture.md#background-loops) |
| Where do tests live; what is exempt? | [architecture.md#tests](architecture.md#tests) |
| Which host belongs to which service? | [services.md#services](services.md#services) |
| How is a game found in a CDN path; what is cached or passed through? | [services.md#profiles](services.md#profiles) |
| What is an object key; which paths are not cached? | [services.md#object-keys](services.md#object-keys), [services.md#plain-paths](services.md#plain-paths) |
| Objects, extents, whole vs partial | [services.md#objects-and-extents](services.md#objects-and-extents) |
| Directory layout on disk | [storage.md#layout](storage.md#layout) |
| Segment file format | [storage.md#segments](storage.md#segments) |
| Object files, `journal.lsj` | [storage.md#object-files-and-the-journal](storage.md#object-files-and-the-journal) |
| Compacting a segment | [storage.md#segment-compaction](storage.md#segment-compaction) |
| The in-memory index, hash collisions | [storage.md#index](storage.md#index) |
| What happens after a crash? | [storage.md#crash-safety](storage.md#crash-safety) |
| Eviction (`max_size`, `keep_free`, last use) | [storage.md#eviction](storage.md#eviction) |
| What happens at startup? | [storage.md#startup](storage.md#startup) |
| `store verify` | [storage.md#verification](storage.md#verification) |
| The database, private files and state | [storage.md#kept-outside-the-store](storage.md#kept-outside-the-store) |
| Connections, deadlines, limits of the HTTP server | [requests.md#http-server](requests.md#http-server) |
| Which requests are refused as malformed? | [requests.md#http-parsing](requests.md#http-parsing) |
| How is a client request answered (ranges, HIT/MISS, 416, headers)? | [requests.md#request-handling](requests.md#request-handling) |
| How are downloads shared, split and retried? | [requests.md#downloads](requests.md#downloads) |
| HTTP and HTTPS pass-through | [requests.md#pass-through](requests.md#pass-through) |
| Upstream HTTP and DNS | [requests.md#upstream](requests.md#upstream) |
| Every configuration key and its default | [configuration.md#keys](configuration.md#keys) |
| What the web UI may change | [configuration.md#settings-the-web-ui-may-change](configuration.md#settings-the-web-ui-may-change) |
| Every command (`serve`, `stop`, `import`, `prefill`, ...) | [operations.md#commands](operations.md#commands) |
| Access and event logs | [operations.md#logs](operations.md#logs) |
| Admin API paths, errors, and who may call it | [operations.md#admin-api](operations.md#admin-api) |
| The status snapshot | [operations.md#status](operations.md#status) |
| Deleting a game; pruning old builds | [operations.md#deleting-a-game](operations.md#deleting-a-game) |
| Changing settings and restarting | [operations.md#settings-and-restart](operations.md#settings-and-restart) |
| Cover art and icons | [operations.md#art](operations.md#art) |
| Game names | [operations.md#game-names](operations.md#game-names) |
| The web UI | [operations.md#web-ui](operations.md#web-ui) |
| `lanstash stop` | [operations.md#clean-stop](operations.md#clean-stop) |
| `lanstash import`; lancache's slice keys | [operations.md#import-from-nginx](operations.md#import-from-nginx) |
| `install`, `uninstall`, `doctor` | [operations.md#install-uninstall-doctor](operations.md#install-uninstall-doctor) |
| The platform port; builds kept up to date | [battlenet.md#the-platform-port](battlenet.md#the-platform-port), [battlenet.md#kept-builds](battlenet.md#kept-builds) |
| Battle.net builds, catalog and game JSON | [battlenet.md#views](battlenet.md#views) |
| Battle.net names, flavours (installs) and art | [battlenet.md#catalog-names-and-art](battlenet.md#catalog-names-and-art) |
| Product codes and CDN directories | [battlenet.md#products](battlenet.md#products) |
| Is a Battle.net game current? What does an install download? | [battlenet.md#builds](battlenet.md#builds), [battlenet.md#plans](battlenet.md#plans) |
| How does Battle.net prefill work; what is checked? | [battlenet.md#prefill](battlenet.md#prefill), [battlenet.md#checked-from-the-cdn](battlenet.md#checked-from-the-cdn) |
| Prefill from lancache's nginx cache; what it holds of a game | [battlenet.md#prefill-from-lancache](battlenet.md#prefill-from-lancache), [battlenet.md#the-nginx-probe](battlenet.md#the-nginx-probe) |
| Steam sign-in, accounts and sessions | [steam.md#sign-in](steam.md#sign-in), [steam.md#sessions](steam.md#sessions) |
| Which Steam account reads a depot? | [steam.md#which-account-reads-a-depot](steam.md#which-account-reads-a-depot) |
| Steam manifests, builds, sizes, prefill, prune | [steam.md#manifests-and-kept-chunk-lists](steam.md#manifests-and-kept-chunk-lists), [steam.md#builds-and-sizes](steam.md#builds-and-sizes), [steam.md#prefill](steam.md#prefill), [steam.md#prune](steam.md#prune) |
| Epic sign-in, the client, token refresh | [epic.md#sign-in-and-the-client](epic.md#sign-in-and-the-client), [epic.md#accounts-and-token-refresh](epic.md#accounts-and-token-refresh) |
| Epic library, manifests, builds, prefill, prune | [epic.md#library](epic.md#library), [epic.md#manifests-and-chunks](epic.md#manifests-and-chunks), [epic.md#builds](epic.md#builds), [epic.md#prefill](epic.md#prefill), [epic.md#prune](epic.md#prune) |
| Why the Epic launcher is set to download over HTTP | [epic.md#windows-setup-file](epic.md#windows-setup-file) |
| How the DNS server's rewrites are planned and applied | [dns.md#the-plan](dns.md#the-plan), [dns.md#apply](dns.md#apply) |
| Setting up AdGuard Home or Pi-hole | [dns.md#setup-check](dns.md#setup-check), [dns.md#adguard-home-adapter](dns.md#adguard-home-adapter), [dns.md#pi-hole-adapter](dns.md#pi-hole-adapter) |
| Who downloaded what; client names | [activity.md#clients](activity.md#clients), [activity.md#client-names](activity.md#client-names) |
| Counters over time, lifetime totals, build history | [activity.md#counters-over-time](activity.md#counters-over-time), [activity.md#lifetime-totals](activity.md#lifetime-totals), [activity.md#build-history](activity.md#build-history) |
| Past prefills; automatic updates | [activity.md#prefill-runs](activity.md#prefill-runs), [activity.md#automatic-updates](activity.md#automatic-updates) |
| Health checks; the background store check | [activity.md#health-checks](activity.md#health-checks), [activity.md#background-store-check](activity.md#background-store-check) |
| The database tables and how long rows are kept | [activity.md#sqlite-tables](activity.md#sqlite-tables) |

## Files

| File | Covers |
|---|---|
| [architecture.md](architecture.md) | decisions, goals, crates, rules, third-party crates, ports, composition root, background loops, tests |
| [services.md](services.md) | services, profiles, object keys, plain paths, extents |
| [storage.md](storage.md) | layout, segments, journal, compaction, index, crash safety, eviction, startup, verification, database and private files |
| [requests.md](requests.md) | HTTP server and parsing, request handling, downloads, pass-through, upstream |
| [configuration.md](configuration.md) | configuration keys, precedence, settings the web UI may change |
| [operations.md](operations.md) | commands, logs, admin API, status, deletion and pruning, settings, art, names, web UI, import, install |
| [battlenet.md](battlenet.md) | the platform port, kept builds, Battle.net views, catalog, products, builds, plans, prefill, prune, nginx probe |
| [steam.md](steam.md) | Steam sign-in, sessions, library, manifests, builds, prefill, prune, lancache probe, art |
| [epic.md](epic.md) | Epic sign-in, accounts, library, manifests, builds, sizes, prefill, prune, setup file, art |
| [dns.md](dns.md) | the DNS plan, apply, setup, AdGuard Home and Pi-hole adapters |
| [activity.md](activity.md) | clients, counters, lifetime totals, build history, prefill runs, automatic updates, health, store check, database tables |
