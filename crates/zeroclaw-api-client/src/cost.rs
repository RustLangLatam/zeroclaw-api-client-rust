//! Model usage and its cost.
//!
//! Gateway handler: `crates/zeroclaw-gateway/src/api.rs` (`/api/cost`);
//! types in `crates/zeroclaw-config/src/cost/types.rs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::client::{Client, query};
use crate::error::Result;

/// Cost endpoint, from [`Client::cost`].
#[derive(Debug, Clone, Copy)]
pub struct CostApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Usage and cost (`/api/cost`).
    pub fn cost(&self) -> CostApi<'_> {
        CostApi { client: self }
    }
}

impl CostApi<'_> {
    /// `GET /api/cost`: usage in `[from, to)` (RFC 3339; unbounded when
    /// unset), broken down by model and by agent.
    pub async fn summary(&self, from: Option<&str>, to: Option<&str>) -> Result<CostSummary> {
        self.fetch(&query(&[("from", from), ("to", to)])).await
    }

    /// `GET /api/cost?agent=`: one agent's session and daily figures (the
    /// gateway ignores a time window here).
    pub async fn agent(&self, agent: &str) -> Result<CostSummary> {
        self.fetch(&[("agent", agent)]).await
    }

    async fn fetch(&self, query: &[(&str, &str)]) -> Result<CostSummary> {
        #[derive(Deserialize)]
        struct Wrapped {
            cost: CostSummary,
        }
        let wrapped: Wrapped = self.client.get(&["api", "cost"], query).await?;
        Ok(wrapped.cost)
    }
}

/// Usage totals.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostSummary {
    /// Cost of the current session, USD.
    pub session_cost_usd: f64,
    /// Cost today, USD.
    pub daily_cost_usd: f64,
    /// Cost this month, USD.
    pub monthly_cost_usd: f64,
    /// Tokens in the window.
    pub total_tokens: u64,
    /// Model calls in the window.
    pub request_count: usize,
    /// By model.
    #[serde(default)]
    pub by_model: BTreeMap<String, ModelStats>,
    /// By agent.
    #[serde(default)]
    pub by_agent: BTreeMap<String, AgentCostStats>,
}

/// One model's usage.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelStats {
    /// Model.
    pub model: String,
    /// Cost, USD.
    pub cost_usd: f64,
    /// Input tokens.
    pub input_tokens: u64,
    /// Cached input tokens.
    #[serde(default)]
    pub cached_input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// All tokens.
    pub total_tokens: u64,
    /// Calls.
    pub request_count: usize,
}

/// One agent's usage.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AgentCostStats {
    /// Agent.
    pub agent_alias: String,
    /// Cost, USD.
    pub cost_usd: f64,
    /// All tokens.
    pub total_tokens: u64,
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Cached input tokens.
    #[serde(default)]
    pub cached_input_tokens: u64,
    /// Calls.
    pub request_count: usize,
}
