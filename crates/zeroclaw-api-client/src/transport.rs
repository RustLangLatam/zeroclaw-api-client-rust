//! The tower stack every HTTP request goes through.
//!
//! From the caller to the network:
//!
//! ```text
//! your layers (ClientBuilder::layer) → concurrency limit → retry → deadline → reqwest
//! ```
//!
//! - **Deadline:** bounds the wait for the response *headers*, so a long
//!   Server-Sent Events body is never cut. A request carrying its own
//!   timeout (the client sets one on long gateway calls such as a manual
//!   cron run) uses it instead of the client's.
//! - **Retry:** idempotent requests (`GET`, `HEAD`, `OPTIONS`, `PUT`,
//!   `DELETE`) are retried on connection failures, timeouts and the statuses
//!   that mean "try again" (408, 429, 502, 503, 504), with exponential
//!   backoff. A `POST` or `PATCH` is never retried: it could apply twice.
//! - **Concurrency limit:** optional, shared by every clone of the client.
//!
//! The WebSocket chat and feeds are not HTTP requests and do not go through
//! this stack.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use reqwest::{Method, Request, Response, StatusCode};
use tower::util::BoxCloneSyncService;
use tower::{Layer, Service, ServiceExt};

/// Any error a layer of the stack can return.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The HTTP service the client calls: requests in, responses out. Custom
/// layers wrap it (see [`crate::ClientBuilder::layer`]).
pub type HttpService = BoxCloneSyncService<Request, Response, BoxError>;

/// When and how often a failed idempotent request is tried again.
///
/// The delay before retry `n` (0-based) is `base_delay × 2ⁿ`, capped at
/// `max_delay`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    max_retries: u32,
    base_delay: Duration,
    max_delay: Duration,
    /// Retries already made for the request this copy belongs to: tower
    /// clones the policy for every request.
    attempt: u32,
}

impl Default for RetryPolicy {
    /// Three retries, 200 ms then doubling, at most 5 s apart.
    fn default() -> Self {
        Self::new(3, Duration::from_millis(200), Duration::from_secs(5))
    }
}

impl RetryPolicy {
    /// Up to `max_retries` retries, `base_delay` doubling up to `max_delay`.
    pub fn new(max_retries: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_retries,
            base_delay,
            max_delay,
            attempt: 0,
        }
    }

    /// No retries.
    pub fn none() -> Self {
        Self::new(0, Duration::ZERO, Duration::ZERO)
    }

    /// How long to wait before retry `attempt` (0-based).
    pub fn delay(&self, attempt: u32) -> Duration {
        let factor = 2u32.saturating_pow(attempt);
        self.base_delay.saturating_mul(factor).min(self.max_delay)
    }
}

/// Whether repeating a request with this method is safe.
pub(crate) fn is_idempotent(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::PUT | Method::DELETE
    )
}

/// Statuses that ask the client to try again later.
pub(crate) fn is_retryable_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 429 | 502 | 503 | 504)
}

/// Failures that a retry may fix: no connection, no answer in time, or a
/// connection the gateway dropped before answering (it does while it
/// restarts).
pub(crate) fn is_retryable_error(error: &BoxError) -> bool {
    if error.is::<tower::timeout::error::Elapsed>() {
        return true;
    }
    error
        .downcast_ref::<reqwest::Error>()
        .is_some_and(|e| e.is_connect() || e.is_timeout() || is_connection_lost(e))
}

