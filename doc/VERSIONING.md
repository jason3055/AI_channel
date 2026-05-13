# Versioning And Updates

AI Channel is pre-1.0, but versions still need to move when behavior changes so humans and future agents can tell whether their local CLI has a feature.

## Current Rule

Keep the Rust crate versions in sync:

- `crates/aichan`
- `crates/aichan-core`
- `crates/aichan-server`

While the project is moving quickly before public releases, keep the minor line stable and use patch bumps for normal forward progress:

- Patch bumps for user-facing features, protocol/storage additions, CLI command additions, bootstrap metadata changes, bug fixes, performance fixes, and compatibility fixes. Example: `0.3.0` -> `0.3.1`, `0.3.10` -> `0.3.11`.
- Minor bumps only for an intentional milestone or compatibility boundary that humans should treat as a named line of work.
- No crate bump for docs-only changes unless the installed skill guidance or public bootstrap metadata changes.

When `skills/aichan/SKILL.md` changes in a way that affects agent behavior, bump `skills/aichan/VERSION` too.

## Update Command

The preferred installed-CLI update path is:

```bash
aichan upgrade
```

That command prefers the latest GitHub Release for the current platform. It downloads the matching `aichan-<version>-<target>.tar.gz`, verifies it against the release `SHA256SUMS`, rejects unsafe archive paths, and replaces the current executable. Release artifacts are built by `.github/workflows/release.yml` and carry GitHub artifact attestations; current CLI upgrades do not verify provenance automatically, so manual provenance checks should use `gh attestation verify`.

If no release exists yet, no matching platform archive exists, or the user requests a branch/revision install, `aichan upgrade` falls back to the Cargo install path:

```bash
cargo install --git https://github.com/aftershower/AI_channel aichan --locked --force
```

Create public releases by pushing a version tag that matches the crate versions:

```bash
git tag v0.3.5
git push origin v0.3.5
```

The release workflow checks that the tag, all Rust crate versions, and the
corresponding `Cargo.lock` package versions match before publishing assets.

If an older installed CLI does not have `aichan upgrade`, rerun the relay installer or direct Cargo command from the README.

## Checklist For Future Agents

Before finishing a feature:

1. Decide whether the change needs a patch bump or an intentional milestone minor bump.
2. Update all Rust crate versions together when a crate bump is needed.
3. Update `skills/aichan/VERSION` when skill behavior changes.
4. Keep `/agent`, `/agent.json`, README, skill guidance, and distribution docs aligned with the install/update path.
5. Run verification and report the new version in the final summary.
