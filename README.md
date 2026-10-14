<h1 align="center">🦀 zeroclaw-api-client</h1>

<p align="center">
  <strong>Your ZeroClaw, from Rust. Typed calls, live streams, resilient by default.</strong>
</p>

<p align="center">
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/RustLangLatam/zeroclaw-api-client-rust/ci.yml?branch=master&label=build" alt="Build Status" /></a>
  <a href="https://crates.io/crates/zeroclaw-api-client"><img src="https://img.shields.io/crates/v/zeroclaw-api-client?logo=rust" alt="crates.io" /></a>
  <a href="https://docs.rs/zeroclaw-api-client"><img src="https://img.shields.io/docsrs/zeroclaw-api-client?label=docs.rs" alt="docs.rs" /></a>
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/releases/latest"><img src="https://img.shields.io/github/v/release/RustLangLatam/zeroclaw-api-client-rust?label=release" alt="Latest release" /></a>
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/LICENSE-APACHE"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache%202.0-blue.svg" alt="License" /></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-edition%202024-orange?logo=rust" alt="Rust Edition 2024" /></a>
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/architecture/decisions/ADR-003-rust-floor.md"><img src="https://img.shields.io/badge/MSRV-1.89-orange?logo=rust" alt="MSRV 1.89" /></a>
  <a href="https://github.com/zeroclaw-labs/zeroclaw"><img src="https://img.shields.io/badge/ZeroClaw-0.8.5-black" alt="ZeroClaw 0.8.5" /></a>
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/graphs/contributors"><img src="https://img.shields.io/github/contributors/RustLangLatam/zeroclaw-api-client-rust?color=green" alt="Contributors" /></a>
</p>

<p align="center">
  <a href="https://tokio.rs"><img src="https://img.shields.io/badge/async-Tokio-463f5c?logo=rust" alt="Tokio" /></a>
  <a href="https://github.com/tower-rs/tower"><img src="https://img.shields.io/badge/middleware-tower-8b5cf6" alt="tower" /></a>
  <a href="https://github.com/seanmonstar/reqwest"><img src="https://img.shields.io/badge/HTTP-reqwest-2563eb" alt="reqwest" /></a>
  <a href="https://github.com/rustls/rustls"><img src="https://img.shields.io/badge/TLS-rustls-0f766e" alt="rustls" /></a>
  <a href="https://github.com/snapview/tokio-tungstenite"><img src="https://img.shields.io/badge/WebSocket-tokio--tungstenite-475569" alt="tokio-tungstenite" /></a>
  <a href="https://serde.rs"><img src="https://img.shields.io/badge/serialization-serde-c2410c" alt="serde" /></a>
  <a href="https://github.com/EmbarkStudios/cargo-deny"><img src="https://img.shields.io/badge/supply%20chain-cargo--deny-15803d" alt="cargo-deny" /></a>
</p>

<p align="center">
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/introduction.md">Docs</a> ·
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/getting-started/index.md">Quick start</a> ·
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/guides/index.md">Guides</a> ·
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/guides/resilience.md">Resilience</a> ·
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/reference/endpoints.md">Endpoint map</a> ·
  <a href="https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/architecture/overview.md">Architecture</a> ·
  <a href="https://github.com/zeroclaw-labs/zeroclaw">ZeroClaw</a>
</p>

---

`zeroclaw-api-client` is a typed, resilient, async Rust client for the
[ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) gateway: its HTTP API,
its Server-Sent Events stream and its WebSocket chat.

> An independent, community-maintained client. It is not affiliated with or
> endorsed by ZeroClaw Labs; ZeroClaw is a trademark of ZeroClaw Labs.

```sh
cargo add zeroclaw-api-client
cargo add tokio --features macros,rt-multi-thread
```

```rust,no_run
use zeroclaw_api_client::Client;

#[tokio::main]
async fn main() -> zeroclaw_api_client::Result<()> {
    let client = Client::new("http://127.0.0.1:42617", "zc_…")?;

    let status = client.system().status(None).await?;
    let channels = client.channels().list().await?;
    client.config().set("agents.helper.model", "gpt-5.4-mini", None).await?;

    let mut chat = client.chat("helper", Default::default()).await?;
    let reply = chat.ask("How many orders are pending?").await?;
    Ok(())
}
```

