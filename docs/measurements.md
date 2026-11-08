# lanstash: measurements

Evidence behind the design (`docs/map/`). Everything here was measured on the setup below unless marked
**[hypothesis]**. Later sections may supersede earlier ones.

## Setup

Setup: a cache server (16 cores, 15 GiB, 25 GbE to the LAN) writing to a NAS over SMB 3.1.1
(`cifs`, multichannel ×2, MTU 1500); the NAS stores on ZFS over SATA SSDs (RAIDZ1). Clients: one PC on 10 GbE.

| Measurement | Result |
|---|---|
| iperf3 server → client PC | ~1050 MB/s **[verified]** |
| nginx, data hot in RAM, 8 streams, from the PC | ~890 MB/s **[verified]** |
| nginx HIT from SMB, 1 stream (Steam behaviour) | **~172–190 MB/s** **[verified]** |
| nginx HIT from SMB, 8–32 streams | ~390–500 MB/s **[verified]** |
| SMB: stat only | 0.10 ms/file **[verified]** |
| SMB: open + read 1 byte + close | **1.58 ms/file** **[verified]** |
| SMB: open + read 1 MiB + close | 3.15 ms/file **[verified]** |
| SMB opens with 1 vs 32 parallel readers | ~630 vs 677 files/s: doesn't scale **[verified]** |
| NAS during 32-way 1 MiB reads (~720 MB/s) | one core at 96–100 %, total ~7 % **[verified]** |
| All lancache SMB I/O | a single `smbd` process (one session, 2 channels) **[verified]** |
| Startup tree walk over SMB | ~10k files/s, ~7 min for ~4.2M files **[verified]** |

**Root cause:** nginx stores every ~1 MB Steam chunk as its own file, and two-thirds
of chunks span two slice files. Every request pays SMB open/close (~1.5 ms),
and opens are handled one at a time in one `smbd` process. **[verified]** that they don't
scale; the exact mechanism inside smbd/ZFS is **[hypothesis]**.

Other problems:
- no record of which chunk belongs to which game (file names are MD5s)
- slow startup
- eviction is purely least-recently-used, with no awareness of games or versions.

## Serving from the stash

**Import** (`lanstash import --dirs 00`, nginx level dir `00` = 70,892 files): 44,066 Steam chunks,
29.02 GB in 94 s (~308 MB/s read+write over SMB), **100 % verified against `x-content-crc`**, 0 mismatches,
350 segments (one series per depot). 7,202 chunks skipped because a later slice was missing in the
nginx cache (**cause not investigated**). Index load at startup: **0.35–0.38 s** for 350 segments.

**A/B benchmark**, same chunks on the same SMB share, from the 10 GbE PC, `curl --parallel`,
3,000 distinct chunks per run, server page cache dropped before each run **[verified]**:

| Streams | nginx (lancache) | lanstash | Speed-up |
|---|---|---|---|
| 1 (Steam-like) | 132.6 MB/s | **530.3 MB/s** | **4.0×** |
| 8 | 423.9 MB/s | **684.1 MB/s** | 1.6× |
| 32 | 541.3 MB/s | **806.6 MB/s** | 1.5× |

Responses were byte-identical to nginx's (MD5 compared). Caveat: the NAS ARC could not be dropped (no shell);
both datasets had been touched by the import minutes before.

NAS busiest core: ~22 % at 1 stream (latency-bound, ~1.5 ms per chunk read), ~74 % at 32 streams
(approaching the single-`smbd` ceiling above).

Next levers, by expected effect **[hypothesis]**: read-ahead of the next records in a segment
(single stream), several SMB sessions or NFS (parallel ceiling), then the miss path.

## Storage path tuning and serve optimisations

**Network (dedicated storage bridge, MTU 9000)** **[verified]**: jumbo frames work
end to end (TCP MSS 8948), but give **no measurable gain** over MTU 1500 on this path
(single SMB stream ~1.0–1.7 GB/s either way).

**SMB sessions** **[verified]**: each `nosharesock` mount of the same IP is its own session and its own
`smbd` process (NAS SMB status). Raw 32-reader 1 MiB reads, NAS ARC warm: 1 session ~1.8 GB/s,
2 ~2.8–3.0, 3 ~3.1–3.4, 4 ~3.5–3.7 GB/s. Multichannel does not help (same session, same process).

**Serve changes**: pipelined body (header + first 256 KiB read together, rest read ahead while sending),
sequential prefetch of the next record (only on sequential access; 196/200 used in a sequential test),
`--read-path` to spread sealed segments over several mounts.

