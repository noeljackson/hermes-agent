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

## Coverage Reporting

Rust code coverage is available as an informational gap-finding tool. It is not
the behavioral oracle and is not a substitute for Python parity fixtures.

- `make coverage` prints a workspace Rust coverage summary.
- `make coverage-html` writes an HTML report to `target/coverage/html`.
- `make coverage-lcov` writes `target/coverage/lcov.info` for CI upload.
- `make tty-smoke` runs a tmux-backed Rust CLI terminal smoke test when `tmux`
  is installed and skips cleanly otherwise.
- `make cutover-check` chains `make check`, Docker parity drift, TTY smoke, and
  the opt-in real-provider/real-gateway smoke gates.

These targets require `cargo-llvm-cov` and fail with an install hint when it is
missing. They do not install tools automatically, do not run host Python, and
are intentionally separate from `make check` and `make python-parity-drift`.

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
  sync-key mapping. The read-only `hermes config show` CLI surface is
  fixture-backed with stable section markers for paths, secret location, API
  key status, model, terminal, compression, and messaging platform sections.
- Config defaults: top-level default config section inventory and selected
  high-value defaults for model, toolsets, agent limits/timeouts, terminal
  sandbox settings, browser safety defaults, memory, security, cron, logging,
  sessions, updates, and LSP.
- Profile migration rules: profile bootstrap directories, clone config files,
  memory subdirectory files, clone-all runtime strip files, default-profile
  infrastructure exclusions, export exclusions, no-bundled-skills marker,
  profile-name normalization/validation, clone-all ignore behavior for default
  vs named profiles, portable export ignore behavior, and concrete synthetic
  profile-tree copy/export policy for durable user state, credentials, profile
  metadata, distribution metadata, install artifacts, caches, sockets, temp
  files, root runtime files, and default-profile infrastructure. The CLI
  profile archive path is fixture-backed for named-profile export/import:
  config, SOUL, memory, and skills are preserved while `.env` and `auth.json`
  are excluded, duplicate import is rejected, and missing-profile export
  returns the Python-compatible error contract. Profile rename is
  fixture-backed for directory move, active-profile marker update, description
  metadata preservation, `SOUL.md` preservation, and no-bundled-skills marker
  preservation.
- Session maintenance: CLI `sessions prune` is fixture-backed for
  `--older-than`, `--source`, and `--yes`, including deletion of only ended
  sessions older than the cutoff, preservation of active/recent sessions,
  source-filter behavior, message row cleanup, and removal of transcript plus
  `request_dump_*` files only for pruned sessions.
- Logs: CLI `logs list`, bounded log tailing, and log filtering by minimum
  level, session substring, and component prefix are fixture-backed with
  synthetic `agent.log`, `errors.log`, and `gateway.log` files.
- Install/update: install-method stamp format, stamped-method detection, and
  recommended update command mapping for NixOS, Homebrew, Docker, pip, git,
  and unknown installs.
- Provider request shape: fake chat-completions request without credentials,
  chat-completions stripping of Codex response-field leaks, Responses/Codex
  standard kwargs, xAI Responses cache routing/header merge behavior, and
  Anthropic Messages kwargs for tools, tool choice, and adaptive thinking.
  Shared transport normalization is fixture-backed for tool-call construction,
  backward-compatible tool-call accessors, usage defaults, provider-data
  accessors, and finish-reason mapping. Provider/model Responses API routing is
  fixture-backed for GPT-5 vendor-prefix stripping, the Nous GPT-5
  chat-completions exception, OpenRouter GPT-5 routing, blank-provider fallback,
  and Copilot non-GPT-5 behavior. Max-token parameter routing is fixture-backed
  for direct OpenAI, Azure OpenAI, GitHub Copilot, OpenRouter, and local
  OpenAI-compatible URLs. Service-tier request overrides and extra-body merge
  precedence are fixture-backed. Codex app-server event projection is
  fixture-backed for ignored streaming deltas, reasoning carry-forward,
  assistant/user messages, command/file-change/MCP/dynamic tool-call
  materialization, tool iteration counters, final-text capture, and opaque
  event fallback. Stream retry diagnostics are fixture-backed for
  exception-chain formatting, selected upstream response-header
  capture/truncation, and fixed-time user/activity retry event text. Selected
  provider recovery classification is fixture-backed for auth, billing,
  transient usage limits, model-not-found, policy blocks, payload/context
  overflow, thinking-signature recovery, llama.cpp grammar retry, rate-limit
  type coercion, remote disconnects, SSL transport retry, and Grok entitlement
  failures. Responses replay/id helpers now cover deterministic call IDs,
  stored `call_id|response_item_id` splitting, `fc_` derivation, chat-message
  to Responses input conversion, xAI encrypted-reasoning stripping, preflight
  input/API-kwargs normalization, validation error strings, and normalized
  response extraction for reasoning/message/tool-call continuity.
