# Rust Cutover And Migration Contract

The Rust rewrite must treat the Python implementation as the behavioral oracle
until each surface has either fixture-backed parity or an explicit non-parity
decision. Cutover is not a reinstall. It is a runtime swap that preserves the
user's profile data.

## Durable State

These paths are user-owned and must survive install, update, rollback, and
runtime reload:

- `config.yaml`
- `.env`
- `state.db`, `state.db-wal`, and `state.db-shm`
- `sessions/`
- `skills/`
- `plugins/`
- `mcp_servers` and provider settings inside `config.yaml`
- gateway state and platform auth files
- cron schedules and job state
- memory files such as `MEMORY.md` and `USER.md`
- tool history, checkpoints, trajectories, and exports
- logs unless the user explicitly prunes them

Rust installers and updaters must not delete or rewrite these paths by default.
When a format migration is required, write a backup first and use an
idempotent migration that can be safely re-run.

## Reloadable State

These paths are runtime or release-owned and may be replaced during update:

- Rust binaries
- generated shell completions
- bundled default config templates
- bundled built-in skills and optional skill catalog metadata
- bundled docs/static assets
- generated parity fixtures in `tests/fixtures/python-parity/`
- build artifacts under `target/`

Reloadable state must not be mixed with durable state. If a reloadable default
is copied into a profile, later updates must preserve user edits.

## Required Pre-Cutover Checks

1. `make check`
2. `make python-parity-drift`
3. `make tty-smoke` on an operator machine with tmux available.
4. Migration dry-run against a copied real profile directory.
5. Rollback dry-run proving the copied profile can still be opened by the
   Python runtime after Rust has inspected it.
6. Secret redaction audit over generated migration reports.

No normal test may require live provider credentials or platform tokens.

The Rust spine includes a non-mutating migration dry-run report helper that
classifies existing profile paths as durable or reloadable without reading file
contents into the report. Its unit test builds a representative Hermes profile
with config, `.env`, SQLite state files, sessions, skills, plugins, cron jobs,
gateway state, memory files, tool history, checkpoints, trajectories, exports,
logs, and reloadable binaries/completions, then verifies the dry-run plans no
writes or deletes and emits no secret material.

Python-reference fixtures also protect the current profile migration rules:
profile bootstrap directories, clone config and memory files, clone-all runtime
strip files, default-profile infrastructure exclusions, export exclusions,
profile-name normalization/validation, clone-all ignore behavior for default vs
named profiles, portable export ignore behavior, and a concrete synthetic
profile tree spanning config, secrets, sessions, skills, plugins, cron, gateway
state, memory, tool history, checkpoints, trajectories, logs, profile metadata,
distribution metadata, install artifacts, cache files, sockets, temp files, and
runtime PID/state files. The tree fixture preserves the Python distinction that
`--clone-all` keeps live state but strips root runtime process files, default
profile exports drop credentials/runtime/cache/infrastructure, and named
profile exports strip only `.env` and `auth.json`. Rust migration validators
must keep these rules fixture-aligned before any mutating cutover path is
enabled.

## Profile Migration Flow

1. Resolve `HERMES_HOME` using the same profile rules as Python.
2. Acquire a profile lock before reading or writing durable files.
3. Read current config, env, session DB schema, skill inventory, cron jobs,
   gateway state, and memory files.
4. Produce a migration report with planned reads, writes, backups, and skipped
   unsupported items.
5. Create a timestamped backup under a durable backup directory.
6. Apply only idempotent migrations.
7. Re-open migrated state using Rust readers.
8. Emit a redacted report.
9. Leave the Python profile usable unless a user explicitly opts into an
   irreversible future format.

## Rollback Contract

Rollback must restore the backed-up durable files and remove only Rust-owned
reloadable files. It must not remove user-created files that appeared after the
backup. If rollback cannot safely decide ownership, it must leave the file in
place and report it.

## Acceptance Criteria

- Existing config keys are preserved unless a fixture-backed migration maps
  them to a new key.
- Secrets remain in `.env` or platform auth stores, never in migration logs.
- Existing sessions open, export, resume, and search after migration.
- Existing skills keep their command keys and platform-gating behavior.
- Existing schedules retain disabled/enabled state and next-run semantics.
- Existing gateway sessions keep their session-key mapping.
- Existing memory files keep ordering and search-visible content.
- The updater can replace Rust binaries without touching durable profile data.
- Rollback restores the pre-cutover profile snapshot.

## Validation Commands

```bash
make check
make python-parity-drift
make tty-smoke
git diff --check
cargo test -p hermes-config migration_dry_run_classifies_profile_without_touching_user_data
cargo test -p hermes-parity profile_migration_matches_python_fixture
```

Opt-in real-provider or real-gateway smoke tests must live behind explicit
environment flags and must be skipped by default.

## Opt-In Smoke Targets

The smoke targets are disabled unless the matching opt-in flag is set:

```bash
make real-provider-smoke
make real-gateway-smoke
```

Provider smoke requires:

- `HERMES_REAL_PROVIDER_SMOKE=1`
- `HERMES_REAL_PROVIDER_BASE_URL`
- `HERMES_REAL_PROVIDER_MODEL`
- `HERMES_REAL_PROVIDER_API_KEY`

Gateway webhook smoke requires:

- `HERMES_REAL_GATEWAY_SMOKE=1`
- `HERMES_REAL_GATEWAY_WEBHOOK_URL`
- optional `HERMES_REAL_GATEWAY_BEARER_TOKEN`

These targets write responses under `target/` and must not be part of normal
CI. They are for explicit operator-run smoke tests only.
