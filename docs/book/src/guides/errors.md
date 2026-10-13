# Errors

Every call returns `zeroclaw_api_client::Result<T>`; the error is an
`Error`:

| Variant | When |
|---|---|
| `Api { status, message, body }` | The gateway answered with a non-success status. `message` is its explanation (the `error` or `message` field, or the body text); `body` keeps the JSON error body. |
| `Transport` | No HTTP answer: connection refused, TLS, a broken connection. |
| `Timeout` | The gateway did not answer in time, after any retries. |
| `Layer` | A tower layer you added refused the request. |
| `Decode` | A successful answer did not have the expected shape. |
| `WebSocket` | A WebSocket failed after its handshake. |
| `Turn { code, message }` | A chat turn ended in an `error` frame, or was aborted (`ABORTED`). |
| `InvalidUrl` | The base URL is not an absolute `http`/`https` URL. |

`status()`, `is_not_found()`, `is_unauthorized()` (401 and 403),
`is_rate_limited()` (429) and `is_transient()` (no connection, a timeout, or
408, 429, 502, 503, 504: worth trying again later) answer the usual
questions. Idempotent calls were already retried before a transient error
reaches you; see [Resilience](./resilience.md).

## The gateway's error bodies

The gateway does not use one error shape, and the client reads all of them
into `message`:

- `{"error": "…"}`, most routes, sometimes with `retry_after` (seconds) on
  429;
- `{"code": "…", "message": "…", "path": "…", "op_index": n}` on
  configuration routes (`path_not_found`, `value_type_mismatch`,
  `config_changed_externally`, …) and uploads;
- `{"channel": "…", "error": "…"}` on channel actions;
- plain text when the request is refused before its handler runs (a
  malformed body, a body over 64 KiB, the device endpoints' 401), and an
  empty body on the gateway's request timeout (408).

Read further fields from `body`:

```rust,no_run
# async fn demo(client: zeroclaw_api_client::Client) {
use zeroclaw_api_client::Error;

match client.config().set("gateway.port", "not a number", None).await {
    Err(Error::Api { status: 400, body: Some(body), .. }) => {
        let code = body["code"].as_str(); // Some("value_type_mismatch")
    }
    _ => {}
}
# }
```
