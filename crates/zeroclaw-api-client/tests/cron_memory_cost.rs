//! Cron jobs, memory and cost at the HTTP boundary.

mod common;

use common::{expect, gateway, json};
use serde_json::json;
use wiremock::matchers::{body_json, query_param};
use zeroclaw_api_client::cron::{CronJobPatch, CronSettingsPatch, Delivery, NewCronJob};
use zeroclaw_api_client::memory::MemoryQuery;

fn job() -> serde_json::Value {
    json!({
        "id": "j1", "expression": "0 9 * * *",
        "schedule": { "kind": "cron", "expr": "0 9 * * *", "tz": null },
        "command": "echo hi", "prompt": null, "name": null, "job_type": "shell",
        "session_target": "isolated", "model": null, "agent_alias": "helper", "enabled": true,
        "delivery": { "mode": "announce", "channel": "whatsapp.main", "to": "+34600000000", "best_effort": true },
        "delete_after_run": false, "uses_memory": true, "source": "imperative",
        "shell_output_format": "raw", "created_at": "t", "next_run": "t",
        "last_run": null, "last_status": null, "last_output": null
    })
}

#[tokio::test]
async fn creates_runs_and_removes_a_job() {
    let (server, client) = gateway().await;
    expect("POST", "/api/cron")
        .and(body_json(json!({
            "agent": "helper", "schedule": "0 9 * * *", "command": "echo hi",
            "delivery": { "mode": "announce", "channel": "whatsapp.main", "to": "+34600000000", "best_effort": true },
            "shell_output_format": "raw"
        })))
        .respond_with(json(json!({ "status": "ok", "job": job() })))
        .mount(&server)
        .await;
    expect("POST", "/api/cron/j1/run")
        .respond_with(json(json!({
            "status": "ok", "job_id": "j1", "success": true, "output": "hi",
            "duration_ms": 5, "started_at": "t", "finished_at": "t"
        })))
        .mount(&server)
        .await;
    expect("GET", "/api/cron/j1/runs")
        .and(query_param("limit", "5"))
        .respond_with(json(json!({ "runs": [{
            "id": 1, "job_id": "j1", "started_at": "t", "finished_at": "t",
            "status": "ok", "output": "hi", "duration_ms": 5
        }] })))
        .mount(&server)
        .await;
    expect("PATCH", "/api/cron/j1")
        .and(body_json(json!({ "enabled": false })))
        .respond_with(json(json!({ "status": "ok", "job": job() })))
        .mount(&server)
        .await;
    expect("DELETE", "/api/cron/j1")
        .respond_with(json(json!({ "status": "ok" })))
        .mount(&server)
        .await;
    expect("GET", "/api/cron")
        .respond_with(json(json!({ "jobs": [job()] })))
        .mount(&server)
        .await;

    let cron = client.cron();
    let created = cron
        .create(&NewCronJob {
            agent: "helper".into(),
            schedule: "0 9 * * *".into(),
            command: Some("echo hi".into()),
            delivery: Some(Delivery::announce("whatsapp.main", "+34600000000")),
            shell_output_format: Some("raw".into()),
            ..NewCronJob::default()
        })
        .await
        .unwrap();
    assert_eq!(created.id, "j1");
    assert_eq!(cron.run("j1").await.unwrap().output, "hi");
    assert_eq!(cron.runs("j1", Some(5)).await.unwrap().len(), 1);
    cron.update(
        "j1",
        &CronJobPatch {
            enabled: Some(false),
            ..CronJobPatch::default()
        },
    )
    .await
    .unwrap();
    cron.delete("j1").await.unwrap();
    assert_eq!(cron.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn scheduler_settings() {
    let (server, client) = gateway().await;
    expect("GET", "/api/cron/settings")
        .respond_with(json(
            json!({ "enabled": true, "catch_up_on_startup": false, "max_run_history": 50 }),
        ))
        .mount(&server)
        .await;
    expect("PATCH", "/api/cron/settings")
        .and(body_json(json!({ "max_run_history": 10 })))
        .respond_with(json(json!({
            "status": "ok", "enabled": true, "catch_up_on_startup": false, "max_run_history": 10
        })))
        .mount(&server)
        .await;

    assert_eq!(client.cron().settings().await.unwrap().max_run_history, 50);
    let patch = CronSettingsPatch {
        max_run_history: Some(10),
        ..CronSettingsPatch::default()
    };
    assert_eq!(
        client
            .cron()
            .update_settings(&patch)
            .await
            .unwrap()
            .max_run_history,
        10
    );
}

#[tokio::test]
async fn stores_recalls_and_forgets_memory() {
    let (server, client) = gateway().await;
    expect("POST", "/api/memory")
        .and(body_json(
            json!({ "key": "k", "content": "c", "category": null, "agent": "helper" }),
        ))
        .respond_with(json(json!({ "status": "ok" })))
        .mount(&server)
        .await;
    expect("GET", "/api/memory")
        .and(query_param("query", "c"))
        .and(query_param("agent", "helper"))
        .respond_with(json(json!({ "entries": [{
            "id": "1", "key": "k", "content": "c", "category": "core", "timestamp": "t",
            "score": 0.9, "kind": { "semantic": "fact" }, "pinned": false
        }] })))
        .mount(&server)
        .await;
    expect("DELETE", "/api/memory/a%2Fb")
        .respond_with(json(json!({ "status": "ok", "deleted": true })))
        .mount(&server)
        .await;

    let memory = client.memory();
    memory.store("k", "c", None, Some("helper")).await.unwrap();
    let found = memory
        .search(&MemoryQuery {
            query: Some("c"),
            agent: Some("helper"),
            ..MemoryQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(found[0].score, Some(0.9));
    assert!(memory.forget("a/b", None).await.unwrap());
}

#[tokio::test]
async fn reports_cost_by_window_and_agent() {
    let (server, client) = gateway().await;
    let cost = json!({ "cost": {
        "session_cost_usd": 0.0, "daily_cost_usd": 0.1, "monthly_cost_usd": 1.0,
        "total_tokens": 10, "request_count": 2,
        "by_model": { "m": { "model": "m", "cost_usd": 1.0, "input_tokens": 6,
                             "cached_input_tokens": 0, "output_tokens": 4, "total_tokens": 10, "request_count": 2 } },
        "by_agent": {}
    } });
    expect("GET", "/api/cost")
        .and(query_param("from", "2026-10-01T00:00:00Z"))
        .respond_with(json(cost.clone()))
        .mount(&server)
        .await;
    expect("GET", "/api/cost")
        .and(query_param("agent", "helper"))
        .respond_with(json(cost))
        .mount(&server)
        .await;

    let window = client
        .cost()
        .summary(Some("2026-10-01T00:00:00Z"), None)
        .await
        .unwrap();
    assert_eq!(window.by_model["m"].total_tokens, 10);
    assert_eq!(
        client.cost().agent("helper").await.unwrap().request_count,
        2
    );
}
