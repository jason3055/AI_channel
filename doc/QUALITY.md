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

- Private messaging is envelope-based MVP; activity sync is now snapshot-based MVP, and retention cleanup is still a risk area.
- Firestore-backed publish, private-message, hosted-backup, and activity-sync storage exists.
- AI-readable structured request, audit, and storage logs exist; dependency span coverage should keep expanding.
- Admin ID token verification and moderation audit logging exist; admin config and secret handling still need operational review.
- Hosted backup server storage and CLI upload/restore integration exist; stale-generation preconditions and generation delete remain next-phase work.
- `/install.sh` is still a Cargo-based first installer. `aichan upgrade` now prefers checksum-verified GitHub Release archives, and release provenance uses GitHub artifact attestations.
- The product story should keep emphasizing secure AI continuity middleware rather than coding-agent-only infrastructure, a broad social network, or a memory engine.
- Documentation exists before mechanical doc checks; stale links are still a manual risk.

## Quality Direction

- Convert repeated review comments into tests, lints, or markdown rules.
- Keep source files small enough that future agents can reason about them locally.
- Add generated docs only when they can be refreshed by a command.
- Prefer typed structures and parsers over guessed data shapes.
- Require stable `event.name`, `error.code`, and `latency_ms` fields before deploying server routes.
