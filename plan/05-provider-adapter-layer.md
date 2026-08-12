# 05 — Provider Adapter Layer

**Status:** Draft (planning phase)
**Author:** subagent fan-out (05)
**Consistency:** This document is consistent with `docs/PLAN-CONTEXT.md`. Where a decision is still open there, it is flagged here under **Open questions** and not decided unilaterally.

**Locked decisions applied:** D12 (4-way approval enum), D13 (crate name `multiplexer-provider`), D14 (OpenRouter/DeepSeek = config variant of in-process Grok), D16 (canonical event vocabulary), D17 (ACP adapter generic), D18 (bounded channel with backpressure), D19 (unified session-start params). These are LOCKED per `docs/DECISIONS.md` and supersede any "open question" wording below.

---

## 1. Purpose and scope

Multiplexer is a **control surface for the Grok Build harness, extensible to other models and harnesses**. The provider-adapter layer is the seam that makes "extensible" real: it is the single, canonical boundary between the Multiplexer runtime (orchestration engine, UI, wire contract, checkpointing) and whatever agent backend is actually executing a turn.

The layer has three jobs:

1. **Abstract the agent backend** behind one Rust trait (`ProviderAdapter`) so the rest of the system never cares whether a turn is being executed by the embedded `xai-grok-shell` runtime, a remote `grok agent stdio`/`serve` process over ACP, or (future) a Claude/Codex/OpenCode backend. OpenRouter/DeepSeek (`ds-flash`) is a **config variant of the in-process Grok adapter** (D14), not a separate backend.
2. **Normalize the event stream** — every backend emits its own flavor of "tool call started", "permission requested", "text delta", "turn finished". The adapter translates those into one canonical `ProviderEvent` enum that the orchestration engine projects into the SQLite read model.
3. **Own the model registry** — which models exist, how they authenticate, and which adapter + config each model routes to.

