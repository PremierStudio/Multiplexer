# 06 — Orchestration Engine

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Orchestration / Core runtime
**Depends on:** `02-architecture.md`, `03-vendored-grok-build.md`, `04-wire-contract.md`, `05-provider-adapter-layer.md`
**Feeds:** `07-checkpointing-and-vcs.md`, `13-mobile-app.md`, `15-testing-strategy.md`, `16-performance.md`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context). Where a decision is not yet settled, it is listed under **Open questions** and is
**not** decided unilaterally here.

**Locked decisions applied (D11, D12, D16, D18):** This doc reflects the locked decisions from
`docs/DECISIONS.md`:
- **D16** — canonical event vocabulary (plan/05 names: `TurnFinished`, `ToolCallFinished`,
  `PermissionRequested`, `TextDelta`).
- **D12** — 4-way approval decision enum (`allow`/`deny`/`allow_once`/`allow_always`).
- **D11** — Multiplexer owns subagent scheduling; we fork the vendored `spawn_subagent`/workflow
  code to raise the 16-child cap (we do NOT inherit it for free).
- **D18** — provider-runtime ingestion uses a **bounded** channel with backpressure.

---

## 1. Purpose

The orchestration engine is the heart of the Multiplexer server runtime. It owns every agent
session, every subagent, every turn, every tool call, every checkpoint, and every HAR capture.
Its job is to make **concurrency safe, deterministic, observable, and recoverable** — so that a
single native Rust binary can drive *dozens of concurrent subagents* (a core differentiator,
PLAN-CONTEXT §"Subagent orchestration at scale") without the serialization bottleneck that
limits T3 Code.

The engine is **event-sourced**: nothing is authoritative except an append-only command/event
log. Every observable state (threads, turns, messages, tool calls, checkpoints, subagent status,
HAR refs) is a *projection* of that log into a SQLite read model, computed by a **pure
`decider` + `projector`** pair inside a single transaction. This gives us:

- **Determinism** — the same command sequence always produces the same read model.
- **Testability** — the decider and projector are pure functions with no I/O; unit- and
  property-testable in isolation.
- **Recovery** — on restart we replay the log; the read model can never durably disagree with it.
- **Observability** — the live orchestration dashboard is just another projection of the log.

---

## 2. Event-sourced orchestration design

### 2.1 The three-layer pipeline

Every mutation flows through a strict pipeline, per thread:

```
Command (from client / scheduler / worker)
        │
        ▼
┌─────────────────┐   ┌──────────────────┐   ┌────────────────────────────┐
│  Command Queue  │──▶│  pure decider     │──▶│  projector → SQLite        │
│  (serialized)   │   │  (Command →       │   │  (Event → read-model rows) │
└─────────────────┘   │   Vec<Event>)     │   └────────────────────────────┘
                      └──────────────────┘            │
                                                      ▼
                                          side effects (I/O) dispatched
                                          to drainable workers, never in
                                          the transaction
```

The three layers are:

1. **Command queue** — a serialized FIFO of `Command`s for a given thread. Only one command for
   a thread is being *decided* at a time. This is the single point of serialization, and it is
   deliberately *per-thread*, not global (see §3).
2. **Decider** — a **pure** function `decide(thread_state, command) -> Vec<Event>`. It reads the
   current thread state (from the read model), validates the command against the state machine,
   and emits zero or more events. It performs **no I/O** — no network, no filesystem, no agent
   calls. It only *decides what happened*.
3. **Projector** — a **pure** function `project(tx, events) -> ()` that applies each event to the
   SQLite read model inside the same transaction that appends the events to the log.

### 2.2 One transaction, atomic append + project

The critical invariant: **the event log and the read model are updated in the same SQLite
transaction.** We never append an event without projecting it, and never project without
appending. Concretely:

