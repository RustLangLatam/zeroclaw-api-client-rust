//! Prints the gateway's version, model and channels.
//!
//! ```sh
//! ZEROCLAW_URL=http://127.0.0.1:42617 ZEROCLAW_TOKEN=zc_… cargo run --example status
//! ```

use std::io::Write;

use zeroclaw_api_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("ZEROCLAW_URL").unwrap_or_else(|_| "http://127.0.0.1:42617".into());
    let token = std::env::var("ZEROCLAW_TOKEN")?;
    let client = Client::new(url, token)?;

    let status = client.system().status(None).await?;
    let mut out = std::io::stdout().lock();
    writeln!(out, "ZeroClaw {} — model {}", status.version, status.model)?;
    for channel in client.channels().list().await? {
        writeln!(
            out,
            "  {:<40} {:<10} {}",
            channel.name, channel.status, channel.health
        )?;
    }
    Ok(())
}
