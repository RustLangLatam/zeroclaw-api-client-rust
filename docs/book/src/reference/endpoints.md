# Endpoint map

Every gateway route the client calls, with the module and function that call
it. The table is generated from the code: each public method's doc comment
starts with its route, and `scripts/endpoint-map.py` collects them.

Routes the client deliberately does not call:

- inbound webhooks (`/whatsapp`, `/linq/{alias}`, `/nextcloud-talk/{alias}`,
  `/webhook/gmail`, the coding-agent hook under `/hooks/`, and the SOP trigger ingress
  `/sop/{path}`): providers and external systems call them, not clients;
- `/api/plugins`, which only a `plugins-wasm` build has: not covered yet;
- the bundled web UI (`/_app/*`) and the HTML API explorer (`/api/docs`);
- the daemon's own protocols: `/acp` (zerocode) and `/ws/nodes` (nodes);
- aliases of routes the client calls: `/a2a/.well-known/agents-card.json`
  (same card as `/.well-known/agents-card.json`) and `GET /api/doctor` (same
  handler as `POST`).

`scripts/route-drift.py <zeroclaw checkout>` compares this table with the
routes the gateway's source declares, and CI runs it against the targeted
fork branch.

Some routes only exist when the gateway is built with a Cargo feature; on a
build without it they answer 404 ([`Error::is_not_found`]):

| Gateway feature | Default | Client methods |
|---|---|---|
| `a2a` | on | `webhook().agent_card`, `agent_catalog`, `a2a_send` (also need `[a2a.server] enabled = true`) |
| `webauthn` | off | `webauthn()` |
| `observability-prometheus` | on | `system().metrics` |
| `plugins-wasm` | off | not covered |

[`Error::is_not_found`]: https://docs.rs/zeroclaw-api-client/latest/zeroclaw_api_client/enum.Error.html#method.is_not_found

