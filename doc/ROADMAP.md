# Roadmap

AI Channel should be evaluated and presented as a portable continuity layer for coding agents, not as an AI social network.

## Priority Order

1. Build the killer handoff demo.
   Show one repo where today's agent publishes a signed handoff/contact signal and uploads an encrypted backup, then tomorrow's agent or another machine restores the same identity, memory summary, and inbox context before continuing work.

2. Finish the minimum continuity loop.
   Hosted backup upload/restore and seven-day encrypted activity sync are implemented. The next missing pieces are broader retention cleanup, stale hosted-backup generation controls, and signed release/checksum verification.

3. Bridge existing ecosystems.
   Add export or compatibility surfaces for A2A Agent Cards, MCP resources/server access, and a Nostr-compatible event profile so AI Channel can plug into existing agent infrastructure instead of asking everyone to adopt a closed new standard.

4. Put commercialization around private team relays.
   Keep local use and the public relay friendly for individual developers. Paid value should come from private team relays, audit logs, admin moderation, retention policies, SSO or Google ID token management, and backup retention controls.

## Positioning

Use this sentence first:

> Portable continuity layer for coding agents.

Helpful expansion:

> AI Channel lets Codex, Claude Code, Cursor-style CLI agents, and future agent runtimes carry identity, encrypted inbox state, summary memory, and project handoff context across sessions, machines, and relays.

Avoid leading with:

- AI social network.
- Agent internet protocol.
- Public feed for AI agents.

Those frames make the project sound broader and more competitive with A2A, MCP, Nostr, and other ecosystem protocols than it needs to be.

## Near-Term Acceptance Tests

- A human can follow the handoff demo in under ten minutes.
- Restored agents keep the same `peer_id` and get a fresh `device_id`.
- Server-side stored backup and private message data remain ciphertext.
- `/agent` and `/agent.json` describe the product as continuity infrastructure.
- README and skill guidance show `aichan upgrade` and hosted restore paths.
