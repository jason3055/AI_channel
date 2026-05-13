# AI Channel Middleware Design

Status: accepted
Date: 2026-05-13

## Positioning

AI Channel is secure continuity middleware for AI agents.

The short product frame is:

> AI Channel is an AI channel middleware layer for identity, verifiable context, encrypted inboxes, and migration.

Coding-agent handoff remains the sharpest demo because it is concrete and easy to verify, but it is not the product category. AI Channel should work for any AI agent runtime that needs stable identity and continuity across sessions, machines, tools, and relays.

## Market Boundary

AI Channel should not compete head-on with broad interoperability, communication, or memory platforms.

- A2A focuses on agent interoperability: discovery, task/message exchange, artifacts, streaming, and task lifecycle.
- AGNTCY and ANP aim at broader agent-network infrastructure, including identity, discovery, messaging, and observability.
- Matrix, Nostr, and DIDComm already cover large parts of decentralized messaging, relay/event models, DID-based identity, and encrypted envelopes.
- Mem0, Zep, and Letta focus on memory and context engineering: memory stores, graph memory, retrieval, stateful agents, and managed memory services.

AI Channel's narrow wedge is continuity packaging and transport:

- Stable AI identity that is local-first and relay-independent.
- Verifiable public contact and context records.
- Encrypted inboxes for asynchronous private handoff.
- Encrypted backup and restore for identity, device state, and summary memory.
- Context packages that make long conversations portable without pasting entire transcripts into prompt context.
- Bridge surfaces so existing ecosystems can read or carry AI Channel state.

## Product Pillars

### Identity

An AI Channel identity is a public-key peer identity, not an account. The relay can verify signed records and authenticated inbox reads, but it cannot impersonate the peer or recover private keys.

The identity layer should stay useful even when a user switches agent tools, machines, or relays.

### Verifiable Context

AI Channel should introduce a signed context package format. A context package is not a raw transcript dump. It is a compact, agent-readable continuity bundle containing:

- Summary memory.
- Decisions and open questions.
- Current tasks and handoff intent.
- References to optional encrypted transcript chunks.
- Hashes, timestamps, source ids, and signatures for integrity.

This is the answer to long conversation limits. The product should not rely on putting 60k or 1M tokens directly into the next prompt. Instead, it should preserve a small trusted manifest plus fetchable encrypted chunks that an agent can inspect on demand.

### Encrypted Inbox

The inbox is asynchronous private transport for follow-up messages and handoff envelopes. It is not a long-term plaintext messaging product.

Default behavior:

- Relays store ciphertext only.
- Decrypted message bodies are session-scoped.
- Long-lived state stores structured summaries and ciphertext caches.
- Optional encrypted transcript storage must remain opt-in.

### Backup And Migration

Backups move identity, config, device-aware sync metadata, and structured memory across machines. Hosted backup storage remains opaque ciphertext. Recovery phrases never leave the client.

Migration should produce visible proof:

- Same `peer_id` after restore.
- Fresh `device_id` on a new environment.
- Restored summary memory.
- Encrypted inbox still decryptable.
- Server cannot read private material.

### Bridges

AI Channel should be easy to embed into existing ecosystems instead of asking them to adopt a closed protocol.

Near-term bridge surfaces:

- A2A Agent Card export describing AI Channel contact, inbox, and continuity capabilities.
- MCP server/resources exposing local AI Channel status, inbox summaries, context packages, and backup status with explicit user consent.
- Nostr-compatible event profile for public contact/context records.
- DIDComm mapping notes for encrypted inbox envelopes and key agreement.

These bridges are adapters. The source of truth remains `aichan/1` and its extension specs.

## Non-Goals

- Not an AI social network.
- Not a general agent internet protocol.
- Not a replacement for A2A, MCP, AGNTCY, ANP, Matrix, Nostr, DIDComm, Mem0, Zep, or Letta.
- Not a managed memory engine or graph-memory retrieval system.
- Not a long-term plaintext transcript archive.
- Not a relay that can decrypt private inbox, activity, backup, or context payloads.

## P0 Changes

- Update user-facing positioning from the old coding-agent-only phrase to "secure continuity middleware for AI agents".
- Keep the coding-agent handoff demo, but label it as a demo and not the category.
- Update `/agent` and `/agent.json` so agents installing from the relay learn the middleware framing.
- Update the skill description and triggers so it applies to general AI agents, not only coding agents.
- Update roadmap and quality notes to protect the new positioning.

## P1 Changes

- Define `context.package` protocol extension.
- Add CLI commands for local context packages:
  - `aichan context status`
  - `aichan context add`
  - `aichan context export`
  - `aichan context restore`
- Add encrypted transcript chunk references with hashes and size limits.
- Add an MCP read-only server exposing context and status resources.
- Add A2A Agent Card export.

## Acceptance Criteria

- Public docs describe AI Channel as AI middleware, not coding-agent-only infrastructure.
- `/agent.json` exposes `positioning` and middleware capabilities that another agent can parse.
- The coding-agent demo remains available as one concrete scenario.
- Long-context guidance says to use structured context packages and encrypted chunks, not raw prompt stuffing.
- Future feature work can be evaluated by whether it strengthens identity, verifiable context, encrypted inbox, or migration.
