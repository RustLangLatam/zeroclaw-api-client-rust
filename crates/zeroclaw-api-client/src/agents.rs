//! Per-agent resources: the agent's workspace files, its personality files
//! and the skills it sees. Agents themselves are configuration entries
//! (`agents.<alias>`), created and removed with [`crate::config`].
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/api_browse.rs`,
//! `api_personality.rs` and `api_skills.rs`. Workspace paths are relative to
//! the agent's workspace; escaping it is refused, and the personality files
//! and `sessions` at its root are protected.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::Result;
use crate::files::{Listing, ReadFile};

/// One agent's endpoints, from [`Client::agent`].
#[derive(Debug, Clone, Copy)]
pub struct AgentApi<'a> {
    client: &'a Client,
    alias: &'a str,
}

impl Client {
    /// The endpoints of agent `alias`.
    pub fn agent<'a>(&'a self, alias: &'a str) -> AgentApi<'a> {
        AgentApi {
            client: self,
            alias,
        }
    }
}

impl AgentApi<'_> {
    /// `GET /api/agents/{alias}/workspace/list`: a workspace directory (the
    /// root without `path`).
    pub async fn list_files(&self, path: Option<&str>) -> Result<Listing> {
        let query: Vec<(&str, &str)> = path.map(|p| ("path", p)).into_iter().collect();
        self.client.get(&self.workspace("list"), &query).await
    }

    /// `GET /api/agents/{alias}/workspace/read`: a workspace file (up to
    /// 4 MiB; binary files come base64-encoded).
    pub async fn read_file(&self, path: &str) -> Result<ReadFile> {
        self.client
            .get(&self.workspace("read"), &[("path", path)])
            .await
    }

    /// `POST /api/agents/{alias}/workspace/mkdir`: creates a directory.
    pub async fn make_dir(&self, path: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::POST, &self.workspace("mkdir"), &[])?
            .json(&serde_json::json!({ "path": path }));
        self.client.empty(request).await
    }

    /// `POST /api/agents/{alias}/workspace/move`: moves or renames a file or
    /// directory.
    pub async fn move_path(&self, from: &str, to: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::POST, &self.workspace("move"), &[])?
            .json(&serde_json::json!({ "from": from, "to": to }));
        self.client.empty(request).await
    }

    /// `DELETE /api/agents/{alias}/workspace/path`: removes a file or
    /// directory.
    pub async fn remove_path(&self, path: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::DELETE, &self.workspace("path"), &[])?
            .json(&serde_json::json!({ "path": path }));
        self.client.empty(request).await
    }

    /// `GET /api/personality`: the agent's personality files and whether
    /// each exists.
    pub async fn personality_files(&self) -> Result<PersonalityFiles> {
        self.client
            .get(&["api", "personality"], &[("agent", self.alias)])
            .await
    }

    /// `GET /api/personality/{filename}`: one personality file (`SOUL.md`,
    /// `IDENTITY.md`, `USER.md`, `AGENTS.md`, `TOOLS.md`, `HEARTBEAT.md`,
    /// `MEMORY.md`).
    pub async fn personality(&self, filename: &str) -> Result<PersonalityFile> {
        self.client
            .get(&["api", "personality", filename], &[("agent", self.alias)])
            .await
    }

    /// `PUT /api/personality/{filename}`: replaces a personality file (at
    /// most 20 000 characters). With `expected_mtime_ms`, the write is
    /// refused (409) if the file changed since it was read.
    pub async fn write_personality(
        &self,
        filename: &str,
        content: &str,
        expected_mtime_ms: Option<i64>,
    ) -> Result<PersonalityWritten> {
        self.client
            .send_json(
                Method::PUT,
                &["api", "personality", filename],
                &[("agent", self.alias)],
                &serde_json::json!({
                    "content": content,
                    "expected_mtime_ms": expected_mtime_ms,
                }),
            )
            .await
    }

    /// `GET /api/personality/templates`: the default personality files
    /// rendered for this agent, without writing them. `context` may set
    /// `agent_name`, `user_name`, `timezone`, `communication_style` and
    /// `include_memory`.
    pub async fn personality_templates(
        &self,
        context: &[(&str, &str)],
    ) -> Result<PersonalityTemplates> {
        let mut query = vec![("agent", self.alias)];
        query.extend_from_slice(context);
        self.client
            .get(&["api", "personality", "templates"], &query)
            .await
    }

    /// `GET /api/agents/{alias}/skills`: the skills the agent loads, where
    /// each comes from, and those dropped by the audit.
    pub async fn skills(&self) -> Result<AgentSkills> {
        self.client
            .get(&["api", "agents", self.alias, "skills"], &[])
            .await
    }

    fn workspace<'b>(&'b self, action: &'b str) -> [&'b str; 5] {
        ["api", "agents", self.alias, "workspace", action]
    }
}

/// The personality files of an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalityFiles {
    /// Each editable file.
    pub files: Vec<PersonalityFileEntry>,
    /// Maximum characters per file.
    pub max_chars: usize,
}

/// A personality file's presence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalityFileEntry {
    /// File name.
    pub filename: String,
    /// Whether it exists.
    pub exists: bool,
    /// Size in bytes.
    #[serde(default)]
    pub size: u64,
    /// Modification time, ms since the epoch.
    #[serde(default)]
    pub mtime_ms: Option<i64>,
}

/// A personality file's content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalityFile {
    /// File name.
    pub filename: String,
    /// Content (empty when the file does not exist).
    #[serde(default)]
    pub content: Option<String>,
    /// Whether it exists.
    pub exists: bool,
    /// Whether the content was cut at the size limit.
    pub truncated: bool,
    /// Modification time, for [`AgentApi::write_personality`].
    #[serde(default)]
    pub mtime_ms: Option<i64>,
}

/// Result of [`AgentApi::write_personality`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalityWritten {
    /// Bytes written.
    pub bytes_written: u64,
    /// New modification time.
    #[serde(default)]
    pub mtime_ms: Option<i64>,
}

/// Rendered default personality files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalityTemplates {
    /// Preset used.
    pub preset: String,
    /// Each file: `{filename, content}`.
    pub files: Vec<Value>,
}

/// The skills an agent loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentSkills {
    /// The agent.
    pub agent: String,
    /// Loaded skills: `name`, `description`, `origin` (`workspace`,
    /// `open-skills`, `plugin`, `bundle`), `editable`, …
    pub skills: Vec<Value>,
    /// Skills left out, with `reason_kind` and `reason`.
    #[serde(default)]
    pub dropped: Vec<Value>,
}
