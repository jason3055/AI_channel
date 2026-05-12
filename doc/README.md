# AI Channel Documentation

This directory is the repository-local system of record for AI Channel. Keep specs, plans, architecture notes, and project rules here so future agent sessions can recover intent without relying on chat history.

## Index

- `ARCHITECTURE.md`: code map, dependency direction, and boundary rules.
- `DEVELOPMENT.md`: local development commands and working conventions.
- `DEPLOYMENT.md`: Firebase, Firestore, Cloud Run, and production rollout notes.
- `GITHUB_ACTIONS.md`: main-branch verification and gated Cloud Run deployment workflow.
- `GOTCHAS.md`: project-specific pitfalls future agents should check before changing code.
- `OBSERVABILITY.md`: AI-readable logs, error taxonomy, performance events, and diagnostic queries.
- `SECURITY.md`: security model and privacy invariants.
- `RELIABILITY.md`: sync, retention, stale device, and deployment reliability notes.
- `QUALITY.md`: quality gates and code health tracking.
- `specs/`: product and design specs.
- `plans/`: active and completed implementation plans.
- `references/`: distilled notes from external references.
- `templates/`: lightweight templates for future specs and plans.
- `generated/`: generated docs that can be refreshed by tooling.

## Documentation Rules

- Future specs go in `doc/specs/`.
- Future plans go in `doc/plans/active/` first.
- Completed plans move to `doc/plans/completed/`.
- External articles should be summarized in `doc/references/` with a source link and the local decisions they imply.
- Root markdown should stay short and point here.
