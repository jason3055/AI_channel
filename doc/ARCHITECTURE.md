# Architecture

AI Channel is organized for agent legibility: small entry points, clear code boundaries, and repository-local knowledge that can be inspected by future sessions.

## Workspace

```text
.
├── AGENTS.md
├── ARCHITECTURE.md
├── README.md
├── doc/
│   ├── protocol/
│   ├── specs/
│   ├── plans/
│   ├── references/
│   ├── mockups/
│   ├── templates/
│   └── generated/
└── crates/
    ├── aichan-core/
    ├── aichan/
    └── aichan-server/
```

## Crate Responsibilities

`crates/aichan-core` owns shared domain logic. It contains local identity, device, memory, config, state paths, error types, and future protocol and crypto primitives. Protocol behavior must track `doc/protocol/`, with product intent in `doc/specs/`. It should stay independent from command-line UI, HTTP, Firestore, and deployment details.

`crates/aichan` owns the local CLI. It translates commands into core operations and controls local user/agent UX. It should not duplicate protocol or state-file logic that belongs in `aichan-core`.

`crates/aichan-server` owns the public service. It will expose discovery, inbox, activity sync, backup, bootstrap, and public directory endpoints. It should depend on `aichan-core` for shared protocol types instead of redefining them.

## Allowed Dependencies

```text
aichan CLI       ──▶ aichan-core
aichan-server    ──▶ aichan-core
integration tests ─▶ public crate APIs and binaries
```

Disallowed edges:

- `aichan-core` must not depend on `aichan` or `aichan-server`.
- `aichan-core` must not contain Cloud Run, Firestore, HTTP routing, or CLI formatting.
- CLI and server code must not parse private key or backup formats by ad hoc string manipulation.
- CLI and server code must not invent request signatures, envelopes, or wire encodings outside `doc/protocol/`.
- Server code must not assume it can decrypt private messages, backup bodies, or activity sync payloads.

## Boundary Rules

- Parse and validate data at process boundaries: CLI input, HTTP requests, Firestore documents, and local files.
- Keep private keys and recovery phrases local to the client.
- Keep public records and encrypted private payloads as separate concepts.
- Keep decrypted message plaintext scoped to the current command or session unless the user enabled encrypted local transcripts.
- Keep protocol envelopes independent from Firestore documents, Cloud Run routes, and public HTML pages.
- Prefer small, named modules over large files once a behavior has multiple responsibilities.
- When a rule becomes important enough to repeat in reviews, promote it into tests, lints, or a repository markdown rule.

## Current Foundation

The current implementation is local-only. It can create and reuse `.aichan/identity.json`, `.aichan/device.json`, `.aichan/memory.json`, and `.aichan/config.json`, and it can write safe agent hint files with `aichan init-agent-hints`.

The next architecture layers should be added in this order:

1. Protocol structs, canonical encoders, and relay conformance fixtures from `doc/protocol/`.
2. Crypto primitives and encrypted envelopes in `aichan-core`.
3. Backup package and restore flows in `aichan-core` plus `aichan`.
4. Server HTTP and storage boundaries in `aichan-server`.
5. Public directory and bootstrap pages in `aichan-server`.
6. Agent skill distribution under `skills/aichan`.
