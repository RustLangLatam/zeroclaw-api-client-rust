//! The chat WebSocket against a minimal gateway that speaks its frames.

#![allow(clippy::disallowed_methods)] // The fake gateway runs on its own task.
// tungstenite fixes the handshake callback's error type.
#![allow(clippy::result_large_err)]

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::{Message, http};
use zeroclaw_api_client::chat::{ChatFrame, ChatOptions};
use zeroclaw_api_client::{Client, Error};

const TOKEN: &str = "zc_chat";

/// A gateway that checks the handshake, opens session `s1` and answers each
/// message with the frames `reply` gives for it.
async fn gateway(reply: fn(&str) -> Vec<Value>) -> Client {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let check = |request: &Request,
                     mut response: Response|
         -> Result<Response, ErrorResponse> {
            let authorized = request
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                == Some(&format!("Bearer {TOKEN}"));
            let query = request.uri().query().unwrap_or_default();
            if !authorized || !query.contains("agent=helper") {
                let mut refusal = ErrorResponse::new(Some("Unauthorized: provide a token".into()));
                *refusal.status_mut() = http::StatusCode::UNAUTHORIZED;
                return Err(refusal);
            }
            response.headers_mut().insert(
                "Sec-WebSocket-Protocol",
                http::HeaderValue::from_static("zeroclaw.v1"),
            );
            Ok(response)
        };
        let Ok(mut socket) = tokio_tungstenite::accept_hdr_async(stream, check).await else {
            return;
        };
        let start = json!({ "type": "session_start", "session_id": "s1", "resumed": false, "message_count": 0 });
        socket
            .send(Message::Text(start.to_string().into()))
            .await
            .unwrap();
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let frame: Value = serde_json::from_str(&text).unwrap();
            for out in reply(frame["content"].as_str().unwrap_or_default()) {
                socket
                    .send(Message::Text(out.to_string().into()))
                    .await
                    .unwrap();
            }
        }
    });
    Client::new(base, TOKEN).unwrap()
}

fn answer(content: &str) -> Vec<Value> {
    vec![
        json!({ "type": "chunk", "content": "Hel" }),
        json!({ "type": "tool_call", "id": "c1", "name": "search", "args": { "q": content } }),
        json!({ "type": "plan", "entries": [] }),
        json!({ "type": "done", "full_response": format!("Hello, {content}"), "model": "m",
                "provider": "p", "tokens_used": 5, "input_tokens": 3, "output_tokens": 2,
                "cost_usd": null, "max_context_tokens": 8192, "last_input_tokens": 3 }),
    ]
}

#[tokio::test]
async fn a_question_gets_the_turns_reply() {
    let client = gateway(answer).await;
    let mut chat = client.chat("helper", ChatOptions::default()).await.unwrap();
    assert_eq!(chat.session_id(), "s1");
    let reply = chat.ask("Ana").await.unwrap();
    assert_eq!(reply.full_response, "Hello, Ana");
    assert_eq!(reply.tokens_used, Some(5));
}

#[tokio::test]
async fn frames_can_be_read_one_by_one() {
    let client = gateway(answer).await;
    let mut chat = client.chat("helper", ChatOptions::default()).await.unwrap();
    chat.send("Ana").await.unwrap();
    assert_eq!(
        chat.next_frame().await.unwrap(),
        Some(ChatFrame::Chunk("Hel".into()))
    );
    assert!(matches!(
        chat.next_frame().await.unwrap(),
        Some(ChatFrame::ToolCall { ref name, .. }) if name == "search"
    ));
    assert!(matches!(
        chat.next_frame().await.unwrap(),
        Some(ChatFrame::Other(_))
    ));
    assert!(matches!(
        chat.next_frame().await.unwrap(),
        Some(ChatFrame::Done(_))
    ));
    chat.close().await.unwrap();
}

#[tokio::test]
async fn a_failed_turn_is_an_error_with_its_code() {
    let client = gateway(|_| {
        vec![json!({ "type": "error", "message": "provider down", "code": "PROVIDER_ERROR" })]
    })
    .await;
    let mut chat = client.chat("helper", ChatOptions::default()).await.unwrap();
    match chat.ask("Hi").await.unwrap_err() {
        Error::Turn { code, message } => {
            assert_eq!(
                (code.as_deref(), message.as_str()),
                (Some("PROVIDER_ERROR"), "provider down")
            );
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn a_refused_handshake_keeps_its_status() {
    let client = gateway(answer).await.with_token("zc_wrong");
    let error = client
        .chat("helper", ChatOptions::default())
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(401));
    assert!(error.to_string().contains("Unauthorized"));
}

/// A gateway that, on `route`, pushes `frames` and closes.
async fn pushing_gateway(route: &'static str, frames: Vec<Value>) -> Client {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let check =
            move |request: &Request, mut response: Response| -> Result<Response, ErrorResponse> {
                assert_eq!(request.uri().path(), route);
                response.headers_mut().insert(
                    "Sec-WebSocket-Protocol",
                    http::HeaderValue::from_static("zeroclaw.v1"),
                );
                Ok(response)
            };
        let mut socket = tokio_tungstenite::accept_hdr_async(stream, check)
            .await
            .unwrap();
        for frame in frames {
            socket
                .send(Message::Text(frame.to_string().into()))
                .await
                .unwrap();
        }
        socket.close(None).await.unwrap();
    });
    Client::new(base, TOKEN).unwrap()
}

#[tokio::test]
async fn sop_runs_and_canvases_stream_their_frames() {
    let client = pushing_gateway(
        "/ws/sops/runs",
        vec![
            json!({ "type": "snapshot", "runs": [] }),
            json!({ "type": "run", "run": { "run_id": "r1" } }),
        ],
    )
    .await;
    let frames: Vec<Value> = client
        .sops()
        .watch_runs()
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await;
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[1]["run"]["run_id"], "r1");

    let client = pushing_gateway(
        "/ws/canvas/board",
        vec![json!({ "type": "frame", "canvas_id": "board", "frame": { "content": "# Hi" } })],
    )
    .await;
    let frames: Vec<Value> = client
        .canvas()
        .watch("board")
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await;
    assert_eq!(frames[0]["frame"]["content"], "# Hi");
}
