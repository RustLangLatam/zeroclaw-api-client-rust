//! Standard operating procedures (SOPs): authoring, graphs, runs, approvals
//! and cancellation.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/api_sop_author.rs` and
//! `api_sop.rs`. An SOP definition (`name`, `description`, `version`,
//! `priority`, `execution_mode`, `triggers`, `steps`, …) is large and
//! versioned with ZeroClaw, so it travels as `serde_json::Value`; so do
//! graphs and run overlays. Every route answers 503 while the SOP subsystem
//! is disabled.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{Client, query};
use crate::error::Result;

/// SOP endpoints, from [`Client::sops`].
#[derive(Debug, Clone, Copy)]
pub struct SopsApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Standard operating procedures (`/api/sops`).
    pub fn sops(&self) -> SopsApi<'_> {
        SopsApi { client: self }
    }
}

impl SopsApi<'_> {
    /// `GET /api/sops`: every SOP definition that loads.
    pub async fn list(&self) -> Result<Vec<Value>> {
        #[derive(Deserialize)]
        struct Sops {
            sops: Vec<Value>,
        }
        let sops: Sops = self.client.get(&["api", "sops"], &[]).await?;
        Ok(sops.sops)
    }

    /// `GET /api/sops/{name}/full`: one SOP definition.
    pub async fn get(&self, name: &str) -> Result<Value> {
        self.client.get(&["api", "sops", name, "full"], &[]).await
    }

    /// `POST /api/sops`: creates an SOP (409 when the name exists).
    pub async fn create(&self, sop: &Value) -> Result<()> {
        let request = self
            .client
            .request(Method::POST, &["api", "sops"], &[])?
            .json(sop);
        self.client.empty(request).await
    }

    /// `PUT /api/sops/{name}`: saves an SOP (steps are renumbered 1..N).
    pub async fn save(&self, name: &str, sop: &Value) -> Result<()> {
        let request = self
            .client
            .request(Method::PUT, &["api", "sops", name], &[])?
            .json(sop);
        self.client.empty(request).await
    }

    /// `DELETE /api/sops/{name}`: deletes an SOP.
    pub async fn delete(&self, name: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::DELETE, &["api", "sops", name], &[])?;
        self.client.empty(request).await
    }

    /// `GET /api/sops/{name}/graph`: the SOP as a node graph with layout and
    /// diagnostics.
    pub async fn graph(&self, name: &str) -> Result<Value> {
        self.client.get(&["api", "sops", name, "graph"], &[]).await
    }

    /// `POST /api/sops/graph-draft`: the graph of an unsaved definition.
    pub async fn graph_draft(&self, sop: &Value) -> Result<Value> {
        self.client
            .send_json(
                Method::POST,
                &["api", "sops", "graph-draft"],
                &[],
                &serde_json::json!({ "sop": sop }),
            )
            .await
    }

    /// `POST /api/sops/wire-draft`: applies a wiring edit to an unsaved
    /// definition; returns `{sop, graph}`. `edit` is `{op: "connect" |
    /// "disconnect", from, to, role, port?}`.
    pub async fn wire_draft(&self, sop: &Value, edit: &Value) -> Result<Value> {
        self.client
            .send_json(
                Method::POST,
                &["api", "sops", "wire-draft"],
                &[],
                &serde_json::json!({ "sop": sop, "edit": edit }),
            )
            .await
    }

    /// `GET /api/sops/graph-legend`: what the graph's roles, pins and run
    /// states mean.
    pub async fn graph_legend(&self) -> Result<Value> {
        self.client.get(&["api", "sops", "graph-legend"], &[]).await
    }

    /// `GET /api/sops/trigger-sources`: the trigger sources, their fields and
    /// the channels that can start an SOP.
    pub async fn trigger_sources(&self) -> Result<Value> {
        self.client
            .get(&["api", "sops", "trigger-sources"], &[])
            .await
    }

    /// `POST /api/sops/{name}/run`: starts the SOP through its manual
    /// trigger; `payload` is JSON text handed to the run. Returns the run id
    /// (an existing run's when the SOP coalesces).
    pub async fn run(&self, name: &str, payload: Option<&str>) -> Result<String> {
        #[derive(Deserialize)]
        struct Started {
            run_id: String,
        }
        let started: Started = self
            .client
            .send_json(
                Method::POST,
                &["api", "sops", name, "run"],
                &[],
                &serde_json::json!({ "payload": payload }),
            )
            .await?;
        Ok(started.run_id)
    }

    /// `GET /api/sops/runs`: active and recent runs, newest first (of one
    /// SOP with `sop`).
    pub async fn runs(&self, sop: Option<&str>) -> Result<Vec<SopRun>> {
        #[derive(Deserialize)]
        struct Runs {
            runs: Vec<SopRun>,
        }
        let runs: Runs = self
            .client
            .get(&["api", "sops", "runs"], &query(&[("sop", sop)]))
            .await?;
        Ok(runs.runs)
    }

    /// `GET /ws/sops/runs`: follows SOP runs live: a
    /// `{"type": "snapshot", "runs": […]}` first, then `{"type": "run",
    /// "run": …}` on every change (`lagged`, `disabled` and `error` frames
    /// too).
    pub async fn watch_runs(&self) -> Result<crate::chat::JsonFeed> {
        let socket = crate::chat::connect(self.client, &["ws", "sops", "runs"], &[]).await?;
        Ok(crate::chat::json_frames(socket))
    }

    /// `GET /api/sops/{name}/runs/{run_id}/overlay`: a run's state per step,
    /// with its tool calls.
    pub async fn run_overlay(&self, name: &str, run_id: &str) -> Result<Value> {
        self.client
            .get(&["api", "sops", name, "runs", run_id, "overlay"], &[])
            .await
    }

    /// `POST /api/sops/{name}/runs/{run_id}/decide`: answers a run waiting
    /// for approval or at a checkpoint. Returns the run overlay.
    pub async fn decide(&self, name: &str, run_id: &str, decision: &Decision) -> Result<Value> {
        self.client
            .send_json(
                Method::POST,
                &["api", "sops", name, "runs", run_id, "decide"],
                &[],
                decision,
            )
            .await
    }

    /// `POST /api/sops/{name}/runs/{run_id}/cancel`: cancels a run at its
    /// next step boundary.
    pub async fn cancel(&self, name: &str, run_id: &str, reason: Option<&str>) -> Result<Value> {
        self.client
            .send_json(
                Method::POST,
                &["api", "sops", name, "runs", run_id, "cancel"],
                &[],
                &serde_json::json!({ "reason": reason }),
            )
            .await
    }

    /// `GET /admin/sop/pending`: runs waiting for an approval or a
    /// checkpoint. Loopback, or remote admin with pairing.
    pub async fn pending(&self) -> Result<Vec<PendingApproval>> {
        #[derive(Deserialize)]
        struct Pending {
            pending: Vec<PendingApproval>,
        }
        let pending: Pending = self.client.get(&["admin", "sop", "pending"], &[]).await?;
        Ok(pending.pending)
    }

    /// `POST /admin/sop/approve`: approves a waiting run. Returns the
    /// outcome (`resumed`, `already_resolved`, `pending_quorum (…)`, …).
    pub async fn approve(&self, run_id: &str) -> Result<String> {
        self.resolve("approve", run_id, None).await
    }

    /// `POST /admin/sop/deny`: denies a waiting run.
    pub async fn deny(&self, run_id: &str, reason: Option<&str>) -> Result<String> {
        self.resolve("deny", run_id, reason).await
    }

    async fn resolve(&self, action: &str, run_id: &str, reason: Option<&str>) -> Result<String> {
        #[derive(Deserialize)]
        struct Resolved {
            outcome: String,
        }
        let resolved: Resolved = self
            .client
            .send_json(
                Method::POST,
                &["admin", "sop", action],
                &[],
                &serde_json::json!({ "run_id": run_id, "reason": reason }),
            )
            .await?;
        Ok(resolved.outcome)
    }
}

