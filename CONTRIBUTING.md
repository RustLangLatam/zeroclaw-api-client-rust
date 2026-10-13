# Contributing to zeroclaw-api-client

Thanks for your interest. Every kind of contribution helps: code, docs, bug
reports and reports of gateway changes the client does not follow yet.

All PRs target **`master`**; never push to it directly.

1. **Find the gateway side.** Every change starts from the ZeroClaw build the
   client targets ([Compatibility](docs/book/src/reference/compatibility.md)):
   the route in `crates/zeroclaw-gateway/src/lib.rs` and its handler.
2. **Branch.** `feat/*`, `fix/*` or `docs/*` from `master`.
3. **Change, test, document.** Follow [AGENTS.md](AGENTS.md) and the
   step-by-step guide in
   [docs/book/src/contributing/architecture.md](docs/book/src/contributing/architecture.md).
4. **Open the PR** with the template filled in and conventional commit titles
   (`feat: …`, `fix: …`, `docs: …`). Do not add bot or AI attribution
   footers, and name no application or company that uses the client. Add
   what changed for users to `CHANGELOG-next.md`.

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

Against a running gateway, the read-only live test checks that every type
decodes real answers:

```sh
ZEROCLAW_URL=http://127.0.0.1:42617 ZEROCLAW_TOKEN=zc_… \
  cargo test --test live -- --ignored --nocapture
```
