# Rust Workspace Architecture

The Rust rewrite is a parity-first spine. Python is the behavioral oracle only
inside Docker fixture generation; Rust crates must not depend on Python at
runtime.

## Crates

- `hermes-cli`: command-line entrypoint and deterministic CLI command subset.
  Owns argument dispatch, profile path resolution, and user-facing exit codes.
- `hermes-config`: config defaults, merge semantics, auth redaction, `.env`
  discovery, install/update helpers, and config-set path mutation.
- `hermes-core`: agent loop coordination, fake-provider execution tests,
  tool-call guardrails, and OpenAI-style message construction.
- `hermes-provider`: provider profiles, fake/local provider abstractions,
  request-shape builders, and normalized transport types.
- `hermes-tools`: tool registry metadata, selected tool schema contracts, and
  deterministic tool dispatcher helpers.
- `hermes-session`: SQLite session storage, export/resume/search, schema
  reconciliation, title handling, and transcript cleanup.
- `hermes-memory`: local memory file storage, memory tool validation, and
  deterministic search/edit semantics.
- `hermes-skills`: skill discovery, `SKILL.md` frontmatter parsing, platform
  gating, command-key resolution, and reload diffs.
- `hermes-cron`: schedule config parsing, legacy job normalization, and
  deterministic next-run computation.
- `hermes-mcp`: MCP server/tool config filtering, schema normalization, safe
  env construction, URL validation, and error redaction.
- `hermes-gateway`: platform-neutral message normalization, session source
  serialization, session-key construction, and gateway config projections.
- `hermes-terminal`: terminal backend config resolution and deterministic local
  foreground command execution.
- `hermes-slash`: shared slash command registry, aliases, gateway projections,
  and help/menu derivations.
- `hermes-parity`: Rust tests that load committed Python parity fixtures and
  compare Rust behavior against them.

## Durable State Ownership

Durable state belongs to the user profile and must not be deleted by runtime
reloads or updates:

- `config.yaml` and `.env`: `hermes-config`, surfaced by `hermes-cli`.
- `state.db` plus WAL/SHM files: `hermes-session`.
- `sessions/` transcript files: `hermes-session`.
- `MEMORY.md` and `USER.md`: `hermes-memory`.
- `skills/` and user skill config: `hermes-skills`.
- cron jobs and schedule state: `hermes-cron`.
- gateway auth/state files: `hermes-gateway`.
- tool history, checkpoints, trajectories, exports, and logs: owning crates
  must preserve these until explicit migration support exists.

## Reloadable State Ownership

Reloadable state may be replaced by update/install flows:

- Rust binaries and shell completions.
- bundled default config templates.
- bundled built-in skill catalog data.
- bundled docs/static assets.
- generated parity fixture artifacts.
- `target/` build output.

Reloadable defaults copied into a user profile become durable user state after
copying and must be preserved on later updates.

## Parity Rule

A Rust behavior is complete only when one of these is true:

1. A Docker-generated Python fixture exists and a Rust test compares against it.
2. A deliberate non-parity note documents why the Python behavior should not be
   cloned.

Normal tests may use fake providers, fake credentials, local files, and
sanitized fixtures. Live providers, live gateways, browser control, voice, and
image flows are opt-in only.
