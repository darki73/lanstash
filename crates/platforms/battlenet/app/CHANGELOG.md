# Changelog

Notable changes to `lanstash-battlenet-app`.

## 0.1.0 (unreleased)

First version.

- The product list, one entry per install with its family, product codes looked up in parallel.
- Each game's live and pre-release builds, with the other products installed from its directory (flavours).
- Install plans, read from the CDN for a game the stash does not hold.
- The launcher catalog, which finds a game's product by its CDN directory or product code.
- How much of a game's current build lancache's nginx cache holds, probed in the background.
- The prefill planner: CDN downloads checked against the hashes in the files' names before they are stored.
- Pruning of what no current or pre-release build needs, and the automatic prune pass.
