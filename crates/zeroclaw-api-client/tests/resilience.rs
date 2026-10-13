//! The tower stack at the HTTP boundary: retries, timeouts, the concurrency
//! limit and custom layers.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::{TOKEN, json};
use futures_util::StreamExt;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};
use zeroclaw_api_client::{Client, Error, RetryPolicy};

/// Answers each status of `statuses` in turn, then 200 with `{"keys": []}`.
struct Sequence {
    statuses: Vec<u16>,
    calls: Arc<AtomicUsize>,
}

impl Respond for Sequence {
    fn respond(&self, _: &Request) -> ResponseTemplate {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match self.statuses.get(call) {
            Some(status) => {
                ResponseTemplate::new(*status).set_body_json(json!({ "error": "busy" }))
            }
            None => json(json!({ "path": "agents", "keys": ["a"] })),
        }
    }
}

fn fast_retries(max: u32) -> RetryPolicy {
    RetryPolicy::new(max, Duration::from_millis(1), Duration::from_millis(5))
}

async fn sequence(
    server: &MockServer,
    verb: &str,
    route: &str,
    statuses: Vec<u16>,
) -> Arc<AtomicUsize> {
    let calls = Arc::new(AtomicUsize::new(0));
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(Sequence {
            statuses,
            calls: calls.clone(),
        })
        .mount(server)
        .await;
    calls
}

