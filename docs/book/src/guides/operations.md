# Cron, memory and cost

## Cron jobs

A job runs a shell command or an agent prompt on a schedule and can deliver
its output to a channel:

```rust,no_run
# async fn demo(client: zeroclaw_api_client::Client) -> zeroclaw_api_client::Result<()> {
use zeroclaw_api_client::cron::{Delivery, NewCronJob};

let job = client
    .cron()
    .create(&NewCronJob {
        agent: "helper".into(),
        schedule: "0 9 * * 1-5".into(),
        tz: Some("Europe/Madrid".into()),
        prompt: Some("Summarize yesterday's orders".into()),
        delivery: Some(Delivery::announce("whatsapp.main", "+34600000000")),
        ..NewCronJob::default()
    })
    .await?;
let result = client.cron().run(&job.id).await?; // runs it now and waits
# Ok(()) }
```

`update` changes a job (pause with `enabled: Some(false)`), `runs` lists its
history, and `settings` / `update_settings` cover the scheduler.

A shell job with `shell_output_format: "raw"` and `delivery` announces its
output verbatim: a way to post a fixed message (or a `[DOCUMENT:…]` marker)
on a channel through an agent's channel binding.

## Memory

`memory().search(query)` recalls entries by text or time (at most 50), or
lists a category; `store` and `forget` write. Each agent can have its own
memory (`agent`) besides the shared one.

## Cost

`cost().summary(from, to)` returns tokens, calls and USD in a window, by
model and by agent; `cost().agent(alias)` an agent's session and daily
figures. Models without known rates count as free unless their prices are
configured.

## The gateway itself

`client.system()` covers health (`health`, public), `status`, Prometheus
`metrics`, `doctor`, `tools`, `integrations`, host `cli_tools`, connected
terminal clients (`tuis`), paged structured `logs`, version checks and
self-upgrades, and the daemon's `reload` and `shutdown`.
