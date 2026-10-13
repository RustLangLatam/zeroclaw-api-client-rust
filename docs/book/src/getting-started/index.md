# Getting Started

## Install

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
zeroclaw-api-client = { git = "https://github.com/RustLangLatam/zeroclaw-api-client-rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The client is async and runs on Tokio. It needs Rust 1.89 or newer
([why](../architecture/decisions/ADR-003-rust-floor.md)).

## What you need from the gateway

- **Its URL.** The gateway listens on `127.0.0.1:42617` by default. Behind a
  reverse proxy, or with `[gateway] path_prefix`, use the full base URL
  (`https://host/zeroclaw`): the client appends routes to it.
- **A bearer token**, unless the gateway runs with pairing off. Tokens come
  from pairing: see [Connecting and pairing](./pairing.md).
- **Remote admin rights**, only for `/admin/*` calls from another host:
  those need `[gateway] allow_remote_admin = true` and pairing on.

Then make your [first calls](./first-calls.md).
