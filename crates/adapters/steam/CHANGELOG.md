# Changelog

Notable changes to `lanstash-adapter-steam`.

## 0.1.0 (unreleased)

First version.

- `wss`: a WebSocket over TLS to Steam's CM servers (rustls, Mozilla's roots), pings answered.
- `https_get` for Steam's CM server list.
- `frame`: client-side WebSocket frames (RFC 6455).
- `link`: the `Link` and `Dialer` ports over `Wss`; a read timeout keeps the connection usable.
- `random`: the `Random` port from ring's system generator.
