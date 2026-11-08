# Changelog

Notable changes to `lanstash-steam-domain`.

## 0.1.0 (unreleased)

First version.

- Which Steam requests are cached and how, cover art and icons, depot naming.
- Steam's connection-manager protocol: protobuf, message framing, logon, the QR and token sign-in, licenses, product info and content servers.
- KeyValues (text and binary) and depot manifests.
- Apps and their depots: which an install for an OS, language and architecture takes, shared and empty depots handled.
- The builds and catalog views, with each app's owning accounts.
- Chunk checks (CRC-32 and size) and sampling lancache's cache.
- `SteamPlatform`, the platform the composition root registers.
