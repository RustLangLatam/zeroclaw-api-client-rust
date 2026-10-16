# Channels

A channel is named `type.alias` (`whatsapp.main`, `telegram.bot`), as
`channels().list()` returns it with its status, health and readiness.

## Identities and logins

- `bind(channel_type, alias, identity)` allows a Telegram, WeChat or LINE user
  to talk to the channel.
- `relink(channel)` clears a QR-paired channel's saved login (WhatsApp Web);
  after `system().reload()` the channel asks to be paired again.
