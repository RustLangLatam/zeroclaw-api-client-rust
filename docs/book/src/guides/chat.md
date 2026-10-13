# Chatting with an agent

## Over the WebSocket

`client.chat(alias, options)` opens `/ws/chat` for agent `alias`. The
connection is a session (`gw_<session_id>`); pass `ChatOptions { session_id,
.. }` to resume one.

`ask` sends a message and waits for the reply:

```rust,no_run
# async fn demo(client: zeroclaw_api_client::Client) -> zeroclaw_api_client::Result<()> {
let mut chat = client.chat("helper", Default::default()).await?;
let reply = chat.ask("How many orders are pending?").await?;
let (text, tokens) = (reply.full_response, reply.tokens_used);
chat.close().await?;
# Ok(()) }
```

To show progress, or to answer tool approvals, read frames one by one:

```rust,no_run
# async fn demo(client: zeroclaw_api_client::Client) -> zeroclaw_api_client::Result<()> {
use zeroclaw_api_client::chat::ChatFrame;

let mut chat = client.chat("helper", Default::default()).await?;
chat.send("Archive last month's invoices").await?;
while let Some(frame) = chat.next_frame().await? {
    match frame {
        ChatFrame::Chunk(text) => { /* append to the screen */ }
        ChatFrame::ToolCall { name, .. } => { /* show the tool running */ }
        ChatFrame::ApprovalRequest { request_id, .. } => {
            chat.answer_approval(&request_id, "approve").await?;
        }
        ChatFrame::Done(result) => break,
        ChatFrame::Error { message, code } => break,
        _ => {}
    }
}
# Ok(()) }
```

A message sent while a turn runs steers it. Closing the chat, or
`sessions().abort(id)`, cancels the running turn. A failed turn is
`Error::Turn { code, message }` (`PROVIDER_ERROR`, `SESSION_QUEUE_FULL`, …).

## One-shot: webhook and A2A

`client.webhook().send(message, options)` posts to `/webhook` and returns the
agent's reply in one HTTP call. Options carry the agent, the
`X-Webhook-Secret` when the gateway sets one, an idempotency key (a repeat
answers `WebhookReply::Duplicate` without running again) and a session id to
keep calls in one conversation.

`client.webhook().a2a_send(alias, text)` does the same over the A2A protocol
(`message/send`) for agents published with `a2a.server.enabled`.