| Method | Path | Module | Function |
|---|---|---|---|
| `GET` | `/.well-known/agents-card.json` | `webhook` | `agent_catalog` |
| `POST` | `/a2a/{alias}` | `webhook` | `a2a_send` |
| `GET` | `/a2a/{alias}/.well-known/agent-card.json` | `webhook` | `agent_card` |
| `GET` | `/admin/paircode` | `pairing` | `admin_code` |
| `POST` | `/admin/paircode/new` | `pairing` | `admin_new_code` |
| `POST` | `/admin/reload` | `system` | `reload` |
| `POST` | `/admin/shutdown` | `system` | `shutdown` |
| `POST` | `/admin/sop/approve` | `sops` | `approve` |
| `POST` | `/admin/sop/deny` | `sops` | `deny` |
| `GET` | `/admin/sop/pending` | `sops` | `pending` |
| `GET` | `/api/agents/{alias}/skills` | `agents` | `skills` |
| `GET` | `/api/agents/{alias}/workspace/list` | `agents` | `list_files` |
| `POST` | `/api/agents/{alias}/workspace/mkdir` | `agents` | `make_dir` |
| `POST` | `/api/agents/{alias}/workspace/move` | `agents` | `move_path` |
| `DELETE` | `/api/agents/{alias}/workspace/path` | `agents` | `remove_path` |
| `GET` | `/api/agents/{alias}/workspace/read` | `agents` | `read_file` |
| `GET` | `/api/browse` | `files` | `list` |
| `POST` | `/api/browse/mkdir` | `files` | `make_dir` |
| `DELETE` | `/api/browse/rmdir` | `files` | `remove_dir` |
| `GET` | `/api/canvas` | `canvas` | `list` |
| `DELETE` | `/api/canvas/{id}` | `canvas` | `clear` |
| `GET` | `/api/canvas/{id}` | `canvas` | `current` |
| `POST` | `/api/canvas/{id}` | `canvas` | `publish` |
| `GET` | `/api/canvas/{id}/history` | `canvas` | `history` |
| `GET` | `/api/channels` | `channels` | `list` |
| `POST` | `/api/channels/bind` | `channels` | `bind` |
| `POST` | `/api/channels/{channel}/messages/{message_id}/redeliver` | `channels` | `redeliver` |
| `POST` | `/api/channels/{channel}/relink` | `channels` | `relink` |
| `GET` | `/api/channels/{channel}/rooms` | `channels` | `rooms` |
| `POST` | `/api/channels/{channel}/rooms` | `channels` | `create_room` |
| `POST` | `/api/channels/{channel}/rooms/{room}/invites` | `channels` | `invite` |
| `GET` | `/api/channels/{channel}/rooms/{room}/members` | `channels` | `room_members` |
| `DELETE` | `/api/channels/{channel}/rooms/{room}/members/{user}` | `channels` | `remove_member` |
| `POST` | `/api/channels/{channel}/send` | `channels` | `send` |
| `GET` | `/api/cli-tools` | `system` | `cli_tools` |
| `GET` | `/api/config` | `config` | `snapshot` |
| `OPTIONS` | `/api/config` | `config` | `schema` |
| `PATCH` | `/api/config` | `config` | `patch` |
| `GET` | `/api/config/agent-options` | `config` | `agent_options` |
| `GET` | `/api/config/catalog` | `config` | `catalog` |
| `GET` | `/api/config/catalog/models` | `config` | `catalog_models` |
| `GET` | `/api/config/delete-plan` | `config` | `delete_plan` |
| `GET` | `/api/config/drift` | `config` | `drift` |
| `POST` | `/api/config/init` | `config` | `init` |
| `GET` | `/api/config/list` | `config` | `list` |
| `DELETE` | `/api/config/map-key` | `config` | `delete_map_key` |
| `POST` | `/api/config/map-key` | `config` | `create_map_key` |
| `GET` | `/api/config/map-keys` | `config` | `map_keys` |
| `POST` | `/api/config/migrate` | `config` | `migrate` |
| `POST` | `/api/config/model-providers/{type}/{alias}/refresh-context-window` | `config` | `refresh_context_window` |
| `DELETE` | `/api/config/prop` | `config` | `reset` |
| `GET` | `/api/config/prop` | `config` | `get` |
| `OPTIONS` | `/api/config/prop` | `config` | `prop_schema` |
| `PUT` | `/api/config/prop` | `config` | `set` |
| `GET` | `/api/config/reload-status` | `config` | `pending_reload` |
| `POST` | `/api/config/rename-map-key` | `config` | `rename_map_key` |
| `GET` | `/api/config/resolve-alias-source` | `config` | `alias_values` |
| `GET` | `/api/config/sections` | `config` | `sections` |
| `GET` | `/api/config/sections/{section}` | `config` | `section_items` |
| `POST` | `/api/config/sections/{section}/items/{key}` | `config` | `select_section_item` |
| `GET` | `/api/config/status` | `config` | `status` |
| `GET` | `/api/config/templates` | `config` | `templates` |
| `GET` | `/api/cost` | `cost` | `agent` |
| `GET` | `/api/cost` | `cost` | `summary` |
| `GET` | `/api/cron` | `cron` | `list` |
| `POST` | `/api/cron` | `cron` | `create` |
| `GET` | `/api/cron/settings` | `cron` | `settings` |
| `PATCH` | `/api/cron/settings` | `cron` | `update_settings` |
| `DELETE` | `/api/cron/{id}` | `cron` | `delete` |
| `PATCH` | `/api/cron/{id}` | `cron` | `update` |
| `POST` | `/api/cron/{id}/run` | `cron` | `run` |
| `GET` | `/api/cron/{id}/runs` | `cron` | `runs` |
| `GET` | `/api/devices` | `pairing` | `devices` |
| `POST` | `/api/devices/me/capabilities` | `pairing` | `set_capabilities` |
| `DELETE` | `/api/devices/{id}` | `pairing` | `revoke_device` |
| `POST` | `/api/devices/{id}/token/rotate` | `pairing` | `rotate_device_token` |
| `POST` | `/api/doctor` | `system` | `doctor` |
| `GET` | `/api/events` | `events` | `subscribe` |
| `GET` | `/api/events/history` | `events` | `history` |
| `GET` | `/api/health` | `system` | `runtime_health` |
| `GET` | `/api/integrations` | `system` | `integrations` |
| `GET` | `/api/integrations/settings` | `system` | `integration_settings` |
| `GET` | `/api/logs` | `system` | `logs` |
| `GET` | `/api/memory` | `memory` | `search` |
| `POST` | `/api/memory` | `memory` | `store` |
| `DELETE` | `/api/memory/{key}` | `memory` | `forget` |
| `GET` | `/api/openapi.json` | `system` | `openapi` |
| `POST` | `/api/pair` | `pairing` | `pair` |
| `POST` | `/api/pairing/initiate` | `pairing` | `new_code` |
| `GET` | `/api/personality` | `agents` | `personality_files` |
| `GET` | `/api/personality/templates` | `agents` | `personality_templates` |
| `GET` | `/api/personality/{filename}` | `agents` | `personality` |
| `PUT` | `/api/personality/{filename}` | `agents` | `write_personality` |
| `POST` | `/api/quickstart/apply` | `quickstart` | `apply` |
| `POST` | `/api/quickstart/dismiss` | `quickstart` | `dismiss` |
| `POST` | `/api/quickstart/fields` | `quickstart` | `fields` |
| `GET` | `/api/quickstart/state` | `quickstart` | `state` |
| `POST` | `/api/quickstart/validate` | `quickstart` | `validate` |
| `GET` | `/api/sessions` | `sessions` | `list` |
| `GET` | `/api/sessions/running` | `sessions` | `running` |
| `DELETE` | `/api/sessions/{id}` | `sessions` | `delete` |
| `PUT` | `/api/sessions/{id}` | `sessions` | `rename` |
| `POST` | `/api/sessions/{id}/abort` | `sessions` | `abort` |
| `GET` | `/api/sessions/{id}/messages` | `sessions` | `messages` |
| `POST` | `/api/sessions/{id}/messages` | `sessions` | `add_message` |
| `GET` | `/api/sessions/{id}/state` | `sessions` | `state` |
| `GET` | `/api/skills/bundles` | `skills` | `bundles` |
| `GET` | `/api/skills/bundles/{bundle}/skills` | `skills` | `list` |
| `POST` | `/api/skills/bundles/{bundle}/skills` | `skills` | `create` |
| `DELETE` | `/api/skills/bundles/{bundle}/skills/{name}` | `skills` | `delete` |
| `GET` | `/api/skills/bundles/{bundle}/skills/{name}` | `skills` | `read` |
| `PUT` | `/api/skills/bundles/{bundle}/skills/{name}` | `skills` | `write` |
| `GET` | `/api/skills/slash-option-kinds` | `skills` | `slash_option_kinds` |
| `GET` | `/api/sops` | `sops` | `list` |
| `POST` | `/api/sops` | `sops` | `create` |
| `POST` | `/api/sops/graph-draft` | `sops` | `graph_draft` |
| `GET` | `/api/sops/graph-legend` | `sops` | `graph_legend` |
| `GET` | `/api/sops/runs` | `sops` | `runs` |
| `GET` | `/api/sops/trigger-sources` | `sops` | `trigger_sources` |
| `POST` | `/api/sops/wire-draft` | `sops` | `wire_draft` |
| `DELETE` | `/api/sops/{name}` | `sops` | `delete` |
| `PUT` | `/api/sops/{name}` | `sops` | `save` |
| `GET` | `/api/sops/{name}/full` | `sops` | `get` |
| `GET` | `/api/sops/{name}/graph` | `sops` | `graph` |
| `POST` | `/api/sops/{name}/run` | `sops` | `run` |
| `POST` | `/api/sops/{name}/runs/{run_id}/cancel` | `sops` | `cancel` |
| `POST` | `/api/sops/{name}/runs/{run_id}/decide` | `sops` | `decide` |
| `GET` | `/api/sops/{name}/runs/{run_id}/overlay` | `sops` | `run_overlay` |
| `GET` | `/api/status` | `system` | `status` |
| `GET` | `/api/tools` | `system` | `tools` |
| `POST` | `/api/tools/param-options` | `system` | `tool_param_options` |
| `GET` | `/api/tuis` | `system` | `tuis` |
| `POST` | `/api/upload` | `files` | `upload_image` |
| `GET` | `/api/version/check` | `system` | `version_check` |
| `POST` | `/api/version/upgrade` | `system` | `upgrade` |
| `GET` | `/api/version/upgrade/status` | `system` | `upgrade_status` |
| `POST` | `/api/webauthn/auth/finish` | `webauthn` | `auth_finish` |
| `POST` | `/api/webauthn/auth/start` | `webauthn` | `auth_start` |
| `GET` | `/api/webauthn/credentials` | `webauthn` | `credentials` |
| `DELETE` | `/api/webauthn/credentials/{id}` | `webauthn` | `delete_credential` |
| `POST` | `/api/webauthn/register/finish` | `webauthn` | `register_finish` |
| `POST` | `/api/webauthn/register/start` | `webauthn` | `register_start` |
| `GET` | `/health` | `system` | `health` |
| `GET` | `/metrics` | `system` | `metrics` |
| `POST` | `/pair` | `pairing` | `pair_legacy` |
| `GET` | `/pair/code` | `pairing` | `initial_code` |
| `POST` | `/webhook` | `webhook` | `send` |
| `GET` | `/ws/canvas/{id}` | `canvas` | `watch` |
| `GET` | `/ws/chat` | `chat` | `chat` |
| `GET` | `/ws/sops/runs` | `sops` | `watch_runs` |
