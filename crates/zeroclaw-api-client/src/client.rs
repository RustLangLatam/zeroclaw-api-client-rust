//! The gateway client: connection settings, authentication and the HTTP
//! plumbing every resource module goes through.

use std::sync::Arc;
use std::time::Duration;

use reqwest::{Method, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::{Error, Result};
use crate::transport::{self, BoxError, BoxLayer, HttpService, RetryPolicy};

/// How long a request waits for the gateway's answer (its headers: a
/// streamed body is not bounded) unless the builder says otherwise. The
/// gateway's own limit is 30 s; calls that do longer work set their own.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long opening a connection may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// A connection to one ZeroClaw gateway.
///
/// Cheap to clone: clones share the HTTP connection pool. Resource groups are
/// reached through accessors such as [`Client::cron`] or [`Client::config`].
///
/// ```no_run
/// # async fn demo() -> zeroclaw_api_client::Result<()> {
/// let client = zeroclaw_api_client::Client::builder("http://127.0.0.1:42617")
///     .token("zc_…")
///     .build()?;
/// let status = client.system().status(None).await?;
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    /// Builds requests; they are sent through `service`.
    http: reqwest::Client,
    service: HttpService,
    base: Url,
    token: Option<String>,
    timeout: Option<Duration>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The bearer token never reaches logs.
        f.debug_struct("Client")
            .field("base", &self.inner.base.as_str())
            .field("authenticated", &self.inner.token.is_some())
            .finish()
    }
}

/// Settings for a [`Client`].
#[must_use]
pub struct ClientBuilder {
    base_url: String,
    token: Option<String>,
    timeout: Option<Duration>,
    retry: RetryPolicy,
    concurrency: Option<usize>,
    layers: Vec<BoxLayer>,
    http: Option<reqwest::Client>,
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("base_url", &self.base_url)
            .field("authenticated", &self.token.is_some())
            .field("timeout", &self.timeout)
            .field("retry", &self.retry)
            .field("concurrency", &self.concurrency)
            .field("layers", &self.layers.len())
            .finish()
    }
}

impl ClientBuilder {
    /// Bearer token sent with every request (obtained by pairing, see
    /// [`crate::pairing`]). Without one, only public endpoints answer.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// How long a request waits for the gateway's answer (default 60 s).
    /// It bounds the headers only, so the event stream is not cut; long
    /// gateway calls (a manual cron run) use their own, longer limit.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Waits for the gateway's answers without limit.
    pub fn no_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    /// How failed idempotent requests are retried (default: three retries,
    /// 200 ms doubling to 5 s). [`RetryPolicy::none`] turns retries off.
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    /// At most `max` requests waiting for the gateway's answer at once,
    /// shared by every clone of the client; the rest wait their turn. A
    /// request holds its slot until the answer's headers arrive, retries and
    /// their backoff included; reading a body (an open event stream) holds
    /// none.
    pub fn concurrency_limit(mut self, max: usize) -> Self {
        self.concurrency = Some(max);
        self
    }

    /// Wraps every HTTP request in a tower layer of your own (metrics,
    /// tracing, request signing, a circuit breaker, …). Layers added first
    /// are outermost: they see the request first and the response last,
    /// around the client's own retry and timeout.
    ///
    /// ```no_run
    /// # fn demo() -> zeroclaw_api_client::Result<()> {
    /// use zeroclaw_api_client::Client;
    ///
    /// let client = Client::builder("http://127.0.0.1:42617")
    ///     .token("zc_…")
    ///     .layer(tower::util::MapRequestLayer::new(|mut request: reqwest::Request| {
    ///         request.headers_mut().insert("x-caller", "billing".parse().unwrap());
    ///         request
    ///     }))
    ///     .build()?;
    /// # Ok(()) }
    /// ```
    pub fn layer<L>(mut self, layer: L) -> Self
    where
        L: tower::Layer<HttpService> + Send + Sync + 'static,
        L::Service: tower::Service<reqwest::Request, Response = Response, Error = BoxError>
            + Clone
            + Send
            + Sync
            + 'static,
        <L::Service as tower::Service<reqwest::Request>>::Future: Send + 'static,
    {
        self.layers.push(transport::box_layer(layer));
        self
    }