```rust
pub fn apply(&self, thread_id: ThreadId, cmd: Command) -> Result<Vec<Event>, EngineError> {
    let mut tx = self.db.begin()?;

    // 1. Load current thread state (read model) for the decider.
    let state = self.read_model.load_thread(&tx, thread_id)?;

    // 2. Pure decision: Command -> Vec<Event>.
    let events = decide(&state, cmd)?;

    // 3. Append events to the log AND project them, atomically.
    for ev in &events {
        self.log.append(&tx, thread_id, ev)?;   // event log rows
        project(&tx, &state, ev)?;              // read-model rows
    }

    tx.commit()?;
    Ok(events)
}
```

Because the append and the projection commit together, the read model **cannot durably disagree
with the log**. If the process crashes mid-transaction, SQLite rolls back both; on restart we
replay from the last committed sequence number and both are consistent.

### 2.3 Why this is deterministic

- The decider and projector are **pure**: given the same `(state, command)` they return the same
  `events`, and given the same `(state, event)` they write the same rows. No wall-clock, no
  randomness, no ambient I/O.
- The command queue is **serialized per thread**, so there is a total order of commands per
  thread. There is no interleaving ambiguity.
- All nondeterminism (network latency, model output, tool results) is captured **as data** in
  events (e.g. `TurnFinished { output }`), never as control flow inside the decider. The decider
  only reacts to facts that have already been recorded.

This makes the engine **unit-testable** (feed a command, assert the events and read-model rows),
**property-testable** (proptest over arbitrary command sequences, assert invariants), and
**replayable** (re-run the log through the projector and get an identical read model).

---

## 3. Parallel scheduler

### 3.1 The T3 Code bottleneck we fix

T3 Code uses a **single serialized command queue** for everything. Every turn, every subagent,
every tool call funnels through one queue, so a long-running model turn or a slow subagent blocks
unrelated work. That is a hard ceiling on concurrency and directly contradicts our performance
target of "dozens of concurrent subagents without serialization bottleneck."

Multiplexer fixes this with a **two-tier model**:

1. **Serialized per-thread queues** — each thread (session) has its own command queue. Commands
   *within* a thread are strictly ordered (required for correctness of that session's state
   machine), but threads are **independent** and never block each other.
2. **A parallel scheduler** for cross-thread and subagent work — work that is not bound to a
   single thread's state machine (spawning subagents, dispatching independent tool calls,
   fan-out panels) is scheduled onto a shared pool and runs concurrently.

### 3.2 Concurrency model

```
                    ┌─────────────────────────────────────────────┐
                    │            Parallel Scheduler               │
                    │  (tokio runtime, N worker tasks, budget)    │
                    └─────────────────────────────────────────────┘
                        ▲ spawn / dispatch        │ results
                        │                          ▼
  ┌──────────────┐   ┌──────────────┐   ┌──────────────────────┐
  │ Thread A     │   │ Thread B     │   │ Subagent pool        │
  │ cmd queue    │   │ cmd queue    │   │ (spawn_subagent,     │
  │ (serialized) │   │ (serialized) │   │  Rhai workflows)     │
  └──────────────┘   └──────────────┘   └──────────────────────┘
```

- **Threads** are independent state machines. Each owns a serialized command queue and a
  `tokio::task` that drains it. Dozens of threads run concurrently.
- **Subagents** are scheduled by the parallel scheduler, not by any single thread's queue. A
  parent thread emits a `SpawnSubagent` command; the decider records the intent; the scheduler
  actually launches the child and streams its events back into the parent's log.
- **Concurrency cap:** the scheduler enforces a global **max concurrent children** budget
  (default 16, configurable). Additional fan-out is queued and still acts as a barrier for the
  calling workflow (see §4). **Multiplexer owns this cap (D11):** we do NOT inherit grok-build's
  built-in 16-child limit "for free." Raising it requires forking the vendored
  `spawn_subagent`/workflow code (see §4.1); the default of 16 is our own starting budget, not a
  ceiling inherited from upstream.

### 3.3 Scheduling sketch

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

