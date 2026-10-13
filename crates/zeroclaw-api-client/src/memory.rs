//! Long-term memory: entries the agents store and recall.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/api.rs` (`/api/memory*`);
//! entry type in `crates/zeroclaw-api/src/memory_traits.rs`.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{Client, query};
use crate::error::Result;

/// Memory endpoints, from [`Client::memory`].
#[derive(Debug, Clone, Copy)]
pub struct MemoryApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Long-term memory (`/api/memory`).
    pub fn memory(&self) -> MemoryApi<'_> {
        MemoryApi { client: self }
    }
}

impl MemoryApi<'_> {
    /// `GET /api/memory`: with a text or time filter, the best matches
    /// (at most 50); otherwise every entry (of `category`, when set).
    /// Contents longer than 4096 characters come truncated.
    pub async fn search(&self, filter: &MemoryQuery<'_>) -> Result<Vec<MemoryEntry>> {
        #[derive(Deserialize)]
        struct Entries {
            entries: Vec<MemoryEntry>,
        }
        let entries: Entries = self
            .client
            .get(
                &["api", "memory"],
                &query(&[
                    ("query", filter.query),
                    ("category", filter.category),
                    ("since", filter.since),
                    ("until", filter.until),
                    ("agent", filter.agent),
                ]),
            )
            .await?;
        Ok(entries.entries)
    }

    /// `POST /api/memory`: stores `content` under `key` (category `core`
    /// unless given), in `agent`'s memory or the shared one.
    pub async fn store(
        &self,
        key: &str,
        content: &str,
        category: Option<&str>,
        agent: Option<&str>,
    ) -> Result<()> {
        let request = self
            .client
            .request(Method::POST, &["api", "memory"], &[])?
            .json(&serde_json::json!({
                "key": key,
                "content": content,
                "category": category,
                "agent": agent,
            }));
        self.client.empty(request).await
    }

    /// `DELETE /api/memory/{key}`: forgets an entry; returns whether it
    /// existed.
    pub async fn forget(&self, key: &str, agent: Option<&str>) -> Result<bool> {
        #[derive(Deserialize)]
        struct Deleted {
            deleted: bool,
        }
        let request = self.client.request(
            Method::DELETE,
            &["api", "memory", key],
            &query(&[("agent", agent)]),
        )?;
        let deleted: Deleted = self.client.json(request).await?;
        Ok(deleted.deleted)
    }
}

/// Filters of [`MemoryApi::search`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryQuery<'a> {
    /// Text to recall by.
    pub query: Option<&'a str>,
    /// Exact category (`core`, `daily`, `conversation` or custom).
    pub category: Option<&'a str>,
    /// Entries at or after (RFC 3339).
    pub since: Option<&'a str>,
    /// Entries before (RFC 3339).
    pub until: Option<&'a str>,
    /// An agent's memory instead of the shared one.
    pub agent: Option<&'a str>,
}

/// A memory entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Entry id.
    pub id: String,
    /// Key.
    pub key: String,
    /// Content.
    pub content: String,
    /// Category.
    pub category: String,
    /// When it was stored.
    pub timestamp: String,
    /// Session it came from.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Relevance of a recall match.
    #[serde(default)]
    pub score: Option<f64>,
    /// Namespace.
    #[serde(default)]
    pub namespace: Option<String>,
    /// Importance.
    #[serde(default)]
    pub importance: Option<f64>,
    /// Entry that replaced it.
    #[serde(default)]
    pub superseded_by: Option<String>,
    /// `episodic`, `procedural` or `{"semantic": "fact"|…}`.
    #[serde(default)]
    pub kind: Option<Value>,
    /// Whether it is pinned.
    #[serde(default)]
    pub pinned: bool,
    /// Owning agent.
    #[serde(default)]
    pub agent_alias: Option<String>,
}