- Provider profiles: built-in provider inventory, aliases, API modes, auth
  types, environment key discovery metadata, base URLs, fallback counts,
  max-token defaults, fixed-temperature sentinels, and default header keys.
- Plugin surfaces: valid plugin hook and plugin-kind inventories, entry-point
  group name, controlled manifest parsing, flat and category plugin key
  construction, skip-list behavior, two-level scan depth cap, invalid-kind
  fallback to `standalone`, `plugin.yml` extension support, malformed/non-map
  manifest rejection, explicit `kind: standalone` precedence over source-text
  heuristics, and source-text auto-classification for memory providers
  (`exclusive`) and model providers (`model-provider`). The Rust side records
  manifest metadata only; it does not execute arbitrary Python plugin modules.
  Controlled load-policy fixtures cover disabled-plugin
  precedence, bundled backend/platform auto-enable, bundled standalone opt-in,
  user standalone opt-in, user backend opt-in, hook/command registration
  metadata, project-plugin opt-in precedence over same-key user plugins, and
  the general PluginManager skip of model-provider directories. Provider-style
  plugin registries are fixture-backed for image-generation, web, and browser
  active-provider selection, including get/list semantics, explicit-config
  precedence, availability-filtered fallback, web capability routing, legacy
  preference order, and the browser rule that Firecrawl is not auto-selected.
- Memory-provider plugin boundary: bundled memory-provider directory discovery,
  user memory-provider source-text heuristics, hidden/private/missing-init
  filtering, bundled-over-user collision precedence, `find_provider_dir`
  resolution, and non-memory user plugin exclusion are fixture-backed without
  requiring Rust to import or execute provider plugin code.
- Core runtime: fake provider-driven tool loop, OpenAI-style assistant/tool
  message appends, final response return, unknown-tool errors, and iteration
  limit stop behavior. Python fixture coverage now protects core agent-loop
  guardrails for duplicate tool-call removal, delegate-task call caps, and
  strict OpenAI-compatible API tool-call sanitization. Long-running state
  helpers are fixture-backed for iteration-budget consume/refund/exhaustion,
  pending `/steer` accumulation and drain behavior, and interrupt/clear state
  propagation to execution and worker thread IDs.
- Local memory: add, duplicate, replace, remove, ambiguous-match errors, char
  limit errors, file-backed `MEMORY.md`/`USER.md` persistence, frozen
  system-prompt snapshot behavior across mid-session writes, and selected
  threat-pattern blocking for injection, exfiltration, secret reads, SSH
  backdoors, and invisible Unicode. The memory tool dispatcher is also
  fixture-backed for invalid targets, unknown actions, and missing required
  fields for add/replace/remove.
  Rust-only concurrency coverage verifies shared file-backed memory writes do
  not lose entries under parallel callers.
- Gateway messages: slash command detection, bot mention stripping, argument
  extraction, source serialization, iOS dash normalization, path-like slash
  rejection, narrow DM plaintext restart coercion, session source
  description/round-trip shape, gateway session-key construction, WhatsApp
  identifier normalization in session keys, and shared multi-user session
  detection for groups and threads.