The scheduler is the only component that talks to the embedded grok-build subagent runtime
(`spawn_subagent` + Rhai workflows) and to provider adapters for cross-thread work. It never
touches the read model directly — it emits events, which the owning thread's decider/projector
consume.

---

## 4. Subagent fan-out

### 4.1 Two layers of orchestration

Multiplexer has **two** orchestration layers that compose:

1. **Embedded grok-build orchestration** (vendored, PLAN-CONTEXT §"Key facts about
   grok-build"): `spawn_subagent` (depth 1) and Rhai workflows (`agent()`, `parallel()`,
   `phase()`, budget caps, max 16 concurrent children). We reuse this as a library — it is the
   *mechanism* for running a single subagent or a scripted workflow.
2. **Our orchestration layer** — the event-sourced engine + parallel scheduler described here.
   This is the *control surface*: it tracks every subagent as first-class state in the read
   model, applies our budget caps, and feeds the live orchestration dashboard.

The two compose: a Rhai workflow runs *inside* a thread; its `parallel()` panels are scheduled by
our scheduler; each `agent()` call is a subagent tracked in the read model.

**We own subagent scheduling (D11).** Multiplexer does NOT inherit grok-build's built-in cap of
16 concurrent children "for free." To raise it, we **fork the vendored `spawn_subagent`/workflow
code** (in `third_party/grok-build`, per `03-vendored-grok-build.md`) and implement our own
parallel scheduler on top. The vendored cap is a starting point we must actively raise, not a
limit we inherit. We track upstream's fan-out changes (notably 1.0.1 "bounded fan-out") closely
and reconcile our fork with upstream's approach as it evolves.

### 4.2 Budget caps

- **Per-thread cap:** max concurrent children per parent thread (default 16).
- **Global cap:** max concurrent children across all threads (default 64, configurable). This is
  the "dozens of concurrent subagents" target.
- **Cumulative budget:** a workflow may declare a total child-agent budget (like grok-build's
  `agent_budget`, default 128). A `parallel()` panel that would exceed the remaining budget is
  rejected before any of its children launch.
- **Backpressure:** when a cap is hit, new spawns are queued (not dropped) and act as a barrier
  for the calling workflow, exactly as grok-build's panels do.

### 4.3 Live orchestration dashboard

Every subagent lifecycle transition is an event (`SubagentSpawned`, `SubagentStarted`,
`SubagentEvent`, `SubagentCompleted`, `SubagentFailed`, `SubagentCancelled`). The dashboard is a
read-model projection: a tree of parent → children, with status, budget consumed, and live event
stream. Because it is a projection, it is **free** — no separate bookkeeping, and it survives
restarts. The dashboard is served to desktop and mobile clients over the same JSON-RPC/WebSocket
contract (see `04-wire-contract.md`).

---

## 5. The command/event model

### 5.1 Command types (input to the decider)

Commands are the *intent* — what a client, scheduler, or worker asks the engine to do. They are
serialized per thread.

| Command | Payload (sketch) | Meaning |
|---|---|---|
| `StartSession` | `session_id, provider, model, worktree, config` | Create a thread + session |
| `SendTurn` | `thread_id, user_message, attachments` | Submit a user turn to the model |
| `Interrupt` | `thread_id` | Cancel the in-flight turn |
| `ApprovalRespond` | `thread_id, approval_id, decision: ApprovalDecision, reason` | Answer a tool-approval prompt; `decision` is the **4-way enum** `allow`/`deny`/`allow_once`/`allow_always` (D12) |
| `UserInputRespond` | `thread_id, prompt_id, text` | Answer an interactive prompt |
| `SpawnSubagent` | `parent_thread, spec, budget` | Fan out a child agent |
| `SubagentEvent` | `subagent_id, event` | Child reports progress/result |
| `ToolResult` | `thread_id, tool_call_id, result` | Tool execution outcome |
| `CheckpointRevert` | `thread_id, checkpoint_ref` | Revert to a checkpoint |
| `SessionStop` | `thread_id` | Stop the session |
| `HarCapture` | `thread_id, har_ref, payload` | Record a HAR capture |

