//! One-shot messages to an agent: the generic webhook and the A2A
//! (agent-to-agent) JSON-RPC endpoint.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/lib.rs`
//! (`handle_webhook`) and `a2a.rs`. Both run a whole agent turn and answer
//! with its reply; for streaming, tool approvals or a lasting conversation
//! use [`crate::chat`].

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{Client, query};
use crate::error::{Error, Result};

/// Webhook and A2A endpoints, from [`Client::webhook`].
#[derive(Debug, Clone, Copy)]
pub struct WebhookApi<'a> {
    client: &'a Client,
}

impl Client {
    /// The generic webhook and A2A.
    pub fn webhook(&self) -> WebhookApi<'_> {
        WebhookApi { client: self }
    }
}

/// Optional headers of a webhook call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebhookOptions<'a> {
    /// Agent to answer (the default agent without one).
    pub agent: Option<&'a str>,
    /// `X-Webhook-Secret`, when the gateway sets `webhook_secret`.
    pub secret: Option<&'a str>,
    /// `X-Idempotency-Key`: a repeat within the gateway's window is not run
    /// again.
    pub idempotency_key: Option<&'a str>,
    /// `X-Session-Id`: keeps calls in one conversation (`[A-Za-z0-9._-]`,
    /// at most 128 characters).
    pub session_id: Option<&'a str>,
}

impl WebhookApi<'_> {
    /// `POST /webhook`: sends `message` and returns the agent's reply. A
    /// duplicate idempotency key answers [`WebhookReply::Duplicate`].
    pub async fn send(&self, message: &str, options: WebhookOptions<'_>) -> Result<WebhookReply> {
        let mut request = self
            .client
            .request(
                Method::POST,
                &["webhook"],
                &query(&[("agent", options.agent)]),
            )?
            .json(&serde_json::json!({ "message": message }));
        for (header, value) in [
            ("X-Webhook-Secret", options.secret),
            ("X-Idempotency-Key", options.idempotency_key),
            ("X-Session-Id", options.session_id),
        ] {
            if let Some(value) = value {
                request = request.header(header, value);
            }
        }
        let body: Value = self.client.json(request).await?;
        if body["status"] == "duplicate" {
            return Ok(WebhookReply::Duplicate);
        }
        serde_json::from_value(body)
            .map(WebhookReply::Reply)
            .map_err(|e| Error::Decode(e.to_string()))
    }

    /// `POST /a2a/{alias}` with `message/send`: sends `text` to a published
    /// agent and returns the text of its answer. Needs the `a2a` feature and
    /// `a2a.server.enabled` on the gateway.
    pub async fn a2a_send(&self, alias: &str, text: &str) -> Result<String> {
        let body: Value = self
            .client
            .send_json(
                Method::POST,
                &["a2a", alias],
                &[],
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "message/send",
                    "params": { "message": { "parts": [{ "kind": "text", "text": text }] } },
                }),
            )
            .await?;
        if let Some(error) = body.get("error") {
            return Err(Error::Api {
                status: 200,
                message: error["message"].as_str().unwrap_or("A2A error").to_string(),
                body: Some(body.clone()),
            });
        }
        Ok(body["result"]["artifacts"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|artifact| artifact["parts"].as_array().into_iter().flatten())
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// `GET /a2a/{alias}/.well-known/agent-card.json`: a published agent's
    /// A2A card. Public.
    pub async fn agent_card(&self, alias: &str) -> Result<Value> {
        self.client
            .get(&["a2a", alias, ".well-known", "agent-card.json"], &[])
            .await
    }

    /// `GET /.well-known/agents-card.json`: the gateway's A2A catalog card,
    /// whose skills list the published agents. Public. 404 when the gateway was built
    /// without the `a2a` feature or `[a2a.server] enabled` is false.
    pub async fn agent_catalog(&self) -> Result<Value> {
        self.client
            .get(&[".well-known", "agents-card.json"], &[])
            .await
    }
}

/// Answer of [`WebhookApi::send`].
#[derive(Debug, Clone, PartialEq)]
pub enum WebhookReply {
    /// The agent's reply.
    Reply(AgentReply),
    /// The idempotency key was already used: nothing ran.
    Duplicate,
}

/// An agent's reply to a webhook.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentReply {
    /// The reply.
    pub response: String,
    /// Model that answered.
    pub model: String,
}
