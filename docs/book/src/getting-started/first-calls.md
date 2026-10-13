# First calls

```rust,no_run
use zeroclaw_api_client::Client;

#[tokio::main]
async fn main() -> zeroclaw_api_client::Result<()> {
    let client = Client::new("http://127.0.0.1:42617", std::env::var("ZEROCLAW_TOKEN").unwrap())?;

    // Public: no token needed.
    let health = client.system().health().await?;

    // Version, model, channels and process figures.
    let status = client.system().status(None).await?;

    // Every configured channel and its state.
    for channel in client.channels().list().await? {
        let _ = (channel.name, channel.status, channel.health);
    }

    // One configuration field.
    let port = client.config().get("gateway.port").await?;
    Ok(())
}
```

## Timeouts and retries

A request waits 60 seconds for the gateway's answer unless the builder says
otherwise (`ClientBuilder::timeout`); a manual cron run waits up to 660, as
the gateway allows it 600. The timeout bounds the answer's headers only, so
the live event stream is never cut. Failed idempotent requests are retried
three times with backoff. See [Resilience](../guides/resilience.md).

## Examples

The crate ships runnable examples under
`crates/zeroclaw-api-client/examples/`:

| Example | Shows |
|---|---|
| `status` | version, model and channels |
| `pairing` | exchanging a pairing code for a token |
| `config` | reading fields, listing agents, an atomic patch |
| `group_chats` | channels, their group chats and members |
| `events` | following the live event stream, reconnecting |
| `chat` | streaming an agent's reply over the WebSocket |
| `announce` | posting on a channel through a one-off cron job |
| `resilient` | timeouts, retries, a concurrency limit and a custom tower layer |

```sh
ZEROCLAW_URL=http://127.0.0.1:42617 ZEROCLAW_TOKEN=zc_… \
  cargo run --example status
ZEROCLAW_URL=… ZEROCLAW_TOKEN=… cargo run --example chat -- helper "Hello"
```