## Why an SDK

ZeroClaw is a self-hosted AI agent runtime. Everything an operator can do
with it (configure agents, models and channels, pair devices, run SOPs,
schedule jobs, read sessions and costs, talk to an agent) is exposed by its
gateway over HTTP and WebSockets. Calling that surface by hand from every
application that embeds ZeroClaw means repeating, in each of them:

- **the contract**: about 150 routes, a dozen response shapes for errors, a
  JSON Patch dialect for the configuration, an event stream whose frames have
  no names, a chat protocol with typed frames;
- **the edge cases**: secrets that are never returned, paths that must be
  encoded segment by segment, a gateway that drops connections while it
  reloads, routes that only exist with some build features;
- **the resilience**: timeouts that must not cut a stream, retries that must
  never repeat a write, back-pressure.

This crate does it once, typed and tested, so applications talk to ZeroClaw
in terms of channels, agents and sessions instead of URLs and JSON. When the
gateway changes, one place changes.

It complements what ZeroClaw ships: the gateway's own OpenAPI document
(`/api/openapi.json`, browsable at `/api/docs`) describes part of the router,
and `zerocode` speaks ACP over `/acp`; this client covers the whole REST,
event and chat surface from Rust.

## Features

- **The whole client surface** of the gateway, one module per area:

  | Module | Covers |
  |---|---|
  | `config` | per-field reads and writes, atomic JSON Patch, map aliases (agents, providers, channels), drift, catalogs, setup sections |
  | `pairing` | pairing codes, bearer tokens, paired devices |
  | `channels` | channel state, sending, group chats and their members, identity binding, relinking |
  | `chat` | agent chat over WebSocket: streamed replies, tool calls, tool approvals |
  | `sessions`, `events` | transcripts, session state, the live Server-Sent Events stream |
  | `agents`, `files`, `skills` | agent workspaces, personality files, shared files, image uploads, skill bundles |
  | `cron`, `memory`, `cost` | scheduled jobs, long-term memory, usage and cost |
  | `sops`, `canvas`, `quickstart` | SOP authoring, runs and approvals; canvases; the first-agent setup |
  | `system` | health, status, metrics, diagnostics, tools, logs, upgrades, reload |
  | `webhook`, `webauthn` | one-shot webhook and A2A messages; hardware-key ceremonies |

