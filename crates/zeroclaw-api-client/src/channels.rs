//! Channels (WhatsApp, Telegram, …): their state, sending through them,
//! group chats ("rooms") and their members.
//!
//! Gateway handlers: `crates/zeroclaw-gateway/src/api.rs` and
//! `api_config.rs` (bind). A channel is named `type.alias`, as
//! [`ChannelsApi::list`] returns it. Sending and managing rooms need pairing
//! on and the channel listed in `[gateway] send_channels`; reading rooms
//! needs pairing only. Errors on these routes carry the channel:
//! `{"channel": …, "error": …}`.

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

    /// `POST /api/channels/{channel}/send`: sends plain text to `to`, with no
    /// agent turn.
    pub async fn send(
        &self,
        channel: &str,
        to: &str,
        content: &str,
        thread_id: Option<&str>,
    ) -> Result<()> {
        let request = self
            .client
            .request(Method::POST, &["api", "channels", channel, "send"], &[])?
            .json(&serde_json::json!({
                "to": to,
                "content": content,
                "thread_id": thread_id,
            }));
        self.client.empty(request).await
    }

    /// `GET /api/channels/{channel}/rooms`: the group chats the running
    /// channel takes part in.
    pub async fn rooms(&self, channel: &str) -> Result<Vec<RoomSummary>> {
        #[derive(Deserialize)]
        struct Rooms {
            rooms: Vec<RoomSummary>,
        }
        let rooms: Rooms = self
            .client
            .get(&["api", "channels", channel, "rooms"], &[])
            .await?;
        Ok(rooms.rooms)
    }

    /// `POST /api/channels/{channel}/rooms`: creates a group chat and
    /// invites `invites` (phone numbers on WhatsApp, which are also allowed
    /// to talk to the channel).
    pub async fn create_room(
        &self,
        channel: &str,
        name: &str,
        topic: Option<&str>,
        invites: &[&str],
    ) -> Result<CreatedRoom> {
        self.client
            .send_json(
                Method::POST,
                &["api", "channels", channel, "rooms"],
                &[],
                &serde_json::json!({ "name": name, "topic": topic, "invites": invites }),
            )
            .await
    }

    /// `GET /api/channels/{channel}/rooms/{room}/members`: a group chat's
    /// members.
    pub async fn room_members(&self, channel: &str, room: &str) -> Result<Vec<RoomMember>> {
        #[derive(Deserialize)]
        struct Members {
            members: Vec<RoomMember>,
        }
        let members: Members = self
            .client
            .get(&["api", "channels", channel, "rooms", room, "members"], &[])
            .await?;
        Ok(members.members)
    }

    /// `POST /api/channels/{channel}/rooms/{room}/invites`: adds or invites
    /// `user` to a group chat.
    pub async fn invite(&self, channel: &str, room: &str, user: &str) -> Result<Participant> {
        self.client
            .send_json(
                Method::POST,
                &["api", "channels", channel, "rooms", room, "invites"],
                &[],
                &serde_json::json!({ "user": user }),
            )
            .await
    }

    /// `DELETE /api/channels/{channel}/rooms/{room}/members/{user}`: removes
    /// a member from a group chat.
    pub async fn remove_member(&self, channel: &str, room: &str, user: &str) -> Result<()> {
        let request = self.client.request(
            Method::DELETE,
            &["api", "channels", channel, "rooms", room, "members", user],
            &[],
        )?;
        self.client.empty(request).await
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

/// A group chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSummary {
    /// Id in the channel's addressing (`…@g.us` on WhatsApp).
    pub id: String,
    /// Name (the WhatsApp group subject).
    pub name: String,
    /// Member count.
    pub participants: usize,
}

/// A member of a group chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomMember {
    /// The member: `+<digits>` on WhatsApp when the number is known.
    pub id: String,
    /// Whether they administer the group.
    pub admin: bool,
}

/// Result of [`ChannelsApi::create_room`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatedRoom {
    /// The channel.
    pub channel: String,
    /// The new room's id.
    pub room: String,
    /// Always `created`.
    pub outcome: String,
    /// What happened to each invitee.
    #[serde(default)]
    pub participants: Vec<Participant>,
}

/// What happened to someone added to a group chat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Participant {
    /// Who.
    pub user: String,
    /// `added`, `invited`, `refused` or `failed`.
    pub outcome: String,
    /// Why, when it did not work.
    #[serde(default)]
    pub detail: Option<String>,
}
