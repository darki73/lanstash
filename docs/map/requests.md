# Map: requests, downloads and pass-through

Code references are `path:line`; line numbers may have moved, so search for the named item. Claims are marked **[verified]** or **[hypothesis]**.

## HTTP server

`HttpServer` `crates/adapters/http/src/server.rs:61` (`run` `:113`, `serve_connection` `:223`), settings
`ServerConfig` `:42`, filled by the root in `server_config` `crates/server/src/run/mod.rs:37`. One OS thread per
connection, blocking std I/O; parsing is the domain's ([HTTP parsing](#http-parsing)). **[verified]**

- Limits: `max_connections` connections in all; one client address holds at most `max_per_client`
  (`max_connections / 4`, at least 64) (`take` `:144`). A connection over either limit gets `503` and is closed.
  Tested: `one_client_holds_only_its_share_of_the_connections` `crates/adapters/http/tests/server.rs`. **[verified]**
- Timeouts: idle 60 s between requests; a request head must be whole 15 s after its first byte, however slowly
  it trickles in (`head_timeout`, timed by the `Clock` port); writes 60 s. Tested:
  `a_head_that_trickles_in_is_closed_at_its_deadline`. **[verified]**
- Keep-alive and pipelining: HTTP/1.1 stays open unless `Connection: close`, HTTP/1.0 only with `keep-alive`;
  buffered heads are answered in order (`RequestHead::keep_alive` `crates/domain/src/http/mod.rs:77`). A
  request with a body (`Transfer-Encoding`, or `Content-Length` other than 0) gets `400` and the connection
  closes. A head over 16 KiB gets `431`, any other parse error `400`. **[verified]**
