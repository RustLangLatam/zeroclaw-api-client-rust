# Guides

Each guide covers one area of the gateway and the module that calls it:

| Guide | Module | Accessor |
|---|---|---|
| [Configuration](./configuration.md) | `config` | `client.config()` |
| [Agents, personality and skills](./agents.md) | `agents`, `skills`, `files` | `client.agent(alias)`, `client.skills()`, `client.files()` |
| [Channels and group chats](./channels.md) | `channels` | `client.channels()` |
| [Chatting with an agent](./chat.md) | `chat`, `webhook` | `client.chat(alias, …)`, `client.webhook()` |
| [Sessions and live events](./events.md) | `sessions`, `events` | `client.sessions()`, `client.events()` |
| [Cron, memory and cost](./operations.md) | `cron`, `memory`, `cost`, `system` | `client.cron()`, `client.memory()`, `client.cost()`, `client.system()` |
| [SOPs](./sops.md) | `sops`, `canvas`, `quickstart` | `client.sops()`, `client.canvas()`, `client.quickstart()` |
| [Resilience](./resilience.md) | `transport` | `ClientBuilder` |
| [Errors](./errors.md) | `error` | — |

Pairing is covered in [Getting Started](../getting-started/pairing.md);
WebAuthn (`client.webauthn()`) only carries the JSON of a hardware-key
ceremony run elsewhere.
