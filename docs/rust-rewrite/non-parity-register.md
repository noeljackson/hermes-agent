# Rust Rewrite Non-Parity Register

Every Rust feature should either match a committed Python parity fixture or
appear here with a deliberate reason for not cloning Python behavior.

## Active Non-Parity Decisions

### No Python Runtime Dependency

Rust must not import, shell out to, or embed Python at runtime. Python is allowed
only inside the Docker reference runner used to generate parity fixtures.

Reason: the rewrite goal is an independent Rust runtime, not a wrapper.

### No Live Providers In Normal Tests

Normal tests use fake providers, local providers, or sanitized request fixtures.
Real provider calls are available only through `make real-provider-smoke` with
explicit opt-in environment variables.

Reason: normal CI and local checks must not require credentials, spend money, or
leak request data.

### No Live Gateway Traffic In Normal Tests

Normal tests use gateway normalization fixtures and do not contact messaging
platforms. Real webhook smoke is available only through `make real-gateway-smoke`
with explicit opt-in environment variables.

Reason: platform auth, rate limits, workspace state, and user-visible messages
are outside deterministic parity tests.

### No Browser, Voice, Or Image Tool Runtime Parity Yet

Browser control, voice, image generation, and rich media tools are not ported in
the current Rust spine.

Reason: they require external services, UI/browser state, or large platform
dependencies. They should be added after config, providers, sessions, tools,
skills, memory, scheduler, MCP, gateway abstractions, and terminal backends have
stable fixture-backed contracts.

### No Full Autonomous Behavior Snapshot Tests

The parity harness avoids full autonomous end-to-end transcripts as the primary
correctness signal.

Reason: provider sampling, tool timing, shell environment, and network state
make full transcripts brittle. The harness instead captures deterministic
contracts for inputs, normalized messages, tool dispatch, storage, schedules,
and request shapes.

### Messaging Platform Adapters Are Not Ported First

Rust currently owns platform-neutral gateway normalization and session-key
contracts, not every Telegram, Slack, Discord, WhatsApp, Matrix, email, or SMS
adapter.

Reason: adapter behavior is high variance and stateful. The rewrite starts with
the stable gateway abstraction so platform ports can be added independently.

### Terminal Remote Backends Are Config-Only At This Stage

Rust currently matches terminal backend selection/config shape and deterministic
local foreground execution. Docker, SSH, Modal, Daytona, Singularity, and Vercel
execution are not yet runtime ports.

Reason: remote execution and sandbox isolation need separate security review.
Config parity prevents accidental user-data or setting drift while the runtime
execution layer is designed.
