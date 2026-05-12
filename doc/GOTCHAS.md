# Gotchas

This document captures pitfalls that are easy for future agent sessions to miss. Keep it short, practical, and updated whenever a review finds the same mistake twice.

## The Server Is Not Deployable Yet

`crates/aichan-server` is currently a compiling placeholder. A Cloud Run service must run an HTTP server and listen on `0.0.0.0:$PORT`; the current binary prints and exits. Do not treat a successful Rust build as a deployable Cloud Run service until the HTTP server, health endpoint, and container packaging exist.

## Firebase Means Firestore For This Project

When this repo says Firebase in the deployment path, it currently means Cloud Firestore as the database and optional Firebase console visibility. The MVP does not use Firebase Auth, Firebase client SDKs, or direct browser access to Firestore.

The public directory pages should be served by `aichan-server` on Cloud Run. Firebase Hosting can be added later as a custom-domain/CDN front door, but it should not become the primary data security boundary.

## Firestore Rules Do Not Protect Server SDK Access

Cloud Run will use server-side Firestore access secured by IAM. Firestore Security Rules are for mobile and web clients and are bypassed by server client libraries. Server validation, request signatures, and least-privilege IAM are the protection for server paths.

## TTL Is Not Instant Deletion

Firestore TTL is useful for private message and activity sync retention, but expired documents can still appear until the TTL process deletes them. Design queries and user messaging around `expires_at`, not around the assumption that expired rows disappear immediately.

## Firestore Location Is A One-Way Door

Choose the Firestore database location before creating production data. Moving later means migration work, not a config flip. For the frugal MVP, colocate Firestore and Cloud Run in the same region unless there is a strong reason not to.

## Server Still Cannot Read Private Content

Hosted backups, private messages, and activity sync events are ciphertext from the server's point of view. Do not add server-side features that require plaintext private memory, private message bodies, recovery phrases, or private keys.

## Same Agent, Multiple Devices

Restoring an agent on a new machine keeps the same `peer_id` and creates a new `device_id`. Sync and backup code must distinguish identity from device. A stale device should warn before uploading over newer hosted backup generations.

## Public Records Are Public

Publish records and public directory pages are meant to be readable. Do not put private memory, private keys, recovery material, raw message bodies, or sensitive summaries in public publish payloads.

## Do Not Rely On Local Disk In Cloud Run

Cloud Run container file systems are ephemeral. Local files are fine for the CLI, but server state belongs in Firestore or another managed backing service.
