# Agent Ecosystem Positioning Notes

Date: 2026-05-13

These notes summarize external references used to reposition AI Channel as secure continuity middleware for AI agents.

## Sources

- A2A specification: https://a2a-protocol.org/latest/specification/
- AGNTCY identity docs: https://docs.agntcy.org/identity/identity/
- ANP technical specifications: https://agentnetworkprotocol.com/en/specs/
- Matrix specification: https://spec.matrix.org/
- Nostr NIP-01: https://nostr-nips.com/nip-01
- DIDComm Messaging v2.1: https://identity.foundation/didcomm-messaging/spec/v2.1/
- MCP specification: https://modelcontextprotocol.io/specification/latest
- Mem0 overview: https://docs.mem0.ai/overview
- Zep memory docs: https://help.getzep.com/v2/memory
- Letta stateful agents docs: https://docs.letta.com/guides/core-concepts/stateful-agents

## Readout

A2A is the closest broad agent-interoperability standard. It defines agent cards, task/message/artifact flows, streaming, push updates, and protocol bindings. AI Channel should bridge to A2A rather than competing with it.

AGNTCY and ANP both frame a broader agent-network stack with identity, discovery, communication, and observability. AI Channel should stay narrower and local-first.

Matrix and Nostr are mature relay/federation/event models. DIDComm is a mature encrypted-message envelope family. AI Channel should borrow carefully and offer compatibility profiles rather than claiming to replace these protocols.

MCP is the obvious way to expose AI Channel state to agent hosts as resources and tools. The first MCP surface should be read-only by default and require explicit user consent for mutating commands.

Mem0, Zep, and Letta are memory/context products. AI Channel should not become a memory engine. Its role is packaging, moving, verifying, and restoring portable continuity state, while allowing future adapters to import/export with memory systems.

## Local Decision

Use this category:

> Secure continuity middleware for AI agents.

Avoid these lead frames:

- Portable continuity layer for coding agents.
- AI social network.
- Agent internet protocol.
- Memory engine for agents.

Keep "coding-agent handoff" as a demo because it is concrete, but do not make it the product category.