/// Whether `error` comes from a connection that closed or reset before the
/// answer arrived: an I/O error of that kind, or hyper's "connection closed
/// before message completed", somewhere in its source chain.
pub(crate) fn is_connection_lost(error: &(dyn std::error::Error + 'static)) -> bool {
    use std::io::ErrorKind;
    let mut source = Some(error);
    while let Some(current) = source {
        if let Some(io) = current.downcast_ref::<std::io::Error>()
            && matches!(
                io.kind(),
                ErrorKind::ConnectionReset
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::BrokenPipe
                    | ErrorKind::UnexpectedEof
            )
        {
            return true;
        }
        if current
            .to_string()
            .contains("connection closed before message completed")
        {
            return true;
        }
        source = current.source();
    }
    false
}

impl tower::retry::Policy<Request, Response, BoxError> for RetryPolicy {
    type Future = tokio::time::Sleep;

    fn retry(
        &mut self,
        request: &mut Request,
        result: &mut Result<Response, BoxError>,
    ) -> Option<Self::Future> {
        let retryable = match result {
            Ok(response) => is_retryable_status(response.status()),
            Err(error) => is_retryable_error(error),
        };
        if !retryable || self.attempt >= self.max_retries || !is_idempotent(request.method()) {
            return None;
        }
        let delay = self.delay(self.attempt);
        self.attempt += 1;
        Some(tokio::time::sleep(delay))
    }

    fn clone_request(&mut self, request: &Request) -> Option<Request> {
        if self.max_retries == 0 || !is_idempotent(request.method()) {
            return None;
        }
        request.try_clone()
    }
}

/// Bounds the wait for a response's headers: the request's own timeout, or
/// the client's. Elapsing is [`tower::timeout::error::Elapsed`].
#[derive(Debug, Clone)]
pub(crate) struct Deadline<S> {
    inner: S,
    default: Option<Duration>,
}

impl<S> Service<Request> for Deadline<S>
where
    S: Service<Request, Response = Response>,
    S::Error: Into<BoxError>,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Response, BoxError>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), BoxError>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let limit = request.timeout().copied().or(self.default);
        let response = self.inner.call(request);
        Box::pin(async move {
            match limit {
                Some(limit) => tokio::time::timeout(limit, response)
                    .await
                    .map_err(|_| BoxError::from(tower::timeout::error::Elapsed::new()))?
                    .map_err(Into::into),
                None => response.await.map_err(Into::into),
            }
        })
    }
}

/// A layer added with [`crate::ClientBuilder::layer`], kept boxed so the
/// builder stays one type.
pub(crate) type BoxLayer = Arc<dyn Fn(HttpService) -> HttpService + Send + Sync>;

/// Wraps `layer` so it can be applied to the boxed service.
pub(crate) fn box_layer<L>(layer: L) -> BoxLayer
where
    L: Layer<HttpService> + Send + Sync + 'static,
    L::Service:
        Service<Request, Response = Response, Error = BoxError> + Clone + Send + Sync + 'static,
    <L::Service as Service<Request>>::Future: Send + 'static,
{
    Arc::new(move |service| BoxCloneSyncService::new(layer.layer(service)))
}

/// The stack over `http`, outermost first: `layers` in the order they were
/// added, the concurrency limit, retry, deadline.
pub(crate) fn stack(
    http: reqwest::Client,
    timeout: Option<Duration>,
    retry: RetryPolicy,
    concurrency: Option<usize>,
    layers: &[BoxLayer],
) -> HttpService {
    let mut service = HttpService::new(Deadline {
        inner: http,
        default: timeout,
    });
    service = HttpService::new(tower::retry::Retry::new(retry, service));
    if let Some(max) = concurrency {
        service =
            HttpService::new(tower::limit::GlobalConcurrencyLimitLayer::new(max).layer(service));
    }
    for layer in layers.iter().rev() {
        service = layer(service);
    }
    service
}

