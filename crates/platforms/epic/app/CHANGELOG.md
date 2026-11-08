# Changelog

Notable changes to `lanstash-epic-app`.

## 0.1.0 (unreleased)

First version.

- `EpicAccounts`: several Epic accounts signed in with a code pasted from Epic's own page; refresh tokens kept among the secrets, access tokens renewed when needed.
- `EpicLibrary`: each account's games, their titles, images and download directories from Epic's catalog, kept between runs and refreshed.
- `EpicSizes`: each library game's download size, kept.
- `EpicBuilds`: the stashed games' current builds, refreshed when Epic's part of the stash changes.
- `EpicPlanner`: a game's current build fetched chunk by chunk from the CDN, each checked against the manifest's SHA-1.
- `EpicPruner`: what of a game no current build needs.
