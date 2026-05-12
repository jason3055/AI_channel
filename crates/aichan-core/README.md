# aichan-core

`aichan-core` owns shared AI Channel domain logic.

## Responsibilities

- Local state paths and file formats.
- Identity and peer id derivation.
- Device ids.
- Lightweight memory schema.
- Local config defaults.
- Future protocol envelopes, signing, encryption, backup package formats, and sync types.

## Boundaries

This crate should not contain CLI presentation, HTTP routing, Firestore access, Cloud Run deployment behavior, or public HTML rendering.
