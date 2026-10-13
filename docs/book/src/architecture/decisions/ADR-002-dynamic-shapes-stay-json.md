---
id: ADR-002
title: Dynamic shapes stay JSON
date: 2026-10-13
status: accepted
relates-to:
  - crates/zeroclaw-api-client/src/sops.rs
  - crates/zeroclaw-api-client/src/config.rs
---

# ADR-002: Dynamic Shapes Stay JSON

## Context

Some gateway payloads are large, change with every ZeroClaw release, or
depend on the configuration: the whole-config snapshot and its JSON Schema,
SOP definitions, graphs and run overlays, quickstart submissions, skill front
matter, log and event frames, WebAuthn ceremonies.

## Decision

Those stay `serde_json::Value` in the client. Stable, small shapes (channels,
sessions, cron jobs, memory entries, cost, devices, …) are typed structs.
Typed structs decode leniently: unknown fields are ignored and optional or
newer ones are `Option` or defaulted.

## Consequences

- A ZeroClaw upgrade that reshapes an SOP or a config section does not break
  the client.
- Callers of the JSON-shaped calls index into the value themselves; the docs
  of each call list its top-level fields.
- A shape can move from `Value` to a struct later, as a breaking change, when
  it has stabilized upstream.
