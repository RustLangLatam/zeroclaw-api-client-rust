//! SOPs, quickstart, canvases, webhook, A2A and WebAuthn at the HTTP
//! boundary.

mod common;

use common::{expect, gateway, json};
use serde_json::json;
use wiremock::ResponseTemplate;
use wiremock::matchers::{body_json, header, query_param};
use zeroclaw_api_client::sops::Decision;
use zeroclaw_api_client::webhook::{WebhookOptions, WebhookReply};

#[tokio::test]
async fn authors_and_runs_sops() {
    let (server, client) = gateway().await;
    let sop = json!({ "name": "restock", "description": "d", "version": "1", "priority": "normal",
                      "execution_mode": "auto", "triggers": [{ "type": "manual" }], "steps": [] });
    expect("GET", "/api/sops")
        .respond_with(json(json!({ "sops": [sop] })))
        .mount(&server)
        .await;
    expect("GET", "/api/sops/restock/full")
        .respond_with(json(sop.clone()))
        .mount(&server)
        .await;
    expect("POST", "/api/sops")
        .and(body_json(sop.clone()))
        .respond_with(json(json!({ "created": "restock" })))
        .mount(&server)
        .await;
    expect("PUT", "/api/sops/restock")
        .respond_with(json(json!({ "saved": "restock" })))
        .mount(&server)
        .await;
    expect("DELETE", "/api/sops/restock")
        .respond_with(json(json!({ "deleted": "restock" })))
        .mount(&server)
        .await;
    for route in [
        "/api/sops/restock/graph",
        "/api/sops/graph-legend",
        "/api/sops/trigger-sources",
    ] {
        expect("GET", route)
            .respond_with(json(json!({ "ok": route })))
            .mount(&server)
            .await;
    }
    expect("POST", "/api/sops/graph-draft")
        .and(body_json(json!({ "sop": sop })))
        .respond_with(json(json!({ "nodes": [] })))
        .mount(&server)
        .await;
    expect("POST", "/api/sops/wire-draft")
        .respond_with(json(json!({ "sop": sop, "graph": {} })))
        .mount(&server)
        .await;

    let sops = client.sops();
    assert_eq!(sops.list().await.unwrap().len(), 1);
    assert_eq!(sops.get("restock").await.unwrap()["name"], "restock");
    sops.create(&sop).await.unwrap();
    sops.save("restock", &sop).await.unwrap();
    sops.delete("restock").await.unwrap();
    assert_eq!(
        sops.graph("restock").await.unwrap()["ok"],
        "/api/sops/restock/graph"
    );
    sops.graph_legend().await.unwrap();
    sops.trigger_sources().await.unwrap();
    sops.graph_draft(&sop).await.unwrap();
    let edit = json!({ "op": "connect", "from": 1, "to": 2, "role": "sequence" });
    assert!(sops.wire_draft(&sop, &edit).await.unwrap()["graph"].is_object());
}

#[tokio::test]
async fn follows_and_decides_runs() {
    let (server, client) = gateway().await;
    expect("POST", "/api/sops/restock/run")
        .and(body_json(json!({ "payload": "{\"sku\":1}" })))
        .respond_with(json(json!({ "run_id": "r1" })))
        .mount(&server)
        .await;
    expect("GET", "/api/sops/runs")
        .and(query_param("sop", "restock"))
        .respond_with(json(json!({ "runs": [{
            "run_id": "r1", "sop_name": "restock", "status": "waiting_approval", "current_step": 1,
            "total_steps": 2, "started_at": "t", "completed_at": null, "trigger_source": "manual", "active": true
        }] })))
        .mount(&server)
        .await;
    expect("GET", "/api/sops/restock/runs/r1/overlay")
        .respond_with(json(json!({ "run_id": "r1", "waiting": true })))
        .mount(&server)
        .await;
    expect("POST", "/api/sops/restock/runs/r1/decide")
        .and(body_json(json!({ "deny": { "reason": "not now" } })))
        .respond_with(json(json!({ "run_id": "r1", "status": "failed" })))
        .mount(&server)
        .await;
    expect("POST", "/api/sops/restock/runs/r1/cancel")
        .and(body_json(json!({ "reason": null })))
        .respond_with(
            ResponseTemplate::new(202)
                .set_body_json(json!({ "run_id": "r1", "status": "cancel_requested" })),
        )
        .mount(&server)
        .await;
    expect("GET", "/admin/sop/pending")
        .respond_with(json(json!({ "pending": [{
            "run_id": "r1", "sop_name": "restock", "step": 1, "total_steps": 2,
            "waiting_since": "t", "kind": "approval"
        }] })))
        .mount(&server)
        .await;
    expect("POST", "/admin/sop/approve")
        .and(body_json(json!({ "run_id": "r1", "reason": null })))
        .respond_with(json(json!({ "outcome": "resumed", "run_id": "r1" })))
        .mount(&server)
        .await;
    expect("POST", "/admin/sop/deny")
        .and(body_json(json!({ "run_id": "r1", "reason": "no" })))
        .respond_with(json(json!({ "outcome": "denied", "run_id": "r1" })))
        .mount(&server)
        .await;

    let sops = client.sops();
    assert_eq!(
        sops.run("restock", Some("{\"sku\":1}")).await.unwrap(),
        "r1"
    );
    assert!(sops.runs(Some("restock")).await.unwrap()[0].active);
    assert_eq!(
        sops.run_overlay("restock", "r1").await.unwrap()["waiting"],
        true
    );
    let deny = Decision::Deny {
        reason: Some("not now".into()),
    };
    assert_eq!(
        sops.decide("restock", "r1", &deny).await.unwrap()["status"],
        "failed"
    );
    assert_eq!(
        sops.cancel("restock", "r1", None).await.unwrap()["status"],
        "cancel_requested"
    );
    assert_eq!(sops.pending().await.unwrap()[0].kind, "approval");
    assert_eq!(sops.approve("r1").await.unwrap(), "resumed");
    assert_eq!(sops.deny("r1", Some("no")).await.unwrap(), "denied");
}

