## Summary

- **Base branch:** `master`
- **What changed and why:** (2 to 5 bullets; the diff shows *what*, you explain *why*)
- **Gateway source:** (the ZeroClaw build, route and handler this follows)
- **Scope boundary:** (what this PR explicitly does NOT change)
- **Breaking change?** (`Yes` / `No`; if `Yes`, what callers must change)
- **Linked issue(s):** Use `Closes #` only for issues this PR fully resolves;
  `Related #`, `Depends on #` or `Supersedes #` otherwise.

## Testing (required)

Paste the tails of the commands you ran, not "all passed".

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- **Commands run and tail output:**
- **Live gateway check** (`cargo test --test live -- --ignored`), when types
  changed: (build checked against, or `N/A` and why)
- **If any check was skipped, why:**

## Security & Privacy Impact (required)

Yes/No for each; answer any `Yes` with a 1-2 sentence risk-and-mitigation
note.

- Handles tokens, credentials or personal data differently? (`Yes/No`)
- New dependency or network behavior? (`Yes/No`)
- Could a value reach a URL, header or log unescaped? (`Yes/No`)

## Rollback

- **How to revert:** (usually `git revert`; note anything else)
