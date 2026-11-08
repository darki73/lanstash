# lanstash-adapter-steam

Steam's connection manager (CM) as lanstash reaches it: a WebSocket over TLS (`wss://`), what Valve's client
uses.

## Provides

- `wss`: a TLS connection (rustls, Mozilla's root certificates) with the WebSocket opening handshake, binary
  messages both ways, pings answered; `https_get` for Steam's server list (`GetCMListForConnect`).
- `frame`: WebSocket frames (RFC 6455 section 5), client side: pure, tested on the RFC's examples.
- `link`: the `Link` and `Dialer` ports over `Wss` (a read timeout is `Ok(None)`, the connection stays usable).
- `random`: the `Random` port from ring's system generator.

The ignored test `tests/wss.rs` runs against Steam itself: it lists the CM servers, connects to one and sends
an anonymous logon, which is answered (a `Multi` message, EMsg 1).

## Third-party crates (approved for this crate alone, `crates/architecture` policy)

| Crate | Version | Licence |
|---|---|---|
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT |
| ring (rustls's crypto) | 0.17.14 | Apache-2.0 AND ISC |
| webpki-roots | 1.0.11 | CDLA-Permissive-2.0 |
| rustls-pki-types, rustls-webpki, untrusted | 1.15.1, 0.103.16, 0.9.0 | MIT OR Apache-2.0; ISC; ISC |
| once_cell, cfg-if, getrandom, zeroize | 1.21.4, 1.0.5, 0.2.17, 1.9.1 | MIT OR Apache-2.0 |
| subtle | 2.6.1 | BSD-3-Clause |
| cc, shlex, find-msvc-tools (build) | 1.11.0, 2.0.1, 0.1.15 | MIT OR Apache-2.0 |