- Gateway platform abstractions: built-in platform inventory, platform parsing,
  default platform config shape, session reset policy defaults, and home-channel
  serialization. Shared adapter helper parity now covers message/processing
  enums, media-as-audio routing, UTF-16 length accounting, safe URL logging,
  Telegram/Feishu thread metadata and reply-anchor behavior, webhook loopback
  validation, API-server port/bool/content normalization, and Slack rich-text
  block extraction. Gateway-adjacent routing/config helpers now also cover
  gateway config bool/int/float/mode normalizers, delivery-target parsing and
  stringification with origin fallback, runtime-footer config/format/build
  behavior, restart drain-timeout parsing, channel-query normalization,
  channel target labels, and session-derived channel-directory IDs/names. Live
  platform adapters themselves remain opt-in/future
  work until deterministic adapter fixtures cover each platform boundary.
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
- Voice/TTS/STT helpers: CLI/TUI voice record-key config lookup,
  prompt-toolkit shortcut normalization, `/voice status` display strings,
  TTS provider selection, command-provider config lookup, command-provider
  timeouts/output formats/voice compatibility, provider-specific text length
  caps, command-template placeholder quoting, markdown stripping for spoken
  text, STT enabled/provider selection with fake local/cloud availability,
  local STT model normalization, and audio-file validation errors. These
  fixtures avoid live audio devices, live providers, and host Python.
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
  container CPU, and Docker JSON environment values. Remote backend support
  has deterministic fixture coverage for Docker env/security argument
  normalization, SSH command construction, file-sync shell quoting and parent
  directory discovery, and Modal direct/legacy snapshot key handling. Shared
  backend command wrapping is fixture-backed for CWD marker names, `cd`
  quoting with `~` preservation, snapshot-aware command wrappers, stdin
  heredocs, Modal stdin heredocs with delimiter collision avoidance, and Modal
  sudo password piping. Terminal safety helpers are fixture-backed for
  compound `&& ... &`/`|| ... &` background rewriting, foreground long-lived
  process guidance, shell-level background warning text, notification flag
  conflict resolution, and sudo password command/stdin transformation.
- Terminal execution: deterministic local foreground command execution for
  success and non-zero exits, invalid command-type rejection, and foreground
  timeout ceiling errors. Container/SSH/Modal execution remains outside this
  deterministic slice.
- Session export/resume: deterministic synthetic session export, SQLite-backed
  session/message persistence, resume message shape, export-all shape, and
  idempotent migration of an older SQLite session/message schema, including
  schema-version bumping, declarative column reconciliation, FTS table creation,
  preservation of existing rows, and v11 FTS reindexing that backfills
  `content`, `tool_name`, and `tool_calls` into both standard and trigram FTS
  tables. Session state operations now also cover
  title sanitization, title uniqueness, title lookup, exact/unique/ambiguous
  prefix resolution, session/message counts, delete behavior, transcript-file
  cleanup, child-session orphaning instead of cascade deletion, and
  end/reopen semantics where the first end reason wins until an explicit
  reopen clears it. Session DB
  unavailable error formatting is fixture-backed, including custom prefixes and
  WAL-incompatible filesystem hints for NFS/SMB/FUSE cases.
  Rust-only concurrency coverage verifies multiple SQLite writers can append to
  the same session without message loss.
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
  and `sessions delete --yes`. A narrow profile-management CLI subset is
  executable and fixture-backed for current-profile display, create with
  `--no-alias --no-skills --description`, describe read/write, show, use,
  list, and delete with active-profile reset. Additional deterministic
  subcommand dispatch is fixture-backed for `config check`, `mcp list`,
  empty `sessions list --limit`, `tools list` marker contracts, and
  non-interactive `tools enable` / `tools disable` config mutation for the
  CLI platform toolset list. Deterministic cron subcommands are executable and
  fixture-backed for create, list, pause, resume, and remove against durable
  `cron/jobs.json` state.
