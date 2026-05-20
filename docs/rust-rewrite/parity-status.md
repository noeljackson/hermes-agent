# Rust Rewrite Parity Status

This rewrite uses the Python implementation as the behavioral oracle. Python
reference code must run only in Docker via `make python-parity-fixtures` or
`make python-parity-drift`.

The cutover/update state-preservation contract is tracked in
`docs/rust-rewrite/cutover-migration.md`.

The Rust crate ownership map is tracked in
`docs/rust-rewrite/architecture.md`.

Deliberate non-parity decisions are tracked in
`docs/rust-rewrite/non-parity-register.md`.

## Fixture-Backed Rust Behavior

The following surfaces have committed Python fixtures and Rust behavior checked
against those fixtures in `crates/hermes-parity`:

- Auth discovery helpers: env line sanitization, secret metadata, env-file
  discovery, display masking, and selected runtime text redaction for provider
  tokens, auth headers, env/JSON fields, URL query secrets, DB/userinfo URLs,
  form bodies, private keys, Discord mentions, and phone numbers.
- Config helpers: deep merge, root model key normalization, and legacy
  `max_turns` normalization. The Rust loader applies defaults, preserves
  unknown user sections, and supports config-set value coercion plus dotted
  list/object path updates without dropping sibling data or `${ENV}` templates.
  Terminal settings that Python mirrors into `.env` are fixture-backed for the
  sync-key mapping.
- Config defaults: top-level default config section inventory and selected
  high-value defaults for model, toolsets, agent limits/timeouts, terminal
  sandbox settings, browser safety defaults, memory, security, cron, logging,
  sessions, updates, and LSP.
- Install/update: install-method stamp format, stamped-method detection, and
  recommended update command mapping for NixOS, Homebrew, Docker, pip, git,
  and unknown installs.
- Provider request shape: fake chat-completions request without credentials,
  chat-completions stripping of Codex response-field leaks, Responses/Codex
  standard kwargs, xAI Responses cache routing/header merge behavior, and
  Anthropic Messages kwargs for tools, tool choice, and adaptive thinking.
  Shared transport normalization is fixture-backed for tool-call construction,
  backward-compatible tool-call accessors, usage defaults, provider-data
  accessors, and finish-reason mapping.
- Provider profiles: built-in provider inventory, aliases, API modes, auth
  types, environment key discovery metadata, base URLs, fallback counts,
  max-token defaults, fixed-temperature sentinels, and default header keys.
- Core runtime: fake provider-driven tool loop, OpenAI-style assistant/tool
  message appends, final response return, unknown-tool errors, and iteration
  limit stop behavior. Python fixture coverage now protects core agent-loop
  guardrails for duplicate tool-call removal, delegate-task call caps, and
  strict OpenAI-compatible API tool-call sanitization.
- Local memory: add, duplicate, replace, remove, ambiguous-match errors, char
  limit errors, file-backed `MEMORY.md`/`USER.md` persistence, frozen
  system-prompt snapshot behavior across mid-session writes, and selected
  threat-pattern blocking for injection, exfiltration, secret reads, SSH
  backdoors, and invisible Unicode. The memory tool dispatcher is also
  fixture-backed for invalid targets, unknown actions, and missing required
  fields for add/replace/remove.
- Gateway messages: slash command detection, bot mention stripping, argument
  extraction, source serialization, iOS dash normalization, path-like slash
  rejection, narrow DM plaintext restart coercion, session source
  description/round-trip shape, gateway session-key construction, WhatsApp
  identifier normalization in session keys, and shared multi-user session
  detection for groups and threads.
- Gateway platform abstractions: built-in platform inventory, platform parsing,
  default platform config shape, session reset policy defaults, and home-channel
  serialization. Platform adapters themselves are deliberately not ported yet.
- MCP filtering: server/tool name sanitization, include/exclude filtering
  including include precedence, string filters, invalid filter fallback,
  bool-ish resource/prompt utility gates, and capability-gated utility
  registration for resources/prompts. MCP security/config helpers are also
  fixture-backed for safe subprocess environment filtering, credential
  redaction in MCP errors, remote HTTP(S) URL validation, and numeric config
  coercion.
- MCP schema normalization: draft-07 `definitions` to `$defs` rewriting,
  local `$ref` rewriting, nullable-union collapse, object-shape repair,
  dangling `required` pruning, and empty-schema defaults.
- Cron schedules: interval, cron, timestamp parsing, legacy job
  normalization, skill-field compatibility, schedule display fallbacks, paused
  state defaults, script/id name fallbacks, blank-profile normalization,
  deterministic interval/one-shot next-run computation, one-shot grace behavior,
  interval grace-window clamping, and durable JSON job storage load/save
  round-tripping through normalized records.
- Terminal config: local, Docker, Docker cwd mount, SSH, Modal, Daytona,
  Singularity, and Vercel sandbox environment config, including host-cwd
  safety normalization, container resource defaults, persistent-shell boolean
  coercion, Modal mode coercion, and fail-closed errors for malformed timeout,
  container CPU, and Docker JSON environment values.
