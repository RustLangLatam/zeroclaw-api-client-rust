# Connecting and pairing

A ZeroClaw gateway with pairing on (the default) answers only clients that
send a paired bearer token: `Authorization: Bearer zc_<64 hex>`. A client
gets one by exchanging a six-digit pairing code, which is single use and
valid for ten minutes.

## Getting a code

- **First run:** the daemon prints a code when it starts and no client is
  paired yet; [`pairing().initial_code()`](../reference/endpoints.md) also
  returns it until the first pairing.
- **From a paired client:** `pairing().new_code()` mints one for another
  device.
- **On the gateway host:** `pairing().admin_code()` and
  `pairing().admin_new_code(rotate)` (loopback only); `rotate` can also
  revoke every token (`"all"`) or one device's.

## Exchanging it

```rust,no_run
# async fn demo() -> zeroclaw_api_client::Result<()> {
use zeroclaw_api_client::Client;

let anonymous = Client::builder("http://127.0.0.1:42617").build()?;
let paired = anonymous
    .pairing()
    .pair("123456", Some("billing-service"), Some("server"))
    .await?;
// Store `paired.token` as a secret; it is shown only once.
let client = anonymous.with_token(paired.token);
# Ok(()) }
```

A wrong or expired code is refused with 400; too many attempts are
rate-limited with 429 (`Error::is_rate_limited`), with a `retry_after` field in
the error body.

## Managing devices

`pairing().devices()` lists the paired devices, `revoke_device(id)` revokes
one and its token, and `rotate_device_token(id)` revokes it and mints a code
for it to pair again.

## Keeping the token safe

The client never prints its token: `Debug` output shows only whether it is
authenticated, and `Paired`'s `Debug` redacts the token.