    /// Uses an existing `reqwest` client: its proxy, TLS and connection
    /// settings apply, and the client's timeout and retry still wrap it.
    ///
    /// Do not give it a total `timeout`: reqwest applies that one to the
    /// whole body, so it would cut the event stream and cap long calls such
    /// as `cron().run()`. Use [`ClientBuilder::timeout`] instead; a
    /// `connect_timeout` is fine.
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Builds the client. Fails when the base URL is not an absolute
    /// `http`/`https` URL.
    pub fn build(self) -> Result<Client> {
        let mut base = Url::parse(&self.base_url)?;
        if !matches!(base.scheme(), "http" | "https") || base.cannot_be_a_base() {
            return Err(Error::InvalidUrl(
                url::ParseError::RelativeUrlWithCannotBeABaseBase,
            ));
        }
        // Segments are appended to the base path, so it must not end in "/".
        let trimmed = base.path().trim_end_matches('/').to_string();
        base.set_path(&trimmed);
        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .user_agent(concat!("zeroclaw-api-client/", env!("CARGO_PKG_VERSION")))
                .build()?,
        };
        let service = transport::stack(
            http.clone(),
            self.timeout,
            self.retry,
            self.concurrency,
            &self.layers,
        );
        Ok(Client {
            inner: Arc::new(Inner {
                http,
                service,
                base,
                token: self.token.filter(|t| !t.is_empty()),
                timeout: self.timeout,
            }),
        })
    }
}

impl Client {
    /// Starts configuring a client for the gateway at `base_url`
    /// (e.g. `http://127.0.0.1:42617`).
    pub fn builder(base_url: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            base_url: base_url.into(),
            token: None,
            timeout: Some(DEFAULT_TIMEOUT),
            retry: RetryPolicy::default(),
            concurrency: None,
            layers: Vec::new(),
            http: None,
        }
    }

    /// A client for `base_url` authenticated with `token`, with default
    /// settings.
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Result<Self> {
        Self::builder(base_url).token(token).build()
    }

    /// The same gateway and connection pool with another bearer token, e.g.
    /// the one a pairing just returned.
    pub fn with_token(&self, token: impl Into<String>) -> Self {
        let token: String = token.into();
        Self {
            inner: Arc::new(Inner {
                http: self.inner.http.clone(),
                service: self.inner.service.clone(),
                base: self.inner.base.clone(),
                token: (!token.is_empty()).then_some(token),
                timeout: self.inner.timeout,
            }),
        }
    }

    /// The gateway's base URL.
    pub fn base_url(&self) -> &Url {
        &self.inner.base
    }

    /// The bearer token in use, if any.
    pub(crate) fn token(&self) -> Option<&str> {
        self.inner.token.as_deref()
    }

    /// The client's answer timeout, if any.
    pub(crate) fn timeout(&self) -> Option<Duration> {
        self.inner.timeout
    }

    /// The URL of `segments` under the base, each segment percent-encoded
    /// (an id holding `/` or `?` stays one segment), with `query` appended.
    pub(crate) fn url(&self, segments: &[&str], query: &[(&str, &str)]) -> Result<Url> {
        let mut url = self.inner.base.clone();
        url.path_segments_mut()
            .map_err(|()| Error::InvalidUrl(url::ParseError::RelativeUrlWithCannotBeABaseBase))?
            .extend(segments);
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        Ok(url)
    }

    /// A request to `segments` with the bearer token attached.
    pub(crate) fn request(
        &self,
        method: Method,
        segments: &[&str],
        query: &[(&str, &str)],
    ) -> Result<RequestBuilder> {
        let request = self.inner.http.request(method, self.url(segments, query)?);
        Ok(match self.token() {
            Some(token) => request.bearer_auth(token),
            None => request,
        })
    }

    /// Sends `request`, turning a non-success status into [`Error::Api`].
    pub(crate) async fn send(&self, request: RequestBuilder) -> Result<Response> {
        let response = transport::call(&self.inner.service, request.build()?)
            .await
            .map_err(Error::from_service)?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let text = response.text().await.unwrap_or_default();
        Err(Error::from_response(status.as_u16(), &text))
    }

    /// Sends `request` and decodes its JSON body.
    pub(crate) async fn json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T> {
        let bytes = self.send(request).await?.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| Error::Decode(e.to_string()))
    }

    /// Sends `request` and returns its body as text.
    pub(crate) async fn text(&self, request: RequestBuilder) -> Result<String> {
        Ok(self.send(request).await?.text().await?)
    }

    /// Sends `request`, ignoring any body.
    pub(crate) async fn empty(&self, request: RequestBuilder) -> Result<()> {
        self.send(request).await.map(drop)
    }

    /// `GET` and decode.
    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        query: &[(&str, &str)],
    ) -> Result<T> {
        self.json(self.request(Method::GET, segments, query)?).await
    }

    /// `method` with a JSON body, and decode.
    pub(crate) async fn send_json<B: serde::Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        segments: &[&str],
        query: &[(&str, &str)],
        body: &B,
    ) -> Result<T> {
        self.json(self.request(method, segments, query)?.json(body))
            .await
    }
}