- Terminal execution: deterministic local foreground command execution for
  success and non-zero exits, invalid command-type rejection, and foreground
  timeout ceiling errors. Container/SSH/Modal execution remains outside this
  deterministic slice.
- Session export/resume: deterministic synthetic session export, SQLite-backed
  session/message persistence, resume message shape, export-all shape, and
  idempotent migration of an older SQLite session/message schema, including
  schema-version bumping, declarative column reconciliation, FTS table creation,
  and preservation of existing rows. Session state operations now also cover
  title sanitization, title uniqueness, title lookup, exact/unique/ambiguous
  prefix resolution, session/message counts, delete behavior, transcript-file
  cleanup, and child-session orphaning instead of cascade deletion. Session DB
  unavailable error formatting is fixture-backed, including custom prefixes and
  WAL-incompatible filesystem hints for NFS/SMB/FUSE cases.
- Session search: selected FTS5 query sanitizer behavior, empty query handling,
  compact SQLite-backed search results, source filters, role filters,
  hyphenated-term matching, snippets, neighboring context shape, and search
  matches sourced from tool names and serialized tool-call arguments.
- Skills: `SKILL.md` frontmatter parsing, nested filesystem discovery,
  slash command slug generation, description fallback from body text,
  unsupported-platform filtering, ignored metadata/archive directories, and
  invalid empty-slug suppression. Skill command resolution treats underscores
  and hyphens interchangeably, and reload diff behavior is fixture-backed for
  added, removed, unchanged, total, and command counts. Skill invocation message
  construction is fixture-backed for raw `SKILL.md` injection, skill-directory
  hints, supporting-file hints, user instruction attachment, and runtime notes.
- Slash commands: full central registry metadata, aliases, CLI/gateway flags,
  config-gated gateway command exposure, gateway-known command projection,
  active-session bypass decisions, gateway help line generation, Telegram menu
  command generation, Slack subcommand routing, and mention/argument-safe
  resolution.
- CLI contract: top-level help exit/stderr behavior, deterministic help
  markers, built-in subcommand inventory, and selected subcommand help markers
  for config, tools, MCP, sessions, cron, and gateway. The selected
  subcommand help contracts are executable through the Rust CLI dispatch layer.
  Safe command execution is also fixture-backed for version markers, config
  path/env-path, config
  writes, and secret `.env` writes with fake credentials, including durable
  file-state checks that config values land in `config.yaml` while secrets land
  in `.env`. The no-jobs `cron list` output is fixture-backed. A narrow
  session-management CLI subset is now executable and fixture-backed for
  `sessions list --limit`, `sessions stats`,
  `sessions export -`, `sessions export - --session-id`, `sessions rename`,
  and `sessions delete --yes`.
- Tool registry: built-in tool metadata for name, toolset, async flag,
  env requirements, schema parameter names, required fields, and description
  presence. Selected core tool schemas (`terminal`, `read_file`, `write_file`,
  `patch`, `search_files`, `memory`, `session_search`, `skill_manage`, and
  `skills_list`) also check semantic schema contracts for required args, types,
  defaults, enum values, and numeric bounds. The full stripped JSON Schema
  payloads are fixture-backed for the deterministic file tools: `read_file`,
  `write_file`, `patch`, and `search_files`.
- File-tool helpers: read/search pagination clamps, protected write-path
  detection, binary/image classification, line-number rendering with long-line
  truncation, and selected fuzzy replace behavior including exact,
  replace-all, duplicate-match errors, empty/identical/not-found errors, and
  Unicode-normalized matching.
- Tool execution: selected pure dispatcher behavior for clarify validation,
  missing clarify callback errors, agent-loop-only tool blocking, unknown-tool
  errors, and shared `tool_error` JSON shape. The direct `memory` tool handler
  path is fixture-backed for add, replace, and missing-remove behavior through
  durable local memory files. The `skills_list` handler is fixture-backed for
  all-skills and category-filtered listings from a controlled local skills
  root. The `skill_view` handler is fixture-backed for main `SKILL.md` loading,
  linked reference/script discovery, and linked reference-file reads.
  Deterministic local file-tool handlers are also fixture-backed for `read_file`,
  `write_file` validation, `write_file`, replace-mode `patch`, and
  `search_files` with `target=files`.
- Toolsets: static toolset names, validation, selected composite/platform
  resolution, multiple-toolset union, and detailed toolset info for core
  deterministic toolsets.

## Fixture-Only Coverage

These fixtures currently protect the observed Python contract, but the Rust
runtime implementation is still intentionally incomplete:

- Full subcommand dispatch beyond selected help and the narrow safe-command
  subset.
- Executable handlers for every slash command and CLI subcommand.
- Executable handlers for every built-in tool beyond the deterministic local
  file-tool slice.
- Full JSON Schema payload parity for every built-in tool beyond file tools and
  selected core semantic schema contracts.
- Full historical SQLite migration coverage for every legacy schema version;
  the current fixture proves one representative old schema that Python still
  opens successfully.

## Explicit Non-Goals For This Stage

- No live model/provider calls.
- No live gateway platform traffic.
- No browser, voice, image, or full autonomous behavior parity.
- No attempt to port every Python tool before the Rust spine is stable.
- No Python runtime dependency in Rust crates.