- `TcpSink` `:168` writes the head (`Content-Length` and `Connection` set by lanstash), drops bodies of `HEAD`,
  remembers `X-LanStash` for the access log, and sends stored bodies with `sendfile` where it can
  ([stored bodies](#stored-bodies-and-sendfile)). A response cut off closes the connection. **[verified]**
- The admin port runs the same server with `max_connections` 32 and `max_per_client` 24
  (`crates/server/src/run/serve.rs:169`; [operations.md](operations.md#admin-api)). **[verified]**

## HTTP parsing

`parse_request_head` `crates/domain/src/http/mod.rs:97`, pure, with hard `Limits` (`:14`: 16 KiB head, 100
headers). Tests: `crates/domain/tests/http/main.rs`. **[verified]**

- Request line: three parts, method all upper-case ASCII letters, `HTTP/1.1` or `HTTP/1.0` only.
- Target in origin form only: it starts with `/` and holds no space, control byte or DEL. A full URL
  (`http://host/...`) is refused, so a request can never name a host of its own.
- Header names: no space, control, `:` or non-ASCII byte. Header values: no control character but a tab
  (`value_ok` `:144`), so a value can never become a header or request of its own when sent on.
- `Content-Length`: digits only, and said again only with the same value (`one_length` `:151`).
- Names are lower-cased, values trimmed. `host()` drops a numeric port.
- Writing a head leaves out any header whose name or value holds a line break (`write_headers` `:164`).
- `STORED_RESPONSE_HEADERS` `:204` (kept with an object and sent again on hits), `WHOLE_BODY_HEADERS` `:216`
  (`x-content-crc`, `content-md5`: never with a 206), `RELAYED_RESPONSE_HEADERS` `:246` (passed from an
  origin to clients; framing and hop-by-hop headers are lanstash's own).

## Request handling

The handler is `Cache` `crates/app/src/serve/mod.rs:198`, behind the driving port `RequestHandler` (`:96`).
Dispatch `handle_inner` `:302`; one object `serve_object` `crates/app/src/serve/object.rs:63`; its body
`send_body` `crates/app/src/serve/body.rs:22`. **[verified]**

1. Only `GET` and `HEAD`; others get `405` with `Allow: GET, HEAD`.
2. `/lanstash-heartbeat` and `/lancache-heartbeat` answer `204` with `X-LanCache-Processed-By: lanstash`
   (prefill tools probe it; `HEARTBEAT_PATH` `:104`).
3. A request carrying `X-LanStash-Via` is lanstash's own upstream request come back (the upstream DNS points at
   the cache): `508`, `X-LanStash: LOOP` (`VIA_HEADER` `crates/app/src/fetch/mod.rs:34`).
4. Host (trailing dot dropped, lower-cased) to service: a host no list names gets `403`
   `X-LanStash: HOST-NOT-ALLOWED` and is counted by name (`RefusedHosts`, at most 256 names,
   `crates/app/src/serve/refused.rs`). Service to profile to route ([services.md](services.md#profiles)).
5. Plain paths only are cached: a route whose path holds `\`, `%2e`, `%2f`, `%5c` (any case) or a `.`/`..`
   segment is passed through, never stored (`Route::filter_plain` `crates/domain/src/service/mod.rs:31`,
   `plain_path` `crates/domain/src/object.rs:31`; test `a_path_spelled_with_dot_segments_passes_and_stores_nothing`).
   Routes that are `Pass` (Riot listings, `.version` files, `uncacheable_path` `:68`) go the same way.
6. `Range`: one `bytes=` range (`a-b`, `a-`, `-n`; `parse_range` `crates/domain/src/http/range.rs:43`).
   Several ranges or another unit are passed through.
7. Total length and headers: from the index, else from the object's first download (`Fetcher::learn`,
   [the first request](#the-first-request-for-an-object)). An object marked unusable is passed through.
8. Response: `200` (no range) or `206` with `Content-Range`, always `Accept-Ranges: bytes`, the stored headers
   (without `WHOLE_BODY_HEADERS` on a 206), and the marks below. An unsatisfiable range gives `416` with
   `Content-Range: bytes */<total>` (`X-LanStash: RANGE`). An origin error to the first download (404, 403, 5xx)
   is relayed with its status, headers and body, marked `UPSTREAM`, never stored (`relay_download`
   `crates/app/src/serve/object.rs:25`). An origin that cannot be reached gives `502`
   `X-LanStash: UPSTREAM-ERROR` (`bad_gateway` `crates/app/src/passthrough.rs:38`).
9. Body: the range is walked; each stored part is read from disk, each missing part comes from its download
   (`send_download` `crates/app/src/serve/body.rs:77`), joined or started by `Fetcher::cover`.
10. Sequential prefetch (only with `prefetch`): when a connection reads segment records that lie one after
    another (Steam), the next record is read in the background (`spawn_prefetch` `:205`). Random access never
    triggers it.
11. Bytes sent are counted per game (`ServedByGame`, `crates/app/src/serve/served.rs`) and per client and game
    (`ClientTraffic`, `crates/app/src/serve/clients.rs`): `hit` for a HIT, `miss` otherwise.

Without upstream (`Cache::new` alone, no `with_upstream`) a miss is `404` `X-LanStash: MISS` and a pass is `404`
`X-LanStash: NOT-CACHED`. **[verified]**

### Marks: X-LanStash and the lancache headers

`mark` `crates/app/src/serve/mod.rs:360` adds three headers to every object, relayed and passed answer
(test `every_answer_says_it_came_through_a_lan_cache` `crates/app/tests/serve/object.rs`): **[verified]**

| `X-LanStash` | when | `X-Upstream-Cache-Status` |
|---|---|---|
| `HIT` | every byte of the span was stored when the response started | `HIT` |
| `PARTIAL` | part of the span stored, the rest downloaded (counted as a miss, and in `partial`) | `MISS` |
| `MISS` | nothing of the span stored | `MISS` |
| `UPSTREAM` | an origin error relayed from the first download | `MISS` |
| `PASS` | pass-through | `BYPASS` |

and `X-LanCache-Processed-By: lanstash` on each, the headers a lancache answers with, which tools looking for a
LAN cache read. The control answers carry `X-LanStash` only: `LOOP`, `HOST-NOT-ALLOWED`, `RANGE`,
`UPSTREAM-ERROR`, `ERROR` (a storage read failed for the head: `500`), `NOT-CACHED`.

### Stored bodies and sendfile

A stored part goes by `stream_file` `crates/app/src/serve/body.rs:239`: **[verified]**

- First the sink is asked to send it itself (`ResponseSink::send_file` `crates/app/src/serve/mod.rs:89`). The
  TCP sink does it with `sendfile(2)` when `zero_copy` is on (always, `server_config`) and the reader is an OS
  file: `send_file` `crates/adapters/zerocopy/src/sendfile.rs:25`, the offset passed with each call (the file's
  own position is never moved, many connections share one open file), at most `0x7fff_f000` bytes a call.
  `EINVAL`/`ENOSYS`/`EOPNOTSUPP` before any byte went means "cannot": nothing was sent and the bytes go the
  other way. Not Linux: always "cannot" (`:63`). The crate denies `unsafe` but for that one call (the workspace forbids
  it; `crates/adapters/zerocopy/Cargo.toml`).
- Otherwise the part is read in `send_piece` pieces by a reader thread at most 2 pieces ahead of the socket,
  buffers handed back and refilled (a fresh buffer per piece would cost a page fault and zeroing per page).
- A segment record's header is checked against the key and length before its body is sent; a mismatch marks
  the object unusable (`key_mismatch` `crates/app/src/serve/object.rs:194`). The header and first piece are read
  together for the response head.

## Downloads

### Coordinator

`Fetcher` `crates/app/src/fetch/mod.rs:184`: `learn` `:253` (total length and headers), `cover` `:310` (the
download delivering a position). A download is `Download` `crates/app/src/download.rs:80`; its thread
`run` `crates/app/src/fetch/transfer.rs:60`, its work `download` `:122`. Settings `FetchOptions` `:38`
(`fetch_piece`, `fetch_ahead`, `download_window`, `object_file_min`; 256 KiB upstream reads; 64 MiB for a body
of unknown length). **[verified]**

All downloads of an object are in one in-flight table, under one lock that is also held while the index is
consulted, so two downloads never cover the same bytes: **[verified]**

- A missing part already being downloaded is **joined**: the client reads the bytes as they arrive.
- Only downloads still running are joined or counted as covering (`Flight::covering` `:165`); a finished one is
  about to leave the table, and an error it relayed is not the answer to a new request.
- A missing part nobody downloads is split into pieces of at most `fetch_piece` bytes; at most `fetch_ahead`
  pieces (1 to 64) are started ahead of the client's read position, the next ones as the client moves on. Each
  piece is one upstream request with `Range`. Profiles that fetch whole objects (Steam) start one download
  without `Range`.
- Downloads belong to no client: a client hanging up does not stop them, so what was started is cached (bounded
  by `fetch_ahead` pieces per abandoned request).
- A download that finished is entered in the index before it leaves the table (`forget` `transfer.rs:80`). A
  download thread that panics still ends its download, so its waiters wake (`Unfinished`; test
  `a_download_that_panics_wakes_its_waiters_and_leaves_the_table`).

### The first request for an object

While the total length is unknown only one download, the **probe**, may run for the object; other requests wait
for its response head. The probe asks for the client's range from its start, at most one piece and at most to the
client's end, with `Range`; for whole-object profiles it asks for the whole object. Its response gives the total
length (`Content-Range` or `Content-Length`), the headers to store and the status to relay. A `416` with
`bytes */<total>` becomes an unsatisfiable answer. **[verified]**

### One download

`download` `crates/app/src/fetch/transfer.rs:122`. Request headers: `X-LanStash-Via: 1`, the client's
`User-Agent`, `Range` for a piece. Where it writes: `open_target` `:288`: a whole object below `object_file_min`
to a segment record, anything else to the object's sparse file. **[verified]**

- Redirects (301, 302, 303, 307, 308; at most 5, `MAX_REDIRECTS` `:24`) are followed upstream and never
  cached (`fetch_following` `:94`). An absolute `http://` redirect is followed only to a host the cached
  services' lists name (`Fetcher::with_hosts` `fetch/mod.rs:210`, set by the root): never into the network.
  A relative one keeps the host; any other (`https://`) fails the download. Tests
  `crates/app/tests/fetch/transfer.rs`. **[verified]**
- Only the probe relays an origin error. Any status but 200 or 206 to a later piece fails that piece
  (`upstream answered N for a piece` `:136`): an error page is never served or stored as object bytes. Tests
  `a_piece_answered_with_an_error_fails_and_frees_its_span`, `an_error_answer_for_a_piece_is_never_served_as_object_bytes`. **[verified]**

### Checks

- `206` must carry `Content-Range` with the requested start; the end is capped at the requested end. `200` is
  the whole object.
- The body length must match what was asked (`upstream sent N of M bytes`).
- When the download is the whole object and the profile has a body check (Steam `x-content-crc`), the CRC-32
  must match. Until it does, clients receive at most all bytes but the last; on a mismatch nothing of the body is
  readable and the request fails (test `steam_chunk_with_bad_crc_is_not_completed_nor_stored`).
- Failed bodies are never committed. The bytes of a failed object-file piece that arrived intact before the
  failure are committed if at least 1 MiB. **[verified]**

### Origins that ignore Range

A `200` to a ranged request delivers the whole object (`range-ignored` event and counter). For the probe the
download becomes the whole object; for a later piece the bytes before the piece are read and dropped, and reading stops at its end (the
connection is then not reused).
Test `origin_that_ignores_range_still_gives_the_right_slice`. **[verified]**

### Failures during a response

If a download fails (or ends without a byte) while a client reads from it, the client's request plans the
remaining bytes again from its current position, up to `retries` times (default 3), so the client sees one
uninterrupted response; only then is the connection closed. A relayed error body gets no retries.
Tests `failed_download_is_retried_from_where_it_stopped`, `an_error_answer_for_a_piece_fails_the_request_after_the_retries`. **[verified]**

### Memory

A download keeps about `download_window` bytes (default 4 MiB, at least 64 KiB) in memory, trimmed back once it
holds twice that; a client further behind reads the bytes back from storage (`set_read_back`
`crates/app/src/download.rs:128`). A download that cannot be stored (no read-back) keeps at most
`MEMORY_WINDOWS` (`:16`, 16) windows; a client further behind than that fails and asks again (`push` `:138`;
test `a_download_that_cannot_be_stored_keeps_a_bounded_amount_in_memory`). Total memory is bounded by the number
of downloads, which is bounded by `fetch_ahead` per request and the connection limits. **[verified]**

## Pass-through

### HTTP

Requests that are not cached go to the origin and are streamed back: `Passthrough`
`crates/app/src/passthrough.rs:23`, `relay` `:57`. **[verified]**

- Sent on: the method and target as they came, `X-LanStash-Via: 1`, and only `User-Agent`, `Range`, `Accept`,
  `If-Modified-Since`, `If-None-Match` (test `relays_the_answer_and_forwards_only_known_headers`).
- Answered with the origin's status, its `RELAYED_RESPONSE_HEADERS` and the marks (`PASS`, `BYPASS`). Redirects
  are relayed to the client, not followed.
- A body without `Content-Length` is buffered up to 64 MiB, then answered with its length; larger, or an origin
  that cannot be reached, gives `502` `UPSTREAM-ERROR`. A body that ends early closes the connection.

### HTTPS (listen_tls)

Clients also reach the listed hosts over HTTPS: `TlsPassthrough` `crates/adapters/http/src/tls_passthrough.rs:86`,
settings `TlsConfig` `:25`; the SNI parser `crates/domain/src/tls.rs`. No TLS library is involved and nothing is
cached or decrypted. Tests `crates/adapters/http/tests/tls_passthrough.rs`. **[verified]**

1. The ClientHello must be whole within 10 s in all, however slowly it comes (`read_hello` `:243`); it is
   parsed only once a TLS record is whole, at most 64 KiB (`MAX_HELLO`). Not TLS, no server name or too late:
   `bad_hello`, a `tls-failed` event.
2. The SNI host must be on the cached services' lists (no open relay), else `refused`.
3. The name is resolved through the upstream DNS; at most 3 addresses are tried on port 443 (10 s connect each),
   skipping `0.0.0.0`, broadcast, and the address and port the client reached lanstash on: an upstream DNS that
   answers with the cache's own rewrites is never relayed to in a loop (`connect` `:206`; test
   `a_name_that_resolves_to_lanstash_itself_is_not_relayed_in_a_loop`).
4. The ClientHello is replayed and bytes are copied both ways (`relay` `:279`, `pump` `:307`) until both ends close
   or nothing moved in either direction for 300 s. One direction ending half-closes the other side.

At most `max_connections` relays at once; past that a connection is dropped. Counters (`connections`, `active`,
`relayed`, `refused`, `bad_hello`, `connect_failed`, `bytes_up`, `bytes_down`) are the status's `https` group.

## Upstream

- HTTP client: `HttpUpstream` `crates/adapters/http/src/upstream.rs:46` behind `Upstream`
  `crates/ports/src/upstream.rs:55`; `UpstreamConfig` `:22`: port 80, 5 s connect, 30 s per read or write, up to
  32 idle connections per host. A connection goes back to the pool only after its body was read whole and the
  origin agreed to keep it; a request on a reused connection that fails before any response byte is retried once
  on a fresh one. Interim `1xx` answers are skipped; bodies are framed by length, chunked or until close
  (`exchange` `:96`). **[verified]**
- Names resolve through `upstream_dns`, never the host's resolver (which points at lanstash): `UdpResolver`
  `crates/adapters/dns/src/resolver.rs:62`, `ResolverConfig` `:25`. A-records only; servers tried in order, each
  twice, 2 s per query; answers cached for their TTL held between 30 s and 1 h; one query at a time (others wait
  for its answer: public resolvers drop bursts); when no server answers, an answer expired less than 1 h ago is
  used. An IPv4 literal is its own answer. **[verified]**
- Host rules: the cache-domains lists, plus Steam's content hosts always and Epic's content hosts when
  `epicgames` is cached (`host_rules` `crates/server/src/run/mod.rs:147`). **[verified]**
- Requests lanstash itself asks of the cache (prefill, plans of a game the stash holds) go through the same
  handler as a client's: `via_cache::get` `crates/app/src/via_cache.rs:69` (user agent `lanstash-prefill`,
  whose reads do not count as a use of the game), marked as lanstash's own (`ConnState::own`): counted in
  `own_requests` and `own_bytes` only, never in the clients' requests, hits, misses, their bytes or the bytes
  served per game (`crates/app/tests/serve/served.rs`); what they fetch from the internet still counts in
  `fetched_bytes`. Small whole-file GETs for lanstash's own needs (art, catalogs,
  versions) go straight to the origin: `origin::get`, `origin::get_range` `crates/app/src/origin.rs:20`, `:29`.
  **[verified]**
