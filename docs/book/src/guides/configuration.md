# Configuration

Everything ZeroClaw does is driven by its configuration (`config.toml`), and
the gateway exposes it field by field. Paths are dotted:
`providers.models.anthropic.default.api_key`, `agents.helper.model`,
`channels.whatsapp.main.allowed_groups`.

## Reading

- `config().get(path)` returns a `PropValue`: `Value { path, value, warnings }`
  with the field's display string, or `Secret { path, populated }` for a
  secret, whose value is never returned.
- `config().list(prefix)` enumerates every field under a prefix with its kind
  (`string`, `bool`, `integer`, `enum`, `alias-ref`, …), category, current
  value and whether an environment variable overrides it.
- `config().snapshot()` returns the whole configuration with secrets masked.
  It exists for compatibility; prefer the two calls above.
- `config().schema()` and `config().prop_schema(path)` return the JSON Schema
  (public, static per gateway build).

## Writing

- `config().set(path, value, comment)` writes one field. `value` is any
  serializable value; the gateway coerces it to the field's kind and refuses a
  mismatch with 400 `value_type_mismatch`. A map alias the path implies is
  created (`agents.new_agent.model` creates `agents.new_agent`).
- `config().reset(path)` resets a field to its default.
- `config().patch(ops, override_drift)` applies several operations at once,
  all or nothing:

```rust,no_run
# async fn demo(client: zeroclaw_api_client::Client) -> zeroclaw_api_client::Result<()> {
use zeroclaw_api_client::config::PatchOp;

client
    .config()
    .patch(
        &[
            PatchOp::test("agents.helper.model", "gpt-5.4-mini"),
            PatchOp::replace("agents.helper.model", "gpt-5.5"),
            PatchOp::comment("agents.helper.model", "upgraded by the billing service"),
        ],
        false,
    )
    .await?;
# Ok(()) }
```

If a patched field was edited on disk since the gateway loaded it, the patch
is refused with 409 `config_changed_externally`; `config().drift()` shows
what differs, and `override_drift: true` overwrites it.

## Aliases: agents, providers, channels

Agents and other named entries are map aliases:

- `map_keys("agents")` lists them;
- `create_map_key("agents", "helper")` creates one with defaults;
- `rename_map_key("agents", "helper", "assistant")` renames it and every
  reference to it (and moves an agent's workspace, memory, cron jobs and
  sessions);
- `delete_plan("agents", "helper")` shows what a delete would block on or
  rewrite, and `delete_map_key` does it. Deleting an agent archives its
  workspace and exports its memory first.

`alias_values(AliasSource::ModelProviders)` lists the values an alias
reference may take, and `agent_options()` everything an agent can point to.

## Applying changes

Most writes take effect at once. Some subsystems pick changes up only on a
daemon reload: `config().pending_reload()` says whether one is due, and
`system().reload()` performs it (the connection drops briefly).

## Setup sections and catalogs

`sections()`, `section_items(section)` and `select_section_item(section, key,
alias)` drive the same setup flow as the dashboard. `catalog()` and
`catalog_models(provider)` list the known model providers and their models
(the latter may call the provider with the configured key).
`refresh_context_window(type, alias)` stores a model's context window as the
provider reports it.
