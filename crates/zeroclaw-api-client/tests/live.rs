//! Read-only calls against a real gateway, to check that every type decodes
//! what the gateway actually sends. Ignored by default; run with:
//!
//! ```sh
//! ZEROCLAW_URL=http://127.0.0.1:42617 ZEROCLAW_TOKEN=zc_… \
//!   cargo test --test live -- --ignored --nocapture
//! ```
//!
//! Optional: `ZEROCLAW_AGENT` (an agent alias) adds the per-agent reads.
//! Nothing is written. Calls a build may not offer (features off, SOPs
//! disabled, a channel not running) are reported, not failed.

#![allow(clippy::disallowed_macros)] // The report on stderr is the point.

use zeroclaw_api_client::Client;

fn client() -> Client {
    let url = std::env::var("ZEROCLAW_URL").expect("set ZEROCLAW_URL");
    let token = std::env::var("ZEROCLAW_TOKEN").expect("set ZEROCLAW_TOKEN");
    Client::new(url, token).unwrap()
}

/// Fails on a decode or transport error; a refusal by the gateway (a route
/// or feature it does not offer) is only reported.
fn check<T>(what: &str, result: zeroclaw_api_client::Result<T>) {
    match result {
        Ok(_) => eprintln!("ok      {what}"),
        Err(zeroclaw_api_client::Error::Api {
            status, message, ..
        }) => {
            eprintln!("refused {what}: {status} {message}");
        }
        Err(e) => panic!("{what}: {e}"),
    }
}

#[tokio::test]
#[ignore = "needs a running gateway"]
async fn every_read_decodes() {
    let client = client();
    let system = client.system();
    check("health", system.health().await);
    check("runtime_health", system.runtime_health().await);
    check("metrics", system.metrics().await);
    check("status", system.status(None).await);
    check("tools", system.tools(None).await);
    check("integrations", system.integrations().await);
    check("integration_settings", system.integration_settings().await);
    check("tuis", system.tuis().await);
    check("logs", system.logs(&Default::default()).await);
    check("upgrade_status", system.upgrade_status(None).await);

    let config = client.config();
    check("config.snapshot", config.snapshot().await);
    check("config.get", config.get("gateway.port").await);
    check("config.list", config.list(Some("gateway")).await);
    check("config.drift", config.drift().await);
    check("config.pending_reload", config.pending_reload().await);
    check("config.templates", config.templates().await);
    check("config.map_keys", config.map_keys("agents").await);
    check("config.catalog", config.catalog().await);
    check("config.status", config.status().await);
    check("config.agent_options", config.agent_options().await);
    check("config.sections", config.sections().await);

    check("pairing.devices", client.pairing().devices().await);
    check("cron.list", client.cron().list().await);
    check("cron.settings", client.cron().settings().await);
    check(
        "memory.search",
        client.memory().search(&Default::default()).await,
    );
    check("cost.summary", client.cost().summary(None, None).await);
    check("sessions.list", client.sessions().list().await);
    check("sessions.running", client.sessions().running().await);
    check("events.history", client.events().history().await);
    check("files.list", client.files().list(None).await);
    check("skills.bundles", client.skills().bundles().await);
    check(
        "skills.slash_option_kinds",
        client.skills().slash_option_kinds().await,
    );
    check("sops.list", client.sops().list().await);
    check("sops.runs", client.sops().runs(None).await);
    check("canvas.list", client.canvas().list().await);
    check("quickstart.state", client.quickstart().state().await);

    let channels = client.channels().list().await.unwrap();
    for channel in &channels {
        if let Ok(rooms) = client.channels().rooms(&channel.name).await {
            eprintln!("ok      rooms of {}: {}", channel.name, rooms.len());
            if let Some(room) = rooms.first() {
                check(
                    "room_members",
                    client
                        .channels()
                        .room_members(&channel.name, &room.id)
                        .await,
                );
            }
        }
    }
    if let Some(session) = client.sessions().list().await.unwrap().first() {
        let id = session
            .session_key
            .clone()
            .unwrap_or_else(|| session.session_id.clone());
        check("sessions.messages", client.sessions().messages(&id).await);
        check("sessions.state", client.sessions().state(&id).await);
    }

    if let Ok(alias) = std::env::var("ZEROCLAW_AGENT") {
        let agent = client.agent(&alias);
        check("agent.list_files", agent.list_files(None).await);
        check("agent.personality_files", agent.personality_files().await);
        check("agent.personality", agent.personality("SOUL.md").await);
        check("agent.skills", agent.skills().await);
        check("system.tools(agent)", system.tools(Some(&alias)).await);
        check("cost.agent", client.cost().agent(&alias).await);
    }
}
