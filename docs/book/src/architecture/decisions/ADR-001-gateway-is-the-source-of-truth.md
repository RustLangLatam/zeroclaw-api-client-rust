---
id: ADR-001
title: The gateway router is the source of truth
date: 2026-10-13
status: accepted
relates-to:
  - docs/book/src/reference/compatibility.md
  - docs/book/src/reference/endpoints.md
---

# ADR-001: The Gateway Router Is The Source Of Truth

## Context

ZeroClaw publishes an OpenAPI document (`/api/openapi.json`), but it covers
only the configuration, version, SOP and skill routes: about a tenth of the
gateway. The rest is defined by the router in
`crates/zeroclaw-gateway/src/lib.rs` and its handlers, many of which build
their answers with `serde_json::json!`.

## Decision

The client is written from the router and handlers of the build it targets,
not from the OpenAPI document and not from observed traffic. Every request
and response type names, in its documentation, the gateway handler and the
struct or literal it mirrors. When the gateway changes, the client changes
the same way, against a named build listed in
[Compatibility](../../reference/compatibility.md).

## Consequences

- Types match the gateway field for field, including its inconsistencies
  (several error shapes, `type` versus `@timestamp` event frames).
- Generating the client from OpenAPI is not possible until upstream covers
  the whole router; this decision can be revisited then.
- Supporting a new ZeroClaw build means re-reading its router diff.
