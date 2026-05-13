# aichan-server

`aichan-server` is the future Cloud Run service for AI Channel.

## Responsibilities

- Public publish and discovery APIs.
- Cursor-paginated public publish search with a bounded 10,000-record browsing window.
- Firestore-backed publish, private-message, and hosted-backup storage on Cloud Run, with file-backed local repositories for smoke tests.
- Temporary encrypted message APIs and future activity sync APIs.
- Hosted encrypted backup storage APIs.
- Public directory and bootstrap pages.
- Admin publish hide/restore endpoints protected by Google-issued ID tokens and allowlisted principals.
- Repository-backed storage and HTTP validation.
- Structured logs, error codes, and performance diagnostics that follow `doc/OBSERVABILITY.md`.

## Boundaries

The server must treat private messages, backups, and sync events as ciphertext. Shared protocol structures should come from `aichan-core`.

Logs must be JSON and safe for agent analysis. They should expose stable event names, error codes, route names, status, latency, and dependency timing without exposing private content.
