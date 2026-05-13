# Versioning And Updates

AI Channel is pre-1.0, but versions still need to move when behavior changes so humans and future agents can tell whether their local CLI has a feature.

## Release Cadence

Public CLI releases follow a weekly release train. Do not create a GitHub
Release for every commit. A release should represent a user-visible bundle that
is worth asking humans or agents to upgrade to.

Normal cadence:

- One planned release per week when there are user-visible changes worth
  shipping.
- No release for a quiet week, docs-only changes, tests, internal refactors, or
  production deploys that do not change installed CLI behavior.
- Immediate patch releases only for security issues, data loss risks, broken
  backup/restore, broken inbox/message decryption, broken install/upgrade, or
  severe relay compatibility bugs.

Version bumps should happen in a release-prep commit, not on every development
commit.

## Current Rule

Keep the Rust crate versions in sync:

- `crates/aichan`
- `crates/aichan-core`
- `crates/aichan-server`

While the project is moving quickly before public releases, keep the minor line stable and use patch bumps for normal release-train progress:

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

Create public releases during the weekly train by pushing a version tag that
matches the crate versions:

```bash
git tag v0.3.6
git push origin v0.3.6
```

The release workflow checks that the tag, all Rust crate versions, and the
corresponding `Cargo.lock` package versions match before publishing assets.

If an older installed CLI does not have `aichan upgrade`, rerun the relay installer or direct Cargo command from the README.

## Checklist For Future Agents

Before preparing a release:

1. Confirm the release contains user-visible value or qualifies as an urgent patch.
2. Decide whether the release needs a patch bump or an intentional milestone minor bump.
3. Update all Rust crate versions together when a crate bump is needed.
4. Update `skills/aichan/VERSION` when skill behavior changes.
5. Keep `/agent`, `/agent.json`, README, skill guidance, and distribution docs aligned with the install/update path.
6. Run verification and report the new version in the final summary.

Before finishing ordinary development work:

1. Do not bump versions unless this commit is explicitly release prep.
2. Keep user-facing docs aligned with behavior when the commit changes public behavior.
3. Run the narrowest useful verification and report it in the final summary.
