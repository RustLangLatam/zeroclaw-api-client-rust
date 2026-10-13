# Testing

Four layers of tests, each catching what the others cannot:

| Kind | Where | Catches |
|---|---|---|
| Vector tables | `#[cfg(test)]` in each module | logic mistakes: parsing, mapping, decisions |
| HTTP boundary | `crates/zeroclaw-api-client/tests/*.rs` | wrong method, path, query or body; decoding of the documented shape |
| Response vectors | `tests/vectors/*.json` + `tests/vectors.rs` | types that do not match what a real gateway sends |
| Live | `tests/live.rs` (ignored) | anything a real gateway does differently today |

## Vector tables

Logic is tested as a list of cases and one loop, so a new case is one line
and a failure names the case:

```rust,ignore
let vectors = [
    (Method::GET, 503, 0, true),
    (Method::POST, 503, 0, false),
];
for (method, status, attempt, expected) in vectors {
    assert_eq!(decide(method, status, attempt), expected, "{method} {status}");
}
```

## Response vectors

`scripts/capture-vectors.py` calls the gateway's read-only routes and saves
each answer, anonymized, as `{"route", "build", "response"}`. Phone numbers,
ids, UUIDs, tokens, IPs and e-mails are replaced; `ZEROCLAW_REDACT` replaces
the words of your own deployment (aliases, names); MCP tools and routes that
hold people's words (logs, memory, transcripts) are left out. Review the
files before committing them.

```sh
ZEROCLAW_URL=http://127.0.0.1:42617 ZEROCLAW_TOKEN=zc_… \
  ZEROCLAW_AGENT=<alias> ZEROCLAW_REDACT=<your,own,names> \
  python3 scripts/capture-vectors.py
cargo test --test vectors
```

`tests/vectors.rs` decodes every file into the type its table names, and
re-encodes it to check nothing is lost. A vector without a type fails the
test, so a new capture is never silently untested.