/// An answer to a waiting SOP run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Go on.
    Approve,
    /// Stop the run.
    Deny {
        /// Why.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// Change the checkpoint's text and go on.
    Amend {
        /// New text.
        text: String,
    },
    /// Redo the step with guidance.
    Revise {
        /// Guidance.
        guidance: String,
    },
}

/// A run in a listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SopRun {
    /// Run id.
    pub run_id: String,
    /// SOP.
    pub sop_name: String,
    /// `pending`, `running`, `cancel_requested`, `waiting_approval`,
    /// `paused_checkpoint`, `completed`, `failed` or `cancelled`.
    pub status: String,
    /// Current step.
    pub current_step: u32,
    /// Steps in all.
    pub total_steps: u32,
    /// Start (RFC 3339).
    pub started_at: String,
    /// End.
    #[serde(default)]
    pub completed_at: Option<String>,
    /// What started it.
    pub trigger_source: String,
    /// Whether it is still running.
    pub active: bool,
}

/// A run waiting for an operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingApproval {
    /// Run id.
    pub run_id: String,
    /// SOP.
    pub sop_name: String,
    /// Step waiting.
    pub step: u32,
    /// Steps in all.
    pub total_steps: u32,
    /// Since when.
    #[serde(default)]
    pub waiting_since: Option<String>,
    /// `approval` or `checkpoint`.
    pub kind: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_are_externally_tagged() {
        assert_eq!(serde_json::to_value(Decision::Approve).unwrap(), "approve");
        assert_eq!(
            serde_json::to_value(Decision::Deny {
                reason: Some("no".into())
            })
            .unwrap(),
            serde_json::json!({ "deny": { "reason": "no" } })
        );
        assert_eq!(
            serde_json::to_value(Decision::Revise {
                guidance: "shorter".into()
            })
            .unwrap(),
            serde_json::json!({ "revise": { "guidance": "shorter" } })
        );
    }
}
