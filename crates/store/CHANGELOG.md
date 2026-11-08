# Changelog

Notable changes to `lanstash-store`.

## 0.1.0 (unreleased)

First version.

- Small whole objects in segments, large or partial objects in sparse object files described by a journal.
- One in-memory index over all of it, with lookups that do not mark an object as used.
- A change count per service, so callers know when what they worked out is stale.
- Eviction candidates and a verifier.
- A game's object files with their keys, for pruning.
- Feature `test-support`: in-memory storage doubles and an nginx cache double.
