# Changelog

Notable changes to `lanstash-app`.

## 0.1.0 (unreleased)

First version.

- Serving client requests from the stash or the origin, with pass-through, refused hosts and per-client, per-game traffic.
- Coordinated downloads, eviction, verification and a background store patrol.
- Importing lancache's nginx cache and taking prefills from it.
- A generic prefill runner with a queue, checked CDN downloads and retries on passing errors.
- Game deletion and pruning of segments no current build needs.
- Status, health checks, kept builds per platform and the update history.
- Counters kept by the minute, lifetime totals and the prefill runs that ended. Counted besides: responses
  the client left, bytes sent by `sendfile` and copied; the origin's error statuses, timeouts and wait for
  its first answer; pass-through failures; segments, object files and objects held.
- DNS: the server's rewrites compared with the names lanstash wants, applied and checked.
