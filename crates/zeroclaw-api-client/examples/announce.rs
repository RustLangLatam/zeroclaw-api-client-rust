//! Posts a message on a channel through an agent's channel binding, on
//! builds without `/api/channels/{channel}/send`: a one-off shell cron job
//! whose raw output is announced, run at once and deleted.
//!
//! ```sh
//! ZEROCLAW_URL=… ZEROCLAW_TOKEN=… cargo run --example announce -- helper whatsapp.main +34600000000 "Hello"
//! ```

use std::io::Write;

use zeroclaw_api_client::Client;
use zeroclaw_api_client::cron::{Delivery, NewCronJob};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [agent, channel, to, text] = args.as_slice() else {
        return Err("usage: announce <agent> <channel> <to> <text>".into());
    };
    let url = std::env::var("ZEROCLAW_URL").unwrap_or_else(|_| "http://127.0.0.1:42617".into());
    let client = Client::new(url, std::env::var("ZEROCLAW_TOKEN")?)?;
    let cron = client.cron();

    // `echo` is what an agent's shell policy allows; quote for the shell.
    let command = format!("echo '{}'", text.replace('\'', "'\\''"));
    let job = cron
        .create(&NewCronJob {
            agent: agent.clone(),
            name: Some("announce-example".into()),
            // Never due on its own: it is run by hand right below.
            schedule: "0 0 1 1 *".into(),
            job_type: Some("shell".into()),
            command: Some(command),
            shell_output_format: Some("raw".into()),
            delivery: Some(Delivery::announce(channel, to)),
            ..NewCronJob::default()
        })
        .await?;
    let run = cron.run(&job.id).await;
    cron.delete(&job.id).await?;
    let run = run?;
    writeln!(
        std::io::stdout(),
        "{} in {} ms",
        if run.success { "sent" } else { "failed" },
        run.duration_ms
    )?;
    Ok(())
}
