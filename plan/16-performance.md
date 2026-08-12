# 16 — Performance

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Performance / Core runtime
**Depends on:** `02-architecture.md`, `03-vendored-grok-build.md`, `04-wire-contract.md`, `06-orchestration-engine.md`, `15-testing-strategy.md`
**Feeds:** `19-roadmap-and-milestones.md`, `20-risks-and-open-questions.md`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context) and with `docs/DECISIONS.md` (the locked decisions). Where a decision is not yet
settled, it is listed under **Open questions** and is **not** decided unilaterally here.

> **Locked decisions applied:** D22 (dedicated perf stage in CI), M5 (quantified,
> measurable performance targets), M6 (input-latency measurement method). These are final
> and supersede any earlier "open question" or hedge wording in this doc.

---

## 1. Purpose

Performance is not a nice-to-have for Multiplexer — it is a **core differentiator**. Our
positioning is "a real editor, real performance, and real insight," and our competitors are
either Electron-based (T3 Code, Codex Desktop) or macOS-only (Superset, Conductor). The
performance targets in PLAN-CONTEXT are hard requirements, not aspirations:

| Target | Value | Why it matters |
|---|---|---|
| Cold start → usable editor | **< 300 ms** (first usable editor frame, measured on the reference machine class, §3) | The app must feel instant; no splash-screen wait. |
| Input latency | **< 16 ms p95** (60fps+), measured per §4/M6 | Keystrokes must render on the next frame; no perceptible lag. |
| Subagent fan-out | **48 concurrent subagents** (hard gate, §5) | Our #7 differentiator; fixes T3's single-queue ceiling. |
| Memory | **< 500 MB idle, < 1.5 GB under heavy fan-out** (reference machine class, §7) | No Chromium, no Node, no DOM; native buffers. |

This doc is the implementation plan for how we **hit, measure, and enforce** those targets. It
covers the native advantage, cold start, input latency, concurrency, the resource monitor,
memory, streaming/backpressure, and the profiling/measurement methodology that keeps us honest.

---

## 2. Why native wins — Rust + GPUI vs Electron/DOM

The performance targets are only credible because of the stack. PLAN-CONTEXT approves **Rust core
+ GPUI (GPU-rendered) UI, NOT Electron**. The structural reasons:

### 2.1 No runtime to boot, no JIT warm-up

- **Electron** ships a full Node.js runtime plus a Chromium renderer. Cold start pays for: Node
  bootstrap, V8 JIT warm-up, DOM/React initialization, and a multi-hundred-MB baseline heap —
  before a single pixel of UI is drawn. This is the "web-perf ceiling" we cite as T3's gap.
- **Multiplexer** is a single native Rust binary. There is no interpreter, no JIT, no GC to warm
  up. The binary loads, the GPUI scene graph initializes, and the first frame renders in tens of
  milliseconds. Deterministic allocation and no GC pauses mean latency is predictable.

### 2.2 GPU-rendered, not DOM

- **GPUI** renders via the GPU (wgpu) with a retained scene graph. The editor, panes, terminal,
  and diff views are GPU-accelerated. A keystroke goes OS → editor buffer → GPU in a handful of
  native calls.
- **Electron/DOM** must run a React reconciliation pass, diff the virtual DOM, mutate the real
  DOM, and let the browser composite — a much longer, less predictable path that is fundamentally
  capped at web performance.

### 2.3 Zed is the proof point

Zed (zed-industries/zed) is a production editor built on **Rust + GPUI** that already
demonstrates the exact class of problem we are solving:

- A real multi-cursor editor with LSP and Vim mode on GPUI.
- **Sub-16 ms input latency** and 60fps+ rendering in normal use.
- Low memory footprint relative to Electron editors.
- Cross-platform (macOS/Linux/Windows) — the Windows path we require.

We stand on that precedent rather than re-arguing it. Our editor (`plan/09`), pane system
(`plan/10`), and terminal embedding (`plan/08`) are the same class of problem Zed solved. Where
Zed's open-source GPUI components and patterns are reusable under compatible licensing, we reuse
them (see `02-architecture.md` §1.2).

