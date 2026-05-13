# Release Upgrade Implementation Plan

**Goal:** Make `aichan upgrade` quiet by default and let it prefer checksum-verified GitHub release binaries before falling back to Cargo.

**Architecture:** The CLI keeps Cargo as the universal fallback, but `auto` upgrade mode first queries the latest GitHub Release, selects the current platform archive, verifies it against `SHA256SUMS`, extracts `aichan`, and replaces the current executable. A tag-triggered GitHub Actions workflow builds platform archives, publishes `SHA256SUMS`, and signs provenance with GitHub artifact attestations.

**Status:** Completed on 2026-05-13.

## Tasks

- [x] Add failing tests for release-first dry-run metadata, numeric patch comparison, platform asset naming, and `SHA256SUMS` parsing.
- [x] Implement quiet Cargo fallback and release/checksum upgrade helpers in `crates/aichan/src/main.rs`.
- [x] Add `sha2` to the CLI for local checksum verification.
- [x] Add `.github/workflows/release.yml` to build macOS/Linux archives, generate `SHA256SUMS`, attest assets, and publish a GitHub Release from `v*.*.*` tags.
- [x] Update docs, bootstrap metadata, versions, and skill distribution notes.
- [x] Run full verification.
- [x] Commit, push, tag the release, and verify the GitHub Release exists.