#[tokio::test]
async fn idempotent_calls_are_retried_until_they_work() {
    let server = MockServer::start().await;
    let calls = sequence(&server, "GET", "/api/config/map-keys", vec![503, 502, 429]).await;
    let client = Client::builder(server.uri())
        .token(TOKEN)
        .retry(fast_retries(3))
        .build()
        .unwrap();

    assert_eq!(client.config().map_keys("agents").await.unwrap(), ["a"]);
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn retries_stop_at_the_policys_limit() {
    let server = MockServer::start().await;
    let calls = sequence(&server, "GET", "/api/config/map-keys", vec![503; 10]).await;
    let client = Client::builder(server.uri())
        .token(TOKEN)
        .retry(fast_retries(2))
        .build()
        .unwrap();

    let error = client.config().map_keys("agents").await.unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert!(error.is_transient());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn non_idempotent_calls_are_never_retried() {
    let server = MockServer::start().await;
    let calls = sequence(&server, "POST", "/api/config/map-key", vec![503, 503]).await;
    let client = Client::builder(server.uri())
        .token(TOKEN)
        .retry(fast_retries(3))
        .build()
        .unwrap();

    let error = client
        .config()
        .create_map_key("agents", "a")
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(503));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn errors_that_a_retry_cannot_fix_are_returned_at_once() {
    let server = MockServer::start().await;
    let calls = sequence(&server, "GET", "/api/config/map-keys", vec![404, 404]).await;
    let client = Client::builder(server.uri())
        .token(TOKEN)
        .retry(fast_retries(3))
        .build()
        .unwrap();

    assert!(
        client
            .config()
            .map_keys("agents")
            .await
            .unwrap_err()
            .is_not_found()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_slow_gateway_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/config/map-keys"))
        .respond_with(json(json!({ "keys": [] })).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let client = Client::builder(server.uri())
        .token(TOKEN)
        .timeout(Duration::from_millis(100))
        .retry(RetryPolicy::none())
        .build()
        .unwrap();

    let error = client.config().map_keys("agents").await.unwrap_err();
    assert!(matches!(error, Error::Timeout), "{error:?}");
    assert!(error.is_transient());
}

#[tokio::test]
async fn an_unreachable_gateway_is_a_transient_transport_error() {
    // Nothing listens on port 9 (discard) on the loopback.
    let client = Client::builder("http://127.0.0.1:9")
        .retry(RetryPolicy::none())
        .build()
        .unwrap();

    let error = client.system().health().await.unwrap_err();
    assert!(matches!(error, Error::Transport(_)), "{error:?}");
    assert!(error.is_transient());
}

#[tokio::test]
async fn the_timeout_does_not_cut_the_event_stream() {
    let server = MockServer::start().await;
    // The body arrives after the client's timeout: only the headers count.
    Mock::given(method("GET"))
        .and(path("/api/events"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string("data: {\"type\":\"agent_end\"}\n\n"),
        )
        .mount(&server)
        .await;
    let client = Client::builder(server.uri())
        .token(TOKEN)
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();

    let mut events = client.events().subscribe().await.unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let first = events.next().await.unwrap().unwrap();
    assert_eq!(first.kind(), Some("agent_end"));
}

#[tokio::test]
async fn custom_layers_see_every_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/config/map-keys"))
        .and(header("x-caller", "billing"))
        .respond_with(json(json!({ "keys": ["a"] })))
        .mount(&server)
        .await;
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = seen.clone();
    let client = Client::builder(server.uri())
        .token(TOKEN)
        .layer(tower::util::MapRequestLayer::new(
            move |mut request: reqwest::Request| {
                counter.fetch_add(1, Ordering::SeqCst);
                request.headers_mut().insert(
                    "x-caller",
                    reqwest::header::HeaderValue::from_static("billing"),
                );
                request
            },
        ))
        .build()
        .unwrap();

    assert_eq!(client.config().map_keys("agents").await.unwrap(), ["a"]);
    assert_eq!(client.config().map_keys("agents").await.unwrap(), ["a"]);
    assert_eq!(seen.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_layer_that_refuses_is_a_layer_error() {
    let server = MockServer::start().await;
    let client = Client::builder(server.uri())
        .layer(tower::util::MapFutureLayer::new(|_| async {
            Err::<reqwest::Response, _>(zeroclaw_api_client::transport::BoxError::from(
                "circuit open",
            ))
        }))
        .build()
        .unwrap();

    let error = client.system().health().await.unwrap_err();
    assert!(
        matches!(&error, Error::Layer(e) if e.to_string() == "circuit open"),
        "{error:?}"
    );
}

#[tokio::test]
async fn the_concurrency_limit_is_shared_by_clones() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/config/map-keys"))
        .respond_with(json(json!({ "keys": [] })).set_delay(Duration::from_millis(100)))
        .mount(&server)
        .await;
    let six_calls = |client: Client| async move {
        let started = std::time::Instant::now();
        let calls = (0..6).map(|_| {
            let client = client.clone();
            async move { client.config().map_keys("agents").await }
        });
        for result in futures_util::future::join_all(calls).await {
            result.unwrap();
        }
        started.elapsed()
    };

    let unlimited = Client::builder(server.uri()).token(TOKEN).build().unwrap();
    let limited = Client::builder(server.uri())
        .token(TOKEN)
        .concurrency_limit(2)
        .build()
        .unwrap();

    // Six calls of 100 ms: all at once without a limit, three rounds of two
    // with it.
    assert!(six_calls(unlimited).await < Duration::from_millis(250));
    assert!(six_calls(limited).await >= Duration::from_millis(300));
}

/// A gateway that drops the first `drops` connections without answering
/// (as it does while it restarts), then answers health checks.
#[allow(clippy::disallowed_methods)] // The fake gateway runs on its own task.
async fn restarting_gateway(drops: usize) -> (String, Arc<AtomicUsize>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let connections = Arc::new(AtomicUsize::new(0));
    let counter = connections.clone();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 4096];
            let _ = socket.read(&mut request).await;
            if counter.fetch_add(1, Ordering::SeqCst) < drops {
                drop(socket);
                continue;
            }
            let body = r#"{"status":"ok","paired":true,"require_pairing":true,"runtime":{}}"#;
            let answer = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(answer.as_bytes()).await;
        }
    });
    (base, connections)
}

#[tokio::test]
async fn a_connection_dropped_before_the_answer_is_retried() {
    let (base, connections) = restarting_gateway(2).await;
    let client = Client::builder(base)
        .retry(fast_retries(3))
        .build()
        .unwrap();

    assert!(client.system().health().await.unwrap().require_pairing);
    assert_eq!(connections.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn a_dropped_connection_without_retries_is_transient() {
    let (base, _) = restarting_gateway(1).await;
    let client = Client::builder(base)
        .retry(RetryPolicy::none())
        .build()
        .unwrap();

    let error = client.system().health().await.unwrap_err();
    assert!(error.is_transient(), "{error:?}");
}