### 2.4 One language, no bridge

UI, core, and harness are all Rust. There is no JS bridge, no serialization boundary between the
renderer and the model, and no IPC marshalling per keystroke. This is what makes < 16 ms input
latency achievable — the frame path is native end to end.

### 2.5 No bundled Chromium

GPUI does not drag in a browser engine. This keeps the binary small and lets us **not** bundle a
browser while still driving the user's real installed browsers via CDP (differentiator #3). An
Electron app cannot make that claim — "no bundled Chromium" would be a contradiction in terms.

---

## 3. Cold start < 300 ms

The target is: from process launch to a **usable editor** in under 300 ms. "Usable" means the
window is up, the editor is interactive, and the user can type — not that every subsystem is
loaded.

**Measurement basis (M5):** the < 300 ms budget is measured as **process launch → first usable
editor frame** (the first frame on which the editor element is interactive and accepts input),
on the **reference machine class** defined in §9.1. The budget **must include editor init** —
the editor is lazy (§3.2), so the first-open editor element's creation and first render are
inside the cold-start budget, not excluded from it. We measure the full path from launch to a
typeable editor, not just the first window frame.

> **Phase-0 validation flag (M5):** GPUI-on-Windows cold start is **unproven** — Zed's
> sub-16 ms / sub-300 ms numbers are demonstrated primarily on macOS/Linux, and our Windows
> path is our responsibility. The < 300 ms cold-start budget must be **validated in Phase 0**
> with a GPUI-on-Windows spike before it is treated as a settled target. If the spike shows
> the budget is not achievable on Windows, the target and/or the reference machine class must
> be revisited (see `20-risks-and-open-questions.md`).

### 3.1 Single static binary, no runtime

- No Node, no JIT warm-up, no Chromium to boot. The binary loads and the GPUI scene graph
  initializes in tens of ms.
- Startup work is minimized: the server starts with just the wire contract, orchestration, and
  the SQLite read model. Everything else is deferred.

### 3.2 Lazy subsystem initialization

The editor, terminal, browser, and HAR subsystems initialize **on first use**, not at startup.
Concretely:

| Subsystem | Startup behavior |
|---|---|
| Wire contract / WS listener | **Eager** — needed to serve the UI. |
| Orchestration engine + read model | **Eager** — opens local SQLite (WAL), replays log tail. |
| Editor | **Lazy** — buffer + GPUI editor element created on first open. |
| Terminal (Ghostty) | **Lazy** — PTY + renderer spawned on first terminal pane. |
| System browser (CDP) | **Lazy** — browser detection/launch on first browser pane. |
| HAR capture | **Lazy** — CDP capture session on first HAR pane. |
| LSP servers | **Lazy** — per-file/workspace on first edit. |
| Resource monitor sidecar | **Lazy** — spawned on first resource pane / telemetry subscribe. |

The startup path is a short, linear sequence: load config → open SQLite → bind WS → draw first
frame. Nothing blocks on network, provider auth, or heavy subsystem init.

### 3.3 Local read model, no network dependency

The UI renders from the **local** SQLite read model. There is no network round-trip to show the
window or the editor. Remote/relay is a transport concern for *clients*, not a startup dependency
of the desktop shell.

### 3.4 Precompiled shaders / cached scene

GPUI shaders and any expensive UI assets are **baked into the binary** or cached on first run, so
the first frame does not pay a shader-compilation or asset-load cost. This is a known GPUI/Zed
practice and directly protects the cold-start budget.

### 3.5 Startup budget accounting

We track the cold-start budget explicitly so regressions are visible (see §9). The rough
allocation:

| Phase | Budget |
|---|---|
| Binary load + static init | ~50 ms |
| Config + SQLite open + log replay | ~100 ms |
| WS bind + first frame | ~100 ms |
| **Total** | **< 300 ms** |