/// Query pairs for the optional parameters that are set.
pub(crate) fn query<'a>(pairs: &[(&'a str, Option<&'a str>)]) -> Vec<(&'a str, &'a str)> {
    pairs
        .iter()
        .filter_map(|(k, v)| v.map(|v| (*k, v)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_vectors() {
        // (base, segments, query, expected)
        type Vector<'a> = (&'a str, &'a [&'a str], &'a [(&'a str, &'a str)], &'a str);
        let vectors: [Vector; 8] = [
            (
                "http://gw:42617",
                &["health"],
                &[],
                "http://gw:42617/health",
            ),
            (
                "http://gw:42617/",
                &["api", "status"],
                &[],
                "http://gw:42617/api/status",
            ),
            (
                "https://host/zc/",
                &["api", "cron", "j 1"],
                &[],
                "https://host/zc/api/cron/j%201",
            ),
            (
                "http://gw",
                &["api", "memory", "a/b"],
                &[],
                "http://gw/api/memory/a%2Fb",
            ),
            (
                "http://gw",
                &["api", "memory", "k?x#y"],
                &[],
                "http://gw/api/memory/k%3Fx%23y",
            ),
            (
                "http://gw",
                &["api", "sessions", "whatsapp_1203@g.us+34600", "messages"],
                &[],
                "http://gw/api/sessions/whatsapp_1203@g.us+34600/messages",
            ),
            (
                "http://gw",
                &["api", "config", "prop"],
                &[("path", "a.b c")],
                "http://gw/api/config/prop?path=a.b+c",
            ),
            (
                "http://gw",
                &["api", "logs"],
                &[("q", "a&b=c"), ("limit", "5")],
                "http://gw/api/logs?q=a%26b%3Dc&limit=5",
            ),
        ];
        for (base, segments, query, expected) in vectors {
            let client = Client::builder(base).build().unwrap();
            assert_eq!(client.url(segments, query).unwrap().as_str(), expected);
        }
    }

    #[test]
    fn base_url_vectors() {
        let vectors = [
            ("http://127.0.0.1:42617", true),
            ("https://gateway.example/zeroclaw/", true),
            ("ws://gateway", false),
            ("ftp://gateway", false),
            ("gateway:42617", false),
            ("", false),
            ("mailto:x@example.com", false),
        ];
        for (base, valid) in vectors {
            assert_eq!(Client::builder(base).build().is_ok(), valid, "{base}");
        }
    }

    #[test]
    fn segments_are_encoded_under_the_base_path() {
        let client = Client::builder("http://gw.local:42617/zeroclaw/")
            .build()
            .unwrap();
        let url = client
            .url(&["api", "memory", "a/b?c"], &[("agent", "x y")])
            .unwrap();
        assert_eq!(
            url.as_str(),
            "http://gw.local:42617/zeroclaw/api/memory/a%2Fb%3Fc?agent=x+y"
        );
    }

    #[test]
    fn only_http_urls_make_a_client() {
        assert!(Client::builder("ftp://x").build().is_err());
        assert!(Client::builder("not a url").build().is_err());
        assert!(Client::builder("https://x").build().is_ok());
    }

    #[test]
    fn debug_output_hides_the_token() {
        let client = Client::new("http://x", "secret-token").unwrap();
        let shown = format!("{client:?}");
        assert!(!shown.contains("secret-token") && shown.contains("authenticated: true"));
    }

    #[test]
    fn unset_parameters_are_left_out() {
        assert_eq!(query(&[("a", Some("1")), ("b", None)]), vec![("a", "1")]);
    }
}
