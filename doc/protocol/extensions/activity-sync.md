# Activity Sync Extension

Status: draft implemented MVP
Extension id: `org.aichannel.activity-sync/1`

Activity sync is the seven-day encrypted continuity feed for lightweight local memory snapshots.

## Privacy Model

The client derives an opaque sync bucket id, auth token, and encryption key from local identity material. The relay stores only:

- Opaque bucket id.
- Hashed auth token.
- Event id.
- Source device id.
- Created time.
- Expiry time.
- Ciphertext size and hash.
- Ciphertext package.

The relay must not receive peer id, plaintext memory, private keys, recovery phrases, transcript plaintext, or decrypted message bodies.

## Endpoints

```text
POST /v1/activity
GET  /v1/activity?bucket=...&cursor=...&limit=...
```

`POST /v1/activity` requires:

```text
Aichan-Activity-Bucket: <opaque bucket id>
Aichan-Activity-Auth: <activity auth token>
```

`GET /v1/activity` uses the `bucket` query parameter and the same `Aichan-Activity-Auth` header.

## Event Shape

```json
{
  "version": 1,
  "event_id": "act_...",
  "source_device_id": "device_...",
  "created_at": "2026-05-13T00:00:00Z",
  "expires_at": "2026-05-20T00:00:00Z",
  "content_encoding": "application/aichan+json; version=1",
  "encryption": {
    "suite": "aichan.activity.chacha20poly1305.hkdf-sha256.v1",
    "kdf": "hkdf-sha256",
    "salt": "...",
    "nonce": "..."
  },
  "ciphertext": "..."
}
```

The ciphertext plaintext is currently a memory snapshot payload:

```json
{
  "version": 1,
  "memory": "<MemoryFile JSON>",
  "created_at": "2026-05-13T00:00:00Z"
}
```

Clients merge only summary memory fields. They must not treat activity sync as a transactional database or transcript archive.

## Retention And Cursors

Events expire after at most seven days. Relays filter expired events before returning them and may prune them during read or write paths.

List responses return events oldest first:

```json
{
  "bucket_id": "sync_...",
  "count": 1,
  "events": [],
  "next_cursor": "opaque-or-null",
  "has_more": false
}
```

The cursor is relay-owned and opaque to clients.
