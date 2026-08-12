# 02 — System Architecture

**Status:** Draft for adversarial review
**Author:** Architecture subagent
**Source of truth:** `docs/PLAN-CONTEXT.md` (this doc is consistent with it; conflicts are flagged in §9 Open questions)
**Scope:** Stack rationale, high-level architecture, embedded harness, crate layout, subsystem map, data flow, concurrency model, performance architecture, open questions.

> **Locked decisions applied: D10, D11, D35.** This doc is consistent with `docs/DECISIONS.md`. In-process embedding is a Phase-0 go/no-go hypothesis (D10); Multiplexer owns subagent scheduling and forks the vendored cap (D11); Windows-first is conditional on the Phase-0 spike with the ACP fallback as contingency (D35).

> **Locked decisions applied: D13, D14, D20.** Crate layout consolidated to `multiplexer-*` (D13); OpenRouter/DeepSeek is a config variant of the in-process Grok adapter, no separate crate (D14); `multiplexer-wire` is the single source of truth for the shared contract, no `mx-mobile-shared` (D20).

---

## 1. Stack decision rationale

### 1.1 Why Rust (not TypeScript/Effect)

T3 Code chose TypeScript + Effect for its server-centric runtime. We deliberately do not. The reasons are structural, not stylistic:

- **We embed the harness in-process.** Our #1 differentiator is calling the grok-build agent runtime directly as a library. grok-build is a Rust workspace (`xai-grok-shell`, `xai-grok-tools`, `xai-grok-workspace`). Embedding it from Rust is a native FFI-free `use`; embedding it from TypeScript would mean either a Node native addon, a sidecar process, or a CLI — all of which reintroduce exactly the process/ACP overhead we exist to eliminate. Rust is the only language where the differentiator is *free*.
- **Performance targets are hard requirements, not aspirations.** Cold start < 300 ms, input latency < 16 ms, dozens of concurrent subagents, memory "far below Electron." A JIT/GC runtime with a large baseline heap and a DOM/React renderer cannot credibly hit these on Windows. Rust gives deterministic allocation, no GC pauses, and a small static binary.
- **Concurrency without footguns.** Event-sourced orchestration with a serialized command queue per thread plus a parallel scheduler is exactly the workload Rust's ownership model and `tokio`/`rayon` make safe and fast. Effect's structured concurrency is elegant but lives in a GC'd, single-threaded-by-default runtime.
- **One binary, one contract.** A single native binary that owns processes, terminals, git, fs, checkpoints, and HAR means no Node runtime to ship, no version skew between server and client, and a trivially distributable artifact (see `plan/18-build-release-distribution.md`).
- **Windows-primary (conditional on the Phase-0 spike, D35).** Rust has first-class, well-trodden Windows support (win32, `windows`/`windows-sys` crates, WSL interop). We build on Windows and ship Windows first — **conditional on the Phase-0 spike** proving the in-process grok-build build on Windows. If that build fails or is delayed, the contingency is the **ACP path** (drive the installed `grok` binary over ACP) on Windows while in-process embedding lands on macOS/Linux first. We frame this as "Windows-primary," not "Windows-only."

We are not dogmatic: TypeScript remains the right tool for the *web* thin client and for any scripting surface. But the core, the orchestration, and the desktop UI are Rust.

### 1.2 Why GPUI (not Electron/DOM)

- **GPU-rendered, not DOM.** GPUI renders via the GPU (wgpu) with a retained scene graph, giving us 60fps+ panes, smooth diff-apply, and a native editor feel. Electron's DOM/Chromium renderer has a hard performance ceiling and a multi-hundred-MB baseline — the exact "web-perf ceiling" we cite as T3's gap.
- **Zed is the proof point.** Zed (zed-industries/zed) is a production, widely-used editor built on Rust + GPUI. It demonstrates: a real multi-cursor editor with LSP and Vim mode on GPUI; sub-16 ms input latency; low memory; and cross-platform (macOS/Linux/Windows). Our editor, pane system, and terminal embedding are the same class of problem Zed already solved. We stand on that precedent rather than re-arguing it.
- **One language across the stack.** UI, core, and harness are all Rust. No JS bridge, no serialization boundary between the renderer and the model, no IPC marshalling for every keystroke. This is what makes < 16 ms input latency achievable.
- **No bundled Chromium.** GPUI does not drag in a browser engine. This keeps the binary small and — critically — lets us *not* bundle a browser while still driving the user's real installed browsers via CDP (differentiator #3). Electron would make "no bundled Chromium" a contradiction in terms.
- **Component-testable UI.** GPUI has a component/element test harness and snapshot testing, which our TDD-at-inception strategy requires (see `plan/15-testing-strategy.md`).

