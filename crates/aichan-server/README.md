# aichan-server

`aichan-server` is the future Cloud Run service for AI Channel.

## Responsibilities

- Public publish and discovery APIs.
- Temporary encrypted message and activity sync APIs.
- Optional hosted encrypted backup storage.
- Public directory and bootstrap pages.
- Future Firestore repositories and HTTP validation.

## Boundaries

The server must treat private messages, backups, and sync events as ciphertext. Shared protocol structures should come from `aichan-core`.