The **editor init** (first-open editor element creation + first render) is included within the
"WS bind + first frame" phase — it is part of the measured "first usable editor frame" and must
not push the total over 300 ms (M5).

---

## 4. Input latency < 16 ms

The target: a keystroke is reflected on screen in under 16 ms (one 60fps frame). This is the
"feels native" bar that Electron editors struggle to meet.

**Measurement method (M6):** keystroke-to-frame latency is measured in **headless/CI** via a
combination of:

- **GPUI frame-time instrumentation** — the render thread records a frame-time histogram
  (already surfaced through the resource monitor, §6.1). We instrument the editor's input
  handler to timestamp the keystroke event and the corresponding rendered frame, giving a
  per-keystroke latency distribution.
- **Synthetic input injection** — the headless test harness injects synthetic key events
  directly into the GPUI event loop (no OS-level input needed), timestamps the injection, and
  reads back the frame timestamp when the keystroke's effect is rendered. This makes the
  measurement deterministic and CI-runnable without a display/input device.

The gate is **< 16 ms p95** over a synthetic keystroke burst, measured on the **reference
machine class** (§9.1). The same instrumentation feeds the `input_latency` benchmark (§9.1)
and the field telemetry (§9.4).

### 4.1 GPU-rendered text, no DOM in the frame path

- Keystrokes go from the OS event loop straight into the GPUI editor buffer and are rendered by
  the GPU on the next frame. There is **no React reconciliation, no virtual-DOM diff, no DOM
  mutation** in the frame path.
- The editor is native: multi-cursor, LSP, and Vim mode operate on an in-memory buffer with
  **incremental rendering** — only the changed region is re-laid-out and re-drawn (the
  Zed-proven model). Large files do not re-render wholesale per keystroke.

### 4.2 In-process harness keeps UI updates cheap

Because the grok-build harness is embedded **in-process**, agent activity (tool calls, diffs,
terminal output) updates the read model and streams to the UI without crossing a process
boundary. UI updates triggered by agent activity are cheap and do not contend with the input
path.

### 4.3 Event streaming, not polling

Clients receive **push notifications** over the wire contract; there is no request/response
round-trip per UI update. The desktop client is a thin shell that applies pushed events to its
local projection and re-renders — no polling loop adds latency.

### 4.4 Frame-path discipline

The frame path (OS event → editor buffer → GPU) must stay free of:
- **Blocking I/O** — no filesystem, network, or SQLite reads on the render thread.
- **Allocation churn** — buffers and scene nodes are reused; the hot path avoids per-frame
  allocations.
- **Lock contention** — the render thread never takes the read-model write lock; it renders from
  a snapshot/stream.

We enforce this with a dedicated render/UI thread and a strict rule that the editor's hot path
touches only in-memory state (see §9 for how we measure it).

---

## 5. Concurrency — dozens of concurrent subagents

Our #7 differentiator is "subagent orchestration at scale — fan out many subagents on specific
tasks, live orchestration dashboard." The performance requirement is **48 concurrent subagents
without a serialization bottleneck** (M5). This is the explicit fix for T3 Code's single
serialized queue.

> **Hard gate (M5):** the fan-out benchmark — spawn and run **48 concurrent subagents** with no
> serialization bottleneck — is a **hard CI gate**, not a soft/trend target. A regression that
> fails to sustain 48 concurrent subagents **fails CI** (enforced in the dedicated perf stage,
> §9.2). This depends on **our own scheduler** (D11) — Multiplexer owns subagent scheduling and
> forks the vendored `spawn_subagent`/workflow code to raise the cap — **not** on the vendored
> 16-child cap. The vendored cap is the floor; our global cap is the ceiling.

### 5.1 The T3 bottleneck we fix

T3 Code funnels every turn, subagent, and tool call through **one serialized command queue**.
A long-running model turn or slow subagent blocks unrelated work — a hard ceiling on concurrency.
Multiplexer replaces this with a **two-tier model** (detailed in `06-orchestration-engine.md` §3):

