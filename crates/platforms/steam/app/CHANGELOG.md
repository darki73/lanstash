# Changelog

Notable changes to `lanstash-steam-app`.

## 0.1.0 (unreleased)

First version.

- A connection to a Steam CM server: the server list, hello, Multi unpacked, requests matched to replies by job id.
- Sign-in by password (RSA-encrypted, Steam Guard) or by QR code with the Steam mobile app; several accounts, one refresh token each.
- One shared session per account, kept alive by a heartbeat and opened again when the connection breaks.
- The accounts' merged library: licensed apps and the depots their licenses grant, DLCs and shared depots included, kept between runs.
- Manifests and their chunk lists, kept between runs by depot and manifest.
- The stashed apps' builds and how much of each install the stash and lancache's nginx cache hold.
- The prefill planner: an app's install as unique chunks, each depot fetched as an account it is granted to.
- Pruning of segments no current manifest needs, and the automatic prune pass.
