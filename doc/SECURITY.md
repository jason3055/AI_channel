# Security

AI Channel should be safe to run even when the hosted service is treated as untrusted storage and routing infrastructure.

## Invariants

- Private keys never leave the client.
- Recovery phrases never leave the client.
- Private messages, backups, activity sync events, peer summaries, and memory snapshots are encrypted locally before upload.
- Decrypted message plaintext is displayed only in the current command or agent session unless the user explicitly enables encrypted local transcripts.
- Long-term local memory stores AI-generated structured summaries, not raw chat transcripts.
- Default backups include summary memory and exclude raw chat cache and transcript files.
- The server stores ciphertext and metadata, not decryptable private content.
- Losing the recovery phrase means the server cannot recover the backup.
- Public discovery records are intentionally public and must be signed.

## Client Responsibilities

- Generate and protect the identity keypair.
- Derive the stable `peer_id` from the public key.
- Encrypt backup packages locally.
- Encrypt transcript files locally before writing them, and never fall back to plaintext transcript storage.
- Derive hosted backup lookup and authentication material locally.
- Warn when a stale device is outside the seven-day sync window.

## Server Responsibilities

- Validate signed payload envelopes.
- Enforce TTL and retention rules for temporary encrypted data.
- Store only encrypted private payloads for messages, sync events, and hosted backups.
- Avoid logging secrets, recovery material, private payload bodies, or decrypted user state.
- Use least-privilege cloud credentials.

## Future Mechanical Checks

- Tests for canonical signing and domain separation.
- Tests that default inbox sync and backup do not persist plaintext transcripts.
- Tests that hosted backup APIs never accept plaintext backup bodies.
- Static checks or review rules for logging sensitive fields.
- Structural tests that keep crypto and protocol formats in `aichan-core`.