#[tokio::test]
async fn walks_the_quickstart() {
    let (server, client) = gateway().await;
    expect("GET", "/api/quickstart/state")
        .respond_with(json(json!({ "quickstart_completed": false })))
        .mount(&server)
        .await;
    expect("POST", "/api/quickstart/fields")
        .and(body_json(
            json!({ "section": "channel", "type_key": "whatsapp" }),
        ))
        .respond_with(json(json!({ "fields": [] })))
        .mount(&server)
        .await;
    expect("POST", "/api/quickstart/validate")
        .respond_with(json(json!({ "kind": "ok" })))
        .mount(&server)
        .await;
    expect("POST", "/api/quickstart/apply")
        .respond_with(json(json!({ "kind": "applied", "daemon_restarted": true })))
        .mount(&server)
        .await;
    expect("POST", "/api/quickstart/dismiss")
        .and(body_json(
            json!({ "run_id": "q1", "surface": "test", "last_step": null }),
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let quickstart = client.quickstart();
    let submission = json!({ "agent": { "name": "helper" } });
    assert_eq!(
        quickstart.state().await.unwrap()["quickstart_completed"],
        false
    );
    assert!(quickstart.fields("channel", "whatsapp").await.unwrap()["fields"].is_array());
    assert_eq!(
        quickstart.validate(&submission).await.unwrap()["kind"],
        "ok"
    );
    assert_eq!(
        quickstart.apply(&submission).await.unwrap()["kind"],
        "applied"
    );
    quickstart.dismiss("q1", "test", None).await.unwrap();
}

#[tokio::test]
async fn publishes_canvas_frames() {
    let (server, client) = gateway().await;
    let frame = json!({ "frame_id": "f1", "content_type": "markdown", "content": "# Hi", "timestamp": "t" });
    expect("GET", "/api/canvas")
        .respond_with(json(json!({ "canvases": ["board"] })))
        .mount(&server)
        .await;
    expect("POST", "/api/canvas/board")
        .and(body_json(
            json!({ "content_type": "markdown", "content": "# Hi" }),
        ))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(json!({ "canvas_id": "board", "frame": frame })),
        )
        .mount(&server)
        .await;
    expect("GET", "/api/canvas/board")
        .respond_with(json(json!({ "canvas_id": "board", "frame": frame })))
        .mount(&server)
        .await;
    expect("GET", "/api/canvas/board/history")
        .respond_with(json(json!({ "canvas_id": "board", "frames": [frame] })))
        .mount(&server)
        .await;
    expect("DELETE", "/api/canvas/board")
        .respond_with(json(json!({ "canvas_id": "board", "status": "cleared" })))
        .mount(&server)
        .await;

    let canvas = client.canvas();
    assert_eq!(canvas.list().await.unwrap(), ["board"]);
    assert_eq!(
        canvas
            .publish("board", "markdown", "# Hi")
            .await
            .unwrap()
            .frame_id,
        "f1"
    );
    assert_eq!(canvas.current("board").await.unwrap().content, "# Hi");
    assert_eq!(canvas.history("board").await.unwrap().len(), 1);
    canvas.clear("board").await.unwrap();
}

#[tokio::test]
async fn webhooks_send_headers_and_detect_duplicates() {
    let (server, client) = gateway().await;
    expect("POST", "/webhook")
        .and(query_param("agent", "helper"))
        .and(header("X-Webhook-Secret", "s3"))
        .and(header("X-Idempotency-Key", "k1"))
        .and(body_json(json!({ "message": "Ping" })))
        .respond_with(json(json!({ "response": "Pong", "model": "m" })))
        .mount(&server)
        .await;
    expect("POST", "/webhook")
        .and(header("X-Idempotency-Key", "k2"))
        .respond_with(json(
            json!({ "status": "duplicate", "idempotent": true, "message": "" }),
        ))
        .mount(&server)
        .await;

    let webhook = client.webhook();
    let options = WebhookOptions {
        agent: Some("helper"),
        secret: Some("s3"),
        idempotency_key: Some("k1"),
        session_id: None,
    };
    assert!(matches!(
        webhook.send("Ping", options).await.unwrap(),
        WebhookReply::Reply(reply) if reply.response == "Pong"
    ));
    let duplicate = WebhookOptions {
        idempotency_key: Some("k2"),
        ..WebhookOptions::default()
    };
    assert_eq!(
        webhook.send("Ping", duplicate).await.unwrap(),
        WebhookReply::Duplicate
    );
}

#[tokio::test]
async fn a2a_returns_the_answer_text_or_the_rpc_error() {
    let (server, client) = gateway().await;
    expect("POST", "/a2a/helper")
        .and(body_json(json!({
            "jsonrpc": "2.0", "id": 1, "method": "message/send",
            "params": { "message": { "parts": [{ "kind": "text", "text": "Hi" }] } }
        })))
        .respond_with(json(json!({ "jsonrpc": "2.0", "id": 1, "result": {
            "id": "t", "kind": "task", "status": { "state": "completed" },
            "artifacts": [{ "artifactId": "a", "parts": [{ "kind": "text", "text": "Hello" }] }]
        } })))
        .mount(&server)
        .await;
    expect("POST", "/a2a/other")
        .respond_with(json(json!({ "jsonrpc": "2.0", "id": 1, "error": { "code": -32000, "message": "agent failed" } })))
        .mount(&server)
        .await;
    expect("GET", "/a2a/helper/.well-known/agent-card.json")
        .respond_with(json(json!({ "name": "helper" })))
        .mount(&server)
        .await;
    expect("GET", "/.well-known/agents-card.json")
        .respond_with(json(
            json!({ "name": "gateway", "skills": [{ "id": "helper" }] }),
        ))
        .mount(&server)
        .await;

    let webhook = client.webhook();
    assert_eq!(webhook.a2a_send("helper", "Hi").await.unwrap(), "Hello");
    assert!(
        webhook
            .a2a_send("other", "Hi")
            .await
            .unwrap_err()
            .to_string()
            .contains("agent failed")
    );
    assert_eq!(
        webhook.agent_card("helper").await.unwrap()["name"],
        "helper"
    );
    assert_eq!(
        webhook.agent_catalog().await.unwrap()["skills"][0]["id"],
        "helper"
    );
}

#[tokio::test]
async fn carries_webauthn_ceremonies() {
    let (server, client) = gateway().await;
    expect("POST", "/api/webauthn/register/start")
        .and(body_json(json!({ "user_id": "u", "user_name": "Ana" })))
        .respond_with(json(json!({ "challenge": "c" })))
        .mount(&server)
        .await;
    for route in [
        "/api/webauthn/register/finish",
        "/api/webauthn/auth/start",
        "/api/webauthn/auth/finish",
    ] {
        expect("POST", route)
            .respond_with(json(json!({ "status": "ok" })))
            .mount(&server)
            .await;
    }
    expect("GET", "/api/webauthn/credentials")
        .and(query_param("user_id", "u"))
        .respond_with(json(json!({ "credentials": [] })))
        .mount(&server)
        .await;
    expect("DELETE", "/api/webauthn/credentials/k1")
        .and(query_param("user_id", "u"))
        .respond_with(json(json!({ "status": "deleted" })))
        .mount(&server)
        .await;

    let webauthn = client.webauthn();
    assert_eq!(
        webauthn.register_start("u", "Ana").await.unwrap()["challenge"],
        "c"
    );
    webauthn.register_finish(&json!({})).await.unwrap();
    webauthn.auth_start("u").await.unwrap();
    webauthn.auth_finish(&json!({})).await.unwrap();
    assert!(webauthn.credentials("u").await.unwrap()["credentials"].is_array());
    webauthn.delete_credential("u", "k1").await.unwrap();
}
