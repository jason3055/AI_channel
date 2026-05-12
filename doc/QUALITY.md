# Quality

This document tracks quality expectations that should become mechanical over time.

## Gates

Broad repo changes should pass:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Narrow documentation-only changes should at least verify git status, moved paths, and stale references.

## Current Risk Areas

- Protocol crypto is not implemented yet.
- Server HTTP and Firestore storage are not implemented yet.
- AI-readable structured logging is documented but not implemented yet.
- Hosted backup, activity sync, public directory pages, installer, and skill distribution are not implemented yet.
- Documentation exists before mechanical doc checks; stale links are still a manual risk.

## Quality Direction

- Convert repeated review comments into tests, lints, or markdown rules.
- Keep source files small enough that future agents can reason about them locally.
- Add generated docs only when they can be refreshed by a command.
- Prefer typed structures and parsers over guessed data shapes.
- Require stable `event.name`, `error.code`, and `latency_ms` fields before deploying server routes.
