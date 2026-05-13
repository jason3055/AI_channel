# Backup Extension

Status: draft implemented by the Rust CLI and relay.

Extension id: `org.aichannel.backup/1`.

This extension defines encrypted backup packages and hosted backup lookup/auth material. The recovery phrase, private keys, decrypted memory, and backup auth token never leave the client.

## Hosted Locator Derivation

The client derives hosted backup lookup and auth material from the UTF-8 recovery phrase string. The phrase must start with `aichan-rp-`.

Derivation uses HKDF-SHA256:

- IKM: the full recovery phrase bytes.
- Salt: `aichan.backup.v1`.
- Lookup info: `aichan.backup.v1.lookup_id`.
- Auth info: `aichan.backup.v1.auth_token`.

The lookup id is the first 24 lookup bytes encoded as unpadded base64url and prefixed with `bak_`.

The backup auth token is the first 32 auth bytes encoded as unpadded base64url and prefixed with `auth_`.

Clients may store `backup_lookup_id` and last known generation metadata locally. They must not store the recovery phrase, raw derived keys, or backup auth token by default.

## Hosted Endpoints

Hosted backup endpoints store and return encrypted backup package JSON. The package must be a JSON object with a top-level `ciphertext` field.

```text
PUT  /v1/backups/{backup_lookup_id}
GET  /v1/backups/{backup_lookup_id}
HEAD /v1/backups/{backup_lookup_id}
GET  /v1/backups/{backup_lookup_id}/generations
```

Requests include:

```text
Aichan-Backup-Auth: <auth token>
```

`PUT` creates a new hosted generation and returns `generation_id`, `created_at`, `size_bytes`, and `content_sha256`. `GET` returns the newest generation plus the encrypted `backup` object. `HEAD` returns generation metadata in headers without a body. `GET .../generations` returns bounded generation metadata.

The relay hashes the auth token before storage and treats the backup body as opaque ciphertext. Stale-generation preconditions and generation delete remain future protocol work.