1. **Serialized per-thread queues** — each thread (session) has its own command queue. Commands
   *within* a thread are strictly ordered (required for that session's state-machine
   correctness), but threads are independent and never block each other.
2. **A parallel scheduler** for cross-thread and subagent work — work not bound to a single
   thread's state machine (spawning subagents, independent tool calls, fan-out panels) runs
   concurrently on a shared pool.

### 5.2 Async runtime: tokio

The scheduler and all I/O-bound work run on **tokio**, Rust's async runtime:

- **Work-stealing multi-threaded executor** — tokio's default multi-thread scheduler steals work
  across worker threads, so a burst of subagent tasks spreads across cores instead of queuing on
  one thread.
- **Non-blocking I/O** — agent streams, WebSocket connections, PTYs, and CDP all use async I/O,
  so dozens of concurrent sessions do not each burn a thread.
- **Structured concurrency** — `JoinSet` and `Semaphore` give us bounded, cancellable fan-out
  (see below).

### 5.3 Work-stealing and the scheduler

The scheduler (from `06-orchestration-engine.md` §3.3) uses a `tokio::task::JoinSet` for live
child tasks and an `Arc<Semaphore>` for the concurrency budget:

```rust
pub struct Scheduler {
    pool: tokio::task::JoinSet<()>,          // live child tasks
    budget: Arc<Semaphore>,                  // max concurrent children
    pending: VecDeque<QueuedWork>,           // work waiting for a budget slot
}

impl Scheduler {
    pub async fn spawn_subagent(&self, spec: SubagentSpec) -> SubagentHandle {
        let permit = self.budget.acquire_owned().await; // backpressure
        let handle = self.pool.spawn(run_subagent(spec, permit));
        SubagentHandle::new(handle)
    }
}
```

- **Budget caps** — per-thread (default 16) and global (default 64, configurable) caps bound
  concurrency; the global cap is the "dozens of concurrent subagents" target. When a cap is hit,
  new spawns are **queued (not dropped)** and act as a barrier for the calling workflow, exactly
  as grok-build's `parallel()` panels do.
- **No global lock on the hot path** — the read model is the only shared mutable structure; it is
  written transactionally and read via snapshots/streams, so concurrent threads do not contend on
  a single queue. The only serialization is the short projector transaction (see §5.4).

### 5.4 The projector transaction is the only shared write

The event-sourced engine serializes **only the read-model write** (decider + projector in one
SQLite transaction), not the *work* itself. Dozens of subagents run concurrently; their results
are folded into the read model in short, atomic transactions. This keeps correctness (per-thread
ordering, atomic append+project) without a global work queue.

### 5.5 Embedded harness concurrency

The vendored grok-build harness ships `spawn_subagent` (depth 1) and Rhai workflows
(`agent()`, `parallel()`, `phase()`, budget caps, max 16 concurrent children). We inherit this as
a library and layer our scheduler on top. **Multiplexer owns subagent scheduling (D11)** — we
fork the vendored `spawn_subagent`/workflow code as needed to raise the cap and implement our
own parallel scheduler. The vendored 16-child cap is the floor, our global cap is the ceiling,
and the 48-concurrent-subagent hard gate (§5) is enforced by **our** scheduler, not the
vendored default (see `06-orchestration-engine.md` §4.1).

---

## 6. Resource monitor — the Rust sidecar

The resource monitor (`multiplexer-resource-monitor`) is a **Rust sidecar emitting NDJSON over stdio**,
with **power-adaptive sampling** (per PLAN-CONTEXT). It feeds the UI's resource pane and the
performance/adaptive-sampling logic, and it provides the backpressure hints that keep us from
over-allocating under load.

### 6.1 Process telemetry

The sidecar samples and reports per-process and system metrics:

