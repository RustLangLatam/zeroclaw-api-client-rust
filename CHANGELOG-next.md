# zeroclaw-api-client — next release

Changes since v0.1.0. They become the release notes of the next version.

## What's New

- Install from crates.io; badges for crates.io, docs.rs and releases.
- The README links to the Go port,
  [zeroclaw-api-client-go](https://github.com/RustLangLatam/zeroclaw-api-client-go).
- `webhook().agent_catalog()`: the gateway's A2A catalog card
  (`GET /.well-known/agents-card.json`).
- `scripts/route-drift.py` and the Route Drift workflow: CI fails when the
  targeted gateway gains a route the client does not call, or drops one it
  does.
- The endpoint map lists which gateway Cargo features each feature-gated
  method needs (`a2a`, `webauthn`, `observability-prometheus`).

## Breaking Changes

None.
