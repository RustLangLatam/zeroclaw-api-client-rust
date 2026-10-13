//! A mock gateway for the HTTP-boundary tests.

#![allow(dead_code)] // Each test binary uses a different subset.

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockBuilder, MockServer, ResponseTemplate};
use zeroclaw_api_client::Client;

/// The token every test client sends.
pub const TOKEN: &str = "zc_0000000000000000000000000000000000000000000000000000000000000000";

/// A mock gateway and a client authenticated against it.
pub async fn gateway() -> (MockServer, Client) {
    let server = MockServer::start().await;
    let client = Client::new(server.uri(), TOKEN).unwrap();
    (server, client)
}

/// A request the gateway expects: `verb` on `route`, with the bearer token.
pub fn expect(verb: &str, route: &str) -> MockBuilder {
    Mock::given(method(verb))
        .and(path(route))
        .and(header("authorization", format!("Bearer {TOKEN}").as_str()))
}

/// A 200 JSON answer.
pub fn json(body: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}
