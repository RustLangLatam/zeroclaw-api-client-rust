//! Talking to an agent over the gateway's chat WebSocket (`/ws/chat`).
//!
//! Gateway handler: `crates/zeroclaw-gateway/src/ws.rs`. A connection is one
//! session (`gw_<session_id>` in [`crate::sessions`]); reconnecting with the
//! same `session_id` resumes it. Each message starts a turn that streams
//! frames (text chunks, tool calls, approval requests, …) and ends with
//! `done`, `aborted` or `error`. A message sent during a turn steers it.
//!
//! ```no_run
//! # async fn demo(client: zeroclaw_api_client::Client) -> zeroclaw_api_client::Result<()> {
//! let mut chat = client.chat("helper", Default::default()).await?;
//! let reply = chat.ask("What is on my calendar today?").await?;
//! println!("{}", reply.full_response);
//! # Ok(()) }
//! ```

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest, http::HeaderValue};

use crate::client::{Client, query};
use crate::error::{Error, Result};

pub(crate) type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The subprotocol ZeroClaw's chat speaks.
const SUBPROTOCOL: &str = "zeroclaw.v1";

/// Where a chat connects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChatOptions<'a> {
    /// Session to resume or create (a new one without it).
    pub session_id: Option<&'a str>,
    /// Name for the session.
    pub name: Option<&'a str>,
    /// Working directory for the session (must exist on the host).
    pub cwd: Option<&'a str>,
}

impl Client {
    /// Opens a chat with agent `alias` (`GET /ws/chat`, upgraded to a
    /// WebSocket) and waits for the gateway to confirm the session.
    pub async fn chat(&self, alias: &str, options: ChatOptions<'_>) -> Result<ChatSession> {
        let socket = connect(
            self,
            &["ws", "chat"],
            &query(&[
                ("agent", Some(alias)),
                ("session_id", options.session_id),
                ("name", options.name),
                ("cwd", options.cwd),
            ]),
        )
        .await?;
        let mut chat = ChatSession {
            socket,
            session_id: String::new(),
            resumed: false,
            message_count: 0,
        };
        match chat.next_frame().await? {
            Some(ChatFrame::SessionStart {
                session_id,
                resumed,
                message_count,
                ..
            }) => {
                chat.session_id = session_id;
                chat.resumed = resumed;
                chat.message_count = message_count;
                Ok(chat)
            }
            Some(ChatFrame::Error { message, code }) => Err(Error::Turn { code, message }),
            other => Err(Error::WebSocket(format!(
                "expected session_start, got {other:?}"
            ))),
        }
    }
}

/// An open chat with an agent.
#[derive(Debug)]
pub struct ChatSession {
    socket: Socket,
    session_id: String,
    resumed: bool,
    message_count: usize,
}

impl ChatSession {
    /// The session id (its key is `gw_<id>`).
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Whether an existing session was resumed.
    pub fn resumed(&self) -> bool {
        self.resumed
    }

    /// Messages the session held when it was opened.
    pub fn message_count(&self) -> usize {
        self.message_count
    }

    /// Sends a message: it starts a turn, or steers the running one.
    pub async fn send(&mut self, content: &str) -> Result<()> {
        self.send_frame(&serde_json::json!({ "type": "message", "content": content }))
            .await
    }

    /// Answers an `approval_request` (`approve`, `deny` or `always`).
    pub async fn answer_approval(&mut self, request_id: &str, decision: &str) -> Result<()> {
        self.send_frame(&serde_json::json!({
            "type": "approval_response",
            "request_id": request_id,
            "decision": decision,
        }))
        .await
    }

    /// The next frame from the gateway; `None` once the socket closed.
    pub async fn next_frame(&mut self) -> Result<Option<ChatFrame>> {
        while let Some(message) = self.socket.next().await {
            match message.map_err(|e| Error::WebSocket(e.to_string()))? {
                tungstenite::Message::Text(text) => return Ok(Some(ChatFrame::parse(&text)?)),
                tungstenite::Message::Close(_) => return Ok(None),
                // Pings are answered by tungstenite; nothing else carries frames.
                _ => {}
            }
        }
        Ok(None)
    }

