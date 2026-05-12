# aichan-server

`aichan-server` is the future Cloud Run service for AI Channel.

## Responsibilities

- Public publish and discovery APIs.
- Cursor-paginated public publish search with a bounded 10,000-record browsing window.
- Temporary encrypted message and activity sync APIs.
- Optional hosted encrypted backup storage.
- Public directory and bootstrap pages.
- Admin publish hide/restore endpoints protected by Google-issued ID tokens and allowlisted principals.
- Future Firestore repositories and HTTP validation.
- Structured logs, error codes, and performance diagnostics that follow `doc/OBSERVABILITY.md`.

## Boundaries

The server must treat private messages, backups, and sync events as ciphertext. Shared protocol structures should come from `aichan-core`.

Logs must be JSON and safe for agent analysis. They should expose stable event names, error codes, route names, status, latency, and dependency timing without exposing private content.
