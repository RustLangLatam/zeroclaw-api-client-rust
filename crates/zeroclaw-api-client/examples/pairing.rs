//! Exchanges a pairing code for a bearer token, then lists the paired
//! devices with it.
//!
//! ```sh
//! ZEROCLAW_URL=http://127.0.0.1:42617 cargo run --example pairing -- 123456 my-service
//! ```

use std::io::Write;

use zeroclaw_api_client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let code = args.next().ok_or("usage: pairing <code> [device name]")?;
    let name = args.next();
    let url = std::env::var("ZEROCLAW_URL").unwrap_or_else(|_| "http://127.0.0.1:42617".into());

    let anonymous = Client::builder(url).build()?;
    let paired = anonymous
        .pairing()
        .pair(&code, name.as_deref(), Some("service"))
        .await?;
    let client = anonymous.with_token(&paired.token);

    let mut out = std::io::stdout().lock();
    writeln!(out, "Paired. Keep this token secret:\n{}\n", paired.token)?;
    for device in client.pairing().devices().await? {
        writeln!(
            out,
            "{}  {:<20} paired {}",
            device.id,
            device.name.unwrap_or_default(),
            device.paired_at
        )?;
    }
    Ok(())
}