/// Sends `request` through `service`.
pub(crate) async fn call(service: &HttpService, request: Request) -> Result<Response, BoxError> {
    service.clone().oneshot(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::retry::Policy;

    fn request(method: Method) -> Request {
        Request::new(method, "http://gateway.test/api/x".parse().unwrap())
    }

    fn response(status: u16) -> Result<Response, BoxError> {
        Ok(Response::from(
            http::Response::builder().status(status).body("").unwrap(),
        ))
    }

    #[test]
    fn idempotency_vectors() {
        let vectors = [
            (Method::GET, true),
            (Method::HEAD, true),
            (Method::OPTIONS, true),
            (Method::PUT, true),
            (Method::DELETE, true),
            (Method::POST, false),
            (Method::PATCH, false),
        ];
        for (method, expected) in vectors {
            assert_eq!(is_idempotent(&method), expected, "{method}");
        }
    }

    #[test]
    fn retryable_status_vectors() {
        let vectors = [
            (200, false),
            (204, false),
            (400, false),
            (401, false),
            (404, false),
            (408, true),
            (409, false),
            (429, true),
            (500, false),
            (502, true),
            (503, true),
            (504, true),
        ];
        for (status, expected) in vectors {
            let status = StatusCode::from_u16(status).unwrap();
            assert_eq!(is_retryable_status(status), expected, "{status}");
        }
    }

    #[test]
    fn connection_lost_vectors() {
        use std::io::{Error as Io, ErrorKind};

        #[derive(Debug)]
        struct Wrapped(Io);
        impl std::fmt::Display for Wrapped {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("request failed")
            }
        }
        impl std::error::Error for Wrapped {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }
        #[derive(Debug)]
        struct Hyper;
        impl std::fmt::Display for Hyper {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("connection closed before message completed")
            }
        }
        impl std::error::Error for Hyper {}

        let vectors: [(Box<dyn std::error::Error>, bool); 7] = [
            (
                Box::new(Wrapped(Io::from(ErrorKind::ConnectionReset))),
                true,
            ),
            (
                Box::new(Wrapped(Io::from(ErrorKind::ConnectionAborted))),
                true,
            ),
            (Box::new(Wrapped(Io::from(ErrorKind::BrokenPipe))), true),
            (Box::new(Wrapped(Io::from(ErrorKind::UnexpectedEof))), true),
            (Box::new(Hyper), true),
            (
                Box::new(Wrapped(Io::from(ErrorKind::PermissionDenied))),
                false,
            ),
            (Box::new(Wrapped(Io::other("tls handshake failed"))), false),
        ];
        for (error, expected) in vectors {
            assert_eq!(is_connection_lost(error.as_ref()), expected, "{error:?}");
        }
    }

    #[test]
    fn retryable_error_vectors() {
        let elapsed: BoxError = tower::timeout::error::Elapsed::new().into();
        let other: BoxError = "a layer refused".into();
        assert!(is_retryable_error(&elapsed));
        assert!(!is_retryable_error(&other));
    }

    #[test]
    fn backoff_vectors() {
        let policy = RetryPolicy::new(10, Duration::from_millis(100), Duration::from_secs(1));
        let vectors = [
            (0, 100),
            (1, 200),
            (2, 400),
            (3, 800),
            (4, 1000),
            (9, 1000),
            (40, 1000),
        ];
        for (attempt, millis) in vectors {
            assert_eq!(
                policy.delay(attempt),
                Duration::from_millis(millis),
                "attempt {attempt}"
            );
        }
        assert_eq!(RetryPolicy::none().delay(3), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_decision_vectors() {
        // (method, outcome status, retries already made, retried?)
        let vectors = [
            (Method::GET, 503, 0, true),
            (Method::GET, 503, 2, true),
            (Method::GET, 503, 3, false),
            (Method::GET, 200, 0, false),
            (Method::GET, 500, 0, false),
            (Method::PUT, 429, 0, true),
            (Method::DELETE, 502, 1, true),
            (Method::POST, 503, 0, false),
            (Method::PATCH, 503, 0, false),
        ];
        for (method, status, attempt, expected) in vectors {
            let mut policy = RetryPolicy {
                attempt,
                ..RetryPolicy::default()
            };
            let retried = policy
                .retry(&mut request(method.clone()), &mut response(status))
                .is_some();
            assert_eq!(retried, expected, "{method} {status} after {attempt}");
            if retried {
                assert_eq!(policy.attempt, attempt + 1);
            }
        }
    }

    #[test]
    fn only_idempotent_requests_are_kept_for_a_retry() {
        let mut policy = RetryPolicy::default();
        assert!(policy.clone_request(&request(Method::GET)).is_some());
        assert!(policy.clone_request(&request(Method::POST)).is_none());
        assert!(
            RetryPolicy::none()
                .clone_request(&request(Method::GET))
                .is_none()
        );
    }
}
