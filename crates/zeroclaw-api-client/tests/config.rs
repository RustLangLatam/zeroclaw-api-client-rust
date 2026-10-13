//! `/api/config*` at the HTTP boundary.

mod common;

use common::{expect, gateway, json};
use serde_json::json;
use wiremock::ResponseTemplate;
use wiremock::matchers::{body_json, header, query_param};
use zeroclaw_api_client::config::{AliasSource, PatchOp, PropValue};

#[tokio::test]
async fn reads_writes_and_resets_one_field() {
    let (server, client) = gateway().await;
    expect("GET", "/api/config/prop")
        .and(query_param("path", "gateway.port"))
        .respond_with(json(json!({ "path": "gateway.port", "value": "42617" })))
        .mount(&server)
        .await;
    expect("PUT", "/api/config/prop")
        .and(body_json(
            json!({ "path": "agents.x.model", "value": "m", "comment": "set" }),
        ))
        .respond_with(json(json!({ "path": "agents.x.model", "value": "m" })))
        .mount(&server)
        .await;
    expect("DELETE", "/api/config/prop")
        .and(query_param("path", "providers.models.x.default.api_key"))
        .respond_with(json(
            json!({ "path": "providers.models.x.default.api_key", "populated": false }),
        ))
        .mount(&server)
        .await;

    let config = client.config();
    assert!(matches!(
        config.get("gateway.port").await.unwrap(),
        PropValue::Value { value, .. } if value == "42617"
    ));
    config
        .set("agents.x.model", "m", Some("set"))
        .await
        .unwrap();
    assert!(matches!(
        config
            .reset("providers.models.x.default.api_key")
            .await
            .unwrap(),
        PropValue::Secret {
            populated: false,
            ..
        }
    ));
}

#[tokio::test]
async fn patches_atomically_and_can_override_drift() {
    let (server, client) = gateway().await;
    expect("PATCH", "/api/config")
        .and(header("X-ZeroClaw-Override-Drift", "true"))
        .and(body_json(json!([
            { "op": "replace", "path": "gateway.port", "value": 1 },
            { "op": "remove", "path": "agents.x.model" }
        ])))
        .respond_with(json(json!({
            "saved": true,
            "results": [
                { "op": "replace", "path": "gateway.port", "value": "1" },
                { "op": "remove", "path": "agents.x.model", "value": null }
            ]
        })))
        .mount(&server)
        .await;

    let patched = client
        .config()
        .patch(
            &[
                PatchOp::replace("gateway.port", 1),
                PatchOp::remove("agents.x.model"),
            ],
            true,
        )
        .await
        .unwrap();
    assert!(patched.saved && patched.results.len() == 2);
}

#[tokio::test]
async fn config_errors_keep_the_gateways_message() {
    let (server, client) = gateway().await;
    expect("GET", "/api/config/prop")
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "code": "path_not_found", "message": "no field `x`", "path": "x"
        })))
        .mount(&server)
        .await;

    let error = client.config().get("x").await.unwrap_err();
    assert!(error.is_not_found());
    assert_eq!(error.to_string(), "gateway answered 404: no field `x`");
}

#[tokio::test]
async fn lists_drift_and_status() {
    let (server, client) = gateway().await;
    expect("GET", "/api/config/list")
        .and(query_param("prefix", "gateway"))
        .respond_with(json(json!({ "entries": [{
            "path": "gateway.port", "category": "gateway", "kind": "integer",
            "type_hint": "u16", "value": "42617", "populated": true, "is_secret": false
        }] })))
        .mount(&server)
        .await;
    expect("GET", "/api/config/drift")
        .respond_with(json(
            json!({ "drifted": [{ "path": "a", "drifted": true }] }),
        ))
        .mount(&server)
        .await;
    expect("GET", "/api/config/reload-status")
        .respond_with(json(json!({ "pending_reload": true })))
        .mount(&server)
        .await;
    expect("GET", "/api/config/status")
        .respond_with(json(json!({
            "needs_quickstart": false, "reason": "has_dispatchable_agent",
            "has_partial_state": false, "missing": []
        })))
        .mount(&server)
        .await;
    expect("GET", "/api/config")
        .respond_with(json(json!({ "gateway": { "port": 42617 } })))
        .mount(&server)
        .await;

    let config = client.config();
    assert_eq!(
        config.list(Some("gateway")).await.unwrap().entries[0].kind,
        "integer"
    );
    assert_eq!(config.drift().await.unwrap()[0].path, "a");
    assert!(config.pending_reload().await.unwrap());
    assert!(!config.status().await.unwrap().needs_quickstart);
    assert_eq!(config.snapshot().await.unwrap()["gateway"]["port"], 42617);
}

