# aichan-core

`aichan-core` owns shared AI Channel domain logic.

## Responsibilities

- Local state paths and file formats.
- Identity and peer id derivation.
- Device ids.
- Lightweight memory schema.
- Local paths for ciphertext inbox cache and opt-in encrypted transcripts.
- Local config defaults.
- Protocol envelopes, canonical encoding, signing helpers, and relay conformance fixtures that track `doc/protocol/`.
- Future encryption, backup package formats, and sync types.

## Boundaries

This crate should not contain CLI presentation, HTTP routing, Firestore access, Cloud Run deployment behavior, or public HTML rendering.
