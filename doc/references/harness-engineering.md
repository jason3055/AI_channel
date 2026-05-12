# Harness Engineering Notes

Source: [Harness engineering: leveraging Codex in an agent-first world](https://openai.com/index/harness-engineering/), OpenAI, February 11, 2026.

## Useful Ideas For AI Channel

- Humans steer and agents execute. The repo should encode intent, boundaries, and feedback loops so future sessions can act without chat history.
- A short `AGENTS.md` should work as a map, not a giant manual.
- Repository knowledge should be structured and versioned. Specs, plans, architecture, security, quality, and references should live in markdown near the code.
- Agent legibility is an architecture goal. Anything important but not discoverable in the repo is effectively absent for future agent runs.
- Enforce important architecture and taste rules with tests, linters, or structural checks when they become stable enough.
- Keep technical debt cleanup continuous and small instead of waiting for large manual cleanup passes.

## Local Decisions

- Use `doc/` as AI Channel's durable documentation root.
- Keep future specs in `doc/specs/`.
- Keep active and completed plans in `doc/plans/`.
- Keep root `AGENTS.md` short and focused on navigation.
- Add crate-level README files so code boundaries are discoverable without reading every source file.
- Promote repeated rules into mechanical checks when the project grows past manual review.
