//! Hardware-key (WebAuthn) registration and assertion, an extra check on
//! top of the bearer token.
//!
//! Gateway handler: `crates/zeroclaw-gateway/src/api_webauthn.rs` (built
//! with the `webauthn` feature; 404 when it is off). The ceremony itself runs
//! in the authenticator (a browser or a security-key library): this module
//! only carries its JSON, with ZeroClaw's snake_case field names and
//! base64url binary fields.

use reqwest::Method;
use serde_json::Value;

use crate::client::Client;
use crate::error::Result;

/// WebAuthn endpoints, from [`Client::webauthn`].
#[derive(Debug, Clone, Copy)]
pub struct WebauthnApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Hardware-key registration and assertion (`/api/webauthn`).
    pub fn webauthn(&self) -> WebauthnApi<'_> {
        WebauthnApi { client: self }
    }
}

impl WebauthnApi<'_> {
    /// `POST /api/webauthn/register/start`: the creation challenge for a new
    /// key of `user_id`.
    pub async fn register_start(&self, user_id: &str, user_name: &str) -> Result<Value> {
        self.post(
            "register/start",
            &serde_json::json!({ "user_id": user_id, "user_name": user_name }),
        )
        .await
    }

    /// `POST /api/webauthn/register/finish`: the authenticator's answer
    /// (`challenge`, `id`, `attestation_object`, `client_data_json`,
    /// `label?`). Returns `{credential_id, label, registered_at}`.
    pub async fn register_finish(&self, credential: &Value) -> Result<Value> {
        self.post("register/finish", credential).await
    }

    /// `POST /api/webauthn/auth/start`: the assertion challenge for
    /// `user_id`.
    pub async fn auth_start(&self, user_id: &str) -> Result<Value> {
        self.post("auth/start", &serde_json::json!({ "user_id": user_id }))
            .await
    }

    /// `POST /api/webauthn/auth/finish`: the authenticator's assertion
    /// (`challenge`, `id`, `authenticator_data`, `client_data_json`,
    /// `signature`). Refused with 401 when it does not verify.
    pub async fn auth_finish(&self, assertion: &Value) -> Result<Value> {
        self.post("auth/finish", assertion).await
    }

    /// `GET /api/webauthn/credentials`: the keys of `user_id`.
    pub async fn credentials(&self, user_id: &str) -> Result<Value> {
        self.client
            .get(&["api", "webauthn", "credentials"], &[("user_id", user_id)])
            .await
    }

    /// `DELETE /api/webauthn/credentials/{id}`: removes a key of `user_id`.
    pub async fn delete_credential(&self, user_id: &str, credential_id: &str) -> Result<()> {
        let request = self.client.request(
            Method::DELETE,
            &["api", "webauthn", "credentials", credential_id],
            &[("user_id", user_id)],
        )?;
        self.client.empty(request).await
    }

    async fn post(&self, action: &str, body: &Value) -> Result<Value> {
        let mut segments = vec!["api", "webauthn"];
        segments.extend(action.split('/'));
        self.client
            .send_json(Method::POST, &segments, &[], body)
            .await
    }
}
