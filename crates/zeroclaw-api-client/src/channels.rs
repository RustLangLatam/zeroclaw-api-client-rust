//! Channels (WhatsApp, Telegram, …): their state, the identities allowed to
//! talk to them and QR logins.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/api.rs` and
//! `api_config.rs` (bind). A channel is named `type.alias`, as
//! [`ChannelsApi::list`] returns it.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::Result;

/// Channel endpoints, from [`Client::channels`].
#[derive(Debug, Clone, Copy)]
pub struct ChannelsApi<'a> {
    client: &'a Client,
}

impl Client {
    /// Channels and their group chats (`/api/channels`).
    pub fn channels(&self) -> ChannelsApi<'_> {
        ChannelsApi { client: self }
    }
}

impl ChannelsApi<'_> {
    /// `GET /api/channels`: every configured channel and its state.
    pub async fn list(&self) -> Result<Vec<Channel>> {
        #[derive(Deserialize)]
        struct Channels {
            channels: Vec<Channel>,
        }
        let channels: Channels = self.client.get(&["api", "channels"], &[]).await?;
        Ok(channels.channels)
    }

    /// `POST /api/channels/bind`: lets `identity` talk to the channel
    /// (Telegram, WeChat and LINE), adding it to the channel's peer group.
    pub async fn bind(&self, channel_type: &str, alias: &str, identity: &str) -> Result<Bound> {
        self.client
            .send_json(
                Method::POST,
                &["api", "channels", "bind"],
                &[],
                &serde_json::json!({
                    "channel_type": channel_type,
                    "alias": alias,
                    "identity": identity,
                }),
            )
            .await
    }

    /// `POST /api/channels/{channel}/relink`: clears a QR-paired channel's
    /// saved login so it pairs again after a reload
    /// ([`crate::system::SystemApi::reload`]).
    pub async fn relink(&self, channel: &str) -> Result<Relinked> {
        let request =
            self.client
                .request(Method::POST, &["api", "channels", channel, "relink"], &[])?;
        self.client.json(request).await
    }
}

/// A configured channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    /// `type.alias`, the name the other calls take.
    pub name: String,
    /// Channel type (`whatsapp`, `telegram`, …).
    #[serde(rename = "type")]
    pub channel_type: String,
    /// Alias.
    pub alias: String,
    /// Agent it belongs to.
    #[serde(default)]
    pub owning_agent: Option<String>,
    /// Whether it is enabled.
    pub enabled: bool,
    /// Whether this build includes its type.
    pub compiled: bool,
    /// `active`, `inactive`, `error`, `unknown` or `not_compiled`.
    pub status: String,
    /// `healthy`, `degraded`, `down` or `unavailable`.
    pub health: String,
    /// What it still needs (`enabled`, `bound_to_agent`, `authenticated`,
    /// `listening`, each `ready`/`missing`/`unknown`, plus notes).
    #[serde(default)]
    pub readiness: Option<Value>,
}

/// Result of [`ChannelsApi::bind`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bound {
    /// Whether the configuration was saved.
    pub saved: bool,
    /// Whether the identity was already allowed.
    pub already_bound: bool,
    /// Peer group it was added to.
    pub group: String,
    /// The channel.
    pub channel: String,
}

/// Result of [`ChannelsApi::relink`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relinked {
    /// The channel.
    pub channel: String,
    /// `cleared` or `nothing_to_clear`.
    pub outcome: String,
    /// Files removed.
    #[serde(default)]
    pub removed: Vec<String>,
    /// Whether a reload is needed for the channel to pair again.
    #[serde(default)]
    pub restart_required: bool,
    /// The gateway's note.
    #[serde(default)]
    pub note: Option<String>,
}
