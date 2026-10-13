//! Errors returned by every client call.

use serde_json::Value;

/// Result alias used across the crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Why a call to the gateway failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The gateway URL, or a URL built from it, is not valid.
    #[error("invalid gateway URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    /// The request never got an HTTP answer: connection refused, TLS failure,
    /// timeout, or a body that could not be read.
    #[error("request to the gateway failed: {0}")]
    Transport(#[from] reqwest::Error),

    /// The gateway answered with a non-success status.
    ///
    /// `message` is the gateway's own explanation (its `error` field) when the
    /// body carries one, otherwise the body text; `body` keeps the JSON body
    /// when there was one, for fields beyond the message.
    #[error("gateway answered {status}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Human-readable reason given by the gateway.
        message: String,
        /// The JSON error body, when the gateway sent one.
        body: Option<Value>,
    },

    /// The gateway answered successfully but the body did not have the
    /// expected shape.
    #[error("unexpected gateway response: {0}")]
    Decode(String),

    /// The gateway did not answer within the timeout (after any retries).
    #[error("the gateway did not answer in time")]
    Timeout,

    /// A layer added with `ClientBuilder::layer` refused the request.
    #[error("a client layer failed: {0}")]
    Layer(#[source] crate::transport::BoxError),

    /// A WebSocket connection to the gateway failed.
    #[error("websocket error: {0}")]
    WebSocket(String),

    /// An agent turn on the chat ended in an `error` frame (or was aborted).
    #[error("agent turn failed: {message}")]
    Turn {
        /// Stable code, e.g. `PROVIDER_ERROR` or `ABORTED`.
        code: Option<String>,
        /// What went wrong.
        message: String,
    },
}

impl Error {
    /// HTTP status of an [`Error::Api`], if this is one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Whether the gateway answered 404 (the resource does not exist).
    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    /// Whether the gateway refused the credentials (401) or the action (403).
    pub fn is_unauthorized(&self) -> bool {
        matches!(self.status(), Some(401 | 403))
    }

    /// Whether the gateway rate-limited the call (429).
    pub fn is_rate_limited(&self) -> bool {
        self.status() == Some(429)
    }

    /// Whether the call may work if made again later: no connection, a
    /// connection dropped before the answer, a timeout, or a "try again"
    /// status (408, 429, 502, 503, 504). The
    /// client already retried idempotent calls before returning it.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Timeout => true,
            Self::Transport(e) => {
                e.is_connect() || e.is_timeout() || crate::transport::is_connection_lost(e)
            }
            Self::Api { status, .. } => matches!(status, 408 | 429 | 502 | 503 | 504),
            _ => false,
        }
    }

    /// The error a failed call through the tower stack is.
    pub(crate) fn from_service(error: crate::transport::BoxError) -> Self {
        if error.is::<tower::timeout::error::Elapsed>() {
            return Self::Timeout;
        }
        match error.downcast::<reqwest::Error>() {
            Ok(e) if e.is_timeout() => Self::Timeout,
            Ok(e) => Self::Transport(*e),
            Err(other) => Self::Layer(other),
        }
    }

    /// Builds an [`Error::Api`] from a failed response's status and body.
    pub(crate) fn from_response(status: u16, text: &str) -> Self {
        let body: Option<Value> = serde_json::from_str(text).ok();
        let message = body
            .as_ref()
            .and_then(|b| b.get("error").or_else(|| b.get("message")))
            .and_then(|m| {
                m.as_str()
                    .map(str::to_string)
                    .or_else(|| Some(m.to_string()))
            })
            .unwrap_or_else(|| text.trim().to_string());
        Self::Api {
            status,
            message,
            body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_error_body_vectors() {
        // (status, body, expected message, not found, unauthorized, rate limited, transient)
        let vectors = [
            (
                404,
                r#"{"error":"Cron job not found: x"}"#,
                "Cron job not found: x",
                true,
                false,
                false,
                false,
            ),
            (
                400,
                r#"{"code":"value_type_mismatch","message":"expected integer","path":"gateway.port"}"#,
                "expected integer",
                false,
                false,
                false,
                false,
            ),
            (
                403,
                r#"{"channel":"whatsapp.main","error":"not in send_channels"}"#,
                "not in send_channels",
                false,
                true,
                false,
                false,
            ),
            (
                401,
                "Unauthorized — provide a token\n",
                "Unauthorized — provide a token",
                false,
                true,
                false,
                false,
            ),
            (
                429,
                r#"{"error":"Too many pairing requests.","retry_after":60}"#,
                "Too many pairing requests.",
                false,
                false,
                true,
                true,
            ),
            (
                502,
                r#"{"channel":"whatsapp.main","error":"channel did not create the room"}"#,
                "channel did not create the room",
                false,
                false,
                false,
                true,
            ),
            (
                503,
                r#"{"error":"SOP subsystem not enabled"}"#,
                "SOP subsystem not enabled",
                false,
                false,
                false,
                true,
            ),
            (408, "", "", false, false, false, true),
            (
                413,
                "payload too large",
                "payload too large",
                false,
                false,
                false,
                false,
            ),
            (
                422,
                "Failed to deserialize the JSON body",
                "Failed to deserialize the JSON body",
                false,
                false,
                false,
                false,
            ),
        ];
        for (status, body, message, not_found, unauthorized, limited, transient) in vectors {
            let e = Error::from_response(status, body);
            assert_eq!(e.status(), Some(status));
            assert_eq!(
                e.to_string(),
                format!("gateway answered {status}: {message}"),
                "{body}"
            );
            assert_eq!(
                (
                    e.is_not_found(),
                    e.is_unauthorized(),
                    e.is_rate_limited(),
                    e.is_transient()
                ),
                (not_found, unauthorized, limited, transient),
                "{status} {body}"
            );
            if let Error::Api {
                body: Some(json), ..
            } = &e
            {
                assert!(json.is_object(), "{body}");
            }
        }
    }

    #[test]
    fn non_api_errors_have_no_status() {
        for e in [
            Error::Timeout,
            Error::Decode("x".into()),
            Error::WebSocket("x".into()),
        ] {
            assert_eq!(e.status(), None);
        }
        assert!(Error::Timeout.is_transient());
        assert!(!Error::Decode("x".into()).is_transient());
    }

    #[test]
    fn api_errors_take_the_gateways_message() {
        let e = Error::from_response(404, r#"{"error":"Cron job not found: x"}"#);
        assert_eq!(e.to_string(), "gateway answered 404: Cron job not found: x");
        assert!(e.is_not_found() && !e.is_unauthorized());

        let e = Error::from_response(401, "Unauthorized\n");
        assert_eq!(e.to_string(), "gateway answered 401: Unauthorized");
        assert!(e.is_unauthorized());

        let e = Error::from_response(400, r#"{"error":{"code":"bad_path"}}"#);
        assert_eq!(e.status(), Some(400));
        assert!(e.to_string().contains("bad_path"));
    }
}
