//! Pairing (how a client gets its bearer token) and the paired devices.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/lib.rs` (`/pair`,
//! `/pair/code`, `/admin/paircode*`) and `api_pairing.rs`. A pairing code is
//! six digits, single use, valid ten minutes; the token it buys is
//! `zc_` + 64 hex characters. Exchange a code for a token with
//! [`PairingApi::pair`], then build a client with [`Client::with_token`].

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::{Client, query};
use crate::error::Result;

/// Pairing endpoints, from [`Client::pairing`].
#[derive(Debug, Clone, Copy)]
pub struct PairingApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Pairing and paired devices.
    pub fn pairing(&self) -> PairingApi<'_> {
        PairingApi { client: self }
    }
}

impl PairingApi<'_> {
    /// `POST /api/pair`: exchanges a pairing `code` for a bearer token,
    /// registering this client as a device. Public.
    pub async fn pair(
        &self,
        code: &str,
        device_name: Option<&str>,
        device_type: Option<&str>,
    ) -> Result<Paired> {
        self.client
            .send_json(
                Method::POST,
                &["api", "pair"],
                &[],
                &serde_json::json!({
                    "code": code,
                    "device_name": device_name,
                    "device_type": device_type,
                }),
            )
            .await
    }

    /// `POST /pair`: the original pairing call, with the code in the
    /// `X-Pairing-Code` header. Public.
    pub async fn pair_legacy(&self, code: &str) -> Result<Paired> {
        let request = self
            .client
            .request(Method::POST, &["pair"], &[])?
            .header("X-Pairing-Code", code);
        self.client.json(request).await
    }

    /// `GET /pair/code`: the first-run pairing code, shown only while no
    /// client is paired yet. Public.
    pub async fn initial_code(&self) -> Result<PairingCode> {
        self.client.get(&["pair", "code"], &[]).await
    }

    /// `POST /api/pairing/initiate`: mints a new pairing code (replacing a
    /// pending one) so another device can pair.
    pub async fn new_code(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct Initiated {
            pairing_code: String,
        }
        let request = self
            .client
            .request(Method::POST, &["api", "pairing", "initiate"], &[])?;
        let initiated: Initiated = self.client.json(request).await?;
        Ok(initiated.pairing_code)
    }

    /// `GET /admin/paircode`: the pending pairing code. Loopback only.
    pub async fn admin_code(&self) -> Result<PairingCode> {
        self.client.get(&["admin", "paircode"], &[]).await
    }

    /// `POST /admin/paircode/new`: mints a pairing code. `rotate` revokes
    /// first: `all` every token and device, otherwise the device with that
    /// id. Loopback only.
    pub async fn admin_new_code(&self, rotate: Option<&str>) -> Result<PairingCode> {
        let request = self.client.request(
            Method::POST,
            &["admin", "paircode", "new"],
            &query(&[("rotate", rotate)]),
        )?;
        self.client.json(request).await
    }

    /// `GET /api/devices`: the paired devices.
    pub async fn devices(&self) -> Result<Vec<Device>> {
        #[derive(Deserialize)]
        struct Devices {
            devices: Vec<Device>,
        }
        let devices: Devices = self.client.get(&["api", "devices"], &[]).await?;
        Ok(devices.devices)
    }

    /// `DELETE /api/devices/{id}`: revokes a device and its token (it may be
    /// this client's own).
    pub async fn revoke_device(&self, id: &str) -> Result<()> {
        let request = self
            .client
            .request(Method::DELETE, &["api", "devices", id], &[])?;
        self.client.empty(request).await
    }

    /// `POST /api/devices/{id}/token/rotate`: revokes a device's token and
    /// mints a pairing code for it to pair again (`None` when a code was
    /// already pending or pairing is off).
    pub async fn rotate_device_token(&self, id: &str) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Rotated {
            #[serde(default)]
            pairing_code: Option<String>,
        }
        let request = self.client.request(
            Method::POST,
            &["api", "devices", id, "token", "rotate"],
            &[],
        )?;
        let rotated: Rotated = self.client.json(request).await?;
        Ok(rotated.pairing_code)
    }

    /// `POST /api/devices/me/capabilities`: replaces the capabilities this
    /// client's device advertises.
    pub async fn set_capabilities(&self, capabilities: &[&str]) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Updated {
            capabilities: Vec<String>,
        }
        let updated: Updated = self
            .client
            .send_json(
                Method::POST,
                &["api", "devices", "me", "capabilities"],
                &[],
                &serde_json::json!({ "capabilities": capabilities }),
            )
            .await?;
        Ok(updated.capabilities)
    }
}

/// A successful pairing.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Paired {
    /// Always true.
    pub paired: bool,
    /// Whether the token was saved in the gateway's configuration.
    #[serde(default)]
    pub persisted: bool,
    /// The bearer token. Keep it secret.
    pub token: String,
    /// The gateway's note.
    #[serde(default)]
    pub message: Option<String>,
}

impl std::fmt::Debug for Paired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Paired")
            .field("paired", &self.paired)
            .field("persisted", &self.persisted)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// A pairing code and whether pairing is required.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairingCode {
    /// Whether the request succeeded.
    #[serde(default)]
    pub success: bool,
    /// Whether the gateway requires pairing.
    pub pairing_required: bool,
    /// The six-digit code, when one is pending and may be shown.
    #[serde(default)]
    pub pairing_code: Option<String>,
    /// The gateway's note.
    #[serde(default)]
    pub message: Option<String>,
}

/// A paired device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// Device id.
    pub id: String,
    /// Name given at pairing.
    #[serde(default)]
    pub name: Option<String>,
    /// Type given at pairing (`legacy` for tokens older than the registry).
    #[serde(default)]
    pub device_type: Option<String>,
    /// When it paired (RFC 3339).
    pub paired_at: String,
    /// When it was last seen (RFC 3339).
    pub last_seen: String,
    /// Address it paired from.
    #[serde(default)]
    pub ip_address: Option<String>,
    /// Advertised capabilities.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pairing_never_shows_its_token() {
        let paired: Paired = serde_json::from_str(
            r#"{"paired":true,"persisted":true,"token":"zc_secret","message":"Pairing successful"}"#,
        )
        .unwrap();
        assert!(!format!("{paired:?}").contains("zc_secret"));
    }
}
