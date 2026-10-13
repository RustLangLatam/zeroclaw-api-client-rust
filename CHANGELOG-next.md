# zeroclaw-api-client v0.1.0

The first release: an independent (not affiliated with ZeroClaw Labs), typed async client for the whole client-facing surface
of the ZeroClaw 0.8.5 gateway, verified against the RustLangLatam fork's
production build `0.8.5+unified.13`.

## Highlights

- **Every client route:** 150 calls over configuration, pairing, channels and
  group chats, sessions, events, agents, skills, cron, memory, cost, SOPs,
  canvases, the quickstart, the webhook, A2A and WebAuthn.
- **Live data as streams:** the Server-Sent Events feed, the agent chat
  WebSocket with typed frames, and the canvas and SOP-run feeds.
- **Resilient by default, built on tower:** every request goes through a
  tower stack: an answer timeout that never cuts a stream, retries with
  exponential backoff for idempotent calls only, an optional shared
  concurrency limit, and any tower layer of the caller's.
- **Errors that keep the gateway's reason:** one `Error` type reads the
  gateway's several error shapes into a message and keeps the JSON body.
- **Safe by construction:** path segments are always percent-encoded, and the
  bearer token never appears in `Debug` output.

## What's New

- `Client` and `ClientBuilder`: base URL (with path prefixes), bearer token,
  timeout or a caller's `reqwest::Client`, `with_token` after pairing.
- Modules `config`, `pairing`, `channels`, `chat`, `sessions`, `events`,
  `agents`, `files`, `skills`, `cron`, `memory`, `cost`, `sops`, `canvas`,
  `quickstart`, `system`, `webhook` and `webauthn`.
- `ClientBuilder::{timeout, no_timeout, retry, concurrency_limit, layer}`,
  `RetryPolicy`, `Error::{Timeout, Layer}` and `Error::is_transient`.
- A connection the gateway drops before answering (it does while it
  restarts) is retried for idempotent calls and counts as transient.
- Live feeds (`events().subscribe()`, `canvas().watch()`,
  `sops().watch_runs()`) are boxed streams: `Unpin`, ready for `.next()`.
- Examples: `status`, `pairing`, `config`, `group_chats`, `events`, `chat`,
  `announce`, `resilient`.
- Tests: vector tables for every piece of logic, HTTP-boundary tests for every
  call and for retries, timeouts, the concurrency limit and layers, a WebSocket
  test gateway, response vectors captured from a real gateway, and an opt-in
  read-only test against a live one.
- The book under `docs/book`: getting started, guides, endpoint map,
  compatibility, architecture and ADRs.
