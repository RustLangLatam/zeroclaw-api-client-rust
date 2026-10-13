# Agents, personality and skills

An agent is a configuration entry (`agents.<alias>`): create, rename and
delete agents with the [configuration](./configuration.md) calls. Its files
and skills are reached through `client.agent(alias)`.

## Workspace

Each agent has a workspace directory. Paths are relative to it; leaving it is
refused, and the personality files and `sessions` at its root are protected.

```rust,no_run
# async fn demo(client: zeroclaw_api_client::Client) -> zeroclaw_api_client::Result<()> {
let agent = client.agent("helper");
let listing = agent.list_files(Some("docs")).await?;
let file = agent.read_file("docs/catalog.pdf").await?; // base64 when binary
agent.make_dir("archive").await?;
agent.move_path("docs/old.pdf", "archive/old.pdf").await?;
agent.remove_path("archive").await?;
# Ok(()) }
```

The shared directory (`<install>/shared/`) has the same calls under
`client.files()`, which also uploads images into an agent's workspace
(`upload_image`) and returns the `[IMAGE:…]` marker to attach them to a chat
message.

## Personality

`SOUL.md`, `IDENTITY.md`, `USER.md`, `AGENTS.md`, `TOOLS.md`, `HEARTBEAT.md`
and `MEMORY.md` shape an agent's behavior. `personality_files()` lists them,
`personality(name)` reads one, and `write_personality(name, content,
expected_mtime_ms)` replaces it (20 000 characters at most). Pass the
`mtime_ms` you read to make the write fail with 409 instead of overwriting
someone else's change. `personality_templates(context)` renders the defaults
without writing them.

## Skills

Skills live in bundles (`skill_bundles.<alias>` in the configuration).
`client.skills()` lists bundles and their skills and creates, reads, writes
and deletes (archives, or purges) a skill: front matter plus a Markdown body.
`client.agent(alias).skills()` shows what an agent actually loads, where each
skill comes from, and which the audit dropped.