**Trade-off we accept:** GPUI is younger and smaller than Electron's ecosystem; some widgets we must build ourselves. We mitigate by reusing Zed's open-source GPUI components and patterns where licensing permits, and by keeping the UI thin over the server contract so a future UI rewrite is cheap.

---

## 2. High-level architecture

Multiplexer is **server-centric**: a single native Rust binary (the *server*) owns every piece of mutable state and every external resource. All clients — desktop, mobile, web — are thin shells that render state and forward intents over **one authenticated JSON-RPC-over-WebSocket contract**.

```
┌────────────────────────────────────────────────────────────────────────────┐
│                            MULTIPLEXER SERVER                              │
│                            (single native Rust binary)                     │
│                                                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  ORCHESTRATION ENGINE (event-sourced)                                │  │
│  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────────────┐  │  │
│  │  │ command     │→ │ pure decider │→ │ projector → SQLite read     │  │  │
│  │  │ queue/thread│  │ (no I/O)     │  │ model (one transaction)     │  │  │
│  │  └─────────────┘  └──────────────┘  └─────────────────────────────┘  │  │
│  │        ▲  parallel scheduler (cross-thread / subagent fan-out)       │  │
│  └────────┼─────────────────────────────────────────────────────────────┘  │
│           │                                                               │
│  ┌────────┴────────────────────────────────────────────────────────────┐  │
│  │  EMBEDDED HARNESS  (vendored xai-org/grok-build crates, in-process) │  │
│  │  xai-grok-shell · xai-grok-tools · xai-grok-workspace               │  │
│  └────────┬────────────────────────────────────────────────────────────┘  │
│           │                                                               │
│  ┌────────┴────────────────────────────────────────────────────────────┐  │
│  │  PROVIDER ADAPTER LAYER  (Rust trait: start_session, send_turn, …) │  │
│  │  Grok(in-proc) · DeepSeek/OpenRouter · Claude · Codex · OpenCode    │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│                                                                            │
│  ┌───────────┬───────────┬───────────┬───────────┬───────────┬──────────┐  │
│  │ Terminal  │ Editor    │ System    │ HAR       │ Resource  │ Check-   │  │
│  │ (Ghostty) │ (GPUI)    │ Browser   │ Profiler  │ Monitor   │ pointing │  │
│  │           │           │ (CDP)     │ (CDP)     │ (sidecar) │ (git)    │  │
│  └───────────┴───────────┴───────────┴───────────┴───────────┴──────────┘  │
│                                                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  WIRE CONTRACT  (JSON-RPC over WebSocket, schema-verified)           │  │
│  │  Auth: local keychain · OAuth · passkeys/DPoP · WS ticket (5-min)    │  │
│  └───────────────┬──────────────────────────────┬───────────────────────┘  │
└──────────────────┼──────────────────────────────┼──────────────────────────┘
                   │                              │
        ┌──────────▼─────────┐        ┌───────────▼──────────┐
        │  DESKTOP CLIENT    │        │  MOBILE / WEB CLIENT │
        │  (GPUI, thin shell)│        │  (thin shell)        │
        │  panes · editor ·  │        │  observe + control   │
        │  terminal · browser│        └──────────────────────┘
        └────────────────────┘
```

**Key properties of this shape:**

- **One owner of truth.** The server is the only process that touches agent processes, terminals, git, the filesystem, checkpoints, and HAR. Clients never hold authoritative state; they hold a projection and send intents.
- **Thin clients, thick server.** A client can be replaced (desktop → mobile → web) without touching orchestration, providers, or the harness. This is what makes the paired mobile app (required) a small increment rather than a rewrite.
- **One contract.** JSON-RPC over WebSocket is language-agnostic, schema-verifiable on both sides (our contract tests), and works identically over localhost, a relay tunnel, or SSH. Remote/relay is a transport concern, not a protocol concern (see `plan/14-remote-and-relay.md`).
- **Everything is observable.** Because every mutation flows through the event-sourced engine and lands in the SQLite read model, any client can render a live, consistent view of agent activity, panes, diffs, and resource usage.

---

## 3. The embedded grok-build harness

