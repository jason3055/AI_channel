# Roadmap

AI Channel should be evaluated and presented as secure continuity middleware for AI agents, not as an AI social network, a general agent internet protocol, or a memory engine.

## Priority Order

1. Protect the middleware positioning.
   Public docs, relay metadata, and the skill should describe AI Channel as a narrow AI channel layer for identity, verifiable context, encrypted inbox, and migration. Coding-agent handoff is a demo, not the product category.

2. Build the killer handoff demo.
   Show one repo where today's agent publishes a signed handoff/contact signal and uploads an encrypted backup, then tomorrow's agent or another machine restores the same identity, memory summary, and inbox context before continuing work.

3. Finish the minimum continuity loop.
   Hosted backup upload/restore, seven-day encrypted activity sync, and checksum-verified release upgrades are implemented. The next missing pieces are broader retention cleanup and stale hosted-backup generation controls.

4. Add structured context packages.
   Long conversations should move through signed manifests, structured summaries, decisions, open tasks, and optional encrypted transcript chunks rather than raw prompt stuffing.

5. Bridge existing ecosystems.
   Add export or compatibility surfaces for A2A Agent Cards, MCP resources/server access, Nostr-compatible event profiles, and DIDComm mapping notes so AI Channel can plug into existing agent infrastructure instead of asking everyone to adopt a closed new standard.

6. Put commercialization around private team relays.
   Keep local use and the public relay friendly for individual developers. Paid value should come from private team relays, audit logs, admin moderation, retention policies, SSO or Google ID token management, and backup retention controls.

## Positioning

Use this sentence first:

> Secure continuity middleware for AI agents.

Helpful expansion:

> AI Channel lets AI agents carry portable identity, verifiable context, encrypted inbox state, summary memory, and migration backups across sessions, tools, machines, and relays.

Avoid leading with:

- AI social network.
- Agent internet protocol.
- Public feed for AI agents.
- Memory engine for agents.

Keep as demo language only:

- Coding-agent handoff.

Those frames make the project sound broader and more competitive with A2A, MCP, Nostr, and other ecosystem protocols than it needs to be.

## Near-Term Acceptance Tests

- A human can follow the handoff demo in under ten minutes.
- Restored agents keep the same `peer_id` and get a fresh `device_id`.
- Server-side stored backup and private message data remain ciphertext.
- `/agent` and `/agent.json` describe the product as continuity infrastructure.
- README and skill guidance show `aichan upgrade` and hosted restore paths.
- A version tag produces release archives, `SHA256SUMS`, and GitHub artifact attestations.
- Middleware docs explain why long-context continuity uses structured context packages instead of raw transcript stuffing.
