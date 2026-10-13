//! Follows the gateway's live events and prints one line per event: agent
//! turns with their tokens and cost, tool calls, cron results and log
//! events. Reconnects when the gateway restarts.
//!
//! ```sh
//! ZEROCLAW_URL=… ZEROCLAW_TOKEN=… cargo run --example events
//! ```

use std::io::Write;
use std::time::Duration;

use futures_util::StreamExt;
use zeroclaw_api_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("ZEROCLAW_URL").unwrap_or_else(|_| "http://127.0.0.1:42617".into());
    let client = Client::new(url, std::env::var("ZEROCLAW_TOKEN")?)?;
    let mut out = std::io::stdout().lock();

    loop {
        let mut events = match client.events().subscribe().await {
            Ok(events) => events,
            Err(e) if e.is_transient() => {
                writeln!(out, "gateway unavailable ({e}); retrying")?;
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        while let Some(event) = events.next().await {
            let Ok(event) = event else { continue };
            let field = |name: &str| event.get(name).cloned().unwrap_or_default();
            match event.kind() {
                Some("agent_end") => writeln!(
                    out,
                    "turn    {} {} tokens ${}",
                    field("agent_alias"),
                    field("tokens_used"),
                    field("cost_usd")
                )?,
                Some("tool_call") => writeln!(
                    out,
                    "tool    {} {} ok={}",
                    field("agent_alias"),
                    field("tool"),
                    field("success")
                )?,
                Some("cron_result") => {
                    writeln!(out, "cron    {} ok={}", field("job_id"), field("success"))?
                }
                Some(other) => writeln!(out, "{other:<7} {}", event.0)?,
                None if event.is_log() => writeln!(out, "log     {}", field("message"))?,
                None => {}
            }
        }
        writeln!(out, "stream closed; reconnecting")?;
    }
}
