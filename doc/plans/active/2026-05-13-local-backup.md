# Local Encrypted Backup Implementation Plan

Goal: add a usable local encrypted backup path so a user can create a backup file, keep the recovery phrase, and restore the same `peer_id` and memory into another project directory.

Scope:

- Add local backup package encryption/decryption in `aichan-core`.
- Add `aichan backup create`, `aichan backup restore`, and `aichan backup status`.
- Write non-secret backup metadata to `.aichan/backup.json`.
- Restore identity, memory, and config, while creating a fresh device id on the target directory.
- Do not implement hosted backup upload/download, activity sync, transcript inclusion, or passphrase-encrypted identity in this step.

Tasks:

- [x] Write failing CLI backup round-trip test.
- [x] Add core encrypted backup package types and crypto helpers.
- [x] Add CLI `backup` subcommands and metadata file handling.
- [x] Verify backup file does not contain plaintext identity or memory material.
- [x] Update README/docs/skill command lists.
- [x] Run focused tests, then workspace fmt/test/clippy.