| Metric | Source | Notes |
|---|---|---|
| CPU (per-process + total) | OS APIs | Percent of a core; aggregate across threads. |
| Memory (RSS / committed) | OS APIs | Per-process working set + commit. |
| Disk I/O | OS APIs | Bytes read/written, for the worktree and DB. |
| Network I/O | OS APIs | Bytes in/out (agent streams, CDP, relay). |
| Thread count | OS APIs | Detects thread explosion in fan-out. |
| GPU / frame time | GPUI | Frame-time histogram for the render thread. |
| Power source / state | OS power APIs | AC vs battery vs constrained (see §6.2). |

### 6.2 Power-adaptive sampling

Sampling frequency adapts to power state so we do not burn battery polling at high frequency:

| Power state | Sample interval | Rationale |
|---|---|---|
| **AC (plugged in)** | **1 s** | Rich telemetry; battery is not a concern. |
| **Battery** | **5 s** | Reduce wakeups and polling overhead. |
| **Constrained** (low battery / power saver / thermal) | **15 s** | Minimal overhead; only coarse signals. |

The sidecar reads the power state and adjusts its own timer; it also reports the current interval
so consumers know the effective resolution.

### 6.3 Bounded in-memory history

The sidecar keeps a **bounded ring buffer** of recent samples (e.g. last N minutes at the current
interval) so the UI can render a rolling graph without unbounded growth. Older samples are
persisted to the SQLite read model (or a telemetry table) on a coarse cadence, not held in memory
forever. The wire contract exposes this via `telemetry.resources` / `telemetry.subscribe`
(`04-wire-contract.md` §4.15).

### 6.4 Backpressure hints

The resource monitor feeds the orchestration engine's **backpressure logic**: if CPU, memory, or
thread count approaches a threshold, the scheduler can throttle new subagent spawns or the
provider ingestion worker can slow down. This is a *hint* channel, not a hard gate — the
scheduler's budget caps remain the primary control (see §5.3 and `06-orchestration-engine.md`).

---

## 7. Memory — far below Electron competitors

The memory target is "far below Electron competitors." This is largely a **structural** win, not
an optimization exercise.

### 7.1 What we do not ship

- **No Chromium** — the single biggest memory sink in Electron competitors (hundreds of MB of
  renderer, GPU, and utility processes) is absent. We drive the user's *installed* browsers via
  CDP instead of bundling one.
- **No Node runtime** — no V8 heap, no JIT code cache, no Node baseline.
- **No DOM** — no DOM tree, no style/layout engine, no React fiber tree.

### 7.2 Native buffers

- The editor holds text in **native in-memory buffers** (rope/`Rope`-style structures, the
  Zed-proven model) with incremental rendering — no per-character DOM nodes.
- Terminal output, diffs, and HAR data are held as compact native buffers, not DOM/JS objects.
- The read model is SQLite with WAL; it is the single source of truth and is not duplicated in
  memory beyond the projection the UI needs.

### 7.3 Bounded caches and streaming

- Large streams (terminal output, diffs, HAR) are **chunked and backpressured** (see §8), so we
  never buffer unbounded data in memory.
- The resource monitor's history is a bounded ring buffer (§6.3).
- Subagent event streams are projected into the read model and streamed to clients; they are not
  retained in memory beyond what the dashboard needs.

### 7.4 Expected footprint

Because we ship no browser engine and no runtime, the baseline footprint is expected to be a
small fraction of an Electron competitor's (typically hundreds of MB). We measure and enforce
this with a memory budget in CI (see §9).

**Concrete budget (M5):** **< 500 MB RSS idle** (app running, no active agent work) and
**< 1.5 GB RSS under heavy fan-out** (48 concurrent subagents, §5), measured on the **reference
machine class** (§9.1). These are hard CI gates in the dedicated perf stage (§9.2). The
reference machine class is a typical Windows desktop (see §9.1) so the budget reflects a
realistic user machine rather than a CI-only artifact.

---

## 8. Streaming & backpressure

Large streams — terminal output, diffs, HAR — must not flood a slow client or the WebSocket
buffer. The wire contract (`04-wire-contract.md` §8) defines the mechanism; this section ties it
to the performance targets.

### 8.1 Chunking

