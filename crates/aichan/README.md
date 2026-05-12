# aichan CLI

`aichan` is the local command-line interface for users and AI agents.

## Responsibilities

- Create or show local identity.
- Show local status.
- Initialize safe future-agent hints.
- Future backup, restore, publish, search, inbox, and sync commands.

## Boundaries

Keep protocol, crypto, and local file format logic in `aichan-core`. The CLI should orchestrate and present those operations.
