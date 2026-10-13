# AGENTS.md - ZeroClaw API client

Core instructions for AI coding assistants working in this repository. Read `docs/book/src/contributing/architecture.md` before a non-trivial change.

## Single Source Of Truth

The gateway is the source of truth for every request and response shape. This SDK mirrors the router of the ZeroClaw build it targets (`crates/zeroclaw-gateway/src/lib.rs` in ZeroClaw) and never invents fields:

1. Before adding or changing a type, find the gateway handler and the struct (or `json!` literal) it serializes, and name it in the type's doc comment.
2. Shapes the gateway builds ad hoc, or that change with configuration (config sections, SOP definitions, catalogs), stay `serde_json::Value` instead of guessed structs.
3. Optional and newer fields are `Option<_>` or `#[serde(default)]`, so an older or newer gateway still decodes.

## Safety And Privacy

- Never commit secrets, tokens, credentials, personal data, or real identities. Tests use invented tokens and hosts.
- The bearer token never appears in `Debug` output, errors, or URLs.
- Production paths propagate errors. Avoid `unwrap()` and `expect()` outside tests.
- Do not suppress unused production code with underscore names or `#[allow(dead_code)]`; remove it or use it.
- A library never prints, logs, or spawns tasks: it returns typed errors and leaves streams to the caller (`clippy.toml` enforces it).

## Working Rules

1. Read the owning module, its tests, and the matching gateway handler before editing.
2. One resource group per module under `crates/zeroclaw-api-client/src/`, reached from a `Client` accessor; path segments go through `Client::url`, never string formatting, so ids are always encoded.
3. Keep one concern per PR. Avoid unrelated cleanup and do not mix broad formatting changes with functional changes.
4. Do not add dependencies for minor convenience, speculative abstractions, or feature flags without a concrete use.
5. Every endpoint gets a test at the HTTP boundary (`tests/`, against a mock gateway) that checks method, path, query, body, and decoding. Logic (parsing, mapping, decisions) gets unit tests written as vector tables: a list of inputs and expected outputs, one assertion loop. New response types get a captured vector (`scripts/capture-vectors.py`).
6. Until the first crates.io release, `master` is the only branch: commit to it directly, each commit passing the full validation below. After that release, work on a non-`master` branch and open a PR to `master`.
7. Use conventional commits. Do not add bot or AI attribution footers.
8. Name no application, company or deployment that uses the client: not in code, docs, test vectors, commits or PRs. Say "an application". Capture vectors with `ZEROCLAW_REDACT` set to your own names.

## User-Facing Text

- Code, comments, errors, and documentation are in English.
- English Markdown under `docs/book/` is the documentation source of truth; rustdoc covers each public item.

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo deny check
```

Docs changes also build the book: `mdbook build docs/book`.