### 5.2 Event types (output of the decider, appended to the log)

Events are the *facts* — what actually happened. They are the only thing the projector reads.

| Event | Produced by | Read-model effect |
|---|---|---|
| `SessionStarted` | `StartSession` | create thread/session rows |
| `TurnStarted` | `SendTurn` | create turn row, status `running` |
| `TurnFinished` | `SendTurn` (async) | turn row → `completed`, message rows |
| `TurnFailed` | `SendTurn` (async) | turn row → `failed`, error |
| `TurnInterrupted` | `Interrupt` | turn row → `interrupted` |
| `TextDelta` | `SendTurn` / `TurnFinished` | message row |
| `ToolCallStarted` | `SendTurn` (async) | tool-call row, status `running` |
| `ToolCallFinished` | `ToolResult` | tool-call row → `completed`, result |
| `PermissionRequested` | `SendTurn` (async) | approval row, status `pending` |
| `PermissionResolved` | `ApprovalRespond` | approval row → `allow`/`deny`/`allow_once`/`allow_always` |
| `SubagentSpawned` | `SpawnSubagent` | subagent row, status `queued` |
| `SubagentStarted` | scheduler | subagent row → `running` |
| `SubagentEvent` | `SubagentEvent` | append to subagent event stream |
| `SubagentCompleted` | scheduler | subagent row → `completed` |
| `SubagentFailed` | scheduler | subagent row → `failed` |
| `SubagentCancelled` | `Interrupt` | subagent row → `cancelled` |
| `CheckpointCreated` | `SendTurn` (async) | checkpoint row + git ref |
| `CheckpointReverted` | `CheckpointRevert` | checkpoint row, thread state |
| `HarCaptured` | `HarCapture` | HAR ref row |
| `SessionStopped` | `SessionStop` | session row → `stopped` |

**Async events:** the decider is pure and synchronous, so events that depend on external results
(e.g. `TurnFinished`) are *not* produced directly by the decider from `SendTurn`. Instead the
decider emits `TurnStarted`, and the **provider runtime worker** (see §6) later feeds a
`TurnFinished`-producing command (`SendTurn` completion) back through the queue. This keeps the
decider pure while still capturing every external fact as an event.

**Canonical event vocabulary (D16).** The engine events above use the canonical plan/05 names
(`TurnFinished`, `ToolCallFinished`, `PermissionRequested`, `TextDelta`). Two engine-local
events have no plan/05 counterpart and are kept as engine-specific names; they map onto the
canonical set as follows:

| Engine event (this doc) | Canonical plan/05 name | Notes |
|---|---|---|
| `TurnFinished` | `TurnFinished` | canonical |
| `ToolCallFinished` | `ToolCallFinished` | canonical |
| `PermissionRequested` | `PermissionRequested` | canonical |
| `TextDelta` | `TextDelta` | canonical |
| `PermissionResolved` | — (no canonical counterpart) | engine-local; the resolution of a `PermissionRequested`; carries the 4-way decision (D12) |
| `TurnStarted` / `TurnFailed` / `TurnInterrupted` | — (no canonical counterpart) | engine-local lifecycle events; the canonical set only names the terminal `TurnFinished` |

The `PermissionResolved` event is the engine's projection of the `ApprovalRespond` command's
4-way decision (D12); it is not a wire/ProviderEvent and needs no canonical alias.

---

## 6. Drainable workers

Side effects (I/O) never happen inside the transaction. They are dispatched to **queue-backed
workers** that consume work items and, when done, feed results back as commands/events. This
decoupling is what makes the engine testable: in tests we drain the workers to empty and assert
on the resulting read model.

### 6.1 The workers