#[tokio::test]
async fn manages_map_aliases() {
    let (server, client) = gateway().await;
    expect("GET", "/api/config/map-keys")
        .and(query_param("path", "agents"))
        .respond_with(json(json!({ "path": "agents", "keys": ["a", "b"] })))
        .mount(&server)
        .await;
    for verb in ["POST", "DELETE"] {
        expect(verb, "/api/config/map-key")
            .and(query_param("path", "agents"))
            .and(query_param("key", "a"))
            .respond_with(json(
                json!({ "path": "agents", "key": "a", "created": verb == "POST" }),
            ))
            .mount(&server)
            .await;
    }
    expect("GET", "/api/config/delete-plan")
        .and(query_param("key", "a"))
        .respond_with(json(json!({
            "path": "agents", "key": "a", "allowed": true, "blockers": [],
            "scrubs": [{ "path": "heartbeat.agent", "raw_value": "a" }],
            "live_acp_sessions": 0, "cascades_owned_state": true
        })))
        .mount(&server)
        .await;
    expect("POST", "/api/config/rename-map-key")
        .and(body_json(
            json!({ "path": "agents", "from": "a", "to": "c" }),
        ))
        .respond_with(json(
            json!({ "path": "agents", "from": "a", "to": "c", "renamed": true }),
        ))
        .mount(&server)
        .await;
    expect("GET", "/api/config/resolve-alias-source")
        .and(query_param("source", "model_providers"))
        .respond_with(json(
            json!({ "source": "model_providers", "values": ["openai.default"] }),
        ))
        .mount(&server)
        .await;

    let config = client.config();
    assert_eq!(config.map_keys("agents").await.unwrap(), ["a", "b"]);
    assert!(config.create_map_key("agents", "a").await.unwrap().created);
    assert!(!config.delete_map_key("agents", "a").await.unwrap().created);
    assert_eq!(
        config
            .delete_plan("agents", "a")
            .await
            .unwrap()
            .scrubs
            .len(),
        1
    );
    assert!(
        config
            .rename_map_key("agents", "a", "c")
            .await
            .unwrap()
            .renamed
    );
    assert_eq!(
        config
            .alias_values(AliasSource::ModelProviders)
            .await
            .unwrap(),
        ["openai.default"]
    );
}

#[tokio::test]
async fn maintains_the_config_file() {
    let (server, client) = gateway().await;
    expect("POST", "/api/config/init")
        .and(query_param("section", "memory"))
        .respond_with(json(json!({ "initialized": ["memory.sqlite"] })))
        .mount(&server)
        .await;
    expect("POST", "/api/config/migrate")
        .respond_with(json(
            json!({ "migrated": true, "backup_path": "/c.bak", "schema_version": 3 }),
        ))
        .mount(&server)
        .await;
    expect("GET", "/api/config/templates")
        .respond_with(json(json!({ "templates": [{
            "path": "agents", "kind": "map", "value_type": "AgentConfig", "description": "Agents"
        }] })))
        .mount(&server)
        .await;
    expect(
        "POST",
        "/api/config/model-providers/anthropic/default/refresh-context-window",
    )
    .respond_with(json(
        json!({ "path": "providers.models.anthropic.default", "context_window": 200000 }),
    ))
    .mount(&server)
    .await;

    let config = client.config();
    assert_eq!(
        config.init(Some("memory")).await.unwrap(),
        ["memory.sqlite"]
    );
    assert_eq!(config.migrate().await.unwrap().schema_version, 3);
    assert_eq!(config.templates().await.unwrap()[0].kind, "map");
    assert_eq!(
        config
            .refresh_context_window("anthropic", "default")
            .await
            .unwrap(),
        200_000
    );
}

#[tokio::test]
async fn reads_catalogs_and_sections() {
    let (server, client) = gateway().await;
    expect("GET", "/api/config/catalog")
        .respond_with(json(json!({ "model_providers": [
            { "name": "openai", "display_name": "OpenAI", "local": false }
        ] })))
        .mount(&server)
        .await;
    expect("GET", "/api/config/catalog/models")
        .and(query_param("model_provider", "openai"))
        .respond_with(json(json!({
            "model_provider": "openai", "models": ["gpt-x"], "local": false, "live": true
        })))
        .mount(&server)
        .await;
    expect("GET", "/api/config/agent-options")
        .respond_with(json(json!({
            "channels": [], "channel_types": [], "model_providers": ["openai.default"],
            "risk_profiles": [], "runtime_profiles": [], "skill_bundles": [],
            "knowledge_bundles": [], "mcp_bundles": [], "agents": ["helper"]
        })))
        .mount(&server)
        .await;
    expect("GET", "/api/config/sections")
        .respond_with(json(json!({ "sections": [{
            "key": "channels", "label": "Channels", "help": "", "has_picker": true, "completed": false
        }] })))
        .mount(&server)
        .await;
    expect("GET", "/api/config/sections/channels")
        .respond_with(json(json!({ "section": "channels", "help": "", "items": [
            { "key": "whatsapp", "label": "WhatsApp", "badge": "configured" }
        ] })))
        .mount(&server)
        .await;
    expect("POST", "/api/config/sections/channels/items/whatsapp")
        .and(body_json(json!({ "alias": "main" })))
        .respond_with(json(
            json!({ "fields_prefix": "channels.whatsapp.main", "created": true }),
        ))
        .mount(&server)
        .await;

    let config = client.config();
    assert_eq!(config.catalog().await.unwrap()[0].name, "openai");
    assert!(config.catalog_models("openai").await.unwrap().live);
    assert_eq!(config.agent_options().await.unwrap().agents, ["helper"]);
    assert!(config.sections().await.unwrap()[0].has_picker);
    assert_eq!(
        config.section_items("channels").await.unwrap().items[0]
            .badge
            .as_deref(),
        Some("configured")
    );
    assert_eq!(
        config
            .select_section_item("channels", "whatsapp", Some("main"))
            .await
            .unwrap()
            .fields_prefix,
        "channels.whatsapp.main"
    );
}

#[tokio::test]
async fn schemas_use_options() {
    let (server, client) = gateway().await;
    expect("OPTIONS", "/api/config")
        .respond_with(json(json!({ "title": "Config" })))
        .mount(&server)
        .await;
    expect("OPTIONS", "/api/config/prop")
        .and(query_param("path", "gateway.port"))
        .respond_with(json(json!({ "x-zeroclaw-prop": { "kind": "integer" } })))
        .mount(&server)
        .await;

    assert_eq!(client.config().schema().await.unwrap()["title"], "Config");
    assert_eq!(
        client.config().prop_schema("gateway.port").await.unwrap()["x-zeroclaw-prop"]["kind"],
        "integer"
    );
}