- **Resilient by default**, built on [tower](https://github.com/tower-rs/tower):
  answer timeouts that never cut a stream, retries with exponential backoff
  for idempotent calls only, an optional shared concurrency limit, and your
  own tower layers (metrics, tracing, signing, circuit breaking) around every
  request.
- **Typed errors** that keep the gateway's status, its explanation and its
  JSON body, with `is_not_found()`, `is_unauthorized()`, `is_rate_limited()`
  and `is_transient()`.
- **Streams, not callbacks**: the event feed and the canvas and SOP-run
  feeds are `Stream`s you drive, and a chat is a session you pull frames
  from (`next_frame()`); the client spawns no tasks.
- **Safe by construction**: every path segment is percent-encoded, the token
  never appears in `Debug` output, nothing is printed or logged.

## Technologies

| Concern | Choice |
|---|---|
| Language | Rust, edition 2024, MSRV 1.89 |
| Async runtime | [Tokio](https://tokio.rs) |
| HTTP | [reqwest](https://github.com/seanmonstar/reqwest) with [rustls](https://github.com/rustls/rustls) (no OpenSSL) |
| Middleware | [tower](https://github.com/tower-rs/tower): `Service`, `Layer`, retry, concurrency limit, boxed services |
| WebSockets | [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite) (`zeroclaw.v1` subprotocol) |
| Serialization | [serde](https://serde.rs) and serde_json |
| Errors | [thiserror](https://github.com/dtolnay/thiserror) |
| URLs | [url](https://github.com/servo/rust-url) |
| Streams | [futures-util](https://github.com/rust-lang/futures-rs) |
| Tests | [wiremock](https://github.com/LukeMathWalker/wiremock-rs), an in-test WebSocket gateway, captured response vectors, vector tables |
| Docs | rustdoc and an [mdBook](https://rust-lang.github.io/mdBook/) book |
| Supply chain | [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) (advisories, licenses, sources) |

## Architecture

```text
 your code
    │  client.channels().rooms("whatsapp.main")
    ▼
 Client ── resource accessors (config, channels, chat, …) ── typed requests/responses
    │
    ├─ HTTP ──► your tower layers ─► concurrency limit ─► retry ─► deadline ─► reqwest ─► gateway
    ├─ SSE ───► the same stack; the body is read as a Stream of events
    └─ WebSocket ─► tokio-tungstenite (chat, canvas and SOP-run feeds)
```

Each method's documentation starts with the route it calls; the
[endpoint map](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/reference/endpoints.md) is generated from them.
See [Architecture](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/architecture/overview.md) and the
[decision records](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/architecture/decisions/index.md).

## Resilience

```rust,no_run
use std::time::Duration;
use zeroclaw_api_client::{Client, RetryPolicy};

# fn demo() -> zeroclaw_api_client::Result<()> {
let client = Client::builder("http://127.0.0.1:42617")
    .token("zc_…")
    .timeout(Duration::from_secs(10))       // wait for an answer at most 10 s
    .retry(RetryPolicy::new(5, Duration::from_millis(250), Duration::from_secs(8)))
    .concurrency_limit(4)                   // shared by every clone
    .layer(tower::util::MapRequestLayer::new(|request: reqwest::Request| request))
    .build()?;
# Ok(()) }
```

Retries apply to `GET`, `HEAD`, `OPTIONS`, `PUT` and `DELETE` after a
connection failure, a connection dropped before the answer, a timeout, or a
408, 429, 502, 503 or 504. A `POST` or
`PATCH` is never repeated. The timeout bounds the wait for the answer's
headers, so the event stream stays open. See the
[resilience guide](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/guides/resilience.md).

## Examples

Runnable programs under `crates/zeroclaw-api-client/examples/`:

| Example | Shows |
|---|---|
| `status` | version, model and channels |
| `pairing` | exchanging a pairing code for a token |
| `config` | reading fields, listing agents, an atomic patch |
| `group_chats` | channels, their group chats and members |
| `events` | following the live event stream, reconnecting |
| `chat` | streaming an agent's reply over the WebSocket |
| `announce` | posting on a channel through a one-off cron job |
| `resilient` | timeouts, retries, a concurrency limit and a custom tower layer |

```sh
ZEROCLAW_URL=http://127.0.0.1:42617 ZEROCLAW_TOKEN=zc_… cargo run --example status
```

## Testing

- **Unit tests with vector tables** for every piece of logic: error mapping,
  URL encoding, SSE parsing, chat frames, retry decisions, backoff, patch
  operations, schedules.
- **HTTP-boundary tests** for every endpoint against a mock gateway: method,
  path, query, body and decoding; plus retry, timeout, concurrency and layer
  behavior.
- **Response vectors**: answers captured from a real gateway
  (`scripts/capture-vectors.py`, anonymized) must decode into the SDK's types
  and re-encode without loss.
- **A live, read-only test** against a running gateway:
  `cargo test --test live -- --ignored`.

## Compatibility

Targets the ZeroClaw **0.8.5** gateway; see
[Compatibility](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/docs/book/src/reference/compatibility.md) for the builds it is
verified against and the routes that need a newer one.

## Documentation

- The book: `mdbook serve docs/book` — getting started, guides, reference,
  architecture and decisions.
- The API: [docs.rs/zeroclaw-api-client](https://docs.rs/zeroclaw-api-client).

## Other languages

The same client, with the same surface, behavior and tests:

| Language | Repository | Install |
|---|---|---|
| Go | [zeroclaw-api-client-go](https://github.com/RustLangLatam/zeroclaw-api-client-go) | `go get github.com/RustLangLatam/zeroclaw-api-client-go` |
| Python | [zeroclaw-api-client-python](https://github.com/RustLangLatam/zeroclaw-api-client-python) | `pip install zeroclaw-api-client` |

## Contributing

Work happens on branches with pull requests to `master`; see
[CONTRIBUTING.md](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/CONTRIBUTING.md) and [AGENTS.md](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/AGENTS.md).

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/LICENSE-APACHE) or
[MIT license](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/LICENSE-MIT), at your option.