| Test | Without | With |
|---|---|---|
| PC, 1 connection, random | 485–571 MB/s | 577–602 MB/s |
| PC, 8 connections, random | 684 MB/s | 844–890 MB/s |
| Server localhost, 1 connection | not measured | 970–990 MB/s (≈ same as serving from RAM) |
| Server localhost, 4×8 connections, 1 path → 3 paths | not measured | 1,324–1,361 → **1,653–1,696 MB/s** |

**Finding**: the single-connection limit from the PC is the HTTP request/response cycle of one
connection, not storage: a chunk served from RAM also reaches only ~549 MB/s from the PC (nginx: 486 MB/s).
lanstash serves a single connection from SMB about as fast as from RAM.

**Note on load generation**: one `curl --parallel` process caps at ~1.3 GB/s; use several processes.

## The miss path

`lanstash serve --upstream-dns 1.1.1.1` on the cache server, port 8090, with an empty store on the
share. Real Steam CDN traffic, `Host: lancache.steamcontent.com`. All results **verified** unless marked.

- **Upstream:** via 1.1.1.1, `lancache.steamcontent.com` → CNAME chain → `origin-tier2.steampipe…` /
  Akamai → Valve's origin tier. The local DNS points the same name at the cache,
  which is why fills use their own resolver.
- **One chunk:** the miss took 0.81 s, the following hit 0.9 ms. The bytes were identical (md5) to the copy
  lancache's nginx had cached, and `x-content-crc` came back on the hit from record metadata.
- **Coalescing:** 6 clients asked for one uncached chunk at the same moment. That was 1 upstream fetch and 5 joins,
  all 6 responses were complete, and every client finished in 0.33 s.
- **Throughput of misses** (16 parallel each, two disjoint uncached halves of one batch run at the same time):
  lanstash averaged **0.19 s per chunk**, direct `curl` to the origin **0.36 s**. lanstash reuses keep-alive
  connections and curl opens a new one per request. Miss throughput is bound by origin latency
  times parallelism (direct: 4.6 MB/s at 8 parallel, 14.8 MB/s at 32), not by lanstash.
- **Totals:** 161 downloads, 161 stored, 0 upstream, CRC or store errors. All 4 segments passed
  `seg verify`. After a restart the index recovered all 161 chunks from the 4 unsealed segments.

**Crash test** (body written before header, unsealed bodies verified, segments continued):
120 uncached chunks, 16 parallel, `kill -9` after 1.5 s. 27 clients had complete responses and 19 chunks were stored;
the 8 still being written were lost, which is harmless because they are fetched again. Restart: "1 continued", 19 chunks.
Re-requesting all 120 gave 19 HIT and 101 MISS, all **byte-identical to nginx's copies**. Both segments passed `seg verify`
with 0 problems (the crash leftovers at the tail were cut off and reused), and a second restart found all 120 chunks
in the same 2 segments.

Known limitations:
- A crash leftover between two valid records (concurrent writes finishing out of order) stays dead space
  until compaction. Leftovers at the tail, the common case, are reused.
- Non-chunk requests (manifests etc.) pass through without caching, like Steam manifests on lancache.

## Non-Steam services

**Measured.** Blizzard, Epic and Riot are cached like lancache: 1 MiB slices, key
`<service><uri>bytes=<a>-<b>`, host → service from the uklans cache-domains lists, redirects followed upstream,
`releaselisting_*` and `*.version` not cached, loop protection (`X-LanStash-Via`, 508). Against the real CDNs,
all responses were byte-identical to direct downloads (Blizzard ranges and whole archives, Epic chunks on
two hosts).

**What Battle.net actually requests** (access log, a 100 GB game install, **verified**):
- Whole-file GETs only for small config and `.index` files (23 requests, under 30 KB each).
- Range GETs for the data: 65 requests, 4.55 GB, of which 14 were under 1 MiB, 38 were 1–16 MiB and 13 over 16 MiB.
  The largest was **822.8 MB in one range**, i.e. whole archives.
- Ranges within one archive mostly move forward (113→115 MB, 117→129 MB, 129→142 MB, …).
- The same archive is requested from `eu.cdn.blizzard.com` and `level3.blizzard.com`. The key is host-independent,
  so each piece is downloaded once.
- First install through lanstash: 32–47 MB/s (Steam on the same line: ~92 MB/s). Each 822 MB request becomes ~785
  upstream slice requests, at ~0.3–1 s each, 8 in parallel.
