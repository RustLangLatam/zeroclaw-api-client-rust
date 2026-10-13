# Sessions and live events

## Sessions

Every conversation is a session: web chats are `gw_<id>`, channel
conversations `whatsapp_<channel>_<peer>…`. `sessions().list()` returns them
with their agent and channel; `messages(id)` the transcript (tool calls and
results included, as JSON text); `state(id)` whether a turn is running.

`add_message(id, content)` appends an assistant message (shown to chat
clients, with no agent turn); `rename`, `abort` and `delete` do what they
say.

## The live event stream

`events().subscribe()` follows `/api/events`, the gateway's Server-Sent
Events stream:

```rust,no_run
# async fn demo(client: zeroclaw_api_client::Client) -> zeroclaw_api_client::Result<()> {
use futures_util::StreamExt;

let mut events = client.events().subscribe().await?;
while let Some(event) = events.next().await {
    let event = event?;
    match event.kind() {
        Some("agent_end") => { /* tokens and cost of a turn: event.get("cost_usd") */ }
        Some("cron_result") => { /* a job ran */ }
        None if event.is_log() => { /* a structured log event */ }
        _ => {}
    }
}
# Ok(()) }
```

Frames have no SSE event name; `Event::kind` reads their `type`. Observer
events (`llm_request`, `tool_call`, `tool_call_start`, `agent_start`,
`agent_end`, `error`, `history_trimmed`) carry `source: "observability"`;
log events carry `@timestamp` instead of a `type`. Frames of a single
session are not published here; they reach that session's chat. The stream
ends when the gateway closes it (on reload, for instance): subscribe again.
`events().history()` returns the latest observer events, kept in memory.

The client's timeout bounds only the wait for the stream to open; once
events flow it stays open as long as the gateway keeps it.
