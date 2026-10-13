//! Channels, sessions and the event stream at the HTTP boundary.

mod common;

use common::{expect, gateway, json};
use futures_util::StreamExt;
use serde_json::json;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;

const CHANNEL: &str = "whatsapp.main";

#[tokio::test]
async fn lists_binds_and_relinks_channels() {
    let (server, client) = gateway().await;
    expect("GET", "/api/channels")
        .respond_with(json(json!({ "channels": [{
            "name": CHANNEL, "type": "whatsapp", "alias": "main", "owning_agent": "helper",
            "enabled": true, "compiled": true, "status": "active", "message_count": 0,
            "last_message_at": null, "health": "healthy",
            "readiness": { "enabled": "ready", "bound_to_agent": "ready", "authenticated": "ready",
                           "listening": "ready", "requirements": [], "notes": [] }
        }] })))
        .mount(&server)
        .await;
    expect("POST", "/api/channels/bind")
        .and(body_json(json!({ "channel_type": "telegram", "alias": "bot", "identity": "42" })))
        .respond_with(json(json!({
            "saved": true, "already_bound": false, "group": "telegram_bot", "channel": "telegram.bot"
        })))
        .mount(&server)
        .await;
    expect("POST", "/api/channels/whatsapp.main/relink")
        .respond_with(json(json!({
            "channel": CHANNEL, "outcome": "cleared", "removed": ["/wa/session.db"],
            "restart_required": true, "note": "reload"
        })))
        .mount(&server)
        .await;

    let channels = client.channels();
    let listed = channels.list().await.unwrap();
    assert_eq!(
        (listed[0].channel_type.as_str(), listed[0].health.as_str()),
        ("whatsapp", "healthy")
    );
    assert_eq!(
        channels.bind("telegram", "bot", "42").await.unwrap().group,
        "telegram_bot"
    );
    assert!(channels.relink(CHANNEL).await.unwrap().restart_required);
}

#[tokio::test]
async fn sends_and_manages_group_chats() {
    let (server, client) = gateway().await;
    let room = "120363000000000000@g.us";
    expect("POST", "/api/channels/whatsapp.main/send")
        .and(body_json(
            json!({ "to": "+34600000000", "content": "Hola", "thread_id": null }),
        ))
        .respond_with(json(
            json!({ "channel": CHANNEL, "to": "+34600000000", "outcome": "sent" }),
        ))
        .mount(&server)
        .await;
    expect("GET", "/api/channels/whatsapp.main/rooms")
        .respond_with(json(json!({ "channel": CHANNEL, "rooms": [
            { "id": room, "name": "Team", "participants": 3 }
        ] })))
        .mount(&server)
        .await;
    expect("POST", "/api/channels/whatsapp.main/rooms")
        .and(body_json(
            json!({ "name": "Team", "topic": null, "invites": ["+34600000000"] }),
        ))
        .respond_with(json(json!({
            "channel": CHANNEL, "room": room, "outcome": "created",
            "participants": [{ "user": "+34600000000", "outcome": "added", "detail": null }]
        })))
        .mount(&server)
        .await;
    expect(
        "GET",
        &format!("/api/channels/whatsapp.main/rooms/{room}/members"),
    )
    .respond_with(json(json!({ "channel": CHANNEL, "room": room, "members": [
            { "id": "+34600000000", "admin": true }
        ] })))
    .mount(&server)
    .await;
    expect("POST", &format!("/api/channels/whatsapp.main/rooms/{room}/invites"))
        .and(body_json(json!({ "user": "+34611111111" })))
        .respond_with(json(json!({
            "channel": CHANNEL, "room": room, "user": "+34611111111", "outcome": "invited", "detail": null
        })))
        .mount(&server)
        .await;
    expect(
        "DELETE",
        &format!("/api/channels/whatsapp.main/rooms/{room}/members/+34611111111"),
    )
    .respond_with(json(json!({
        "channel": CHANNEL, "room": room, "user": "+34611111111", "outcome": "removed"
    })))
    .mount(&server)
    .await;

    let channels = client.channels();
    channels
        .send(CHANNEL, "+34600000000", "Hola", None)
        .await
        .unwrap();
    assert_eq!(channels.rooms(CHANNEL).await.unwrap()[0].participants, 3);
    let created = channels
        .create_room(CHANNEL, "Team", None, &["+34600000000"])
        .await
        .unwrap();
    assert_eq!(created.participants[0].outcome, "added");
    assert!(channels.room_members(CHANNEL, room).await.unwrap()[0].admin);
    assert_eq!(
        channels
            .invite(CHANNEL, room, "+34611111111")
            .await
            .unwrap()
            .outcome,
        "invited"
    );
    channels
        .remove_member(CHANNEL, room, "+34611111111")
        .await
        .unwrap();
}