- TUI gateway contract: JSON-RPC method inventory, long-handler routing
  inventory, `_ok`/`_err`/unknown-method frame shapes, request normalization
  errors for malformed requests and params, event frame shape, dashboard PTY
  resize escape parsing, dashboard event-channel validation, loopback host
  allowlist, sidecar `/api/pub` URL construction, public-bind vs loopback
  WebSocket client admission, channel extraction, and reverse-proxy prefix
  normalization. Higher-level TUI helpers are fixture-backed for command
  resolution, headless CLI blocking hints, `/details` completion item shapes,
  and the `/details` slash-completion response envelope. Fake in-memory
  session RPCs are fixture-backed for missing-session errors, terminal resize,
  no-agent usage, history projection, steer validation/no-agent errors, and
  busy prompt rejection. This protects the Ink/Python gateway boundary and
  dashboard PTY bridge contract before live TUI behavior is ported. The Rust
  validation harness also includes an explicit tmux-backed TTY smoke target for
  terminal rendering of deterministic CLI output.
- Tool registry: built-in tool metadata for name, toolset, async flag,
  env requirements, schema parameter names, required fields, and description
  presence. Selected deterministic tool schemas (`terminal`, `read_file`,
  `write_file`, `patch`, `search_files`, `memory`, `session_search`,
  `skill_manage`, `skills_list`, `skill_view`, `todo`, `clarify`,
  `browser_navigate`, `web_search`, `web_extract`, `image_generate`, and
  `text_to_speech`) check full stripped JSON Schema payload parity plus
  semantic schema contracts for required args, types, defaults, enum values,
  and numeric bounds.
- File-tool helpers: read/search pagination clamps, protected write-path
  detection, binary/image classification, line-number rendering with long-line
  truncation, and selected fuzzy replace behavior including exact,
  replace-all, duplicate-match errors, empty/identical/not-found errors, and
  Unicode-normalized matching.
- Tool execution: selected pure dispatcher behavior for clarify validation,
  missing clarify callback errors, agent-loop-only tool blocking, unknown-tool
  errors, and shared `tool_error` JSON shape. The `todo` handler is
  fixture-backed for replace, merge, read, duplicate-id, and status
  normalization behavior. The direct `memory` tool handler path is
  fixture-backed for add, replace, and missing-remove behavior through durable
  local memory files. The `skills_list` handler is fixture-backed for all-skills
  and category-filtered listings from a controlled local skills root. The
  `skill_view` handler is fixture-backed for main `SKILL.md` loading, linked
  reference/script discovery, and linked reference-file reads. Deterministic
  browser/web/image wrapper behavior is fixture-backed for FAL payload
  translation, empty image prompt errors, web search limit coercion, web extract
  secret/SSRF/search-only/fake-provider envelopes, browser navigation
  secret/metadata/private URL blocks, and website-policy block shape.
  Deterministic local file-tool handlers are also fixture-backed for `read_file`,
  `write_file` validation, `write_file`, replace-mode `patch`, and
  `search_files` with `target=files`.
- Toolsets: static toolset names, validation, selected composite/platform
  resolution, multiple-toolset union, and detailed toolset info for core
  deterministic toolsets.

## Fixture-Only Coverage

These fixtures currently protect the observed Python contract, but the Rust
runtime implementation is still intentionally incomplete:

- Full subcommand dispatch beyond selected help and the narrow deterministic
  safe-command subset.
- Executable handlers for every slash command and CLI subcommand.
- Executable handlers for every built-in tool beyond the deterministic local
  file-tool slice.
- Full JSON Schema payload parity for every built-in tool beyond the selected
  deterministic schema set.
- Full historical SQLite migration coverage for every legacy schema version;
  the current fixtures prove a representative old row schema and the v11 FTS
  reindex path that Python still opens successfully.

## Explicit Non-Goals For This Stage

- No live model/provider calls.
- No live gateway platform traffic.
- No browser, voice, image, or full autonomous behavior parity.
- No attempt to port every Python tool before the Rust spine is stable.
- No Python runtime dependency in Rust crates.