- **Terminal output:** PTY bytes are batched and emitted as `terminal_output` events with a
  base64 payload, **coalesced on a short timer (16–50 ms)** so a burst becomes a few frames, not
  thousands. Clients render coalesced frames. This directly protects the input-latency and
  memory targets under heavy terminal output.
- **Diffs:** `checkpoint.diff` / `git.diff` return a **structured diff** (list of hunks with
  per-line metadata) rather than one giant string. Large diffs are paginated via `{cursor,
  limit}` or returned as a `diff:<id>` stream of `diff_chunk` events.
- **HAR:** `har_event` entries are pushed incrementally; the full document is only materialized
  on `har.stop`. Replay is a separate stream.

### 8.2 Flow control (server → client)

- **Window-based:** each subscription has a server-side send window (e.g. 1024 events or 4 MiB).
  When the window is full, the server stops emitting and the client must send `stream.ack` (with
  the last `seq` it consumed) to reopen it.
- **Slow-consumer policy:** if a client never acks, the server drops the subscription (or
  coalesces) rather than buffering unboundedly. The client re-subscribes and catches up from a
  checkpoint `seq`.
- **Resume:** subscriptions carry an optional `from_seq` so a reconnecting client resumes without
  replaying everything.

### 8.3 Backpressure (client → server)

Large uploads (file writes, HAR replay) are chunked with `fs.write` / `har.replay` accepting
`{offset, chunk}` sequences; the server acks each chunk before the next is sent.

### 8.4 Internal backpressure

Backpressure is not only on the wire. Inside the server:

- The provider ingestion worker drains the `ProviderEvent` stream into the read model in the same
  transaction as the producing command (event-sourced, per `05-provider-adapter-layer.md` §3).
- The scheduler's `Semaphore` provides backpressure for subagent spawns (§5.3).
- The resource monitor provides backpressure hints (§6.4).

The result: no component buffers unboundedly, so memory stays bounded and the frame path stays
fast even under heavy load.

---

## 9. Profiling & measurement

Targets are only meaningful if we **measure and enforce** them. This section defines how.

### 9.1 Benchmarks

We maintain a **benchmark suite** (criterion) covering the hot paths:

| Benchmark | Measures | Target |
|---|---|---|
| `cold_start` | Process launch → first usable editor frame (incl. editor init) | < 300 ms |
| `input_latency` | Keystroke → rendered frame (synthetic injection, M6) | < 16 ms (p95) |
| `frame_time` | Render thread frame-time distribution | 60fps+ (p95 < 16.7 ms) |
| `subagent_fanout` | Time to spawn + run **48** concurrent subagents | **hard gate**: sustains 48 concurrent, no serialization bottleneck |
| `read_model_write` | Decider+projector transaction latency | low, bounded |
| `terminal_throughput` | Coalesced terminal output under burst | bounded frames, no flood |
| `memory_footprint` | RSS at idle and under heavy fan-out | **< 500 MB idle, < 1.5 GB under 48-subagent fan-out** |

Benchmarks run on a **pinned reference machine class** so numbers are comparable across runs.
The reference machine class is a **typical Windows desktop** (e.g. a mid-range x86-64 desktop
with 8–16 cores and 16–32 GB RAM, running Windows 10/11) — representative of a real user
machine, not a bespoke CI-only artifact. The exact runner is pinned in CI (a fixed runner
class or a dedicated reference box) and recorded with every baseline. We record baselines and
flag regressions.

### 9.2 CI performance gates

Per the TDD-at-inception mandate, performance is gated in CI in a **dedicated performance
stage** in the plan/15 pipeline (placed between the integration and component stages, or as
its own named stage — plan/15 names where it lives). Perf checks do **not** ride along
inside integration/coverage; they are their own gate (D22):

- **Hard gates:** cold start < 300 ms, input latency < 16 ms (p95), memory under budget
  (< 500 MB idle, < 1.5 GB under heavy fan-out), and **48-concurrent-subagent fan-out** (M5).
  A regression beyond any threshold **fails CI** — no blind CI, no merge.
