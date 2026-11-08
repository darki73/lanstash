# Changelog

Notable changes to `lanstash-adapter-http`.

## 0.1.0 (unreleased)

First version.

- The HTTP server that hands requests, with the client's address, to a `RequestHandler`.
- Stored bodies sent from the file to the socket by `sendfile` where the system allows.
- The client that reaches the origins; a connect or read that ran out of time is marked as a timeout.
- The HTTPS passthrough that relays TLS by its server name.
- `HttpWeb`: a client for web APIs on the network, with HTTPS (Mozilla's roots, a pinned certificate or unchecked).
