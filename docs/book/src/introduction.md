# ZeroClaw API Client

`zeroclaw-api-client` is a typed, async Rust client for the
[ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) gateway: the HTTP and
WebSocket surface a ZeroClaw daemon exposes for its dashboard, its CLI and
third-party tooling.

With it an application can do everything the gateway allows a client to do:

- read and change the configuration, one field at a time or in atomic
  batches, including agents, providers and channels;
- pair new clients and manage the paired devices;
- see channels, send through them, and manage WhatsApp group chats and their
  members;
- chat with an agent, streaming its reply, tool calls and approval requests;
- follow the live event stream and read session transcripts;
- schedule cron jobs, store and recall memory, and read usage and cost;
- edit agent workspaces, personality files, skills and SOPs, and run SOPs;
- check health, run diagnostics, read logs and upgrade the daemon.

It is a library: it never prints, logs or spawns tasks. Every call returns a
typed result or a typed [error](./guides/errors.md) carrying the gateway's own
explanation.

## How it is organized

A [`Client`](./architecture/overview.md) holds the gateway URL and the bearer
token. Each group of endpoints is reached through an accessor, so call sites
read like the gateway's routes:

```rust,no_run
# async fn demo() -> zeroclaw_api_client::Result<()> {
use zeroclaw_api_client::Client;

let client = Client::new("http://127.0.0.1:42617", "zc_…")?;

let channels = client.channels().list().await?;
let rooms = client.channels().rooms("whatsapp.main").await?;
client.config().set("agents.helper.model", "gpt-5.4-mini", None).await?;
let reply = client.chat("helper", Default::default()).await?.ask("Hi").await?;
# Ok(()) }
```

Start with [Getting Started](./getting-started/index.md). The
[endpoint map](./reference/endpoints.md) lists every route and the function
that calls it.