    /// Sends `content` and waits for the turn to end, ignoring the frames in
    /// between. An `error` frame becomes [`Error::Turn`]; an aborted turn
    /// too, with code `ABORTED`. Tool approvals are not answered here: use
    /// [`ChatSession::next_frame`] for agents that ask for them.
    pub async fn ask(&mut self, content: &str) -> Result<TurnResult> {
        self.send(content).await?;
        loop {
            match self.next_frame().await? {
                Some(ChatFrame::Done(result)) => return Ok(result),
                Some(ChatFrame::Error { message, code }) => {
                    return Err(Error::Turn { code, message });
                }
                Some(ChatFrame::Aborted) => {
                    return Err(Error::Turn {
                        code: Some("ABORTED".into()),
                        message: "the turn was aborted".into(),
                    });
                }
                Some(_) => {}
                None => {
                    return Err(Error::WebSocket(
                        "the gateway closed the chat before the turn ended".into(),
                    ));
                }
            }
        }
    }

    /// Closes the chat. Closing during a turn cancels it.
    pub async fn close(mut self) -> Result<()> {
        self.socket
            .close(None)
            .await
            .map_err(|e| Error::WebSocket(e.to_string()))
    }

    async fn send_frame(&mut self, frame: &Value) -> Result<()> {
        self.socket
            .send(tungstenite::Message::Text(frame.to_string().into()))
            .await
            .map_err(|e| Error::WebSocket(e.to_string()))
    }
}

/// Opens a WebSocket to `segments` on the gateway (`ws` for `http`, `wss`
/// for `https`), offering ZeroClaw's subprotocol and the bearer token.
pub(crate) async fn connect(
    client: &Client,
    segments: &[&str],
    query: &[(&str, &str)],
) -> Result<Socket> {
    let mut url = client.url(segments, query)?;
    let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
    url.set_scheme(scheme)
        .map_err(|()| Error::WebSocket(format!("cannot use {scheme} with {url}")))?;
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|e| Error::WebSocket(e.to_string()))?;
    let headers = request.headers_mut();
    headers.insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static(SUBPROTOCOL),
    );
    if let Some(token) = client.token() {
        let bearer = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| Error::WebSocket("the token is not a valid header value".into()))?;
        headers.insert("Authorization", bearer);
    }
    let handshake = tokio_tungstenite::connect_async(request);
    let (socket, _) = match client.timeout() {
        Some(limit) => tokio::time::timeout(limit, handshake)
            .await
            .map_err(|_| Error::Timeout)?,
        None => handshake.await,
    }
    .map_err(handshake_error)?;
    Ok(socket)
}

/// The JSON frames of a live WebSocket feed (canvas, SOP runs).
pub type JsonFeed = futures_util::stream::BoxStream<'static, Result<Value>>;

/// The JSON frames a server-push WebSocket sends, until it closes.
pub(crate) fn json_frames(socket: Socket) -> JsonFeed {
    futures_util::stream::unfold(socket, |mut socket| async move {
        loop {
            match socket.next().await? {
                Ok(tungstenite::Message::Text(text)) => {
                    let frame = serde_json::from_str(&text)
                        .map_err(|e| Error::Decode(format!("frame is not JSON: {e}")));
                    return Some((frame, socket));
                }
                Ok(tungstenite::Message::Close(_)) => return None,
                Ok(_) => {}
                Err(e) => return Some((Err(Error::WebSocket(e.to_string())), socket)),
            }
        }
    })
    .boxed()
}

/// The handshake's failure, keeping the gateway's HTTP refusal (401 without
/// a valid token, 400 for an unknown agent) as [`Error::Api`].
fn handshake_error(e: tungstenite::Error) -> Error {
    match e {
        tungstenite::Error::Http(response) => {
            let body = response
                .body()
                .as_deref()
                .map(String::from_utf8_lossy)
                .unwrap_or_default();
            Error::from_response(response.status().as_u16(), &body)
        }
        other => Error::WebSocket(other.to_string()),
    }
}

