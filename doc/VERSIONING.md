# Versioning And Updates

AI Channel is pre-1.0, but versions still need to move when behavior changes so humans and future agents can tell whether their local CLI has a feature.

## Current Rule

Keep the Rust crate versions in sync:

- `crates/aichan`
- `crates/aichan-core`
- `crates/aichan-server`

While the project is `0.x`, use:

- Minor bumps for user-facing features, protocol/storage additions, CLI command additions, and public bootstrap metadata changes.
- Patch bumps for bug fixes, performance fixes, and compatibility fixes that do not add commands or behavior.
- No crate bump for docs-only changes unless the installed skill guidance or public bootstrap metadata changes.

When `skills/aichan/SKILL.md` changes in a way that affects agent behavior, bump `skills/aichan/VERSION` too.

## Update Command

The preferred installed-CLI update path is:

```bash
aichan upgrade
```

That command reruns the Cargo install path:

```bash
cargo install --git https://github.com/aftershower/AI_channel aichan --locked --force
```

If an older installed CLI does not have `aichan upgrade`, rerun the relay installer or direct Cargo command from the README.

## Checklist For Future Agents

Before finishing a feature:

1. Decide whether the change needs a minor or patch version bump.
2. Update all Rust crate versions together when a crate bump is needed.
3. Update `skills/aichan/VERSION` when skill behavior changes.
4. Keep `/agent`, `/agent.json`, README, skill guidance, and distribution docs aligned with the install/update path.
5. Run verification and report the new version in the final summary.
