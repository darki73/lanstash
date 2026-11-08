# Map: storage and startup

Code references are `path:line`; line numbers may have moved, so search for the named item. Claims are marked **[verified]** or **[hypothesis]**.

## Layout

```
<root>/<service>/<game>/
    seg/00000.lcseg ...    small whole objects, segment format v1, at most segment_size (1 GiB) each
    obj/<cdn path>         one sparse file per large or partially stored object, at its full length
    journal.lsj            which object files exist and which byte ranges of them are valid
    used.lsu               when each segment and object file was last used, for eviction
```

Code: `GameDir` `crates/store/src/layout.rs:21` and the names `:12`-`:17`; roots per service `Roots`
`crates/store/src/store.rs:22` (`for_service` `:42`, `games` `:50`) **[verified]**.

- A service can have its own root (`store.<service>` in the configuration), e.g. Steam on a faster disk. A
  service directory found in a root that is not responsible for it is listed as ignored, not loaded
  (`Roots::games`).
- `<game>` and every path component are restricted to `[A-Za-z0-9._-]`; other bytes are percent-encoded, and a
  component that would be `.`/`..`/empty or longer than 200 bytes is replaced by its hash
  ([services.md, storage names](services.md#storage-names)). So a CDN path can never escape its directory.
- The file system adapter checks every path again: relative, `/`-separated, no empty, `.` or `..` components,
  no `\`, `:` or NUL, components of at most 255 bytes (`valid` `crates/adapters/fs/src/dir_fs.rs:34`)
  **[verified]**.
- **Symbolic links are refused**: opening a store file for reading or writing fails when the path is a
  symbolic link; it is never followed (`refuse_link` `crates/adapters/fs/src/fs_file.rs:58`; test
  `a_symbolic_link_is_not_followed` `crates/adapters/fs/tests/fs_file.rs:29`) **[verified]**.
- Creating a file or renaming one syncs its directory on Unix (`sync_dir` `crates/adapters/fs/src/dir_fs.rs:126`)
  **[verified]**.
- Sealed segments can be opened through extra read-only paths to the same directory (one SMB mount per session),
  round-robin (`with_read_paths` `crates/adapters/fs/src/dir_fs.rs:55`, `open_read_immutable` `:146`); the views
  must list the same entries **[verified]**.
- A whole object smaller than `object_file_min` (default 16 MiB) that is downloaded in one piece goes to the
  game's active segment. Everything else (large objects, and any object downloaded in parts) goes to an object
  file ([services.md](services.md#objects-and-extents)). Steam chunks (about 1 MiB, always whole) therefore land
  in segments, and Battle.net's large archives in object files.
- Deleting a game while running: `DELETE /api/v1/games/<platform>/<game>`
  ([operations.md](operations.md#deleting-a-game)); it deletes the game's segments and object files the way
  eviction does (`crates/app/src/delete_game.rs`). Moving a game is moving its directory (with lanstash
  stopped, or followed by a restart).
- Sparse files cost only the space of written bytes on the SMB share **[verified]**; gaps that no client asks
  for cost nothing.

## Segments

`.lcseg`, format v1:

```
0               SegmentHeader (64 B, zero-padded to 4096)  magic "LANSTASH", version 1, alignment, segment id,
                                                            service code, game id, created_at, max_bytes, CRC-32C
4096            Record*       RecordHeader (48 B fixed + key + meta, magic "LSRC", CRC-32C over all) + body,
                              padded to `alignment` (4096)
footer_offset   Footer        (sealed only) FooterEntry {key_hash u64, offset u64, body_len u64, flags u32,
                              header_len u32} (32 B each), sorted by (key_hash, offset)
EOF-64          Trailer       (sealed only) magic "LSTRAILR", footer_offset, entry_count, footer CRC, CRC
```

Format (pure): header `SegmentHeader` `crates/domain/src/segment/header.rs:54`, record `RecordHeader`
`crates/domain/src/segment/record.rs:85` (flags `:30`), footer entry and trailer
`crates/domain/src/segment/footer.rs:16`, `:90`, key hash (FNV-1a 64) `:31`. Writing: `SegmentAppender`
`crates/store/src/segment/writer.rs:87` (`reserve` `:176`, `write_body` `:200`, `commit` `:217`, `abandon`
`:240`, `seal` `:295`, `close` `:311`, `seal_when_idle` `:330`, `resume` `:137`). Reading: `read_sealed_index`
`crates/store/src/segment/scan.rs:137`, `scan` `:174`. Rotation per game: `Game::reserve_record`
`crates/store/src/game/write.rs:86` **[verified]**.

- Records hold whole objects only (`range_start` 0, `total_len` = body length). Records with the `SLICED` flag
  (slices of a larger object, a layout this store does not use) are skipped and counted at startup; they are not
  migrated.
- Space for a record is reserved from `Content-Length` when the download starts, under a short lock; the body is
  written into the segment as it arrives, outside the lock, instead of being buffered.
- The body is written before the record header; the header (with its CRC) commits the record and is written only
  after the checks passed. A record enters the footer only after both writes succeeded. An abandoned reservation
  leaves its space unused.
- Sealing (footer + trailer) **syncs the records first, then writes footer and trailer, then syncs again**
  (`seal_locked` `crates/store/src/segment/writer.rs:271`): a sealed segment's bodies are trusted without their
  checksums, so the footer must never reach the disk before them (test
  `the_records_are_synced_before_the_footer_is_written` `crates/store/tests/segment/writer.rs:44`) **[verified]**.
- A segment is sealed when full (the game opens the next one), when closed with no reservation open, or by
  the last reservation to end after a close (deferred seal). Sealed segments are immutable and are indexed from
  their footer alone.
- Segment ids in headers come from one counter per store, seeded from the clock (`now_ms * 1000`,
  `crates/store/src/store.rs:163`); the game id is the directory name when numeric, else 0
  (`crates/store/src/game/write.rs:134`) **[verified]**.

## Object files and the journal

One append-only journal per game directory (`journal.lsj`). Entries:

```
entry   = magic "LJ" u16 type, u32 payload_len, payload, u32 CRC-32C(type..payload)
HEADER  = u16 version (the first entry of every journal; version 1)
OBJECT  = u32 object_no, u64 total_len, u16 key_len, key, u16 path_len, path, u16 meta_len, meta
EXTENT  = u32 object_no, u64 start, u64 len, u32 CRC-32C of the bytes
FORGET  = u32 object_no
```

Format (pure): `Entry` `crates/domain/src/journal.rs:35`, `read_journal` `:225`, `live_objects` `:307`. File:
`JournalFile` `crates/store/src/journal.rs:11` (`open` `:49`, `append` `:87`). Object files: `ObjectFile`
`crates/store/src/game/mod.rs:61`; created by `Game::object_file` `crates/store/src/game/write.rs:171`, extents
committed by `commit_extent` `:237`, journal compaction `maybe_compact` `:30` **[verified]**.

- `OBJECT` declares an object file (`object_no` is local to the journal; `path` is the file path relative to
  the game directory; `meta` the stored response headers). It is written right after the file was created at
  its full length (`set_len`). A file already at that name that no journal entry knows (left over from a crash)
  is removed and created again, never written through (test
  `a_file_left_over_from_a_crash_is_replaced_not_written_through` `crates/store/tests/game/write.rs:71`).
- `EXTENT` declares `start..start+len` of that file valid. It is written only after those bytes were written and
  the file was synced (`sync_data`), so after a crash an `EXTENT` never points at missing data.
- Appends are not synced themselves (a lost entry only means a download is repeated), except `FORGET`
  ([eviction](#eviction)). A failed append cuts the journal back so a partial entry cannot hide later ones.
- Reading the journal **[verified]** (`read_journal`; tests in `crates/store/tests/journal.rs`):
  - A damaged entry with valid entries after it is skipped and its bytes counted (`skipped`); the entries after
    it are kept (`a_damaged_entry_in_the_middle_keeps_the_ones_after_it` `:64`).
  - A damaged or incomplete entry with nothing valid after it is a torn tail: reading stops there, and the file
    is truncated there before new entries are appended (`torn_tail_is_cut_and_overwritten` `:27`).
  - A journal whose header is of a **newer version**, or that lacks a valid header while valid entries follow,
    is **refused**: nothing of it is loaded and the file is left as it is
    (`a_journal_of_a_newer_version_is_refused_and_left_as_it_is` `:53`). Unknown entry types are skipped.
  - A journal with no valid entry at all (torn while being created) is started again.
- Object numbers that conflict at load (a second object with the same number or key) are counted as bad
  entries and not loaded (`apply_journal` `crates/store/src/game/load.rs:168`).
- Adjacent extents are merged in memory. The journal is rewritten (temp file + sync + rename, object numbers
  kept) when it has at least 1000 dead entries (`COMPACT_MIN_DEAD` `crates/store/src/game/mod.rs:251`) and more
  dead than live ones: at startup and after evictions or deletions of object files.
- Object file names on a case-insensitive file system: names in use are tracked lower-cased; a taken name falls
  back to the key's hash ([services.md, storage names](services.md#storage-names)).

## Segment compaction

`Store::compact_segment` (`crates/store/src/compact.rs:46`): copies the objects of a sealed segment that the
caller still wants (by key) into the game's active segment, then deletes the segment. A platform decides which
objects are wanted: a prune (`crates/app/src/prune_segments.rs`, run through `delete_game.rs:291`) compacts a
segment that mixes current and old objects once at most half of it is current and it was not used within the
grace period; a segment with nothing current goes whole ([operations.md](operations.md)) **[verified]**.

- Only the copy an object is served from is copied (`Index::serves` `crates/store/src/index/records.rs:38`), and
  only when its body passes its CRC-32C; anything else is fetched again when asked for.
- The copies are written first and indexed as duplicates (not served); the active segment is synced, then
  `Index::replace_segment` (`crates/store/src/index/records.rs:52`) forgets the old segment and serves the copies
  in one step under the index lock, so no lookup sees them missing. Then the file is deleted
  (`Game::replace_segment` `crates/store/src/game/evict.rs:123`; test `the_copies_are_synced_before_the_segment_goes`
  `crates/store/tests/compact.rs:65`) **[verified]**.
- A crash between the copy and the delete leaves both on disk: the next start serves the older copy, and the
  newer ones are dead weight that a later compaction drops **[hypothesis, by design: not exercised by a crash
  test]**.

## Index

In memory: `Index` `crates/store/src/index/mod.rs:131`, its state `crates/store/src/index/inner.rs`. Lookups take
a read lock, inserts a write lock **[verified]**.

- `key_hash` (FNV-1a 64 of the object key) to object: total length, extents with locations (`Loc`
  `crates/store/src/index/mod.rs:31`: segment record or object file), the object file if any.
- Built at startup from segment footers (sealed), segment scans (unsealed, bodies verified) and journals.
- Overlaps: the CDNs are content-addressed, so the same key with the same total means the same bytes. A new
  object-file extent adds only the bytes not yet stored; a second whole record of a stored object is ignored.
  A different total, or a record over part of an object file, is a conflict: the object is marked unusable and
  passed through (`insert` `crates/store/src/index/inner.rs:92`).
- Every segment read checks the full key in the record header, and object files carry their key. A key that
  does not match (a 64-bit hash collision, about 2e-6 for 9 million objects) makes that object unusable
  (`mark_unusable` `crates/store/src/index/mod.rs:328`, called from `crates/app/src/serve/object.rs:195`):
  requests for it are passed through. Wrong data is never served.
- An object left with no pieces and no file is forgotten completely, a conflict with it too (`drop_pieces`
  `crates/store/src/index/inner.rs:115`).
- Per service, a change counter (`changes` `crates/store/src/index/mod.rs:378`) tells cheaply whether what it
  holds changed; per segment and object file, the last use for eviction.
- Memory: about 60 B per whole object (Steam: 9 M chunks, about 550 MB) **[hypothesis]**; object files a few
  hundred bytes each.

## Crash safety

| Write | Order | After a crash |
|---|---|---|
| Segment record | reserve, body, header | header missing: no record (its space reads as unwritten); header present, body torn: unsealed segments are scanned with body CRCs at startup and the record dropped |
| Segment seal | sync records, footer + trailer, sync | footer never on disk before the records; a segment without a valid footer is scanned as unsealed |
| Segment tail | none | the newest unsealed segment is continued at the end of its last valid record (the torn tail is cut and reused); older unsealed ones are sealed |
| Object file data | write, `sync_data`, `EXTENT` | data without `EXTENT`: treated as missing, downloaded again |
| Journal entry | append (not synced) | torn tail: cut before the next append; damaged entry in the middle: skipped, the rest kept |
| New object file | create + `set_len`, `OBJECT` | file without `OBJECT`: removed and created again when the object is next stored |
| Eviction of an object file | `FORGET` + sync, index, unlink | the file never comes back, and the journal never describes a file that is gone |
| Journal compaction, `used.lsu` | temp file, sync, rename | the old file or the new one |
| Segment compaction | copies, sync, index swap, unlink | both copies on disk; the older one served |

A clean stop only shortens recovery: `Shutdown` `crates/app/src/shutdown.rs:23` closes every active segment
(each seals once the records being written into it are done; the root waits up to 20 s), saves the last-use
times, and leaves whatever is still open to crash recovery (`stop_hook` `crates/server/src/run/serve.rs:351`)
**[verified]**.

## Eviction

Policy (pure): `Limits` `crates/domain/src/evict.rs:42`, `choose` `:84`, `Recency` `:102`. Store side:
`eviction_candidates` `crates/store/src/evict.rs:57`, `evict` `:79`, per game `candidates`
`crates/store/src/game/evict.rs:76`, `evict_segment` `:99`, `evict_file` `:156`. Use case: `Evictor`
`crates/app/src/evict.rs:74` (`run_once` `:112`, `pick` below it); started by the root as the `lanstash-evict`
loop (`start_eviction` `crates/server/src/run/serve.rs:295`) **[verified]**.

- Two limits, each optional, the stricter wins: `max_size` (bytes stored in the whole store, like lancache's
  `CACHE_DISK_SIZE`) and `keep_free` (bytes or percent that each root's file system must keep free; the disk may
  be shared with other data). Checked every `evict_secs` (default 30). `max_size` is met from all candidates,
  then each root's shortfall of `keep_free` from that root's own.
- No eviction by age: CDN objects are named by their content and never go stale, so deleting old ones while
  space is left only turns future hits into misses.
- Units: a sealed segment (up to `segment_size` of small objects of one game) or one whole object file. The
  segment being written is never evicted. Victims are the least recently used; items used within the last 10
  minutes are protected (`protect_ms`, downloads still writing them, clients still reading them)
  (`EvictOptions` `crates/app/src/evict.rs:35`).
- Last use is kept in memory per segment and object file (a hit, a write) and saved every 5 minutes, and after
  every pass that evicted, to `<game>/used.lsu` (temp file + sync + rename; `save_recency`
  `crates/store/src/game/evict.rs:52`). After a crash, items without a saved time count as used at startup; a
  game without the file starts from its segments' creation times (`load_recency` `:21`).
- Segment order: out of the index first (no new reader finds it), then the file. A segment whose file cannot be
  removed is put back and deleted by a later pass.
- Object file order: a `FORGET` appended **and synced**, then out of the index, then the file removed, **all under
  the game's files lock**: its name stays taken until the file is gone, so a new object of the same name cannot
  be created in between and deleted with it (`evict_file` `crates/store/src/game/evict.rs:156`). Journals
  mostly made of dead entries are compacted after the pass (`compact_journals` `crates/store/src/evict.rs:111`)
  **[verified]**.
- Readers that already hold the file keep reading it: deleting an open file on an SMB (cifs) mount was checked
  to keep the open handle readable with the right bytes **[verified]**.
- After an eviction the next passes wait 2 minutes (`cooldown_ms`): ZFS behind SMB may report freed space late
  **[hypothesis]**, and acting on the old number would delete too much.
- Free space comes from `df -B1 --output=avail,size` (no `unsafe`, no extra crate), with a 10 s timeout for a
  hung mount (`df_space` `crates/adapters/fs/src/disk_space.rs:31`, `SPACE_TIMEOUT`
  `crates/adapters/fs/src/dir_fs.rs:21`); a root that does not answer counts as unknown.

## Startup

`Store::open` `crates/store/src/store.rs:162`; per game `Game::load` `crates/store/src/game/mod.rs:298`, which
runs `load_segments` `crates/store/src/game/load.rs:20`, then `load_journal` `:126` (`read_journal` `:150` when
read-only), then `load_recency` `crates/store/src/game/evict.rs:21` **[verified]**.

1. For every configured root, list the service directories it is responsible for, and their game directories.
2. Per game: segments in sequence order (footer, or a scan with body verification if unsealed or the footer is
   unreadable) and the journal.
3. Per game, the newest unsealed segment becomes the active segment again; older unsealed ones are sealed.
   The next sequence number is one past the highest found, never reused.
4. Compact a journal that is mostly dead entries; load the last-use times.
5. Report, then start listening.

Read-only (`StoreOptions::read_only` `crates/store/src/store.rs:117`, used by `store stats`): nothing is
continued, sealed, cut or compacted, and writes are refused; safe next to a running server.

The report (`LoadReport` `crates/store/src/store.rs:122`, `GameReport` `crates/store/src/game/mod.rs:202`,
printed by `open_store` `crates/server/src/run/mod.rs:84`): games, records and bytes in sealed and open
segments, object files with extents and bytes, the load time, and, when not zero, open segments continued and
sealed, records dropped (body checksum failed), slice records skipped, journal bytes cut after a torn write,
journal bytes of damaged entries skipped, journal entries ignored, journals compacted; then every file that could
not be read and every service directory that belongs to another root. Files not read and damaged records or
entries also reach the health page (`Loaded` `crates/server/src/run/mod.rs:129`) **[verified]**.

Directory listings and footer reads are the only work for sealed segments, so startup time grows with the number
of segments and games, not with the number of objects.

## Verification

`lanstash store verify` checks every body and extent against its CRC-32C: plan `crates/store/src/verify.rs:74`
(one item per segment or object file, from the directory listings and journals, not from a loaded store), each
item read completely by `VerifyItem::check` `:142`, run in parallel by `verify` `crates/app/src/verify.rs:71`.
It writes nothing, so it is safe next to a running server **[verified]**.

- **Records still being written are not damage**: their header is not written yet, so their space reads as an
  unwritten gap (`Problem::Unwritten` `crates/store/src/segment/scan.rs:74`), which verification does not report
  (`check_segment` `crates/store/src/verify.rs:165`; test `a_record_still_being_written_is_not_damage`
  `crates/store/tests/verify.rs:66`). An extent is journaled only after its bytes are synced, so a journaled
  extent is always checkable.
- Reported: body checksum mismatches, corrupt regions, unreadable or mismatching footers, extents whose bytes do
  not match the journal, files that cannot be opened, journals refused, bytes after the last valid journal entry,
  damaged journal entries skipped, entries that make no sense (test
  `a_missing_object_file_and_a_torn_journal_are_reported` `:101`).
- The command says what a full check reads and asks before it starts; with no one to ask (no terminal, as under
  systemd) it starts only with `--accept` (`confirm` `crates/server/src/run/store.rs:24`).
- In the background, `verify_every` (days) runs the same check while serving, paced by `verify_rate`, resuming
  its place after a restart; whatever looked wrong is checked once more against a fresh listing, since a file
  deleted meanwhile is not damage (`Patrol` `crates/app/src/patrol.rs:81`, started by `start_patrol`
  `crates/server/src/run/serve.rs:206`).

## Kept outside the store

The stash holds only CDN objects. Everything else lanstash keeps across a restart is in the `state` directory
(default `/var/lib/lanstash/state`, `crates/server/src/config.rs:32`) **[verified]**. Losing it costs history
and settings, never cached data.

### The database

`lanstash.db` in the state directory (`FILE` `crates/server/src/run/kept.rs:20`), one SQLite database through
`lanstash-adapter-sqlite` (`Database` `crates/adapters/sqlite/src/db.rs:68`): one connection behind a lock, WAL
journal, `synchronous = NORMAL`, 5 s busy timeout (`open` `:83`) **[verified]**.

- Schema: `STEPS` `crates/adapters/sqlite/src/db.rs:11`; step `n` takes `PRAGMA user_version` from `n` to `n + 1`,
  each in a transaction; a step is never changed once shipped. A database at a newer schema than this lanstash
  knows is refused (`ready` `:100`).
- Tables and what writes them:

| Table | Port, adapter | Written by | Kept |
|---|---|---|---|
| `history_events`, `history_seen`, `history_platforms` | `HistoryLog`, `crates/adapters/sqlite/src/history.rs:12` | `BuildHistory` (`crates/app/src/build_history.rs`): what each platform's builds did | the newest 50 000 events (`EVENTS_KEPT` `crates/adapters/sqlite/src/history.rs:10`) |
| `stats_minutes` | `StatsLog`, `crates/adapters/sqlite/src/stats.rs:8` | `CountersKept` (`crates/app/src/counters_kept.rs`, `counters-kept` loop): each counter's increase per minute (status groups, HTTPS passthrough, bytes per game and per client) | 180 days (`KeptOptions` `crates/app/src/counters_kept.rs:31`), forgotten hourly; gauges and `lifetime.*`/`verify.*` left out |
| `prefill_runs` | `PrefillLog`, `crates/adapters/sqlite/src/runs.rs:15` | `PrefillRuns` (`crates/app/src/prefill_runs.rs`, `prefill-runs` loop): every prefill job as it ends | the newest 2 000 (`RUNS_KEPT` `crates/adapters/sqlite/src/runs.rs:13`) |
| `client_labels` | `ClientLabels`, `crates/adapters/sqlite/src/labels.rs:8` | the names given to client addresses in the web UI | until removed |

- Wired by the root in `crates/server/src/run/kept.rs` (database `:23`, counters `:38`, prefill runs `:66`)
  and `crates/server/src/run/clients.rs`; opened only when `admin_listen` is set.

### Private files

`PrivateDir` `crates/adapters/fs/src/private_dir.rs:15` implements `Keep` and `Secrets` over the state directory
**[verified]**: the directory is mode 0700 and each file 0600 (on Unix); names are `[a-z0-9-]` only; every write
goes to a temporary file of its own, is synced and renamed over the old one, so a file is the old one or the new
one, never half. What is kept there (by name constant):

- Secrets: the lists of accounts signed in, `steam-accounts` (`crates/platforms/steam/app/src/login.rs:24`) and
  `epic-accounts` (`crates/platforms/epic/app/src/accounts.rs:29`), each account's refresh token under a name of
  its own, a single-account Steam layout (`steam-refresh-token`, `steam-account`) moved into the list when
  found, `epic-client` (`:38`), and the DNS server's `dns-password` (`crates/server/src/dns.rs:20`).
- State: `lifetime` totals (`crates/app/src/lifetime.rs:18`), `verify` patrol place and last round
  (`crates/app/src/patrol.rs:18`), `auto-update` (`crates/app/src/auto_update.rs:27`), `dns` setup
  (`crates/server/src/dns.rs:18`), `steam-library`, `steam-lancache`, `epic-library-2`, `epic-sizes` (the
  platform apps' `KEPT` constants).
- Beside them, the settings changed in the web UI: `settings` (`FILE` `crates/server/src/settings.rs:183`),
  written by `LiveSettings` through a temp file and a rename, read back by `config` before the command-line flags
  (`crates/server/src/run/mod.rs:59`).
