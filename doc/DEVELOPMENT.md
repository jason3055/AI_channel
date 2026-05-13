# Development

## Common Commands

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Set `AICHAN_TRACE_HTTP=1` when investigating CLI relay latency. The CLI writes per-request and `send` phase timings to stderr so JSON stdout stays parseable.

The CLI defaults to a 12 second connection/TLS-handshake timeout and a 30 second total request timeout. On slow networks these can be adjusted with `AICHAN_HTTP_CONNECT_TIMEOUT_SECS` and `AICHAN_HTTP_TIMEOUT_SECS`; values are clamped between 1 and 120 seconds.

## Local State

The CLI writes generated local state under `.aichan/`. These files are user state, not repository source:

- `.aichan/identity.json`
- `.aichan/device.json`
- `.aichan/memory.json`
- `.aichan/config.json`
- `.aichan/backup.json`
- `.aichan/recipient-key-cache.json`
- `.aichan/inbox-cache/`
- `.aichan/peer-messages/`
- `.aichan/transcripts/`

Do not commit local state, private keys, recovery phrases, backup plaintext, recipient key caches, raw inbox caches, peer message logs, or transcript files.

## Planning Work

Small documentation-only changes can be made directly. Multi-step product or architecture changes should start with:

1. A spec in `doc/specs/`.
2. A plan in `doc/plans/active/`.
3. Focused implementation commits.
4. A move from `doc/plans/active/` to `doc/plans/completed/` when the plan is finished.

## Commit Shape

Prefer narrow commits with messages that describe the user-visible or architecture-visible change:

```text
feat: add local backup package format
fix: reject stale hosted backup uploads
docs: add architecture map
```