Our #1 differentiator is **in-process embedding**: we vendor `xai-org/grok-build` (Apache 2.0) and call its agent runtime directly as libraries — no shelling out to a CLI, no ACP protocol overhead. **Per D10, this is a Phase-0 go/no-go hypothesis to be proven, not a settled fact.** The first Phase-0 deliverable is a spike: clone grok-build, consume `xai-grok-shell` as a library, run a headless turn in-process, and get the crates building on Windows. If the shell is not cleanly embeddable, we fall back to the **ACP path** (drive `grok agent stdio`/`serve`), which is fully supported and documented. The plan keeps both paths (§3.4).

### 3.1 Vendoring method

Per PLAN-CONTEXT, we maintain our **own fork** (upstream does not accept external contributions and Windows is "best-effort, not currently tested"). The recommended method (pending decision #5) is a **vendored fork under `third_party/` + `[patch]`**:

- `third_party/grok-build/` holds our fork as a git submodule or vendored copy (decision pending).
- The workspace root `Cargo.toml` uses `[patch.crates-io]` (or path dependencies) to point the `xai-grok-*` crates at our fork.
- **Windows build support is our responsibility.** We add and maintain the Windows build configuration, CI matrix, and any `cfg(windows)` fixes upstream lacks.
- The root `Cargo.toml` of grok-build is generated/read-only; we edit per-crate `Cargo.toml` files only.

### 3.2 What we embed, what we replace

| Upstream crate | Our use |
|---|---|
| `xai-grok-shell` | **Embed as a library** — the agent runtime (leader/stdio/headless). This is the heart of what we call in-process. |
| `xai-grok-tools` | **Embed as a library** — tool implementations (fs, git, terminal, search, session, auth). |
| `xai-grok-workspace` | **Embed as a library** — fs/VCS/execution/checkpoints. |
| `xai-grok-pager` / `xai-grok-pager-bin` | **Replace** — the TUI composition root is swapped for our GPUI UI. We keep the composition logic but point it at our renderer. |
| config / MCP / markdown / sandbox crates | **Embed as needed** — config parsing, MCP client, markdown rendering, sandboxing. |

### 3.3 What embedding buys us

- **No ACP/CLI round-trip.** A turn goes from our orchestration engine straight into `xai-grok-shell`'s runtime in the same process. No stdio framing, no JSON-RPC serialization per tool call, no process spawn per session.
- **Shared state, zero-copy where possible.** The workspace, git refs, and checkpoints are already in our process; the harness and our UI read the same state.
- **Subagent orchestration — we own it (D11).** grok-build ships `spawn_subagent` (depth 1) and Rhai workflows (`agent()`, `parallel()`, `phase()`, budget caps, max 16 concurrent children). **Multiplexer owns subagent scheduling**: we do NOT inherit the 16-child cap "for free." We fork the vendored `spawn_subagent`/workflow code as needed to raise the cap and implement our own parallel scheduler on top (see §7). We track upstream's fan-out changes (1.0.1 "bounded fan-out") and reconcile our fork with upstream's approach.
- **Model plug-in via config.** `[model.<id>]` + `[auth_provider.<id>]` in `config.toml` already let the user run `ds-flash` (DeepSeek V4 Flash via OpenRouter). Our model registry manages this surface.

### 3.4 The ACP escape hatch

Even though we embed, we keep the ACP surface (`grok agent stdio` / `serve` / `headless` + xAI extensions `x.ai/fs/*`, `x.ai/git/*`, `x.ai/terminal/*`, `x.ai/search/*`, `x.ai/session/*`, `x.ai/auth/*`) available. It is the fallback for providers we cannot embed (Claude, Codex, OpenCode) and the compatibility path for external tooling. In-process is the fast path; ACP is the universal path. (Full detail in `plan/03-vendored-grok-build.md`.)

---

## 4. Crate / workspace layout

A Rust workspace with one crate per bounded concern. Each crate is independently testable (unit + property + mutation), which is a precondition for our CI gates.

```
multiplexer/
├── Cargo.toml                      # workspace root ([patch] to third_party/grok-build)
├── third_party/
│   └── grok-build/                 # vendored fork (submodule or copy — decision #5)
├── crates/
│   ├── multiplexer-wire/           # shared wire contract: JSON-RPC schema, types, codec, validation (single source of truth; codegen for Swift/Kotlin/TS clients)
│   ├── multiplexer-provider/       # ProviderAdapter trait + canonical ProviderEvent; Grok in-process + ACP adapters; model registry
│   ├── multiplexer-core/           # orchestration engine, decider, projector, read model
│   ├── multiplexer-server/         # composition root: owns everything, serves WS
│   ├── multiplexer-ui/             # GPUI app shell: panes, windows, chrome
│   ├── multiplexer-terminal/       # Ghostty embedding + PTY management
│   ├── multiplexer-browser/        # system-browser detection/import + CDP driver
│   ├── multiplexer-har/            # HAR capture, waterfall, replay
│   └── multiplexer-mobile-shared/  # shared contract/types for the mobile client (consumes multiplexer-wire via codegen)
└── apps/
    ├── multiplexer-desktop/         # desktop binary (thin GPUI shell over multiplexer-server)
    ├── multiplexer-mobile/          # mobile client (Expo/React Native — decision #2)
    └── multiplexer-web/             # optional web thin client
```

**Dependency discipline:** `multiplexer-core` depends on nothing but `multiplexer-wire` (pure domain). `multiplexer-provider` depends on `multiplexer-core` and the provider-adapter trait (not concrete providers). `multiplexer-server` is the only place that wires concrete providers, the harness, and the wire contract together. This keeps the decider/projector pure and unit-testable without any I/O.

---

## 5. Subsystem map

Each subsystem is a bounded concern; arrows indicate the primary dependency/connection.

### 5.1 Orchestration engine (`multiplexer-core`)
The event-sourced heart. Owns per-thread serialized command queues, the pure decider, the projector, and the parallel scheduler for cross-thread/subagent work. It is the only subsystem that mutates the read model. Connects to: every provider adapter (via the trait), the checkpoint subsystem, and the wire contract (to stream events to clients). Full detail in `plan/06-orchestration-engine.md`.

### 5.2 Provider adapter layer (`multiplexer-provider`)
The Rust trait `start_session`, `send_turn`, `interrupt_turn`, `approval_respond`, `user_input_respond`, `checkpoint_revert`, `session_stop` plus a canonical `ProviderEvent` stream. Concrete adapters: Grok in-process (the fast path) and the generic ACP adapter (Claude/Codex/OpenCode). OpenRouter/DeepSeek (`ds-flash`) is a config variant of the in-process Grok adapter, not a separate adapter. Connects to: orchestration (consumes the trait), model registry (selects config). Full detail in `plan/05-provider-adapter-layer.md`.

### 5.3 Model registry (in `multiplexer-provider`)
Manages `[model.*]` and `[auth_provider.*]` config, resolves a model id to a provider adapter + auth provider, and selects per thread. Connects to: provider adapters, auth subsystem, config parsing from the vendored grok-build config crate.

### 5.4 Checkpointing / VCS (`multiplexer-checkpoint`)
Hidden Git refs per turn; diff queries between checkpoints; revert. Consumed by orchestration (checkpoint_revert) and by the editor/UI (inline diff display). Connects to: `xai-grok-workspace` (git primitives), orchestration, UI. Full detail in `plan/07-checkpointing-and-vcs.md`.

### 5.5 Terminal (`multiplexer-terminal`)
Ghostty embedding with splits, plus PTY management. Renders into GPUI panes and the pop-up terminal. Connects to: UI (rendering), orchestration (agent terminal tool), resource monitor. Full detail in `plan/08-terminal.md`.

### 5.6 Editor (`multiplexer-editor`)
Native GPUI editor: multi-cursor, LSP, Vim mode, inline diff-apply. The "real editor" differentiator. Connects to: UI (pane), checkpoint (diffs), LSP servers (via `xai-grok-tools`/own LSP client). Full detail in `plan/09-editor.md`.

### 5.7 Pane UI (`multiplexer-ui`)
Outlook-style layout: left chat sidebar, center build pane, multi-purpose right bar (browser/HAR/files/diff/terminal/agent activity), optional pop-up terminal below; every pane can pop out to its own window. Connects to: all renderable subsystems (editor, terminal, browser, HAR, orchestration activity). Full detail in `plan/10-ui-pane-system.md`.

### 5.8 System-browser integration (`multiplexer-browser`)
Detect/import installed browsers (Chrome, Edge, Firefox, Safari, Arc, Brave), launch/authorize, drive via CDP. **No bundled Chromium.** Connects to: UI (browser pane), HAR (shares CDP), orchestration (Design Mode: browser element → agent). Full detail in `plan/11-system-browser-integration.md`.

### 5.9 HAR profiler/replayer (`multiplexer-har`)
Capture network via CDP, visualize waterfalls, replay recorded sessions. Connects to: browser (CDP capture), UI (HAR pane). Full detail in `plan/12-har-profiler-replayer.md`.

### 5.10 Mobile (`multiplexer-mobile` + `multiplexer-wire`)
Paired mobile app (required) — observe/control agents from the phone. Thin shell over the same wire contract; consumes `multiplexer-wire` via codegen (no separate shared-contract crate). Connects to: wire contract, remote/relay. Full detail in `plan/13-mobile-app.md`.

### 5.11 Remote / relay (`multiplexer-remote`)
Local + paired + relay tunnel + SSH; WebSocket ticket auth (5-min TTL); Tailscale serve. Connects to: wire contract (transport), auth. Full detail in `plan/14-remote-and-relay.md`.

### 5.12 Resource monitor (`multiplexer-resource-monitor`)
Rust sidecar emitting NDJSON over stdio, power-adaptive sampling. Feeds the UI's resource pane and the performance/adaptive-sampling logic. Connects to: UI, orchestration (backpressure hints). 

### 5.13 Auth (`multiplexer-auth`)
OS keychain for local secrets; OAuth for providers; passkeys/DPoP for remote. Connects to: model registry, remote, wire contract (session auth). Full detail in `plan/17-security-and-secrets.md`.

---

## 6. Data flow — a user prompt end to end

1. **UI intent.** The user types in the chat sidebar (GPUI). The desktop client serializes an intent (e.g. `session.sendTurn { threadId, text }`) into the JSON-RPC wire contract and sends it over WebSocket.
2. **Wire contract.** `multiplexer-wire` validates the message against the schema, dispatches it to `multiplexer-server`, which routes it to the orchestration engine for that thread.
3. **Command queue.** The intent becomes a command enqueued on the thread's serialized command queue.
4. **Decider.** The pure decider consumes the command and the current read-model state, and emits domain events (e.g. `TurnStarted`, `ToolInvoked`, `TurnFinished` — canonical vocabulary per D16). No I/O here.
5. **Projector.** In the same transaction, the projector folds the events into the SQLite read model. The read model is now consistent and queryable.
6. **Provider dispatch.** The decider's side-effect instruction is handed to the provider adapter for the thread's model. For Grok, this calls `xai-grok-shell` **in-process**; for others, the ACP adapter. OpenRouter/DeepSeek (`ds-flash`) runs through the in-process Grok adapter as a config variant.
7. **Harness execution.** The embedded harness runs the turn: model calls, tool invocations (fs/git/terminal/search via `xai-grok-tools`/`xai-grok-workspace`), checkpoints written as hidden git refs.
8. **Streaming events back.** Every state change the harness produces is normalized into the canonical `ProviderEvent` stream, projected into the read model, and **streamed to all subscribed clients** over the wire contract as JSON-RPC notifications (e.g. `event.toolInvoked`, `event.diffUpdated`, `event.terminalOutput`).
9. **Client render.** Each thin client applies the event to its local projection and re-renders. Because the server is the single source of truth, desktop and mobile stay consistent with no client-side reconciliation.

```
User → UI intent → WS/JSON-RPC → server → command queue → decider → events
   → projector → SQLite read model ──────────────┐
   → provider adapter → embedded harness → tool calls / checkpoints
   → ProviderEvent stream → projector → WS notifications → all clients render
```

---

## 7. Concurrency model

### 7.1 The problem with T3's model
T3 Code uses a **single serialized command queue** for orchestration. That is correct for *correctness* (per-thread ordering) but is a **bottleneck for fan-out**: dozens of subagents all serialize through one queue, so cross-thread work stalls. This is a gap we explicitly exploit.

### 7.2 Our model: serialized queue per thread + parallel scheduler
- **Serialized command queue per thread.** Each thread (session) has its own command queue. Commands within a thread are strictly ordered — this preserves the event-sourcing invariant that a thread's events are totally ordered and the read model is consistent.
- **Parallel scheduler for cross-thread/subagent work.** Independent threads and subagents run on a parallel scheduler (tokio/rayon). Fan-out of dozens of subagents proceeds concurrently; only the *shared* read-model writes are serialized (via the projector transaction), not the *work* itself.
- **No global lock on the hot path.** The read model is the only shared mutable structure; it is written transactionally and read via snapshots/streams, so concurrent threads don't contend on a single queue.

### 7.3 Pure decider + projector in one transaction
- **Pure decider:** takes (command, read-model state) → emits events. No I/O, no side effects. This makes it trivially unit-testable and property-testable (proptest over state machines).
- **Projector:** folds events into the SQLite read model. The decider + projector run **in one transaction**, so the read model never observes a half-applied command.
- **Side effects are deferred.** The decider does not perform I/O; it emits events that the orchestration engine then dispatches to adapters/harness. This keeps the decider pure and the side effects auditable.

```
Thread A queue ─┐
Thread B queue ─┼─► parallel scheduler ─► decider ─► events ─► projector ─► SQLite
Subagent fan-out┘        (concurrent)        (pure)              (one txn)
```

---

## 8. Performance architecture

Targets (from PLAN-CONTEXT): cold start < 300 ms, input latency < 16 ms, dozens of concurrent subagents, memory far below Electron.

### 8.1 Cold start < 300 ms
- **Single static binary, no runtime.** No Node, no JIT warm-up, no Chromium to boot. The binary loads and the GPUI scene graph initializes in tens of ms.
- **Lazy subsystem init.** The editor, terminal, browser, and HAR subsystems initialize on first use, not at startup. The server starts with just the wire contract, orchestration, and read model.
- **SQLite read model is local and fast.** No network dependency to render the UI; the read model is a local file opened at startup.
- **Precompiled shaders / cached scene.** GPUI shaders and any expensive UI assets are baked into the binary or cached on first run.

### 8.2 Input latency < 16 ms
- **GPU-rendered UI, no DOM.** GPUI renders directly; keystrokes go from the OS to the editor to the GPU without a DOM/React reconciliation pass.
- **In-process harness.** Tool calls and agent state updates don't cross a process boundary, so UI updates triggered by agent activity are cheap.
- **Event streaming, not polling.** Clients receive push notifications; there is no request/response round-trip per UI update.
- **Editor is native.** Multi-cursor, LSP, and Vim mode operate on an in-memory buffer with incremental rendering — the Zed-proven model.

### 8.3 Dozens of concurrent subagents
- **Parallel scheduler** (§7) removes the single-queue bottleneck; subagent work runs concurrently.
- **We own the subagent cap (D11).** We fork the vendored `spawn_subagent`/workflow code to raise grok-build's built-in 16-concurrent-children cap and implement our own parallel scheduler; we do not inherit the cap "for free."
- **Transactional read model** means concurrent writers don't corrupt state; they serialize only on the short projector transaction.

### 8.4 Memory
- No Chromium, no Node, no DOM — the two biggest memory sinks in Electron competitors are absent.
- Resource monitor (power-adaptive sampling) feeds backpressure so we don't over-allocate under load.

(Full detail and measurement methodology in `plan/16-performance.md`.)

---

## 9. Open questions

These are the pending decisions from PLAN-CONTEXT that this doc touches but does **not** decide unilaterally:

1. **Stack (Rust + GPUI vs Electron+React).** This doc assumes Rust + GPUI per the approved architecture; the user decision is still open and this doc should be revisited if it flips.
2. **Mobile (native SwiftUI/Kotlin vs Expo/React Native).** The architecture is agnostic (thin client over the wire contract); the concrete mobile stack is pending. Mobile consumes `multiplexer-wire` via codegen either way.
3. **MVP scope (Grok-only vs multi-provider from day one).** The provider-adapter layer supports both; whether MVP ships only the in-process Grok path is pending.
4. **Editor scope (full native editor in MVP vs lighter editor first).** The crate layout assumes the full editor; a lighter MVP editor would trim `multiplexer-editor` scope, not the architecture.
5. **grok-build vendoring (submodule vs vendored copy vs `[patch]`).** This doc recommends vendored fork under `third_party/` + `[patch]` (per PLAN-CONTEXT) but the exact mechanism is pending.
6. **Branding (which domain is the product brand vs redirect).** Not architecture-relevant; noted for completeness.
7. **Orca baseline scope (match all Orca features in MVP vs subset).** Affects which subsystems ship in MVP, not their design.
8. **Windows-first (conditional on Phase-0 spike, D35).** This doc assumes Windows-first per the approved architecture, **conditional on the Phase-0 spike** proving the in-process grok-build build on Windows; the ACP fallback is the contingency (see §1.1).

**Flagged consistency note:** none found — this doc is consistent with PLAN-CONTEXT. If any of the above decisions flip, the affected sections (§1, §3, §4, §8) must be revisited.
