# Resilience

Every HTTP request goes through a [tower](https://github.com/tower-rs/tower)
service stack. From your code to the network:

```text
your layers → concurrency limit → retry → deadline → reqwest → gateway
```

## Timeouts

`ClientBuilder::timeout` (default 60 s) bounds how long a request waits for
the gateway's **answer headers**. A streamed body is not bounded, so
`events().subscribe()` stays open for as long as the gateway keeps it. Calls
that do long work on the gateway carry their own limit: `cron().run()` waits
up to 660 s, as the gateway allows a manual run 600 s.
`ClientBuilder::no_timeout` waits forever. If you pass your own
`reqwest::Client` (`ClientBuilder::http_client`), do not give it a total
`timeout`: reqwest applies that one to whole bodies and would cut the stream. Opening a WebSocket (chat, canvas
and SOP-run feeds) uses the same timeout for its handshake.

A timeout is `Error::Timeout`.

## Retries

A failed request is tried again when **both** hold:

- its method is idempotent: `GET`, `HEAD`, `OPTIONS`, `PUT`, `DELETE`. A
  `POST` (creating a cron job, sending a message, starting an SOP) or a
  `PATCH` (a config patch) is never repeated: it could apply twice;
- the failure is one a retry can fix: no connection, a connection dropped
  before the answer (the gateway does it while it restarts), a timeout, or a
  408, 429, 502, 503 or 504 answer. A 4xx that describes the request (400, 404,
  409, …) and a 500 are returned at once.

`RetryPolicy` sets how many retries and how far apart: retry `n` waits
`base × 2ⁿ`, capped. The default is three retries, 200 ms doubling to 5 s
(200, 400, 800 ms). `RetryPolicy::none()` turns them off.

```rust,no_run
# fn demo() -> zeroclaw_api_client::Result<()> {
use std::time::Duration;
use zeroclaw_api_client::{Client, RetryPolicy};

let client = Client::builder("http://127.0.0.1:42617")
    .token("zc_…")
    .retry(RetryPolicy::new(5, Duration::from_millis(250), Duration::from_secs(8)))
    .build()?;
# Ok(()) }
```

The gateway restarts itself on `system().reload()` and some configuration
changes; for a few seconds it refuses connections. Retries carry reads
through that window.

## Concurrency limit

`ClientBuilder::concurrency_limit(n)` lets at most `n` requests wait for the
gateway's answer at once; the rest wait their turn. A request holds its slot
until the answer's headers arrive, its retries and their backoff included;
reading a body afterwards (an open event stream) holds none. Every clone of
the client shares the limit, so one setting protects the gateway from a
whole application.

## Your own layers

`ClientBuilder::layer` wraps every request in any tower layer whose service
takes a `reqwest::Request` and answers a `reqwest::Response` with a boxed
error: metrics, tracing, request signing, a circuit breaker, a rate limiter
of your own. Layers added first are outermost: they see each call once, with
the client's retries inside them.

```rust,no_run
# fn demo() -> zeroclaw_api_client::Result<()> {
use std::time::Instant;
use zeroclaw_api_client::Client;

let client = Client::builder("http://127.0.0.1:42617")
    .token("zc_…")
    // Adds a header to every request.
    .layer(tower::util::MapRequestLayer::new(|mut request: reqwest::Request| {
        request.headers_mut().insert("x-caller", "billing".parse().unwrap());
        request
    }))
    // Times every call.
    .layer(tower::util::MapFutureLayer::new(|answer| async move {
        let started = Instant::now();
        let answer = answer.await;
        let _elapsed = started.elapsed(); // report it to your metrics
        answer
    }))
    .build()?;
# Ok(()) }
```

A layer that fails a request makes the call return `Error::Layer` with its
error.

## What the client does not do

It spawns no tasks: the stack runs on the caller's task, and streams are
driven by whoever reads them. It does not reconnect streams by itself either:
when `events().subscribe()` ends (the gateway reloaded), subscribe again; the
`events` example shows the loop. The WebSocket chat is not an HTTP request
and does not go through the stack; a failed turn is reported, not retried.
