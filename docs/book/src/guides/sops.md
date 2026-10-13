# SOPs

Standard operating procedures are step-by-step workflows the agents run,
started by triggers (manual, cron, webhook, channel messages, …). Their
definitions are large and evolve with ZeroClaw, so `client.sops()` handles
them as JSON ([why](../architecture/decisions/ADR-002-dynamic-shapes-stay-json.md)).

- **Authoring:** `list`, `get`, `create`, `save`, `delete`; `graph` returns
  an SOP as a node graph with layout and diagnostics; `graph_draft` and
  `wire_draft` do the same for unsaved edits; `graph_legend` and
  `trigger_sources` describe what can be used.
- **Running:** `run(name, payload)` starts one through its manual trigger and
  returns the run id; `runs` lists active and recent runs; `run_overlay`
  shows a run's state per step with its tool calls; `watch_runs` streams run
  changes live; `cancel` stops a run at its next step boundary.
- **Approvals:** a run can wait for an operator. Answer it with
  `decide(name, run_id, &Decision::Approve)` (or `Deny`, `Amend`, `Revise`),
  or, on the gateway host, with `pending`, `approve` and `deny`.

Every SOP route answers 503 while the SOP subsystem is disabled.

## Canvases and the quickstart

`client.canvas()` publishes HTML, SVG, Markdown or text frames to a named
canvas, reads them back and streams them live (`watch`).
`client.quickstart()` drives the first-agent setup the dashboard offers:
`state`, `fields`, `validate`, `apply`.
