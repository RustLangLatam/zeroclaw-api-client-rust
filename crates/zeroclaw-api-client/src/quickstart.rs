//! The quickstart: setting up a first agent (model provider, risk and
//! runtime profiles, memory, channels and personality) in one submission.
//!
//! Gateway handler: `crates/zeroclaw-gateway/src/api_quickstart.rs`;
//! submission type `BuilderSubmission` in
//! `crates/zeroclaw-config/src/presets.rs`. Validation problems come back in
//! a successful answer (`{"kind": "errors", …}`), not as HTTP errors.

use reqwest::Method;
use serde_json::Value;

use crate::client::Client;
use crate::error::Result;

/// Quickstart endpoints, from [`Client::quickstart`].
#[derive(Debug, Clone, Copy)]
pub struct QuickstartApi<'a> {
    client: &'a Client,
}

impl Client {
    /// The first-agent quickstart (`/api/quickstart`).
    pub fn quickstart(&self) -> QuickstartApi<'_> {
        QuickstartApi { client: self }
    }
}

impl QuickstartApi<'_> {
    /// `GET /api/quickstart/state`: what exists and what can be chosen
    /// (provider and channel types, presets, memory kinds, …).
    pub async fn state(&self) -> Result<Value> {
        self.client.get(&["api", "quickstart", "state"], &[]).await
    }

    /// `POST /api/quickstart/fields`: the fields a `section`
    /// (`model_provider`, `channel` or `peer_group`) of type `type_key`
    /// needs.
    pub async fn fields(&self, section: &str, type_key: &str) -> Result<Value> {
        self.client
            .send_json(
                Method::POST,
                &["api", "quickstart", "fields"],
                &[],
                &serde_json::json!({ "section": section, "type_key": type_key }),
            )
            .await
    }

    /// `POST /api/quickstart/validate`: checks a submission;
    /// `{"kind": "ok"}` or `{"kind": "errors", "errors": […]}`.
    pub async fn validate(&self, submission: &Value) -> Result<Value> {
        self.client
            .send_json(
                Method::POST,
                &["api", "quickstart", "validate"],
                &[],
                submission,
            )
            .await
    }

    /// `POST /api/quickstart/apply`: saves a submission. On
    /// `{"kind": "applied", …, "daemon_restarted": true}` the gateway drops
    /// connections briefly while it reloads.
    pub async fn apply(&self, submission: &Value) -> Result<Value> {
        self.client
            .send_json(
                Method::POST,
                &["api", "quickstart", "apply"],
                &[],
                submission,
            )
            .await
    }

    /// `POST /api/quickstart/dismiss`: records that the quickstart was closed
    /// (`surface`: `web`, `tui`, `cli` or `test`).
    pub async fn dismiss(
        &self,
        run_id: &str,
        surface: &str,
        last_step: Option<&str>,
    ) -> Result<()> {
        let request = self
            .client
            .request(Method::POST, &["api", "quickstart", "dismiss"], &[])?
            .json(&serde_json::json!({
                "run_id": run_id,
                "surface": surface,
                "last_step": last_step,
            }));
        self.client.empty(request).await
    }
}