| Worker | Consumes | Produces (back into the log) |
|---|---|---|
| **Provider runtime ingestion** | provider `ProviderEvent` stream (from `05-provider-adapter-layer.md`) | `TurnFinished`, `TurnFailed`, `ToolCallStarted`, `PermissionRequested`, `TextDelta`, … |
| **Command reactor** | commands that need I/O (agent calls, tool execution, subagent spawn) | `ToolResult`, `SubagentEvent`, `SubagentCompleted`, … |
| **Checkpoint reactor** | `CheckpointCreated` intents | `CheckpointCreated` (with git ref), `CheckpointReverted` |
| **HAR reactor** | `HarCapture` intents | `HarCaptured` (with ref) |

Each worker is a `tokio` task with an internal queue. It is **drainable**: it exposes a
`drain()` that processes all currently queued items to completion (or to a barrier) and returns
when the queue is empty.

**Bounded ingestion with backpressure (D18).** The **provider runtime ingestion** worker is the
bounding point for the provider event channel: it consumes a **bounded** channel (NOT
`mpsc::UnboundedReceiver`), consistent with plan/04's window-based flow control and plan/05's
bounded adapter channel. When the bounded queue is full, the provider adapter applies
backpressure (its `send` awaits a slot) rather than dropping or unboundedly buffering events.
This prevents a fast provider from unboundedly growing memory and keeps the engine's
drain-to-empty semantics well-defined.

### 6.2 Deterministic draining in tests

```rust
// Integration test: drive the engine, then drain all workers and assert.
engine.send(StartSession { .. }).await?;
engine.send(SendTurn { .. }).await?;

// Pump the provider runtime + command reactor until quiescent.
engine.drain_all().await?;   // drains provider, reactor, checkpoint, HAR

let state = engine.read_model().thread(&id)?;
assert_eq!(state.turn.status, TurnStatus::Completed);
```

`drain_all()` runs the workers' queues to empty (bounded by a test timeout to catch infinite
loops). Because all nondeterminism is captured as events and all I/O is behind drainable queues,
a test can reach a **fully deterministic quiescent state** and assert on the read model without
sleeping or polling.

---

## 7. Determinism & testability

### 7.1 Pure decider + projector

The decider and projector are the only code that mutates authoritative state, and they are pure.
This is the foundation of every testing strategy in `15-testing-strategy.md`:

- **Unit tests** feed a `(state, command)` and assert the exact `Vec<Event>` and the exact
  read-model rows. No mocks, no I/O, no timing.
- **Property tests (proptest)** generate arbitrary command sequences and assert invariants that
  must hold for *any* sequence (see below).
- **Replay tests** re-run a recorded log through the projector and assert the read model is
  byte-identical to the original.

### 7.2 Invariants for proptest

The engine must satisfy these invariants under arbitrary command sequences:

1. **Log/read-model agreement:** after every command, replaying the log through the projector
   reproduces the read model exactly.
2. **State-machine validity:** no event is ever projected from an invalid state (e.g. no
   `TurnFinished` without a `TurnStarted`; no `SubagentCompleted` for an unknown subagent).
3. **Idempotent recovery:** replaying the log from an empty DB after a simulated crash yields the
   same read model as the live run.
4. **No lost updates:** concurrent threads never corrupt each other's state (each thread's log is
   independent).
5. **Budget invariants:** cumulative child-agent budget is never exceeded; a rejected `parallel()`
   panel emits no child events.

Proptest generates `Vec<Command>` (with a valid thread-id generator and a state-machine-aware
command generator so we also test *invalid* commands are rejected cleanly), runs them through the
engine, and checks the invariants.

---

## 8. Persistence & recovery

### 8.1 What is persisted

- **Event log** — append-only rows `(seq, thread_id, event_type, payload, timestamp)`. This is
  the source of truth.
- **Read model** — SQLite tables (threads, turns, messages, tool_calls, approvals, subagents,
  checkpoints, har_refs). A projection, always consistent with the log (same transaction).
- **Checkpoints** — hidden Git refs per turn (see `07-checkpointing-and-vcs.md`); the refs are
  recorded in the read model.

### 8.2 Crash / restart recovery

On startup the engine:

1. Opens the SQLite DB (WAL mode for crash safety + concurrent readers).
2. Reads the last committed sequence number.
3. Replays any log rows after the last committed projection point through the projector (normally
   none, because append+project are atomic — this is a safety net for a partially-written WAL).
4. Marks all `running` turns/subagents as `interrupted`/`failed` (they died with the process) and
   emits recovery events so the dashboard reflects reality.
5. Re-enqueues any durable work items that were in flight.

Because the log is the source of truth and the read model is a pure projection, recovery is
**deterministic**: the same log always yields the same read model, regardless of how many times
the process crashed.

### 8.3 Storage layout

```
<data_dir>/
  multiplexer.db          # SQLite: event log + read model (WAL)
  worktrees/              # per-session worktrees (see 07)
  checkpoints/            # hidden git refs
  har/                    # captured HAR files, referenced by ref
```

---

## 9. Testing

All testing follows the TDD-at-inception mandate (PLAN-CONTEXT §Testing) and the CI gate order:
fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e →
coverage.

### 9.1 Unit tests (decider / projector)

Co-located `#[cfg(test)]` modules. For each command type, test:
- **Happy path:** `decide(state, cmd)` returns the expected events; `project` writes the expected
  rows.
- **Invalid transitions:** the decider rejects commands that violate the state machine with a
  typed `EngineError`, and emits **no** events (so nothing is projected).
- **Projector idempotence:** projecting the same event twice is impossible by construction (log
  append is unique), but we test that a replayed log yields identical rows.

### 9.2 Property tests (proptest)

As in §7.2 — arbitrary command sequences against the five invariants. This is where the "pure
decider/projector" design pays off: proptest can run thousands of sequences in milliseconds with
no I/O.

### 9.3 Integration tests (real core + mock agent)

- **Real core, mock agent:** the engine runs against a fake provider adapter / fake `grok agent
  stdio` (per PLAN-CONTEXT §Testing: "real core + mock ACP agent"). We drive real commands through
  the engine, let the mock agent produce `ProviderEvent`s, and assert on the **read model**.
- **The "drain to empty" pattern:** after each scenario, call `engine.drain_all()` and assert the
  read model reached the expected quiescent state — no sleeps, no polling.
- **Concurrency tests:** spawn dozens of subagents across several threads, assert the global and
  per-thread caps are respected, and assert no cross-thread corruption.
- **Recovery tests:** simulate a crash mid-transaction (drop the DB handle without commit),
  restart, and assert the read model matches the log.
- **Real-binary smoke tests** when a real `grok` binary is available (CI-optional).

### 9.4 Mutation testing

cargo-mutants over the decider/projector/scheduler. CI gates: ≥85% line, ≥80% branch, ≥70%
mutation score killed. The pure decider/projector are prime mutation targets — a killed mutant
here means a real correctness regression is caught.

---

## 10. Open questions

These reference pending decisions from `docs/PLAN-CONTEXT.md` §"Open questions" and are **not**
decided here:

1. **MVP scope (Grok-only vs multi-provider):** the engine is provider-agnostic by design (via
   `05-provider-adapter-layer.md`), but whether the MVP wires only Grok or all adapters affects
   how many `ProviderEvent` shapes the ingestion worker must handle day one.
2. **Concurrency defaults:** the global max-concurrent-children default (64) and per-thread
   default (16) are proposals; the right numbers depend on real resource-monitor data
   (`16-performance.md`) and should be tuned, not assumed.
3. **grok-build vendoring form (submodule vs vendored copy vs `[patch]`):** the scheduler's
   ability to raise the built-in 16-child cap depends on how deeply we fork the vendored
   `spawn_subagent`/workflow code (`03-vendored-grok-build.md`).
4. **Dashboard scope:** whether the live orchestration dashboard ships in MVP or is a
   post-MVP surface affects how much subagent state the read model must expose early.
5. **Recovery semantics for in-flight turns:** whether an interrupted turn is auto-resumed or
   surfaced for the user to re-run is a product decision, not an engine one.
