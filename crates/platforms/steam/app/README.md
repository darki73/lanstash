# lanstash-steam-app

Steam's use cases over the ports: a connection to a Steam CM server
(`connection`: the server list, hello, Multi unpacked, service calls
matched by job id), signing in once with account, password and Steam Guard
and keeping the refresh token (`login`), signing in by QR code with the
Steam mobile app and no password (`qr_login`), the account the service
shows and its QR sign-in on a thread (`account`), and logging on with that token
(`logon`, with the heartbeat). The password is RSA-encrypted for Steam and
never kept; each account's token goes to the `Secrets` port.

For the running service: one shared session per account (`session`), the
accounts' merged library (`library`, read from Steam by `read`, kept between
runs by `raw`), manifests and their chunk lists (`manifests`,
`chunk_lists`), the stashed apps' builds (`builds`), the prefill planner
(`prefill`), pruning (`prune`) and how much of an app lancache's nginx cache
holds (`lancache`, `inspect`).

## Place in the workspace

- Layer: platform (steam).
- Depends on: `lanstash-domain`, `lanstash-ports`, `lanstash-steam-domain`.
- Dev-depends on: none.
- No I/O of its own: the link (`Dialer`/`Link`), the prompt, randomness and
  secrets are ports; the composition root wires the WebSocket adapter, the
  terminal and the secrets directory (`lanstash steam login|status|logout`
  and the service).

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file per module, against a scripted Steam
(`tests/support`).
