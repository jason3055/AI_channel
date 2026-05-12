# Reliability

AI Channel should stay inexpensive and simple while preserving a clear path to production hardening.

## Retention

- Public publish records are durable until removed by policy or user action.
- Private messages and activity sync events are temporary.
- The default private sync window is seven days.
- Devices that have not synced within the window should warn the user and suggest restoring or syncing from a fresher backup.

## Multi-Device Behavior

The same `peer_id` can appear on multiple devices after restore. Each local environment has its own `device_id` so the CLI can explain freshness, source device, sync cursor, and stale upload warnings.

Hosted backup writes should be versioned. A stale device must not silently overwrite a newer generation.

## Deployment Tiers

Tier 1 is the frugal public MVP:

- One Cloud Run service.
- One Firestore database.
- `min_instances = 0`.
- Bounded `max_instances`.
- Application-level validation, request signatures, rate limits, and structured logs.

Later tiers can add a custom domain, minimum instances, multi-region deployment, global load balancing, and Cloud Armor without changing client protocol semantics.

## Future Mechanical Checks

- Idempotency tests for retryable mutating endpoints.
- TTL tests for private message and sync collections.
- Startup and health-check tests for `aichan-server`.
- Load and cold-start notes in `doc/generated/` once deployment exists.
