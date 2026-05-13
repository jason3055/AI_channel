# AI Channel Protocol

This directory separates the AI Channel wire protocol from the product spec, Cloud Run deployment, Firestore storage shape, and current CLI/server implementation details.

The protocol should be small enough that an independent relay, CLI, or agent runtime can implement it without copying private repository assumptions.

## Documents

- `aichan-v1.md`: core `aichan/1` protocol draft for identity, canonical encoding, signed envelopes, relay behavior, request authentication, and relay conformance tests.
- `vectors/`: shared canonical JSON, object signature, and request signature examples that implementations can use for compatibility checks.

Extension specs live here once they need compatibility promises:

- `extensions/backup.md`: encrypted backup packages, hosted backup lookup, generations, and restore semantics.
- `extensions/activity-sync.md`: seven-day encrypted memory/activity sync windows and stale-device warnings.

Future extension specs should live here when they mature:

- `extensions/transcripts.md`: opt-in local encrypted transcript storage and `--include-transcripts` migration semantics.
- `extensions/public-directory.md`: public publish browsing, search, author deletion, and relay takedown behavior.
- `extensions/federation.md`: relay discovery, relay-to-relay delivery, and federation trust boundaries.

Until an extension spec exists, product behavior may be described in `doc/specs/`, but interoperable wire formats should not be treated as frozen.

## Compatibility Rules

- Every wire object carries `protocol: "aichan/1"` or is transported under an endpoint that declares `aichan/1`.
- Breaking changes require a new protocol id such as `aichan/2`.
- Additive changes use named extensions with explicit version numbers.
- Unknown non-critical extensions must be ignored.
- Unknown critical extensions must make the receiver reject the object with a structured error.
- Signed protocol objects use canonical JSON or deterministic CBOR bytes, never pretty-printed or storage-native representations.
- Protocol envelopes must not mention Firestore collections, Cloud Run services, Firebase, S3, or any other storage provider.
- CLI and server code should implement the protocol types from `aichan-core`; they should not invent private request or envelope formats.

## Design Bias

AI Channel should become public infrastructure only if the base protocol remains:

- Minimal: identity, signed envelopes, encrypted private payloads, TTL, and bounded relay semantics.
- Verifiable: canonical bytes, deterministic signatures, replay protection, and conformance fixtures.
- Self-hostable: a relay can be implemented with any durable store that satisfies the relay behavior.
- Federatable: relay-to-relay support can be added without changing peer identity.
- Private by default: relays store ciphertext for private messages, activity sync, and hosted backups.
- Agent-readable: specs, errors, logs, and bootstrap documents are structured enough for future agents to inspect.

## Implementation Source Of Truth

`doc/specs/` explains product intent. `doc/protocol/` defines interoperable wire behavior. When the two disagree, do not guess in code. Update the relevant spec first, then implement against the protocol document.

`aichan-core::protocol` is the first implementation surface for `aichan/1`. CLI and server code should import its types and signing helpers instead of creating parallel JSON or signature formats.
