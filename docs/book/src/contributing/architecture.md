# Contributing

Read [`AGENTS.md`](https://github.com/RustLangLatam/zeroclaw-api-client-rust/blob/master/AGENTS.md)
first: it holds the rules every change follows. This page explains the most
common change, covering a gateway route.

## Adding or changing an endpoint

1. **Find the handler.** In the ZeroClaw build listed in
   [Compatibility](../reference/compatibility.md), find the route in
   `crates/zeroclaw-gateway/src/lib.rs` and read its handler: auth, query,
   body, every status it answers, and the struct or `json!` it returns.
2. **Pick the module.** Routes belong to the module of their area (`config`,
   `channels`, …). A new area gets a new module with an accessor on `Client`.
3. **Write the method.** Its doc comment starts with the route
   (`` /// `POST /api/cron/{id}/run`: … ``). Build the path with
   `Client::url` segments, never string formatting. Unwrap single-field
   envelopes (`{"jobs": […]}` returns `Vec<CronJob>`).
4. **Write the types.** Mirror the gateway's fields; `Option` or
   `#[serde(default)]` for anything optional or newer; `serde_json::Value`
   for dynamic shapes ([ADR-002](../architecture/decisions/ADR-002-dynamic-shapes-stay-json.md)).
5. **Test it.** Add the call to the module's test in
   `crates/zeroclaw-api-client/tests/`, against the mock gateway: assert the
   method, path, query and body the gateway expects, and decode a body copied
   from the handler. Capture a response vector for new types (see
   [Testing](./testing.md)).
6. **Update the docs.** Regenerate the [endpoint map](../reference/endpoints.md)
   with `python3 scripts/endpoint-map.py --write` (CI fails while it is
   stale), and update the guide of the area.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo +1.89 check --all-targets
cargo deny check
mdbook build docs/book
```

## Branches and pull requests

Until the first crates.io release, `master` is the only branch: commit to it
directly, every commit passing the checks above. After it, branch from
`master` (`feat/*`, `fix/*`, `docs/*`) and open a PR with the template filled
in. Use conventional commits.
