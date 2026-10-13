//! A client tuned for an unreliable network: a short answer timeout, five
//! retries with backoff, at most four requests in flight, and a tower layer
//! of its own that times every request.
//!
//! ```sh
//! ZEROCLAW_URL=… ZEROCLAW_TOKEN=… cargo run --example resilient
//! ```

use std::io::Write;
use std::time::{Duration, Instant};

use zeroclaw_api_client::{Client, Error, RetryPolicy};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("ZEROCLAW_URL").unwrap_or_else(|_| "http://127.0.0.1:42617".into());
    let client = Client::builder(url)
        .token(std::env::var("ZEROCLAW_TOKEN")?)
        .timeout(Duration::from_secs(10))
        .retry(RetryPolicy::new(
            5,
            Duration::from_millis(250),
            Duration::from_secs(8),
        ))
        .concurrency_limit(4)
        // Outermost layer: sees each call once, retries included.
        .layer(tower::util::MapFutureLayer::new(|answer| async move {
            let started = Instant::now();
            let answer = answer.await;
            let _ = writeln!(std::io::stderr(), "  call took {:?}", started.elapsed());
            answer
        }))
        .build()?;

    let mut out = std::io::stdout().lock();
    match client.system().status(None).await {
        Ok(status) => writeln!(
            out,
            "ZeroClaw {} up {}s",
            status.version, status.uptime_seconds
        )?,
        Err(e @ Error::Timeout) => writeln!(out, "no answer even after retries: {e}")?,
        Err(e) if e.is_transient() => writeln!(out, "still unavailable: {e}")?,
        Err(e) => return Err(e.into()),
    }
    // Clones share the connection pool and the concurrency limit.
    let calls = (0..8).map(|_| {
        let client = client.clone();
        async move { client.channels().list().await.map(|c| c.len()) }
    });
    for channels in futures_util::future::join_all(calls).await {
        writeln!(out, "{} channels", channels?)?;
    }
    Ok(())
}
