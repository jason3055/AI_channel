# Activity Sync P0 Implementation Plan

Goal: implement the first end-to-end `aichan sync` loop for seven-day encrypted memory/activity continuity.

Approach:

- Add a small core activity module that derives an opaque sync bucket and auth token from local identity material, then encrypts/decrypts memory snapshot events locally.
- Add relay endpoints `POST /v1/activity` and `GET /v1/activity?bucket=...&cursor=...` that store only opaque ciphertext, bucket id, event id, device id, creation time, expiry time, and ciphertext size.
- Add CLI `aichan sync` that uploads the local memory snapshot, fetches recent encrypted activity events, skips events from the same device, merges safe summary memory fields, updates cursors, and reports stale-device warnings.
- Keep the version bump constrained to patch level: `0.3.0` -> `0.3.1`.

Non-goals:

- No server-side plaintext memory, transcript, recovery phrase, private key, or peer id access.
- No CRDT merge or full transcript migration.
- No signed binary releases in this P0; current GitHub release list is empty and release work remains a separate follow-up.

Result:

- Implemented core activity locator/encryption/decryption.
- Implemented `aichan sync`.
- Implemented file and Firestore activity stores.
- Implemented `/v1/activity` upload/list endpoints with cursor paging and expired-event filtering.
- Added stale sync-window warnings to `aichan status`.
- Bumped the CLI/core/server/skill version to `0.3.1`.

Verification:

- Added server API tests for activity upload/list auth, ciphertext-only storage, cursor paging, expired-event filtering, and Firestore activity document shape.
- Added CLI tests for `aichan sync` two-device memory continuity and stale-device warnings.
- Ran `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and skill validation.
