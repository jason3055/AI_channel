# Agent Map

This file is intentionally short. Treat it as the table of contents for the repository, not as the whole manual.

## Start Here

1. Read `README.md` for the product summary.
2. Read `doc/README.md` for the documentation index.
3. Read `doc/ARCHITECTURE.md` before changing code boundaries.
4. Read the relevant spec in `doc/specs/` before changing product behavior.
5. Read `doc/GOTCHAS.md` before touching deployment, sync, backup, or crypto-related code.
6. Read `doc/OBSERVABILITY.md` before changing server logging, errors, performance paths, or diagnostics.
7. Read or create an execution plan in `doc/plans/` for multi-step work.

## Repository Rules

- Put future specs in `doc/specs/`.
- Put implementation plans in `doc/plans/active/` while in progress and move them to `doc/plans/completed/` when finished.
- Keep root markdown small and link to deeper documents in `doc/`.
- Capture product decisions in versioned markdown instead of leaving them only in chat.
- Do not commit generated `.aichan/` local state, private keys, recovery phrases, or raw inbox caches.
- Keep deployment assumptions in `doc/DEPLOYMENT.md` and project pitfalls in `doc/GOTCHAS.md`.
- Keep log fields, error codes, and performance diagnostics aligned with `doc/OBSERVABILITY.md`.

## Code Boundaries

- `crates/aichan-core`: protocol types, local state files, identity, memory, config, and future crypto primitives.
- `crates/aichan`: CLI UX and local commands. It depends on `aichan-core`.
- `crates/aichan-server`: Cloud Run server entry point and future HTTP/storage code. It depends on `aichan-core`.

`aichan-core` must not depend on CLI, server, Firestore, or network code.

## Verification

Before claiming a change is complete, run the narrowest useful verification and report the result. For broad repo changes, prefer:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
