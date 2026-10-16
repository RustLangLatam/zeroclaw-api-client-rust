# Compatibility

## ZeroClaw versions

The client targets the official ZeroClaw **0.8.5** gateway (tag `v0.8.5`),
whose router (its routes are in `scripts/gateway-routes.txt`) is the
[source of truth](../architecture/decisions/ADR-001-gateway-is-the-source-of-truth.md)
for every request and response shape here. A route enters the client once
official ZeroClaw has it, or at least a pull request that adds it.

Routes that only patched builds have are kept on the branch
`dev/fork-channel-routes` until official ZeroClaw adopts them: sending
through a channel, its group chats and members, and handing a received
message to its agent again (declined upstream in
[#11434](https://github.com/zeroclaw-labs/zeroclaw/issues/11434)). That branch
is not released.

Some routes exist only when the gateway is built with a feature: WebAuthn
(`webauthn`) and A2A (`a2a`). Without it they answer 404.

Response types decode leniently: fields a gateway adds are ignored, and
optional or newer fields are `Option` or defaulted, so a nearby build still
decodes.

## Rust

The minimum supported Rust version is **1.89**
([ADR-003](../architecture/decisions/ADR-003-rust-floor.md)). CI builds with
it.

## Runtime

The client runs on Tokio and uses `reqwest` with rustls, so it needs no
system OpenSSL.