#[tokio::test]
async fn channel_refusals_keep_their_reason() {
    let (server, client) = gateway().await;
    expect("POST", "/api/channels/whatsapp.main/send")
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "channel": CHANNEL, "error": "channel whatsapp.main is not in [gateway] send_channels"
        })))
        .mount(&server)
        .await;

    let error = client
        .channels()
        .send(CHANNEL, "x", "y", None)
        .await
        .unwrap_err();
    assert!(error.is_unauthorized());
    assert!(error.to_string().contains("send_channels"));
}

#[tokio::test]
async fn reads_and_manages_sessions() {
    let (server, client) = gateway().await;
    let session = json!({
        "session_id": "abc", "session_key": "gw_abc", "created_at": "t", "last_activity": "t",
        "message_count": 2, "agent_alias": "helper", "channel_id": null
    });
    expect("GET", "/api/sessions")
        .respond_with(json(json!({ "sessions": [session] })))
        .mount(&server)
        .await;
    expect("GET", "/api/sessions/running")
        .respond_with(json(json!({ "sessions": [
            { "session_id": "abc", "created_at": "t", "last_activity": "t", "message_count": 2 }
        ] })))
        .mount(&server)
        .await;
    expect("GET", "/api/sessions/gw_abc/messages")
        .respond_with(json(
            json!({ "session_id": "gw_abc", "session_persistence": true, "messages": [
            { "role": "user", "content": "hi", "created_at": "t" }
        ] }),
        ))
        .mount(&server)
        .await;
    expect("POST", "/api/sessions/gw_abc/messages")
        .and(body_json(json!({ "content": "Reminder" })))
        .respond_with(json(json!({ "status": "ok" })))
        .mount(&server)
        .await;
    expect("GET", "/api/sessions/gw_abc/state")
        .respond_with(json(
            json!({ "session_id": "gw_abc", "state": "running", "turn_id": "t1" }),
        ))
        .mount(&server)
        .await;
    expect("PUT", "/api/sessions/gw_abc")
        .and(body_json(json!({ "name": "Support" })))
        .respond_with(json(json!({ "session_id": "gw_abc", "name": "Support" })))
        .mount(&server)
        .await;
    expect("POST", "/api/sessions/gw_abc/abort")
        .respond_with(json(json!({ "status": "no_active_response" })))
        .mount(&server)
        .await;
    expect("DELETE", "/api/sessions/gw_abc")
        .respond_with(json(json!({ "deleted": true, "session_id": "gw_abc" })))
        .mount(&server)
        .await;

    let sessions = client.sessions();
    assert_eq!(
        sessions.list().await.unwrap()[0].session_key.as_deref(),
        Some("gw_abc")
    );
    assert_eq!(sessions.running().await.unwrap().len(), 1);
    assert_eq!(sessions.messages("gw_abc").await.unwrap()[0].role, "user");
    sessions.add_message("gw_abc", "Reminder").await.unwrap();
    assert_eq!(sessions.state("gw_abc").await.unwrap().state, "running");
    sessions.rename("gw_abc", "Support").await.unwrap();
    assert!(!sessions.abort("gw_abc").await.unwrap());
    sessions.delete("gw_abc").await.unwrap();
}

#[tokio::test]
async fn streams_and_replays_events() {
    let (server, client) = gateway().await;
    expect("GET", "/api/events")
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(concat!(
                    ": keep-alive\n\n",
                    "data: {\"type\":\"agent_end\",\"source\":\"observability\",\"cost_usd\":0.01}\n\n",
                    "data: {\"@timestamp\":\"t\",\"event\":{\"category\":\"channel\"}}\n\n",
                )),
        )
        .mount(&server)
        .await;
    expect("GET", "/api/events/history")
        .respond_with(json(
            json!({ "events": [{ "type": "tool_call", "tool": "shell" }] }),
        ))
        .mount(&server)
        .await;

    let events: Vec<_> = client
        .events()
        .subscribe()
        .await
        .unwrap()
        .map(Result::unwrap)
        .collect()
        .await;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind(), Some("agent_end"));
    assert!(events[1].is_log());
    assert_eq!(
        client.events().history().await.unwrap()[0].kind(),
        Some("tool_call")
    );
}