This is the "bring over other models" seam called out in `PLAN-CONTEXT.md` (differentiator #6, architecture bullet "Provider Adapter contract").

### Relationship to other plan docs

| Doc | Relationship |
|-----|--------------|
| `02-architecture.md` | This layer is the "Provider Adapter contract" bullet; the orchestration engine consumes `ProviderEvent`s and issues commands through `ProviderAdapter`. |
| `03-vendored-grok-build.md` | Defines how `xai-grok-shell`, `xai-grok-tools`, `xai-grok-workspace` are vendored and embedded; the Grok adapter is the in-process consumer of those crates. |
| `04-wire-contract.md` | The JSON-RPC-over-WebSocket contract exposes the same command/event vocabulary to thin clients; the adapter is the server-side implementation of that vocabulary. |
| `06-orchestration-engine.md` | The serialized command queue + parallel scheduler drive `ProviderAdapter`; the projector consumes `ProviderEvent`. |
| `17-security-and-secrets.md` | Auth providers, keychain, and `op://` references live here in the registry. |

---

## 2. The `ProviderAdapter` contract

The trait is the heart of the layer. It is deliberately small and command-shaped: every method is a **command** the runtime issues, and every result is delivered asynchronously through the `ProviderEvent` stream (not as a return value). This keeps the adapter non-blocking and lets the orchestration engine treat the backend as a black box.

```rust
// crates/multiplexer-provider/src/adapter.rs

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;

/// A provider-specific session handle. One per active agent thread/session.
pub struct ProviderSession {
    pub id: SessionId,
    pub model: ModelId,
    pub adapter: Arc<dyn ProviderAdapter>,
    /// Canonical event stream for this session. Bounded with backpressure
    /// (D18) — see "Stream semantics" below.
    pub events: mpsc::Receiver<ProviderEvent>,
}

/// The canonical provider contract. Implementations are Send + Sync and
/// must be cheaply clonable (Arc-backed) so the orchestration engine can
/// hold handles to many sessions at once.
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// Adapter identity, used by the registry and for diagnostics.
    fn kind(&self) -> ProviderKind;

    /// Start a new agent session with the unified session-start params
    /// (D19): `{provider, model, workspace, initial_prompt, resume, config}`.
    /// Returns a session handle whose `events` stream begins immediately
    /// (session_ready is the first event).
    async fn start_session(
        &self,
        params: SessionStartParams,
    ) -> Result<ProviderSession, ProviderError>;

    /// Send a user turn (a new prompt, or a reply to an in-flight
    /// user_input request) into the session. Non-blocking; completion is
    /// reported via ProviderEvent::TurnFinished / TurnFailed.
    async fn send_turn(
        &self,
        session: &ProviderSession,
        turn: TurnInput,
    ) -> Result<(), ProviderError>;

    /// Request the backend to stop the current turn as soon as it can.
    /// The backend may take a moment; the runtime treats
    /// ProviderEvent::TurnInterrupted as the authoritative completion.
    async fn interrupt_turn(
        &self,
        session: &ProviderSession,
    ) -> Result<(), ProviderError>;

    /// Respond to a pending permission/approval request
    /// (ProviderEvent::PermissionRequested). `decision` is the 4-way
    /// approval enum (D12); `reason` is surfaced to the agent.
    async fn approval_respond(
        &self,
        session: &ProviderSession,
        request_id: RequestId,
        decision: ApprovalDecision,
        reason: Option<String>,
    ) -> Result<(), ProviderError>;

    /// Respond to a pending user-input elicitation
    /// (ProviderEvent::UserInputRequested). `input` is the user's answer.
    async fn user_input_respond(
        &self,
        session: &ProviderSession,
        request_id: RequestId,
        input: String,
    ) -> Result<(), ProviderError>;

    /// Revert the session to a prior checkpoint (hidden git ref) and
    /// continue from there. `checkpoint` identifies the ref; `reason`
    /// is recorded in the event log.
    async fn checkpoint_revert(
        &self,
        session: &ProviderSession,
        checkpoint: CheckpointRef,
        reason: Option<String>,
    ) -> Result<(), ProviderError>;

    /// Stop the session entirely and release backend resources.
    /// Idempotent: calling twice is a no-op. Emits ProviderEvent::SessionStopped.
    async fn session_stop(
        &self,
        session: &ProviderSession,
    ) -> Result<(), ProviderError>;
}
```

### Design notes

- **Command-shaped, event-delivered.** Every method returns `Result<(), ProviderError>` — the *acceptance* of the command — while the *outcome* arrives on the event stream. This is what lets the orchestration engine serialize commands per thread without blocking on the backend.
- **`Send + Sync + Arc`.** The engine holds dozens of concurrent sessions (subagent fan-out target: "dozens of concurrent subagents"). Adapters must be shareable across tasks.
- **`ProviderKind`** distinguishes `InProcessGrok`, `AcpGrok`, and future `Claude`, `Codex`, `OpenCode` — used for routing, capability flags, and diagnostics. OpenRouter/DeepSeek (`ds-flash`) is **not** a distinct kind: it is a config variant of `InProcessGrok` (D14).
- **Errors are typed.** `ProviderError` distinguishes backend-not-available, auth-failure, session-not-found, timeout, and protocol errors so the engine can decide whether to retry, surface to the user, or fail the thread.

### Supporting types

```rust
pub enum ProviderKind { InProcessGrok, AcpGrok, Claude, Codex, OpenCode }

/// The 4-way approval decision (D12). Carried by `approval_respond` and the
/// wire contract (plan/04); `allow_once`/`allow_always` are real permission
/// modes, not a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Allow,        // grant this one request
    Deny,         // refuse this request
    AllowOnce,    // grant this request only (do not remember)
    AllowAlways,  // grant and remember for this tool/scope
}

/// Unified session-start params (D19), shared with plan/04's `session.start`.
pub struct SessionStartParams {
    pub provider: ProviderId,        // which adapter/backend
    pub model: ModelId,              // which model config
    pub workspace: WorkspaceId,      // which workspace
    pub initial_prompt: Option<Prompt>,
    pub resume: Option<ResumePoint>,
    pub config: SessionConfig,       // per-session overrides (window, caps, etc.)
}

pub struct TurnInput {
    pub prompt: Prompt,          // text + optional attachments (file refs, images)
    pub parent_turn: Option<TurnId>, // for subagent lineage
}

pub struct ResumePoint {
    pub session_id: SessionId,   // backend session id (e.g. grok sessionId)
    pub resume_cursor: Option<String>, // backend cursor for continuing
}

pub struct CheckpointRef {
    pub ref_name: String,        // hidden git ref, e.g. refs/multiplexer/ckpt/<turn>
    pub commit: String,
}
```

---

## 3. The canonical `ProviderEvent` stream

Every backend event is normalized into this enum. The orchestration engine's projector consumes it; the wire contract (`04`) serializes it for thin clients; the UI renders it. **Nothing outside the adapter layer ever sees a backend-specific event.**

```rust
// crates/multiplexer-provider/src/event.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderEvent {
    /// Session established; the stream is now live.
    SessionReady {
        session_id: SessionId,
        backend_session_id: String,
        model: ModelId,
    },

    /// A turn has begun (a new prompt is being processed).
    TurnStarted { turn_id: TurnId, parent_turn: Option<TurnId> },

    /// Incremental text delta from the agent (streamed to the UI).
    TextDelta { turn_id: TurnId, delta: String },

    /// A tool call was initiated by the agent.
    ToolCallStarted {
        turn_id: TurnId,
        call_id: CallId,
        tool: ToolName,
        args: serde_json::Value,
    },

    /// A tool call completed with a result.
    ToolCallFinished {
        turn_id: TurnId,
        call_id: CallId,
        result: ToolResult,
    },

    /// The agent is requesting permission to perform an action
    /// (file write, shell command, network, git push, etc.).
    PermissionRequested {
        turn_id: TurnId,
        request_id: RequestId,
        tool: ToolName,
        detail: serde_json::Value,
    },

    /// The agent is asking the user a question and will block until
    /// user_input_respond is called.
    UserInputRequested {
        turn_id: TurnId,
        request_id: RequestId,
        prompt: String,
    },

    /// A checkpoint was created for the current turn (hidden git ref).
    CheckpointCreated {
        turn_id: TurnId,
        checkpoint: CheckpointRef,
    },

    /// The turn completed successfully.
    TurnFinished { turn_id: TurnId, summary: Option<String> },

    /// The turn was interrupted (by interrupt_turn or a backend stop).
    TurnInterrupted { turn_id: TurnId, reason: Option<String> },

    /// The turn failed with an error.
    TurnFailed { turn_id: TurnId, error: ProviderError },

    /// The session was stopped and its resources released.
    SessionStopped { session_id: SessionId },

    /// Backend-level diagnostic/status (usage, cost, model info).
    Status { session_id: SessionId, detail: serde_json::Value },
}
```

### Canonical vocabulary (D16)

This enum is the **single canonical event vocabulary** for the whole system. The names `TurnFinished`, `ToolCallFinished`, `PermissionRequested`, and `TextDelta` are the standardized set (D16). **plan/06 must use these same names** for its engine events (it must stop using `TurnCompleted`/`ToolCallCompleted`/`ApprovalRequested`/`MessageAppended`), or provide an explicit mapping table to them.

### Stream semantics

- **Ordering is meaningful** within a session: `TurnStarted` → `TextDelta*`/`ToolCall*` → `TurnFinished|TurnInterrupted|TurnFailed`. The projector relies on this to build the read model.
- **One stream per session.** The engine fans out by holding one `ProviderSession` per thread; cross-thread/subagent orchestration lives in `06`, not here.
- **Backpressure (D18).** The stream is a **bounded** `mpsc::Receiver` at the adapter boundary, consistent with plan/04's window-based flow control. The **provider-ingestion worker (plan/06) is the bounding point**: it drains the bounded channel into the SQLite read model in the same transaction as the command that produced it (event-sourced, per `PLAN-CONTEXT`). If the consumer falls behind, the channel fills and the adapter's producer awaits — it never grows without bound. The channel capacity is a tunable constant shared with plan/04's flow-control window.

---

## 4. The Grok adapter (primary, in-process)

This is the differentiator: **no shelling out, no ACP overhead** — we call the embedded `xai-grok-shell` agent runtime directly in-process. This is the "nobody else does this" advantage from `PLAN-CONTEXT` differentiator #1.

### 4.1 Wiring

`03-vendored-grok-build.md` vendors the crates under `third_party/` (with `[patch]`). The Grok adapter is the thin translation layer between those crates and our `ProviderAdapter`/`ProviderEvent`:

```rust
// crates/multiplexer-provider/src/grok/in_process.rs

pub struct InProcessGrokAdapter {
    /// Embedded agent runtime (xai-grok-shell), configured per model.
    runtime: Arc<GrokRuntime>,
    /// Tool registry (xai-grok-tools) — file, shell, git, search, etc.
    tools: Arc<ToolRegistry>,
    /// Workspace/VCS/checkpoint services (xai-grok-workspace).
    workspace: Arc<WorkspaceService>,
}
```

### 4.2 Mapping harness events → canonical events

The embedded runtime emits its own event types (from `xai-grok-shell`). The adapter subscribes and maps them:

| Embedded runtime event | Canonical `ProviderEvent` |
|------------------------|---------------------------|
| session created / `sessionId` | `SessionReady` |
| turn started | `TurnStarted` |
| streaming text chunk | `TextDelta` |
| tool invocation begin | `ToolCallStarted` |
| tool invocation result | `ToolCallFinished` |
| permission/approval request | `PermissionRequested` |
| user-input elicitation | `UserInputRequested` |
| checkpoint created (hidden ref) | `CheckpointCreated` |
| turn complete | `TurnFinished` |
| turn aborted / interrupted | `TurnInterrupted` |
| turn error | `TurnFailed` |
| session closed | `SessionStopped` |

The mapping is a pure function over the embedded event stream — no side effects — which makes it trivially unit-testable and keeps the adapter a thin, auditable seam.

### 4.3 Command mapping

- `start_session` → construct a `GrokRuntime` session bound to the model's config + workspace; return a `ProviderSession` whose event task runs the mapping loop.
- `send_turn` → push the prompt into the runtime's turn queue.
- `interrupt_turn` → call the runtime's abort/stop for the current turn.
- `approval_respond` (with the 4-way `ApprovalDecision`, D12) / `user_input_respond` → resolve the pending request handle in the runtime.
- `checkpoint_revert` → ask `xai-grok-workspace` to reset the worktree to the hidden ref and resume.
- `session_stop` → drop the runtime session and release resources.

### 4.4 Why in-process matters

- **Latency:** no process spawn, no JSON-RPC serialization per event — directly relevant to the `< 16ms` input-latency and subagent fan-out targets.
- **Shared state:** the embedded runtime shares the same workspace/VCS/checkpoint services, so checkpointing and diff queries are native, not reconstructed over a protocol.
- **The seam is preserved:** even though Grok is in-process, it still goes through `ProviderAdapter`. That is what makes the multi-provider story (section 6) a config change, not a rewrite.

---

## 5. The ACP adapter (generic machinery; Grok is one instance)

ACP (Agent Client Protocol, JSON-RPC over stdio or WebSocket) is **generic multi-provider machinery** (D17): it drives any agent that speaks ACP. `AcpGrokAdapter` is a **Grok-specific instance** of that generic adapter — used by Grok-over-ACP (fallback) and by future Claude/Codex/OpenCode. It is not a separate concept.

When we **cannot** embed — driving the installed `grok` binary, headless/remote, or a version mismatch — we fall back to ACP. This mirrors T3 Code's approach (`AcpSessionRuntime` + `XAiAcpExtension`) but is our *fallback*, not our primary path.

### 5.1 When it is used

- **Remote/relay mode** (`14-remote-and-relay.md`): the server runtime on a remote host drives a locally-installed `grok` binary.
- **Headless / CI** where embedding is undesirable.
- **Fallback** if in-process embedding hits an unsupported platform or a broken vendored build.
- **Testing** the ACP path itself (contract tests, section 8).
- **Future providers:** Claude/Codex/OpenCode reuse the same generic ACP machinery (section 7).

### 5.2 Generic ACP machinery + the Grok instance

```rust
// crates/multiplexer-provider/src/acp/mod.rs  — generic ACP machinery

pub struct AcpAdapter {
    /// Spawned `agent stdio` (or connected `serve` WebSocket) child.
    client: AcpClient,
    /// Provider-specific ACP extensions (e.g. xAI's x.ai/fs/*, x.ai/git/*,
    /// x.ai/terminal/*, x.ai/search/*, x.ai/session/*, x.ai/auth/*).
    extensions: AcpExtensions,
}

// crates/multiplexer-provider/src/acp/grok.rs — Grok-specific instance (D17)

pub struct AcpGrokAdapter {
    inner: AcpAdapter,
    /// xAI ACP extensions for the Grok backend.
    extensions: XAiAcpExtension,
}
```

- **Transport:** spawn `grok agent stdio` (JSON-RPC over stdio) for local/headless; connect to `grok agent serve` over WebSocket for remote.
- **Session:** use ACP session lifecycle + `x.ai/session/*` extension for `sessionId`/`resumeCursor`.
- **Events:** ACP `text_delta`, `tool_call`, `permission_request`, `user_input_request`, `session/updated` map onto the same canonical `ProviderEvent` table as section 4.2.
- **Checkpoints:** `x.ai/git/*` + `x.ai/session/*` expose the hidden-ref checkpoint model so `checkpoint_revert` works over ACP too.
- **Generic by design (D17):** the core `AcpAdapter` is provider-agnostic; provider specifics (extensions, event mapping) are injected per backend. Future Claude/Codex/OpenCode adapters reuse `AcpAdapter` with their own extensions.

### 5.3 Cost/benefit vs in-process

| | In-process (primary) | ACP (fallback) |
|---|---|---|
| Latency | Lowest (no IPC) | Higher (JSON-RPC over stdio/socket) |
| Setup | Vendored crates, Windows build support | Installed `grok` binary |
| Remote | Not directly | Native (`serve` over WebSocket) |
| Fidelity | Full (shared workspace/VCS) | Protocol-bounded |
| When | Default desktop | Remote/headless/fallback |

---

## 6. Model registry

The registry manages `[model.*]` and `[auth_provider.*]` config and routes each model to the right adapter. This is the user-facing "add a model" surface and the per-thread model selector.

### 6.1 Config shape (mirrors grok-build `config.toml`)

```toml
# ~/.multiplexer/config.toml
[auth_provider.openrouter]
type = "env"                 # read key from env / session file
env_key = "OPENROUTER_API_KEY"
# or: secret_ref = "op://Vault/Item/field"  (never a raw value)

[model.grok]
name = "Grok (in-process)"
base_url = "in-process"      # sentinel: use embedded xai-grok-shell
api_backend = "grok-inprocess"
adapter = "in-process-grok"
context_window = 131072

[model.ds-flash]
name = "DeepSeek V4 Flash (OpenRouter)"
base_url = "https://openrouter.ai/api/v1"
api_backend = "openrouter"
api_key = "env:OPENROUTER_API_KEY"   # or auth_provider = "openrouter"
auth_provider = "openrouter"
adapter = "in-process-grok"  # same embedded runtime, different model config
context_window = 65536

[model.claude]
name = "Claude Sonnet"
base_url = "https://api.anthropic.com/v1"
api_backend = "anthropic"
auth_provider = "anthropic"
adapter = "claude"           # future adapter (section 7)
context_window = 200000
```

### 6.2 Registry API

```rust
// crates/multiplexer-provider/src/registry.rs

pub struct ModelRegistry {
    models: HashMap<ModelId, ModelConfig>,
    auth: HashMap<AuthProviderId, AuthProviderConfig>,
    adapters: HashMap<AdapterId, Arc<dyn ProviderAdapter>>,
}

impl ModelRegistry {
    pub fn load(config: &Config) -> Result<Self, RegistryError>;
    pub fn resolve(&self, model: ModelId) -> Result<ResolvedModel, RegistryError>;
    pub fn list_models(&self) -> Vec<ModelSummary>;
    pub fn add_model(&mut self, cfg: ModelConfig) -> Result<(), RegistryError>;
    pub fn remove_model(&mut self, id: ModelId) -> Result<(), RegistryError>;
    pub fn adapter_for(&self, model: ModelId) -> Arc<dyn ProviderAdapter>;
}
```

### 6.3 UI for adding a model

- **"Add model" dialog** (GPUI component): name, base_url, api_backend (dropdown: grok-inprocess / openrouter / anthropic / custom), api_key (env var name or `op://` ref — **never a raw value**, per `17-security-and-secrets.md`), auth_provider, context_window, adapter.
- **Validation** on save: reachable base_url, key present (via auth provider), adapter exists.
- **Per-thread selector:** each thread's header shows the active model; switching mid-thread is allowed only between turns (never mid-turn) and is recorded as a `Status` event.

### 6.4 The user's existing `ds-flash` setup

The user already runs `ds-flash` = **DeepSeek V4 Flash via OpenRouter** (see `PLAN-CONTEXT` key facts and the global `flash-delegation` rule). Multiplexer must import that existing `config.toml` `[model.ds-flash]` + `[auth_provider.openrouter]` on first run so the user's working setup carries over. The registry's config loader reads the same `config.toml` format grok-build uses, so this is a direct import, not a migration.

---

## 7. Other providers (future)

Claude, Codex, and OpenCode plug in through the **same** `ProviderAdapter` trait. No changes to the orchestration engine, wire contract, or UI are required — only a new adapter implementation and a registry entry.

**OpenRouter/DeepSeek is NOT here (D14).** `ds-flash` is a **config variant of the in-process Grok adapter** — same embedded `xai-grok-shell` runtime, different `[model.ds-flash]` + `[auth_provider.openrouter]` config. Routing it through the in-process Grok adapter gives it the full harness tool loop for free. It is not a separate adapter crate and not a future HTTP adapter.

| Provider | Adapter | Transport | Notes |
|----------|---------|-----------|-------|
| Claude | `ClaudeAdapter` | Anthropic Messages API + tool use | Tool loop implemented in adapter |
| Codex | `CodexAdapter` | Codex CLI / ACP | Reuse generic ACP machinery from section 5 |
| OpenCode | `OpenCodeAdapter` | OpenCode SDK / ACP | 75+ providers via Models.dev; adapter maps its events |

### The deferred HTTP adapter (D14)

A generic **HTTP adapter** (OpenAI/Anthropic-compatible REST, no ACP) is **deferred** and only considered for a model **without** the harness tool loop. It is not the path for `ds-flash` — that already gets the tool loop via the in-process Grok adapter. If we ever need a model that lacks the harness tool loop, we add a REST-only adapter then; until then it is out of scope.

### How they plug in

1. Implement `ProviderAdapter` for the backend (reuse the generic ACP client for any that speak ACP; write an HTTP client for REST-only ones).
2. Map backend events → canonical `ProviderEvent` (same table pattern as 4.2).
3. Add a `[model.*]` + `[auth_provider.*]` entry and register the adapter in the registry.
4. Done — the rest of Multiplexer is provider-agnostic.

**MVP note:** per `PLAN-CONTEXT` open question #3 (Grok-only vs multi-provider from day one), the adapter *contract* is built from day one, but whether non-Grok adapters ship in MVP is a pending decision (see Open questions).

---

## 8. Session lifecycle

A session moves through a small, well-defined state machine. The adapter owns the backend half; the orchestration engine owns the read-model half.

```
            start_session
                 │
                 ▼
   ┌───────── CREATING ──────┐
   │        │ SessionReady   │
   │        ▼                │
   │      READY ◄────────────┐
   │        │ send_turn      │
   │        ▼                │
   │     RUNNING ──► (tool/permission/user-input sub-states)
   │        │ TurnFinished / TurnInterrupted / TurnFailed
   │        ▼                │
   │      READY ─────────────┘
   │        │ session_stop
   │        ▼
   └──── STOPPED
```

### 8.1 Create

`start_session` takes the unified `SessionStartParams` (D19) — `{provider, model, workspace, initial_prompt, resume, config}` — binds the model + workspace, optionally resumes from a prior session, and returns a `ProviderSession`. First event is `SessionReady`. The same shape is used by plan/04's `session.start`.

### 8.2 Resume

Resume uses the backend's `sessionId` + `resumeCursor` (grok's `x.ai/session/*` ACP extension, or the embedded runtime's equivalent). `ResumePoint` carries both. The registry/checkpoint layer (`07`) maps a Multiplexer thread to a backend session so a restart or a mobile handoff can pick up where it left off.

### 8.3 Interrupt

`interrupt_turn` is a best-effort stop; the authoritative completion is `TurnInterrupted`. The engine treats the turn as in-flight until that event arrives, so a slow-to-stop backend can't corrupt the read model.

### 8.4 Stop

`session_stop` is idempotent and releases backend resources; emits `SessionStopped`. The engine then finalizes the thread's read model and closes the wire-contract subscription.

---

## 9. Permission / approval flow

Permission requests and user-input elicitation are the two places the agent blocks on the human. Both flow through the adapter as **requests** (events) and **responses** (commands).

### 9.1 Permission (approval)

```
Agent wants to run a shell command
        │
        ▼
ProviderEvent::PermissionRequested { request_id, tool, detail }
        │  (projected to read model; surfaced to UI / mobile)
        ▼
User picks a 4-way decision (D12)
        │
        ▼
adapter.approval_respond(session, request_id, decision, reason)
        │   decision ∈ { allow, deny, allow_once, allow_always }
        ▼
Backend proceeds or aborts → ToolCallFinished / TurnFinished
```

- **4-way decision (D12):** `approval_respond` carries `ApprovalDecision` — `allow` / `deny` / `allow_once` / `allow_always` — not a boolean. `allow_once` grants only this request; `allow_always` grants and remembers for the tool/scope. The adapter relays the enum verbatim to the backend.
- **Policy layer:** Multiplexer can auto-approve based on per-thread rules (allow-list of tools, sandbox mode) *before* surfacing to the user — but the adapter is agnostic; it just relays `approval_respond` with the chosen `ApprovalDecision`.
- **Mobile:** the paired app (`13`) receives the same `PermissionRequested` event over the wire contract and can respond from the phone.

### 9.2 User input

```
ProviderEvent::UserInputRequested { request_id, prompt }
        │
        ▼
adapter.user_input_respond(session, request_id, input)
```

The turn is blocked until a response arrives. The engine may surface a default/quick-reply set in the UI.

---

## 10. Testing

TDD at inception is non-negotiable (`PLAN-CONTEXT` testing section). The adapter layer is tested at four levels.

### 10.1 Unit tests (co-located `#[cfg(test)]`)

- **Event mapping:** feed synthetic embedded-runtime events and assert the exact canonical `ProviderEvent` emitted (pure-function mapping → exhaustive table tests).
- **Registry:** config parsing, `[model.*]`/`[auth_provider.*]` resolution, duplicate-id rejection, `op://` ref validation (never a raw value).
- **State machine:** session lifecycle transitions (create → ready → running → ready → stopped; interrupt/fail paths) via proptest for the state machine.

### 10.2 Property-based (proptest)

- Session state machine: arbitrary command sequences never leave an invalid state.
- Event-stream ordering: generated event sequences always satisfy the ordering invariant (TurnStarted before TurnFinished, etc.).

### 10.3 Integration tests (real core + mock agent)

- **Mock ACP agent:** a fake `grok agent stdio` process that speaks the ACP wire protocol; drive `AcpGrokAdapter` against it and assert the read model. This is the "real core + mock ACP agent" integration test from `PLAN-CONTEXT`.
- **In-process:** drive `InProcessGrokAdapter` against the embedded runtime with a stub tool registry; assert `ProviderEvent` flow end-to-end.
- **Real-binary smoke:** when a `grok` binary is available, a smoke test drives the ACP adapter against it (marked `#[ignore]` unless the binary is present).

### 10.4 Contract tests

- The `ProviderEvent` enum and command vocabulary are serialized to JSON and schema-verified on both the adapter side and the wire-contract side (`04`), so a change to either breaks CI loudly.

### 10.5 CI gates

fmt → clippy (deny warnings) → unit+property → mutation (cargo-mutants; ≥85% line, ≥80% branch, ≥70% mutation score) → integration → component → e2e → coverage. The adapter's pure mapping functions are prime mutation targets — high mutation score here is a hard gate.

---

## 11. Open questions

These reference pending decisions from `PLAN-CONTEXT`; none are decided here.

1. **MVP scope: Grok-only vs multi-provider from day one** (`PLAN-CONTEXT` OQ #3). The `ProviderAdapter` contract and registry are built regardless; the open question is whether any non-Grok adapter (Claude/Codex/OpenCode) ships in MVP or is post-MVP. This doc assumes the contract is in place but does not commit to shipping non-Grok adapters in MVP. (OpenRouter/DeepSeek is not part of this question — it is a config variant of the in-process Grok adapter, D14.)
2. **In-process vs ACP as the default desktop path.** This doc recommends in-process (differentiator #1) with ACP as fallback, but the final call depends on `03-vendored-grok-build.md`'s Windows-build feasibility and `PLAN-CONTEXT` OQ #5 (vendoring strategy).
3. **`ds-flash` import fidelity.** Whether to import the user's existing `config.toml` verbatim or normalize it into Multiplexer's own schema on first run — affects the registry loader design.
4. **Auto-approval policy placement.** Whether the permission policy layer lives in the adapter (per-provider) or the orchestration engine (global) — this doc sketches it as engine-side but leaves the boundary open.
5. **Resume fidelity across backends.** `resumeCursor` semantics differ per backend; whether resume is a hard MVP requirement for non-Grok providers is open.

---

## 12. Summary

The provider-adapter layer is a small, command-shaped Rust trait plus a canonical event enum, backed by a model registry. Its primary implementation embeds `xai-grok-shell` in-process (our differentiator); an ACP fallback drives the installed `grok` binary for remote/headless. Future providers plug in by implementing the same trait. The layer is fully testable in isolation, which is what makes the multi-provider story safe to build incrementally.