- **Soft gates / trend tracking:** frame-time p95, transaction latency. These are tracked as
  trends; a sustained regression is flagged for review even if under the hard threshold.
- **Mutation/unit gates** still apply to the pure decider/projector and scheduler (see
  `06-orchestration-engine.md` §9 and `15-testing-strategy.md`).

### 9.3 Flamegraphs and profiling

For diagnosis, we use:

- **`perf` / samply / Tracy** (platform-dependent) for CPU flamegraphs of the frame path and the
  scheduler.
- **GPUI's built-in frame-time instrumentation** for the render thread.
- **tokio console** for async-task scheduling and to detect starvation or unbounded queues in the
  scheduler.
- **The resource monitor** (§6) for long-running memory/CPU trends in the field.

Profiling is a **diagnostic** tool; the **benchmarks + CI gates** are the enforcement mechanism.

### 9.4 Field telemetry (opt-in)

The resource monitor and telemetry (`telemetry.*` in `04-wire-contract.md`) give us opt-in
field data on real machines (cold start, frame time, memory, fan-out) so we can validate that the
reference-machine numbers hold in the wild. This is opt-in and privacy-respecting (see
`17-security-and-secrets.md`).

---

## 10. Open questions

These reference pending decisions from `docs/PLAN-CONTEXT.md` §"Open questions" and are **not**
decided here:

1. **Stack (Rust + GPUI vs Electron+React).** This entire doc assumes Rust + GPUI per the
   approved architecture. If the stack decision flips, every target in §1 and the rationale in §2
   must be revisited — the targets would likely be unachievable on Electron.
2. **Concurrency defaults.** The global max-concurrent-children default (64) and per-thread
   default (16) are proposals; the right numbers depend on real resource-monitor data and should
   be tuned, not assumed (see `06-orchestration-engine.md` §10). The **48-concurrent-subagent
   hard gate** (M5) is fixed regardless of these defaults.
3. **grok-build vendoring depth.** ~~The scheduler's ability to raise the built-in 16-child cap
   depends on how deeply we fork the vendored `spawn_subagent`/workflow code
   (`03-vendored-grok-build.md`). This affects the fan-out ceiling we can guarantee.~~ **Resolved
   by D11:** Multiplexer owns subagent scheduling and forks the vendored
   `spawn_subagent`/workflow code as needed to raise the cap. The vendored 16-child cap is the
   floor; our global cap is the ceiling, and the 48-subagent hard gate is enforced by our
   scheduler. The remaining question is only *how deeply* we fork, not *whether* we can raise
   the cap.
4. **Stream ack granularity.** Window-based ack vs per-event ack for the wire contract is
   proposed as window-based (`04-wire-contract.md` §10); the choice affects backpressure
   behavior and should be confirmed against these performance targets.
5. **Editor scope in MVP.** Whether the full native editor ships in MVP or a lighter editor first
   affects the input-latency and memory budgets we must hold in the first release
   (`09-editor.md`).
6. **Reference machine for benchmarks.** ~~The exact CI runner class / pinned reference machine for
   the perf gates is not yet fixed; it must be representative of a typical Windows desktop.~~
   **Resolved by M5:** the reference machine class is a typical Windows desktop (mid-range
   x86-64, 8–16 cores, 16–32 GB RAM, Windows 10/11), pinned in CI (§9.1). The remaining detail
   is the exact pinned runner, which is a CI-configuration choice, not an open design decision.
7. **Field telemetry scope.** Whether opt-in field telemetry ships in MVP affects how well we can
   validate the reference-machine numbers in the wild.

---

## 11. Consistency note

This document is consistent with `docs/PLAN-CONTEXT.md`, `docs/DECISIONS.md` (D22, M5, M6),
and the related plan docs (`02-architecture.md` §8 performance architecture,
`04-wire-contract.md` §8 backpressure, `06-orchestration-engine.md` §3 parallel scheduler).
No conflicts found. If any of the open questions above are decided differently, the affected
sections (§2, §5, §6, §9) must be revisited.
