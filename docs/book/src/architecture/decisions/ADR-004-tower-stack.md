---
id: ADR-004
title: Requests go through a tower stack
date: 2026-10-13
status: accepted
relates-to:
  - crates/zeroclaw-api-client/src/transport.rs
  - docs/book/src/guides/resilience.md
---

# ADR-004: Requests Go Through A Tower Stack

## Context

A ZeroClaw gateway restarts on reloads and some configuration changes,
refusing connections for seconds; it answers 502 and 503 while a channel
reconnects and 429 when rate-limited; some calls take minutes. Applications
need timeouts, retries and back-pressure, and often their own cross-cutting
concerns (metrics, tracing). Hand-written loops in each client method would
repeat that logic and could not be extended by callers.

## Decision

Every HTTP request is sent through a `tower::Service` stack built once per
client: the caller's layers, an optional global concurrency limit, a retry
policy and a deadline, around `reqwest::Client`, which implements
`tower::Service` itself. The stack is a `BoxCloneSyncService`, so the client
stays `Clone + Send + Sync` and callers add layers without generic types
leaking into the API.

- **Retry** only for idempotent methods and failures a retry can fix, with
  exponential backoff; never a `POST` or `PATCH`.
- **Deadline** is a small layer of the crate's own rather than
  `tower::timeout::Timeout`: it bounds the response headers only, so streams
  are not cut, and honors a per-request timeout for long gateway calls.
- **No `Buffer`, no rate limiter of the crate's own:** they need a background
  task, and the client spawns none. Callers who want them add them as layers.

## Consequences

- Resilience is configured, not coded per call, and covered by vector tests
  of each decision (method, status, attempt).
- Callers get tower's ecosystem for free through `ClientBuilder::layer`.
- WebSockets are outside the stack: a dropped chat or feed is the caller's to
  reopen.