/// A frame the gateway sends on the chat.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ChatFrame {
    /// The session is open.
    SessionStart {
        /// Session id.
        session_id: String,
        /// Whether it existed.
        resumed: bool,
        /// Messages it holds.
        message_count: usize,
        /// Its name.
        name: Option<String>,
    },
    /// Answer to a `connect` frame.
    Connected,
    /// A piece of the reply.
    Chunk(String),
    /// A piece of the model's reasoning.
    Thinking(String),
    /// The agent calls a tool.
    ToolCall {
        /// Call id.
        id: String,
        /// Tool.
        name: String,
        /// Arguments.
        args: Value,
    },
    /// A tool's result.
    ToolResult {
        /// Call id.
        id: String,
        /// Tool.
        name: String,
        /// Output.
        output: Value,
    },
    /// The agent needs approval to run a tool; answer with
    /// [`ChatSession::answer_approval`] (denied after `timeout_secs`).
    ApprovalRequest {
        /// Request id.
        request_id: String,
        /// Tool.
        tool: String,
        /// What it would do.
        arguments_summary: String,
        /// Seconds before it is denied.
        timeout_secs: u64,
    },
    /// The turn ended.
    Done(TurnResult),
    /// The turn was cancelled.
    Aborted,
    /// The turn (or the session) failed.
    Error {
        /// What went wrong.
        message: String,
        /// Stable code (`PROVIDER_ERROR`, `SESSION_QUEUE_FULL`, …).
        code: Option<String>,
    },
    /// Any other frame (`plan`, `history_trimmed`, `cron_result`, session
    /// events, …), as sent.
    Other(Value),
}

/// The end of a turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnResult {
    /// The whole reply.
    pub full_response: String,
    /// Input tokens.
    #[serde(default)]
    pub input_tokens: Option<u64>,
    /// Output tokens.
    #[serde(default)]
    pub output_tokens: Option<u64>,
    /// All tokens.
    #[serde(default)]
    pub tokens_used: Option<u64>,
    /// Cost, USD.
    #[serde(default)]
    pub cost_usd: Option<f64>,
    /// Model used.
    #[serde(default)]
    pub model: String,
    /// Provider used.
    #[serde(default)]
    pub provider: String,
    /// The model's context size.
    #[serde(default)]
    pub max_context_tokens: Option<u64>,
    /// Input tokens of the last model call.
    #[serde(default)]
    pub last_input_tokens: Option<u64>,
}

