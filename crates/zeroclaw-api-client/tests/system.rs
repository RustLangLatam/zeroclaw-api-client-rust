//! Health, status, diagnostics, logs, versions and administration at the
//! HTTP boundary.

mod common;

use std::collections::BTreeMap;

use common::{expect, gateway, json};
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};
use zeroclaw_api_client::system::LogQuery;

fn snapshot() -> serde_json::Value {
    json!({ "pid": 7, "updated_at": "t", "uptime_seconds": 60, "components": {
        "gateway": { "status": "ok", "updated_at": "t", "last_ok": "t", "last_error": null, "restart_count": 0 }
    } })
}

#[tokio::test]
async fn health_and_metrics_are_public() {
    let (server, client) = gateway().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(json(json!({
            "status": "ok", "paired": true, "require_pairing": true, "runtime": snapshot()
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/metrics"))
        .respond_with(ResponseTemplate::new(200).set_body_string("# HELP x\nx 1\n"))
        .mount(&server)
        .await;
    expect("GET", "/api/health")
        .respond_with(json(json!({ "health": snapshot() })))
        .mount(&server)
        .await;

    let system = client.system();
    assert!(system.health().await.unwrap().require_pairing);
    assert!(system.metrics().await.unwrap().contains("x 1"));
    assert_eq!(
        system.runtime_health().await.unwrap().components["gateway"].status,
        "ok"
    );
}

#[tokio::test]
async fn status_asks_about_one_agent() {
    let (server, client) = gateway().await;
    expect("GET", "/api/status")
        .and(query_param("agent", "helper"))
        .respond_with(json(json!({
            "version": "0.8.5", "model_provider": "openai.default", "model": "m",
            "temperature": 0.7, "uptime_seconds": 1, "daemon_started_at": "t",
            "gateway_port": 42617, "locale": "en", "memory_backend": "sqlite", "paired": true,
            "channels": { "whatsapp.main": true }, "nodes": { "connected": [], "mdns_peers": [] },
            "health": snapshot(), "agent_alias": "helper",
            "process": { "rss_bytes": 1, "system_ram_total_bytes": 2, "cpu_percent": null, "num_cpus": 4 },
            "check_updates": true, "allow_self_upgrade": false, "restart_mode": "manual",
            "restart_hint": "", "live_channel_config": ["sender_routes"]
        })))
        .mount(&server)
        .await;

    let status = client.system().status(Some("helper")).await.unwrap();
    assert_eq!(status.agent_alias.as_deref(), Some("helper"));
    assert_eq!(status.live_channel_config, ["sender_routes"]);
}

#[tokio::test]
async fn diagnoses_and_lists_tools_and_integrations() {
    let (server, client) = gateway().await;
    expect("POST", "/api/doctor")
        .respond_with(json(json!({
            "results": [{ "severity": "warn", "category": "memory", "message": "m" }],
            "summary": { "ok": 0, "warnings": 1, "errors": 0 }
        })))
        .mount(&server)
        .await;
    expect("GET", "/api/tools")
        .respond_with(json(json!({ "tools": [{
            "name": "shell", "description": "d", "parameters": { "type": "object" }
        }] })))
        .mount(&server)
        .await;
    expect("POST", "/api/tools/param-options")
        .and(body_json(
            json!({ "domain": "channel_refs", "agent": null, "args": null }),
        ))
        .respond_with(json(json!({ "options": [{ "value": "whatsapp.main" }] })))
        .mount(&server)
        .await;
    expect("GET", "/api/integrations")
        .respond_with(json(json!({ "integrations": [{
            "name": "WhatsApp", "description": "d", "category": "Chat",
            "category_label": "Chat Providers", "status": "Active"
        }] })))
        .mount(&server)
        .await;
    expect("GET", "/api/integrations/settings")
        .respond_with(json(json!({ "settings": {
            "WhatsApp": { "enabled": true, "category": "Chat", "status": "Active" }
        } })))
        .mount(&server)
        .await;
    expect("GET", "/api/cli-tools")
        .respond_with(json(json!({ "cli_tools": [
            { "name": "git", "path": "/usr/bin/git", "version": "2", "category": "VersionControl" }
        ] })))
        .mount(&server)
        .await;
    expect("GET", "/api/tuis")
        .respond_with(json(json!({ "tuis": [] })))
        .mount(&server)
        .await;

    let system = client.system();
    assert_eq!(system.doctor().await.unwrap().summary.warnings, 1);
    assert_eq!(system.tools(None).await.unwrap()[0].name, "shell");
    assert_eq!(
        system
            .tool_param_options("channel_refs", None, None)
            .await
            .unwrap()[0]
            .value,
        "whatsapp.main"
    );
    assert_eq!(system.integrations().await.unwrap()[0].status, "Active");
    assert!(system.integration_settings().await.unwrap()["WhatsApp"].enabled);
    assert_eq!(system.cli_tools().await.unwrap()[0].name, "git");
    assert!(system.tuis().await.unwrap().is_empty());
}

#[tokio::test]
async fn log_filters_become_query_parameters() {
    let (server, client) = gateway().await;
    expect("GET", "/api/logs")
        .and(query_param("until_line_offset", "40"))
        .and(query_param("limit", "10"))
        .and(query_param("hide_internal", "true"))
        .and(query_param("agent_alias", "helper"))
        .respond_with(json(json!({
            "events": [{ "@timestamp": "t" }], "next_cursor": null,
            "next_cursor_line_offset": 30, "at_end": false,
            "daemon_started_at": "t", "attribution_keys": ["agent_alias"]
        })))
        .mount(&server)
        .await;

    let page = client
        .system()
        .logs(&LogQuery {
            until_line_offset: Some(40),
            limit: Some(10),
            hide_internal: true,
            attributes: BTreeMap::from([("agent_alias".into(), "helper".into())]),
            ..LogQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(page.next_cursor_line_offset, Some(30));
}

#[tokio::test]
async fn checks_and_runs_upgrades() {
    let (server, client) = gateway().await;
    expect("GET", "/api/version/check")
        .and(query_param("force", "true"))
        .respond_with(json(json!({
            "current_version": "0.8.5", "latest_version": "0.8.6", "is_newer": true
        })))
        .mount(&server)
        .await;
    expect("POST", "/api/version/upgrade")
        .and(body_json(json!({ "version": null, "auto_restart": true })))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({ "handoff_id": "h1" })))
        .mount(&server)
        .await;
    expect("GET", "/api/version/upgrade/status")
        .and(query_param("handoff_id", "h1"))
        .respond_with(json(json!({ "state": "running", "phase": 2 })))
        .mount(&server)
        .await;

    let system = client.system();
    assert!(system.version_check(true, None).await.unwrap().is_newer);
    assert_eq!(system.upgrade(None, true).await.unwrap(), "h1");
    assert_eq!(
        system.upgrade_status(Some("h1")).await.unwrap().phase,
        Some(2)
    );
}

#[tokio::test]
async fn administers_the_daemon() {
    let (server, client) = gateway().await;
    for (route, message) in [
        ("/admin/reload", "Daemon reload initiated"),
        ("/admin/shutdown", "Gateway shutdown initiated"),
    ] {
        expect("POST", route)
            .respond_with(json(json!({ "success": true, "message": message })))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/api/openapi.json"))
        .respond_with(json(json!({ "openapi": "3.1.0" })))
        .mount(&server)
        .await;

    let system = client.system();
    assert!(system.reload().await.unwrap().success);
    assert!(system.shutdown().await.unwrap().success);
    assert_eq!(system.openapi().await.unwrap()["openapi"], "3.1.0");
}
