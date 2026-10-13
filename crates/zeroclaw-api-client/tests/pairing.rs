//! Pairing and devices at the HTTP boundary.

mod common;

use common::{expect, gateway, json};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};
use zeroclaw_api_client::Client;

#[tokio::test]
async fn a_code_buys_a_token_that_later_calls_use() {
    let (server, _) = gateway().await;
    Mock::given(method("POST"))
        .and(path("/api/pair"))
        .and(body_json(
            json!({ "code": "123456", "device_name": "sdk", "device_type": null }),
        ))
        .respond_with(json(json!({
            "paired": true, "persisted": true, "token": "zc_new", "message": "Pairing successful"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/devices"))
        .and(header("authorization", "Bearer zc_new"))
        .respond_with(json(json!({ "devices": [{
            "id": "d1", "name": "sdk", "device_type": null, "paired_at": "t",
            "last_seen": "t", "ip_address": "127.0.0.1"
        }], "count": 1 })))
        .mount(&server)
        .await;

    let anonymous = Client::builder(server.uri()).build().unwrap();
    let paired = anonymous
        .pairing()
        .pair("123456", Some("sdk"), None)
        .await
        .unwrap();
    let client = anonymous.with_token(paired.token);
    assert_eq!(client.pairing().devices().await.unwrap()[0].id, "d1");
}

#[tokio::test]
async fn wrong_codes_are_refused_with_the_gateways_reason() {
    let (server, client) = gateway().await;
    Mock::given(method("POST"))
        .and(path("/api/pair"))
        .respond_with(
            ResponseTemplate::new(400).set_body_json(
                json!({ "paired": false, "error": "Invalid or expired pairing code" }),
            ),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/pair"))
        .and(header("X-Pairing-Code", "000000"))
        .respond_with(ResponseTemplate::new(429).set_body_json(
            json!({ "error": "Too many pairing requests. Please retry later.", "retry_after": 60 }),
        ))
        .mount(&server)
        .await;

    let error = client.pairing().pair("1", None, None).await.unwrap_err();
    assert_eq!(error.status(), Some(400));
    assert!(error.to_string().contains("Invalid or expired"));
    assert!(
        client
            .pairing()
            .pair_legacy("000000")
            .await
            .unwrap_err()
            .is_rate_limited()
    );
}

#[tokio::test]
async fn codes_are_minted_and_devices_managed() {
    let (server, client) = gateway().await;
    Mock::given(method("GET"))
        .and(path("/pair/code"))
        .respond_with(json(
            json!({ "success": true, "pairing_required": true, "pairing_code": null }),
        ))
        .mount(&server)
        .await;
    expect("POST", "/api/pairing/initiate")
        .respond_with(json(
            json!({ "pairing_code": "654321", "message": "New pairing code generated" }),
        ))
        .mount(&server)
        .await;
    expect("GET", "/admin/paircode")
        .respond_with(json(
            json!({ "success": true, "pairing_required": true, "pairing_code": "111111" }),
        ))
        .mount(&server)
        .await;
    expect("POST", "/admin/paircode/new")
        .and(query_param("rotate", "all"))
        .respond_with(json(
            json!({ "success": true, "pairing_required": true, "pairing_code": "222222" }),
        ))
        .mount(&server)
        .await;
    expect("DELETE", "/api/devices/d1")
        .respond_with(json(
            json!({ "message": "Device revoked", "device_id": "d1" }),
        ))
        .mount(&server)
        .await;
    expect("POST", "/api/devices/d2/token/rotate")
        .respond_with(json(
            json!({ "device_id": "d2", "pairing_code": "333333", "message": "" }),
        ))
        .mount(&server)
        .await;
    expect("POST", "/api/devices/me/capabilities")
        .and(body_json(json!({ "capabilities": ["chat"] })))
        .respond_with(json(
            json!({ "message": "Capabilities updated", "capabilities": ["chat"] }),
        ))
        .mount(&server)
        .await;

    let pairing = client.pairing();
    assert_eq!(pairing.initial_code().await.unwrap().pairing_code, None);
    assert_eq!(pairing.new_code().await.unwrap(), "654321");
    assert_eq!(
        pairing.admin_code().await.unwrap().pairing_code.as_deref(),
        Some("111111")
    );
    assert_eq!(
        pairing
            .admin_new_code(Some("all"))
            .await
            .unwrap()
            .pairing_code
            .as_deref(),
        Some("222222")
    );
    pairing.revoke_device("d1").await.unwrap();
    assert_eq!(
        pairing.rotate_device_token("d2").await.unwrap().as_deref(),
        Some("333333")
    );
    assert_eq!(pairing.set_capabilities(&["chat"]).await.unwrap(), ["chat"]);
}