impl ChatFrame {
    fn parse(text: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(text)
            .map_err(|e| Error::Decode(format!("chat frame is not JSON: {e}")))?;
        let text_of = |field: &str| value[field].as_str().unwrap_or_default().to_string();
        Ok(match value["type"].as_str().unwrap_or_default() {
            "session_start" => Self::SessionStart {
                session_id: text_of("session_id"),
                resumed: value["resumed"].as_bool().unwrap_or_default(),
                message_count: value["message_count"].as_u64().unwrap_or_default() as usize,
                name: value["name"].as_str().map(str::to_string),
            },
            "connected" => Self::Connected,
            "chunk" => Self::Chunk(text_of("content")),
            "thinking" => Self::Thinking(text_of("content")),
            "tool_call" => Self::ToolCall {
                id: text_of("id"),
                name: text_of("name"),
                args: value["args"].clone(),
            },
            "tool_result" => Self::ToolResult {
                id: text_of("id"),
                name: text_of("name"),
                output: value["output"].clone(),
            },
            "approval_request" => Self::ApprovalRequest {
                request_id: text_of("request_id"),
                tool: text_of("tool"),
                arguments_summary: text_of("arguments_summary"),
                timeout_secs: value["timeout_secs"].as_u64().unwrap_or_default(),
            },
            "done" => Self::Done(
                serde_json::from_value(value)
                    .map_err(|e| Error::Decode(format!("done frame: {e}")))?,
            ),
            "aborted" => Self::Aborted,
            "error" => Self::Error {
                message: value["message"]
                    .as_str()
                    .or_else(|| value["error"].as_str())
                    .unwrap_or("unknown error")
                    .to_string(),
                code: value["code"].as_str().map(str::to_string),
            },
            _ => Self::Other(value),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_frame_vectors() {
        let vectors: [(&str, ChatFrame); 9] = [
            (
                r#"{"type":"connected","message":"Connection established"}"#,
                ChatFrame::Connected,
            ),
            (
                r#"{"type":"chunk","content":"Ho"}"#,
                ChatFrame::Chunk("Ho".into()),
            ),
            (
                r#"{"type":"thinking","content":"…"}"#,
                ChatFrame::Thinking("…".into()),
            ),
            (r#"{"type":"aborted"}"#, ChatFrame::Aborted),
            (
                r#"{"type":"tool_call","id":"c1","name":"shell","args":{"cmd":"ls"}}"#,
                ChatFrame::ToolCall {
                    id: "c1".into(),
                    name: "shell".into(),
                    args: serde_json::json!({ "cmd": "ls" }),
                },
            ),
            (
                r#"{"type":"tool_result","id":"c1","name":"shell","output":"ok"}"#,
                ChatFrame::ToolResult {
                    id: "c1".into(),
                    name: "shell".into(),
                    output: "ok".into(),
                },
            ),
            (
                r#"{"type":"approval_request","request_id":"r1","tool":"shell","arguments_summary":"rm x","timeout_secs":120}"#,
                ChatFrame::ApprovalRequest {
                    request_id: "r1".into(),
                    tool: "shell".into(),
                    arguments_summary: "rm x".into(),
                    timeout_secs: 120,
                },
            ),
            (
                r#"{"type":"error","error":"needs_onboarding","code":"NEEDS_ONBOARDING","message":"Set up first"}"#,
                ChatFrame::Error {
                    message: "Set up first".into(),
                    code: Some("NEEDS_ONBOARDING".into()),
                },
            ),
            (
                r#"{"type":"session_start","session_id":"s","resumed":true,"message_count":4,"name":"Support"}"#,
                ChatFrame::SessionStart {
                    session_id: "s".into(),
                    resumed: true,
                    message_count: 4,
                    name: Some("Support".into()),
                },
            ),
        ];
        for (text, expected) in vectors {
            assert_eq!(ChatFrame::parse(text).unwrap(), expected, "{text}");
        }
        for unknown in [
            r#"{"type":"plan","entries":[]}"#,
            r#"{"type":"cron_result"}"#,
            r#"{"no_type":1}"#,
        ] {
            assert!(
                matches!(ChatFrame::parse(unknown).unwrap(), ChatFrame::Other(_)),
                "{unknown}"
            );
        }
        assert!(ChatFrame::parse("not json").is_err());
    }

    #[test]
    fn frames_parse_into_their_variants() {
        assert_eq!(
            ChatFrame::parse(r#"{"type":"chunk","content":"Hi"}"#).unwrap(),
            ChatFrame::Chunk("Hi".into())
        );
        let done = ChatFrame::parse(
            r#"{"type":"done","full_response":"Hi there","model":"m","provider":"p","input_tokens":3,"output_tokens":2,"tokens_used":5,"cost_usd":null,"max_context_tokens":8192,"last_input_tokens":3}"#,
        )
        .unwrap();
        assert!(
            matches!(done, ChatFrame::Done(ref r) if r.full_response == "Hi there" && r.tokens_used == Some(5))
        );
        assert_eq!(
            ChatFrame::parse(r#"{"type":"error","message":"boom","code":"PROVIDER_ERROR"}"#)
                .unwrap(),
            ChatFrame::Error {
                message: "boom".into(),
                code: Some("PROVIDER_ERROR".into())
            }
        );
        assert!(matches!(
            ChatFrame::parse(r#"{"type":"plan","entries":[]}"#).unwrap(),
            ChatFrame::Other(_)
        ));
    }
}
