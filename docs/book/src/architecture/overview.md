# Architecture

The crate is one library, `zeroclaw-api-client`, with one module per area of
the gateway.

## The client

`Client` holds a `reqwest::Client` (its connection pool), the base URL and
the bearer token, behind an `Arc`: clones are cheap and share connections.
`ClientBuilder` sets the token, a timeout or a `reqwest::Client` of the
caller's.

Requests are built with the `reqwest::Client` and sent through a tower
service stack (`transport.rs`, [ADR-004](./decisions/ADR-004-tower-stack.md)):
the caller's layers, an optional concurrency limit, retry and a deadline,
around reqwest. All HTTP goes through a few `pub(crate)` helpers in
`client.rs`:

- `url(segments, query)` appends **path segments**, each percent-encoded, to
  the base URL. Ids are never formatted into a path string, so an id holding
  `/` or `?` stays one segment;
- `request` attaches the bearer token;
- `send` passes the request through the stack and turns a non-success status
  into `Error::Api`, keeping the gateway's message and JSON body;
- `json`, `text`, `empty`, `get` and `send_json` decode.

## Resource accessors

Each module adds an accessor to `Client` that borrows it and returns a small
`Copy` struct (`ConfigApi<'_>`, `ChannelsApi<'_>`, …) whose async methods are
the endpoints. `client.agent(alias)` also carries the agent alias. A method's
doc comment starts with the route it calls, which the
[endpoint map](../reference/endpoints.md) is generated from.

## Streams

Two kinds of live data are `Stream`s the caller drives (the client spawns no
tasks):

- **Server-Sent Events** (`events().subscribe()`): `events.rs` splits the
  byte stream into frames and decodes their JSON;
- **WebSockets** (`chat.rs`): `connect` opens a socket with ZeroClaw's
  `zeroclaw.v1` subprotocol and the bearer token, turning a refused handshake
  into `Error::Api`. The chat reads typed `ChatFrame`s; the canvas and SOP-run
  feeds are plain JSON frame streams.

## Types

Response types mirror the gateway's serialized structs and `json!` literals,
field for field, and name them in their docs. Shapes that are dynamic or
versioned with ZeroClaw stay `serde_json::Value`
([ADR-002](./decisions/ADR-002-dynamic-shapes-stay-json.md)).
