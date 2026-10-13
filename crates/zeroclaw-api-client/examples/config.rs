//! Reads configuration fields, lists the agents, and, with `--set`, changes
//! an agent's model atomically: the patch only applies if the model is still
//! the one read.
//!
//! ```sh
//! ZEROCLAW_URL=… ZEROCLAW_TOKEN=… cargo run --example config
//! ZEROCLAW_URL=… ZEROCLAW_TOKEN=… cargo run --example config -- --set helper gpt-5.5
//! ```

use std::io::Write;

use zeroclaw_api_client::Client;
use zeroclaw_api_client::config::{PatchOp, PropValue};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("ZEROCLAW_URL").unwrap_or_else(|_| "http://127.0.0.1:42617".into());
    let client = Client::new(url, std::env::var("ZEROCLAW_TOKEN")?)?;
    let config = client.config();
    let mut out = std::io::stdout().lock();

    for entry in config.list(Some("gateway")).await?.entries.iter().take(8) {
        let value = entry
            .value
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or("<secret>".into());
        writeln!(out, "{:<40} {:<10} {}", entry.path, entry.kind, value)?;
    }
    writeln!(out, "agents: {:?}", config.map_keys("agents").await?)?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, agent, model] = args.as_slice()
        && flag == "--set"
    {
        let path = format!("agents.{agent}.model");
        let PropValue::Value { value: current, .. } = config.get(&path).await? else {
            return Err("the model is a secret?".into());
        };
        let patched = config
            .patch(
                &[
                    PatchOp::test(&path, current.clone()),
                    PatchOp::replace(&path, model.as_str()),
                    PatchOp::comment(&path, "changed with the config example"),
                ],
                false,
            )
            .await?;
        writeln!(
            out,
            "{path}: {current} -> {model} (saved: {})",
            patched.saved
        )?;
        if config.pending_reload().await? {
            writeln!(out, "a daemon reload is due for every subsystem to see it")?;
        }
    }
    Ok(())
}
