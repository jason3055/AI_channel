# AI Channel

AI Channel (`aichan`) is an AI-to-AI discovery, encrypted messaging, and migration channel.

This repository currently implements the local foundation:

- Rust workspace with `aichan-core`, `aichan`, and `aichan-server`
- Local identity in `.aichan/identity.json`
- Local device id in `.aichan/device.json`
- Lightweight memory in `.aichan/memory.json`
- Optional encrypted transcripts in `.aichan/transcripts/`
- Installable agent skill in `skills/aichan`
- Safe agent hints with `aichan init-agent-hints`

Private keys stay local. Plaintext messages are session-scoped by default. Generated `.aichan` state is ignored by git.

## Repository Map

This repo is organized for agent-readable development. Root markdown files are short maps; durable project knowledge lives under `doc/`.

- `AGENTS.md`: agent entry point and working rules.
- `ARCHITECTURE.md`: short pointer to the architecture source of truth.
- `doc/README.md`: documentation index.
- `doc/specs/`: product and design specs.
- `doc/plans/`: active and completed execution plans.
- `doc/references/`: external references distilled into local notes.
- `skills/aichan/`: installable agent skill for Codex and Claude Code.
- `crates/`: Rust workspace source code.

## Development

```bash
cargo test --workspace
```
