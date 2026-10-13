//! The gateway's live event stream (Server-Sent Events) and its recent
//! history.
//!
//! Gateway handler: `crates/zeroclaw-gateway/src/sse.rs`. Every frame is
//! `data: <one JSON object>` with no event name, so frames are told apart by
//! their content: observer events and cron results carry a `type`
//! (`llm_request`, `tool_call`, `tool_call_start`, `agent_start`,
//! `agent_end`, `error`, `history_trimmed`, `cron_result`, …); structured log
//! events carry `@timestamp` instead. Frames missed while lagging are not
//! replayed.

use futures_util::{Stream, StreamExt};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::{Error, Result};

/// Event endpoints, from [`Client::events`].
#[derive(Debug, Clone, Copy)]
pub struct EventsApi<'a> {
    client: &'a Client,
}

impl Client {
    /// The live event stream (`/api/events`).
    pub fn events(&self) -> EventsApi<'_> {
        EventsApi { client: self }
    }
}

impl EventsApi<'_> {
    /// `GET /api/events`: subscribes to the live stream. The stream ends
    /// when the gateway closes the connection (e.g. on reload); subscribe
    /// again to resume. The client's timeout bounds only the wait for the
    /// stream to open, never the stream itself.
    pub async fn subscribe(&self) -> Result<EventStream> {
        let request = self
            .client
            .request(Method::GET, &["api", "events"], &[])?
            .header(reqwest::header::ACCEPT, "text/event-stream");
        let response = self.client.send(request).await?;
        Ok(sse_frames(response.bytes_stream()).boxed())
    }

    /// `GET /api/events/history`: the latest observer events, oldest first
    /// (up to 500, kept in memory).
    pub async fn history(&self) -> Result<Vec<Event>> {
        #[derive(Deserialize)]
        struct History {
            events: Vec<Event>,
        }
        let history: History = self.client.get(&["api", "events", "history"], &[]).await?;
        Ok(history.events)
    }
}

/// The live events of [`EventsApi::subscribe`].
pub type EventStream = futures_util::stream::BoxStream<'static, Result<Event>>;

/// One event, as the gateway sent it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Event(pub Value);

impl Event {
    /// The event's `type` (`agent_end`, `cron_result`, …); `None` for log
    /// events.
    pub fn kind(&self) -> Option<&str> {
        self.0.get("type").and_then(Value::as_str)
    }

    /// Whether it is a structured log event (`@timestamp`, `event`,
    /// `service`, `zeroclaw` attribution).
    pub fn is_log(&self) -> bool {
        self.0.get("@timestamp").is_some()
    }

    /// A field of the event.
    pub fn get(&self, field: &str) -> Option<&Value> {
        self.0.get(field)
    }
}

/// Turns an SSE byte stream into events: `data:` lines are joined until a
/// blank line ends the frame; comments (keep-alives) are skipped.
fn sse_frames<S, B, E>(bytes: S) -> impl Stream<Item = Result<Event>> + Send + 'static
where
    S: Stream<Item = std::result::Result<B, E>> + Send + 'static,
    B: AsRef<[u8]>,
    E: Into<Error>,
{
    let state = (Box::pin(bytes), String::new(), false);
    futures_util::stream::unfold(state, |(mut bytes, mut buffer, mut done)| async move {
        loop {
            if let Some(end) = buffer.find("\n\n") {
                let frame: String = buffer.drain(..end + 2).collect();
                if let Some(event) = parse_frame(&frame) {
                    return Some((event, (bytes, buffer, done)));
                }
                continue;
            }
            if done {
                return None;
            }
            match bytes.next().await {
                Some(Ok(chunk)) => {
                    buffer.push_str(&String::from_utf8_lossy(chunk.as_ref()));
                    // SSE allows CRLF line endings.
                    if buffer.contains('\r') {
                        buffer = buffer.replace("\r\n", "\n");
                    }
                }
                Some(Err(e)) => return Some((Err(e.into()), (bytes, buffer, true))),
                None => {
                    done = true;
                    buffer.push_str("\n\n");
                }
            }
        }
    })
}

/// The event in one SSE frame, if it carries data.
fn parse_frame(frame: &str) -> Option<Result<Event>> {
    let data: Vec<&str> = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|d| d.strip_prefix(' ').unwrap_or(d))
        .collect();
    if data.is_empty() {
        return None;
    }
    Some(
        serde_json::from_str(&data.join("\n"))
            .map(Event)
            .map_err(|e| Error::Decode(format!("event is not JSON: {e}"))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `chunks` to the SSE parser and returns the event types (or
    /// `log` for log events, `error` for a frame that is not JSON).
    async fn kinds(chunks: &[&'static str]) -> Vec<String> {
        let chunks: Vec<std::result::Result<&'static [u8], Error>> =
            chunks.iter().map(|c| Ok(c.as_bytes())).collect();
        sse_frames(futures_util::stream::iter(chunks))
            .map(|e| match e {
                Ok(e) if e.is_log() => "log".to_string(),
                Ok(e) => e.kind().unwrap_or("none").to_string(),
                Err(_) => "error".to_string(),
            })
            .collect()
            .await
    }

    #[tokio::test]
    async fn sse_vectors() {
        let vectors: [(&[&'static str], &[&str]); 9] = [
            (&["data: {\"type\":\"a\"}\n\n"], &["a"]),
            (
                &["data: {\"type\":\"a\"}\n\ndata: {\"type\":\"b\"}\n\n"],
                &["a", "b"],
            ),
            (&["data: {\"ty", "pe\":\"a\"}\n", "\n"], &["a"]),
            (&["data: {\"type\":\"a\"}\r\n\r\n"], &["a"]),
            (&[": keep-alive\n\n", ": keep-alive\n\n"], &[]),
            (&["event: x\nid: 3\ndata: {\"type\":\"a\"}\n\n"], &["a"]),
            (&["data: {\"@timestamp\":\"t\"}\n\n"], &["log"]),
            (
                &["data: not json\n\ndata: {\"type\":\"a\"}\n\n"],
                &["error", "a"],
            ),
            (&["data: {\"type\":\"a\"}"], &["a"]),
        ];
        for (chunks, expected) in vectors {
            assert_eq!(kinds(chunks).await, expected, "{chunks:?}");
        }
    }

    #[tokio::test]
    async fn multi_line_data_is_joined() {
        assert_eq!(kinds(&["data: {\"type\":\ndata: \"a\"}\n\n"]).await, ["a"]);
    }

    #[tokio::test]
    async fn frames_split_across_chunks_become_events() {
        let chunks: Vec<std::result::Result<&'static [u8], Error>> = vec![
            Ok(b": keep-alive\n\ndata: {\"type\":\"agent_start\",\"mo"),
            Ok(b"del\":\"m\"}\r\n\r\ndata: {\"@timestamp\":\"t\"}\n"),
            Ok(b"\n"),
        ];
        let events: Vec<Event> = sse_frames(futures_util::stream::iter(chunks))
            .map(|e| e.unwrap())
            .collect()
            .await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind(), Some("agent_start"));
        assert_eq!(events[0].get("model"), Some(&Value::from("m")));
        assert!(events[1].is_log() && events[1].kind().is_none());
    }

    #[tokio::test]
    async fn a_last_frame_without_its_blank_line_still_counts() {
        let chunks: Vec<std::result::Result<&'static [u8], Error>> =
            vec![Ok(b"data: {\"type\":\"cron_result\"}")];
        let events: Vec<_> = sse_frames(futures_util::stream::iter(chunks))
            .collect()
            .await;
        assert_eq!(events.len(), 1);
    }
}
