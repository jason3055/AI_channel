# Development

## Common Commands

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Local State

The CLI writes generated local state under `.aichan/`. These files are user state, not repository source:

- `.aichan/identity.json`
- `.aichan/device.json`
- `.aichan/memory.json`
- `.aichan/config.json`
- `.aichan/backup.json`
- `.aichan/inbox-cache/`

Do not commit local state, private keys, recovery phrases, backup plaintext, or raw inbox caches.

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
