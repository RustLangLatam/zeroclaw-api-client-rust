# Compatibility

## ZeroClaw versions

The client targets the ZeroClaw **0.8.5** gateway. It is verified against the
RustLangLatam fork's production build, `0.8.5+unified.13` (branch
`fix/whatsapp-silence-stopped-channel`), whose router is the
[source of truth](../architecture/decisions/ADR-001-gateway-is-the-source-of-truth.md)
for every request and response shape here.

Against an upstream ZeroClaw 0.8.5 build everything works except these
routes, which the fork adds; upstream answers them with its web UI fallback
or 404:

| Route | Function |
|---|---|
| `POST /api/channels/{channel}/send` | `channels().send` |
| `GET /api/channels/{channel}/rooms` | `channels().rooms` |
| `POST /api/channels/{channel}/rooms` | `channels().create_room` |
| `GET /api/channels/{channel}/rooms/{room}/members` | `channels().room_members` |
| `POST /api/channels/{channel}/rooms/{room}/invites` | `channels().invite` |
| `DELETE /api/channels/{channel}/rooms/{room}/members/{user}` | `channels().remove_member` |

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
