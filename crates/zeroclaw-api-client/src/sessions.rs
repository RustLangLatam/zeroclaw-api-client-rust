//! Conversation sessions: listing, transcripts, state, renaming, aborting
//! and adding messages.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/api.rs`
//! (`/api/sessions*`). A session id is either the display id or the full
//! session key (`gw_…`, `whatsapp_…`); the gateway resolves both.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Result;

/// Session endpoints, from [`Client::sessions`].
#[derive(Debug, Clone, Copy)]
pub struct SessionsApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Conversation sessions (`/api/sessions`).
    pub fn sessions(&self) -> SessionsApi<'_> {
        SessionsApi { client: self }
    }
}

impl SessionsApi<'_> {
    /// `GET /api/sessions`: every stored session (empty when the gateway
    /// keeps none).
    pub async fn list(&self) -> Result<Vec<Session>> {
        #[derive(Deserialize)]
        struct Sessions {
            sessions: Vec<Session>,
        }
        let sessions: Sessions = self.client.get(&["api", "sessions"], &[]).await?;
        Ok(sessions.sessions)
    }

    /// `GET /api/sessions/running`: the gateway (web chat) sessions.
    pub async fn running(&self) -> Result<Vec<Session>> {
        #[derive(Deserialize)]
        struct Sessions {
            sessions: Vec<Session>,
        }
        let sessions: Sessions = self
            .client
            .get(&["api", "sessions", "running"], &[])
            .await?;
        Ok(sessions.sessions)
    }

    /// `GET /api/sessions/{id}/messages`: the transcript (empty for an
    /// unknown session).
    pub async fn messages(&self, id: &str) -> Result<Vec<SessionMessage>> {
        #[derive(Deserialize)]
        struct Messages {
            messages: Vec<SessionMessage>,
        }
        let messages: Messages = self
            .client
            .get(&["api", "sessions", id, "messages"], &[])
            .await?;
        Ok(messages.messages)
    }

    /// `POST /api/sessions/{id}/messages`: appends an assistant message to
    /// the session (shown to chat clients; no agent turn runs).
    pub async fn add_message(&self, id: &str, content: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::POST, &["api", "sessions", id, "messages"], &[])?
            .json(&serde_json::json!({ "content": content }));
        self.client.empty(request).await
    }

    /// `GET /api/sessions/{id}/state`: whether a turn is running.
    pub async fn state(&self, id: &str) -> Result<SessionState> {
        self.client
            .get(&["api", "sessions", id, "state"], &[])
            .await
    }

    /// `PUT /api/sessions/{id}`: renames a session.
    pub async fn rename(&self, id: &str, name: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::PUT, &["api", "sessions", id], &[])?
            .json(&serde_json::json!({ "name": name }));
        self.client.empty(request).await
    }

    /// `DELETE /api/sessions/{id}`: cancels its running turn and deletes it.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::DELETE, &["api", "sessions", id], &[])?;
        self.client.empty(request).await
    }

    /// `POST /api/sessions/{id}/abort`: cancels the running turn; returns
    /// whether there was one.
    pub async fn abort(&self, id: &str) -> Result<bool> {
        #[derive(Deserialize)]
        struct Aborted {
            status: String,
        }
        let request = self
            .client
            .request(Method::POST, &["api", "sessions", id, "abort"], &[])?;
        let aborted: Aborted = self.client.json(request).await?;
        Ok(aborted.status == "aborted")
    }
}

/// A stored session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Display id (`gw_` removed).
    pub session_id: String,
    /// Full key (`gw_…`, `whatsapp_<channel>_<peer>…`).
    #[serde(default)]
    pub session_key: Option<String>,
    /// Creation time (RFC 3339).
    pub created_at: String,
    /// Last activity (RFC 3339).
    pub last_activity: String,
    /// Messages in it.
    pub message_count: usize,
    /// Agent that answers it.
    #[serde(default)]
    pub agent_alias: Option<String>,
    /// Channel it came through (`whatsapp.alias`).
    #[serde(default)]
    pub channel_id: Option<String>,
    /// Name given to it.
    #[serde(default)]
    pub name: Option<String>,
}

/// A message of a transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionMessage {
    /// `user`, `assistant`, `tool`, …
    pub role: String,
    /// Content (tool calls and results are JSON text).
    pub content: String,
    /// When it was stored.
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Whether a session is busy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionState {
    /// Session id as asked.
    pub session_id: String,
    /// `idle`, `running` or `error`.
    pub state: String,
    /// The running turn.
    #[serde(default)]
    pub turn_id: Option<String>,
    /// When it started.
    #[serde(default)]
    pub turn_started_at: Option<String>,
}
