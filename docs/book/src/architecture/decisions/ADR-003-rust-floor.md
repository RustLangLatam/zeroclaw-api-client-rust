---
id: ADR-003
title: Rust 1.89 floor
date: 2026-10-13
status: accepted
relates-to:
  - Cargo.toml
---

# ADR-003: Rust 1.89 Floor

## Context

ZeroClaw itself requires Rust 1.96. The client is meant to be embedded in
other applications, and those often pin older toolchains than the daemon
they talk to.

## Decision

The workspace declares `rust-version = "1.89"`, the lowest version its
dependencies allow, and CI checks it. The client uses edition 2024, which
1.89 supports.

## Consequences

- New dependencies, or newer versions of the current ones, must build on
  1.89, or the floor moves in a recorded change.
- Language features newer than 1.89 are not used.
