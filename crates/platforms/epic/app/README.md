# lanstash-epic-app

Epic Games' use cases over the ports: the accounts lanstash signs in to
(`accounts`). A person signs in on Epic's own page and pastes the one-time
code it shows; lanstash trades it for tokens, keeps each account's refresh
token among the secrets and renews its access token when one is needed.
Several accounts may be signed in. Their library (`library`): each account's
games, read as that account, their titles and images from Epic's catalog,
kept between runs (`saved`). Each game's download size (`sizes`), the
stashed games' builds (`builds`), the prefill planner, its chunks checked
against the manifest's SHA-1 (`prefill`), and pruning (`prune`).

The requests name the Epic Games launcher's own public client (built in),
unless another is set from the account page
(kept as the secret `epic-client`, `<id>:<secret>`).

## Place in the workspace

- Layer: platform (epic).
- Depends on: `lanstash-domain`, `lanstash-ports`, `lanstash-epic`.
- Dev-depends on: none.
- `std`, but no I/O of its own: Epic's web APIs, the secrets and the clock are ports.

The dependency, import, size, test-mirror and documentation rules are
checked by `crates/architecture` (verified: `cargo test -p lanstash-architecture`).

## Tests

`tests/` mirrors `src/`: one test file (or directory) per module.