- Reinstall: all 7.85 GB fetched before came back as HIT at ~220 MB/s, the same ceiling as the Steam client on
  that PC (220–325 MB/s), while the server itself reads at 1.6 GB/s. So the PC side is the likely limit (**hypothesis**).

## Clients, CDN behaviour and storage probes

lanstash 1 MiB-slice build on port 80 with `--access-log-headers`; Battle.net client on the PC. All **verified**
unless marked.

**Request headers.** Two Battle.net programs talk to the CDN:
- The Battle.net app (catalog files, `/tpr/catalogs/…`) sends `User-Agent: Battle.net/2.53.8.17986 (retail)` and
  `Accept: */*`.
- The Agent, which downloads the games, sends only `Host`, `Range` (when ranged) and `Accept: */*`: no User-Agent
  and no other identifying header. The Agent links libcurl (its strings are curl's), and libcurl sends no
  User-Agent unless the program sets one.

So game data can only be recognised by host. Blizzard's CDN hosts serve nothing else, so that is enough.

**How the Agent reads archives** (Diablo Immortal, already cached):
- Many archives are requested whole as a range, `bytes=0-<size-1>`, up to 822 MB.
- Others are read in parts, with gaps that are never requested. For example, archive `085292b4…` was read as
  0–0.25, 1.6–55.6, 59.0–59.9, 63.8–76.1, 77.2–78.7, 78.7–127.8 and 128.7–135.1 MB. The gaps are 0.9–3.9 MB.
- Requests run in parallel and arrive out of order (77.2 MB before 1.6 MB).

So a miss must fetch exactly the requested range. Fetching to the end of the object would download parts no
client wants.

**"Initializing" phase** (Hearthstone, never cached before; the client showed about 13 MB):

| Requests | Content | Size | Running total |
|---|---|---|---|
| 1–5 | 2 config files, 3 small data files | 3.9 MB | 3.9 MB |
| 6–139 | 132 `.index` files (archive and patch indexes) | 5.8 MB | 9.6 MB |
| 140–144 | 5 metadata files, two requested a second time | 3.9 MB | 13.5 MB |
| 145 on | game content | 13 GB | |

The 5 files are probably the encoding table and the install/download manifests **[hypothesis]**. Everything
in this phase is content-addressed and cacheable. Two files were requested twice within seconds; coalescing
fetched each once.

**First-download speed with 1 MiB slices:** 35 MB/s for Hearthstone, the same 32–47 MB/s as in the section above.

**The same file with and without a range** (archive `…/18/72/1872279e…`, 795,075,223 bytes, from the CDN directly):

| Request | Status | Size reported |
|---|---|---|
| `HEAD` | 200 | `Content-Length: 795075223` |
| `GET`, `Range: bytes=0-1048575` | 206 | `Content-Length: 1048576`, `Content-Range: bytes 0-1048575/795075223` |
| `HEAD`, same `Range` | 200 | `Content-Length: 795075223` (the range is ignored) |
| `HEAD` on the other host (`eu.cdn` vs `level3`) | 200 | same length and `Last-Modified` |

The first range response tells the full object size. level3 also sends an `ETag`; eu.cdn sends none.

**Sparse files over the SMB share** (cifs 3.1.1 to ZFS):

| Step | File length | Space used (`du`) |
|---|---|---|
| Write 1 MiB at offset 0 and 1 MiB at offset 700 MiB | 701 MiB | 2 MiB |
| Then write 1 MiB at offset 300 MiB | 701 MiB | 2 MiB at once, 3 MiB about 10 s later |

Holes cost no space, and filling a hole later works. ZFS reports the new space only after its transaction
commit.

**Hearthstone, complete first download** (1 MiB-slice build): 2,406 requests, 13.04 GB in 556 s, i.e.
**23 MB/s on average** including the Initializing phase (35 MB/s at its peak). Data requests: 1,322 ranges
from offset 0 (12.31 GB, mostly whole archives), 949 ranges at later offsets (0.73 GB), 91 without a range
(4 MB, index files).

## Full downloads through lanstash [verified]

lanstash on port 80, empty store on the SMB share, one client PC. Figures from the access log (first to last
request of the PC, bytes as sent).

| Run | Requests | Data | Time | Average | Cache |
|---|---|---|---|---|---|
| Hearthstone, first download | 2,403 | 13.04 GB | 320 s | **40.7 MB/s** (1 MiB-slice build: 23 MB/s) | all MISS |
| Hearthstone, reinstall | 2,403 | 13.04 GB | 39 s | **338 MB/s** | all HIT |
| Deadlock (Steam), first download | 40,690 chunks | 15.07 GB | 235 s | **64.2 MB/s** | MISS (2 HIT) |
| Deadlock, reinstall | 40,943 chunks | 15.21 GB | 51 s | **300 MB/s** (nginx from SMB: ~172–190) | all HIT |

- No failed downloads, CRC mismatches, retries, storage errors, incomplete responses or events in any run.
- Hearthstone: the merged requested ranges come to 13.04 GB, so the requests did not overlap. `du` over SMB
  reported only 4.5 GB allocated (object files almost 0) although everything was then served from storage:
  `du` on the share is not a usable measure of stored bytes.
- Steam first download: ~1.25 of 16 cores, ~30 downloads in flight. The client and the CDN
  set the pace **[hypothesis]**; a direct Steam download at the same time would decide it.
- Battle.net first download: 3–4 downloads in flight (the Agent's own concurrency); one 10 s window fell to
  14 MB/s during a burst of ~220 small range requests.
- `store stats` afterwards: Hearthstone 13.36 GB stored for 13.04 GB requested. Cause **[verified]**: in the build
  measured, the first download of an object fetched a whole 16 MiB piece from the client's start, even for a
  smaller range (~2.5 % extra origin traffic). The first download now stops at the end of the client's range.

## Import from lancache

Plan-driven import (`?from=lancache`): each request of a build's plan read out of lancache's nginx cache by slice
key, checked against the hash in its path, stored as a download would be; the CDN for the rest. Same SMB share
for both caches. **[verified]** unless marked.

| Game (build) | Plan | lancache held (probe) | From lancache | From CDN | Rejected | Time | Rate |
|---|---|---|---|---|---|---|---|
| Overwatch 2 (2.25.1.0.154905) | 83.62 GB, 41,393 requests | 98.3 % | 83.42 GB (39,715 requests) | ~1.4 GB | 0 | 245 s | ~340 MB/s |
| Diablo IV (3.2.4.74043, HD pack included) | 171.05 GB, 73,036 requests | 99.9 % | (not recorded) | | 0 | | |
| Diablo Immortal (5.1.3.3935504), second run | 91.75 GB, 595 requests | 100 % | 89.73 GB (540 requests) | 0 | 0 | 227 s | ~396 MB/s |
| Diablo III (2.8.0.99920) | 16.74 GB | 8 % | (stopped) | | | | |

- Check rules verified on real files: BLTE encoding key = MD5 of the header (whole file when header size 0:
  hypothesis), each chunk against its listed MD5 (3 Overwatch files, up to 345 chunks, also fed in 1 MiB parts);
  config name = MD5 of content; index name = MD5 of its 28-byte footer.
- Independent check: one 268 MB Overwatch archive fetched through lanstash (imported ranges + the rest from the
  CDN, `cache=PARTIAL`) had the same MD5 as the archive straight from Blizzard.
- Diablo Immortal's loose files exceed 512 MiB. Holding a piece whole sends most of their bytes to the CDN (the
  first run); streaming one 1 MiB slice per worker removes the limit (the second run, above).
- Diablo IV: reading its 915 indexes one at a time runs at ~2/s. With a single flight per build and 8 at a time,
  planning + probe took 98 s.
- Diablo IV's manifest has a resolution tag: without the HD pack (`lowres`) 95.30 GB, with it 170.66 GB
  (Windows, enUS, EU). lancache held the HD files, so the LAN downloads them.

## Steam prefill from lancache

- Steam app info shows Deadlock (app 1422450, build 26467182) needs depots 1422451, 1422452 and 1422456 on
  Windows, English, 64-bit. Their manifests list 40,695 unique chunks, 15.06 GB on the wire.
- `POST /api/v1/prefill/steam/1422450?from=lancache` finished in 91 s:
  - 18,520 chunks were already stashed;
  - 21,368 chunks (8.37 GB) came from lancache's cache, none rejected. Each was checked by CRC-32
    against the `x-content-crc` lancache kept, and by the size in the manifest;
  - 807 chunks (about 0.31 GB) came from the CDN;
  - none were missing.
- Asking the cache with a CDN name from Steam's server list gets 403: it is not among Steam's cache-domains
  (`lancache.steamcontent.com` only), so the import asks with that name.
