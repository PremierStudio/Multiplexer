# 15 — Testing Strategy (TDD at Inception)

> **Status:** Authoritative plan doc. Consistent with `docs/PLAN-CONTEXT.md` (the shared plan context). If any statement here conflicts with PLAN-CONTEXT, PLAN-CONTEXT wins and the conflict should be flagged in `plan/20-risks-and-open-questions.md`.
>
> **Purpose:** Define the full testing strategy — the test pyramid, the tooling, the CI gates, and what "deep assertions" means — such that Multiplexer ships with TDD at inception and a mutation-testing gate that **no competitor enforces**. This doc is the input to the roadmap in `plan/19-roadmap-and-milestones.md` and the risk register in `plan/20-risks-and-open-questions.md`.

> **Locked decisions applied:** This doc reflects the following LOCKED decisions from `docs/DECISIONS.md` — **D21** (mutation scope covers all core logic, including terminal/editor/browser/pane-system), **D22** (dedicated performance stage in CI enforcing plan/16's hard gates), **D32** (e2e cadence = merge gate for critical paths + nightly for the full suite, no skip path), **D33** (70% mutation score is the merge floor, bar may rise over time). These supersede the open questions below.

---

## 1. The Mandate

TDD at inception is **differentiator #10** and a **non-negotiable** design principle (see `plan/00` §2, §4). It is not a retrofit bolted on after the product works — it is the way we build from the first commit. The mandate has four legs:

1. **Full unit + mutation tests.** Every module ships with co-located `#[cfg(test)]` unit tests, and the mutation-testing gate proves those tests actually catch real faults (see §3.3).
2. **Full component tests.** The GPUI UI is tested at the component level — panes render, lay out, and respond to interaction — not just "the app launches."
3. **Full integration tests.** The real core is driven end-to-end against a mock ACP agent, asserting on the **read model**, not on incidental return values.
4. **Deep assertions.** Tests assert on the *observable state of the system* — the read model, the event stream, and invariants — rather than shallow return-value checks.

### 1.1 Why this is a differentiator

No competitor enforces a mutation-testing gate in CI:

| Product | Unit tests | Mutation gate | Component tests | E2E |
|---|---|---|---|---|
| **Multiplexer** | ✅ full | ✅ **≥70% killed** | ✅ GPUI components | ✅ real/headless |
| Orca | partial | ❌ | partial | ❌ |
| T3 Code | partial | ❌ | partial | ❌ (explicit gap) |
| Superset | partial | ❌ | partial | ❌ |
| Conductor | closed | ❌ | unknown | ❌ |

Mutation testing is the difference between "we have tests" and "our tests would catch a real bug." A test suite that passes but fails to kill mutations is a false sense of security. Multiplexer treats the mutation score as a **first-class CI gate**, which is the strongest quality signal in the competitive set.

The **≥70% mutation score is the merge floor** — the minimum required to merge — not a ceiling or a target to hit once and forget. The bar may be raised over time as the suite matures (see §2.3).

### 1.2 The user's explicit ask

This is the user's explicit requirement, restated verbatim from PLAN-CONTEXT §Testing:

> **Unit:** co-located `#[cfg(test)]`; **property-based** with proptest for state machines/deciders/projectors/serializers.
> **Mutation:** cargo-mutants; CI gates: ≥85% line, ≥80% branch, ≥70% mutation score killed.
> **Integration:** real core + mock ACP agent (fake `grok agent stdio`); assert on read model; real-binary smoke tests when available.
> **Contract:** JSON-RPC wire contract schema-verified on both sides.
> **Component (GPUI):** element/component tests, snapshot tests for pane layouts.
> **E2E:** drive the real app/headless — this beats T3 Code (no e2e).
> **Mobile:** native unit + integration against shared contract; mock server for offline determinism.
> **CI gates:** fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e → coverage. All green before merge. No blind CI.

This document operationalizes every line of that mandate.

---

## 2. Test Pyramid (Rust Core)

The Rust core is the heart of the product — it owns the agent runtime, orchestration, terminal, git, filesystem, checkpoints, and HAR. It must be the most heavily tested layer. The pyramid, bottom to top:

```
        e2e (few, slow, real binary)
      component (GPUI panes)
    integration (real core + mock agent)
  contract (JSON-RPC wire, both sides)
unit + property + mutation (many, fast, per-module)
```

The rule of thumb: **the lower the layer, the more tests and the faster they run.** Unit/property tests number in the thousands and run in milliseconds; e2e tests number in the dozens and run in minutes. Every layer is mandatory — a pyramid with a fat top is a red flag.

### 2.1 Unit tests — co-located `#[cfg(test)]`

Every module carries its tests beside the code it tests, in the idiomatic Rust style:

```rust
// src/orchestration/decider.rs
pub fn decide(state: &ThreadState, cmd: Command) -> Vec<Effect> { /* ... */ }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupt_while_idle_is_noop() {
        let state = ThreadState::idle();
        let effects = decide(&state, Command::Interrupt);
        assert!(effects.is_empty());
    }
}
```

**Conventions:**

- Tests live in `#[cfg(test)] mod tests` at the bottom of each module file — no separate `tests/` tree for unit tests (integration tests use `tests/`).
- Every public function and every non-trivial private function has at least one test.
- Test names are sentences: `interrupt_while_idle_is_noop`, `projector_creates_thread_on_first_turn`.
- No test depends on wall-clock time, ambient environment, or network. All I/O is injected (see §7).

### 2.2 Property-based testing with proptest

For **state machines, deciders, projectors, and serializers**, hand-written example tests are not enough — the state space is too large. We use **proptest** to generate thousands of inputs and assert **invariants** that must hold for *all* of them.

**Where property tests are mandatory:**

| Component | Property under test |
|---|---|
| **Thread state machine** | From any reachable state, any legal command transitions to a legal state; illegal commands are rejected, never panic |
| **Decider** | For any command + state, the returned effects are a subset of the legal effect set; effects never reference unknown thread ids |
| **Projector** | Projecting an event stream is **idempotent** — replaying the same stream yields the same read model; reordering is not allowed to corrupt invariants |
| **Serializers** | `serialize` then `deserialize` is the identity for any value (round-trip); JSON-RPC params/envelopes survive the wire |
| **Command queue** | The serialized command queue is a **total order** — no command is lost, duplicated, or reordered; drain-to-empty terminates |
| **Checkpoint diff** | Diff queries over hidden git refs are consistent with the underlying commit graph |

**Example — projector idempotency:**

```rust
proptest! {
    #[test]
    fn projector_is_idempotent(events in any::<Vec<Event>>()) {
        let mut m1 = ReadModel::default();
        let mut m2 = ReadModel::default();
        for e in &events { m1.project(e.clone()); }
        for e in &events { m2.project(e.clone()); }
        // Replaying the same stream twice must not change the model.
        for e in &events { m2.project(e.clone()); }
        assert_eq!(m1, m2);
    }
}
```

**Example — serializer round-trip:**

```rust
proptest! {
    #[test]
    fn envelope_round_trips(msg in any::<Envelope>()) {
        let wire = serde_json::to_vec(&msg).unwrap();
        let back: Envelope = serde_json::from_slice(&wire).unwrap();
        assert_eq!(msg, back);
    }
}
```

Property tests are seeded and reproducible: a failing case is minimized by proptest and persisted as a regression example so the failure is deterministic on re-run.

### 2.3 Mutation testing with cargo-mutants

**Mutation testing is the core differentiator.** It works by automatically introducing small faults ("mutants") into the code — flipping a comparison, deleting a branch, replacing `+` with `-`, removing a method body — and running the test suite against each mutant. A mutant that the tests **fail to catch** is a "survived" mutant, meaning our tests would not have caught that class of bug.

```
Original:   if x > 0 { return A } else { return B }
Mutant:     if x >= 0 { return A } else { return B }   // operator flip
Test run:   does any test fail?  →  if no, the mutant SURVIVED (bad)
```

**CI gates (from PLAN-CONTEXT, non-negotiable):**

| Gate | Threshold |
|---|---|
| Line coverage | **≥ 85%** |
| Branch coverage | **≥ 80%** |
| **Mutation score killed** | **≥ 70%** |

**Workflow:**

```bash
# Local dev loop (same as CI — no blind CI)
cargo mutants --in-place --timeout 30        # run mutation testing
# cargo-mutants writes a report; inspect survived mutants
cargo mutants --list                       # list all mutants
cargo test --test <name> <mutant-id>       # reproduce a specific survivor
```

**Operational rules:**

- The mutation gate applies to **all core logic across all subsystems** — orchestration, provider-adapter, wire contract, checkpointing, HAR, **and the terminal (PTY, scrollback, backpressure), editor (buffer, diff-apply, undo, selection), browser (detection, launch, port-parsing, security controls), and pane system (layout engine)**. This is the full core; there is no carve-out for any subsystem. UI-only rendering code is covered by component tests instead.
- **Survived mutants are a code smell, not just a test gap.** When a mutant survives, the first question is "is this dead code / an unreachable branch?" and the second is "is there a missing test?" Both are fixed — never silence a survivor by excluding it from the run without a written justification in the PR.
- Mutation runs are **slow** (each mutant re-runs the suite). We keep the unit+property suite fast (§7) so the mutation gate stays tractable in CI. Incremental mutation (`--in-place`) limits blast radius to changed files on PRs.
- The 70% kill threshold is the **merge floor**, not a target. New core modules must reach it before merge; the bar may be raised over time.

### 2.4 Integration tests — real core + mock ACP agent

Integration tests exercise the **real core** (the vendored grok-build runtime, orchestration engine, projector, read model) driven end-to-end, but with a **mock ACP agent** standing in for the real model. This gives us full determinism and speed without burning tokens or depending on a live model.

**The mock agent** is a fake `grok agent stdio` we write for tests. It speaks the same ACP wire protocol on stdin/stdout as the real binary, but instead of calling a model it follows a scripted, deterministic behavior:

```rust
// tests/mock_agent.rs — a fake `grok agent stdio`
// Reads ACP JSON-RPC from stdin, emits scripted responses:
//   - on session/start → session/started
//   - on turn with "edit file X" → x.ai/fs/write + turn/complete
//   - on turn with "run tests" → x.ai/terminal/exec + turn/complete
//   - on interrupt → turn/interrupted
```

**What integration tests assert on — the read model, not return values:**

```rust
#[tokio::test]
async fn full_session_updates_read_model() {
    let core = Core::builder()
        .with_agent(MockAgent::scripted("edit-and-test"))
        .with_sqlite_in_memory()
        .build();

    let thread = core.start_session(ThreadConfig::default()).await.unwrap();
    core.send_turn(thread.id, "add a test for the decider").await.unwrap();

    // DRAIN: wait until the command queue is empty and the projector has caught up.
    core.drain().await;

    // DEEP ASSERTION: assert on the read model, not on the return value of send_turn.
    let model = core.read_model(thread.id).await;
    assert_eq!(model.turns.len(), 1);
    assert_eq!(model.turns[0].status, TurnStatus::Completed);
    assert!(model.turns[0].events.iter().any(|e| matches!(e, Event::FileWritten { .. })));
    assert_eq!(model.thread.status, ThreadStatus::Idle);
}
```

**Covered scenarios:**

- Full session lifecycle: start → turn → tool calls → complete → stop.
- Interrupt mid-turn: the mock agent emits `turn/interrupted`, the read model reflects it, and the command queue recovers.
- Approval flow: `approval_respond` routes to the mock agent and the turn resumes.
- Checkpoint revert: a turn writes files, we revert to the hidden git ref, and the read model + filesystem agree.
- Subagent fan-out: a parent turn spawns subagents; the scheduler runs them in parallel and the read model records the hierarchy.
- Error paths: mock agent crashes mid-turn; the core marks the turn failed and the queue does not wedge.

### 2.5 Real-binary smoke tests

When a real `grok` binary is installed and available, we run a small set of **smoke tests** against it (guarded so CI without a binary skips them, never fails):

```rust
#[test]
#[ignore = "requires installed grok binary; run explicitly or in smoke CI job"]
fn real_grok_round_trip() { /* spawn real `grok agent stdio`, one short turn */ }
```

These are **not** part of the default gate (they need a model + tokens). They run in a dedicated, opt-in CI job and locally via `cargo test -- --ignored`. Their purpose is to catch drift between the mock agent's assumptions and the real binary's behavior — the mock is authoritative for the gate, the real binary is the ground truth we periodically reconcile against.

### 2.6 Contract tests — JSON-RPC wire, both sides

The JSON-RPC-over-WebSocket contract (see `plan/04-wire-contract.md`) is **schema-verified on both sides**. We define the wire schema once (JSON Schema) and generate/verify both the Rust server and every client against it.

- **Server side:** every request/response/notification is validated against the schema before dispatch and before send. A malformed message is rejected with a typed error, never a panic.
- **Client side (desktop, mobile, web):** the same schema is used to validate inbound messages and to type-check outbound calls.
- **Round-trip contract tests:** a shared fixture corpus of valid + invalid envelopes is run against both the Rust server and each client's serialization layer, asserting both accept the valid set and reject the invalid set identically.

```rust
// tests/contract.rs
#[test]
fn server_rejects_malformed_envelope() {
    let bad = br#"{"jsonrpc":"2.0","method":123}"#; // method must be a string
    let resp = server.handle_raw(bad).await;
    assert!(resp.is_err());
    assert!(matches!(resp.unwrap_err(), WireError::Schema { .. }));
}
```

The contract schema lives in a single source of truth (e.g. `schemas/wire.schema.json`) and is the artifact both sides build against — no hand-maintained duplicate types.

---

## 3. UI Tests (GPUI)

The GPUI UI is tested at three levels: **component tests**, **snapshot tests**, and **e2e**. This is where we beat T3 Code, which ships **no e2e tests at all**.

### 3.1 Component tests

GPUI provides an element/component testing harness. We render individual panes and assert on layout, state, and interaction — without launching the full app:

```rust
#[test]
fn chat_sidebar_renders_thread_list() {
    let cx = TestAppContext::new();
    let model = ReadModel::with_threads(vec![thread_a, thread_b]);
    let pane = ChatSidebar::new(model);
    pane.render(&mut cx);

    assert_eq!(cx.element_count("thread-row"), 2);
    assert!(cx.find("thread-row").text().contains("add a test"));
}

#[test]
fn clicking_thread_selects_it() {
    let cx = TestAppContext::new();
    let mut pane = ChatSidebar::new(model);
    pane.render(&mut cx);
    cx.click("thread-row[0]");
    assert_eq!(pane.selected_thread(), Some(thread_a.id));
}
```

**What component tests cover:**

- Every pane: chat sidebar, build pane, right bar (browser / HAR / files / diff / terminal / agent activity), pop-out terminal.
- Layout: panes render in the correct positions and sizes; split-anything behaves.
- State: selection, focus, hover, disabled states, empty states.
- Interaction: clicks, keyboard shortcuts, drag-to-resize, pop-out-to-window.
- The orchestration dashboard: subagent cards render, progress updates, and status transitions.

### 3.2 Snapshot tests for pane layouts

Pane layout is a core differentiator (pop-out panes, split-anything), so layout regressions are caught with **snapshot tests**. We render a pane tree and compare against a committed golden snapshot:

```rust
#[test]
fn default_layout_snapshot() {
    let cx = TestAppContext::new();
    let layout = Layout::default(); // chat | build | right-bar, terminal below
    let snapshot = render_layout(&layout, &mut cx);
    insta::assert_snapshot!("default_layout", snapshot);
}
```

- Snapshots are committed to the repo and reviewed in PRs (via `insta` review flow) — a layout change is a deliberate, visible diff, not a silent regression.
- We snapshot the **layout tree** (pane ids, sizes, splits, pop-out state) rather than raw pixels, so tests are fast and platform-independent. Pixel-level golden images are reserved for a small set of critical screens in e2e.

### 3.3 E2E — drive the real app / headless

E2E drives the **real application** — the actual binary — either in a window or headless, against the real core with a mock agent. This is the top of the pyramid and the layer T3 Code lacks entirely.

```bash
# Headless e2e: launch the real binary with a mock agent, drive it over the wire contract
cargo run --bin multiplexer -- --headless --mock-agent tests/mock_agent.rs
# then drive via the JSON-RPC WebSocket contract and assert on the read model
```

**E2E scenarios:**

- Cold start → usable editor under 300ms (also feeds `plan/16-performance.md`).
- Full user journey: open a worktree, start a session, watch the agent edit, apply a diff inline, comment on a diff line, route it back to the agent.
- Pop-out a pane to its own window and back.
- Browser pane: import a system browser, launch, drive via CDP, capture HAR.
- Subagent fan-out: launch a dozen subagents, watch the dashboard, interrupt one.
- Mobile pairing: a mobile client connects to the same server runtime and observes/steers a session.

E2E is the slowest layer, so it runs on a **merge gate** (critical paths) and **nightly** (full suite), not on every commit. The **critical-path subset** (cold start, full user journey, pop-out, browser/HAR, subagent fan-out, mobile pairing) is mandatory-green on the merge gate; the **full suite** runs nightly. There is **no "skip e2e for small changes" path** — e2e is part of the definition of done for every merge, and "no blind CI" means we run the same gates locally.

---

## 4. Mobile Tests

The mobile app (see `plan/13-mobile-app.md`) is a thin client over the shared JSON-RPC contract, so its tests are **native unit + integration against the shared contract**, with a **mock server** for offline determinism.

- **Native unit tests** (Swift XCTest / Kotlin JUnit) cover the client's own logic: view models, state mapping, local caching, offline queueing, and the contract serialization layer.
- **Contract conformance:** the mobile client runs the same shared fixture corpus as the desktop client (§2.6) — both must accept/reject the same wire messages. This keeps the two clients honest against one contract.
- **Integration against a mock server:** a local mock server (the same mock agent + core, or a lightweight contract stub) serves deterministic responses so tests are **offline and deterministic** — no network, no real model, no flaky CI.
- **Offline determinism:** the mobile client's offline behavior (queue commands, show cached read model, reconnect + reconcile) is tested against the mock server with injected disconnects.

Mobile tests run in the same CI pipeline (on macOS/Windows runners for the native toolchains) and gate the merge like every other layer.

---

## 5. CI Gates

The CI pipeline is a **strict, ordered gate**. Each stage must be green before the next runs, and **all must be green before merge**. This is the exact order from PLAN-CONTEXT:

```
1. fmt            (cargo fmt --check)
2. clippy         (cargo clippy -- -D warnings)   ← deny warnings
3. unit + property(cargo test --lib --bins)
4. mutation       (cargo mutants, ≥70% killed)
5. integration    (cargo test --test '*')
6. performance    (plan/16 hard gates: cold start <300ms, input latency <16ms p95, memory under budget, dozens of subagents)
7. component      (GPUI component + snapshot tests)
8. e2e            (real/headless app, merge gate: critical paths)
9. coverage       (cargo llvm-cov, ≥85% line / ≥80% branch)
```

**Performance stage.** A dedicated **performance stage** sits between integration and component (its own named stage). It enforces plan/16's hard gates — **cold start <300ms, input latency <16ms p95, memory under budget, and dozens of concurrent subagents** — so the performance budget has a concrete home in CI rather than living only in a design doc. See `plan/16-performance.md` for the measurement method and reference machine.

**Rules:**

- **No blind CI.** The dev loop runs the *same* gates locally before pushing. CI is a second confirmation, not the first place quality is checked. A PR that fails locally is never pushed.
- **Deny warnings.** `clippy -D warnings` is a hard gate — a single warning fails the build. This keeps the codebase clean and the mutation/coverage numbers meaningful.
- **Order matters.** fmt/clippy fail fast (cheap); mutation, performance, and e2e run late (expensive). A formatting error never wastes a mutation run.
- **Coverage gate** runs last because it needs the full test suite executed; it enforces ≥85% line / ≥80% branch on top of the mutation gate.
- **Merge requires all green.** E2E runs on the merge gate (critical paths) and nightly (full suite). There is **no "skip e2e for this small change" path** — if a change is too small to justify e2e, it is too small to merge without it; the gate is uniform.

### 5.1 Local dev loop (no blind CI)

```bash
# One command runs the full local gate, mirroring CI exactly:
pnpm sdlc:pre-push   # or: cargo xtask gate
#   fmt → clippy → unit+property → mutation → integration → performance → component → e2e → coverage
```

The local gate is the same script CI runs. This is the "no blind CI" guarantee: what CI enforces, the developer already ran and passed locally.

---

## 6. Deep Assertions — What "Deep" Means

"Deep assertions" is the discipline that separates a meaningful suite from a decorative one. A **shallow** test asserts on a return value; a **deep** test asserts on the *observable state of the system* and its *invariants*.

**Shallow (avoid):**

```rust
let result = send_turn(thread, "hello").await;
assert!(result.is_ok());   // only proves the call didn't error
```

**Deep (prefer):**

```rust
core.drain().await;
let model = core.read_model(thread).await;
assert_eq!(model.turns.len(), 1);
assert_eq!(model.turns[0].status, TurnStatus::Completed);
assert!(model.events.iter().any(|e| matches!(e, Event::TurnFinished { .. })));
assert_eq!(model.thread.status, ThreadStatus::Idle);
```

**The four dimensions of "deep":**

1. **Read model.** Assert on the projected read model — the durable, observable state — not on the return value of the triggering call. This catches projector bugs, not just handler bugs.
2. **Event streams.** Assert on the event stream itself: the right events were emitted, in the right order, with the right payloads. This is the source of truth for the read model, so testing it directly catches the deepest faults.
3. **Invariants.** Property-based tests assert invariants that must hold for *all* inputs — idempotency, total ordering, round-trip identity, state-machine legality. These catch whole *classes* of bugs, not single instances.
4. **Cross-layer consistency.** After an operation, the read model, the filesystem, the git refs, and the terminal state must agree. A deep test asserts the agreement, not just one layer.

**The deep-assertion checklist** for any test:

- [ ] Does it assert on the read model / event stream, not just a return value?
- [ ] Does it check an invariant (idempotency, ordering, round-trip)?
- [ ] Does it verify cross-layer consistency where relevant (model ↔ fs ↔ git)?
- [ ] Does it exercise an error/edge path, not just the happy path?
- [ ] Would a mutation in the code under test be caught by this test?

---

## 7. Test Infrastructure — Fast and Deterministic

The mutation gate re-runs the suite thousands of times, so **speed and determinism are infrastructure requirements**, not nice-to-haves. The principles:

### 7.1 Drain-to-empty pattern

The core is event-sourced with a serialized command queue per thread plus a parallel scheduler. Tests must not race the async machinery. The **drain-to-empty** pattern waits until the queue is empty and the projector has caught up before asserting:

```rust
async fn drain(core: &Core) {
    // Wait until every thread's command queue is empty and the projector
    // has applied all pending events to the read model.
    core.wait_until_idle(Duration::from_secs(5)).await;
}
```

Every integration test calls `drain()` before asserting. This makes tests deterministic — no `sleep(100ms)` hacks, no flaky timing.

### 7.2 Mock agents

The mock ACP agent (§2.4) replaces the real model with scripted, deterministic behavior. No network, no tokens, no model nondeterminism. The mock is the default for all integration, component, and e2e tests.

### 7.3 In-memory SQLite

The read model lives in SQLite. Tests use **in-memory SQLite** (`:memory:` or a temp file) so each test gets a clean, isolated database with zero I/O latency and no cross-test contamination.

### 7.4 Headless GPUI

Component and e2e tests run **headless** — no window, no GPU, no display server. GPUI's test harness renders off-screen. This makes UI tests fast, deterministic, and runnable in CI without a display. (A small set of pixel-golden e2e tests may require a display and run on a dedicated runner.)

### 7.5 Deterministic time and randomness

- All clocks are injected (`Clock` trait / `tokio::time` paused) so time-dependent logic (timeouts, power-adaptive sampling) is testable.
- proptest uses a fixed seed in CI for reproducibility; property failures are minimized and persisted as regression tests.
- No test reads ambient environment, user config, or the network.

### 7.6 Test isolation

Each test gets a fresh `Core` with a fresh in-memory SQLite and a fresh mock agent. No shared global state. Parallel test execution (`cargo test` default) is safe because nothing is shared.

---

## 8. Coverage Tooling

| Tool | Purpose | Gate |
|---|---|---|
| **cargo-llvm-cov** | Line + branch coverage | ≥85% line, ≥80% branch |
| **cargo-mutants** | Mutation testing | ≥70% mutants killed |
| **proptest** | Property-based testing | part of unit+property gate |
| **insta** | Snapshot testing (pane layouts) | part of component gate |
| **cargo-nextest** | Fast, parallel test runner | used across all Rust gates |

- **cargo-llvm-cov** produces per-crate coverage reports; the coverage gate is enforced on the core crates and reported per-PR so regressions are visible.
- **cargo-mutants** writes a report of survived mutants; survivors are reviewed and either fixed (test or dead code) or explicitly justified.
- **cargo-nextest** runs the suite in parallel with per-test isolation, keeping the mutation gate tractable.

**Coverage is a floor, not a target.** The mutation gate is the stronger signal: 100% line coverage with 0% mutation kill is worthless, while 70% mutation kill with 85% line coverage means the tests genuinely catch faults. We optimize for the mutation score and use line/branch coverage as the guardrail.

---

## 9. Open Questions

These reference pending decisions from PLAN-CONTEXT and `plan/20`; this doc does **not** decide them unilaterally.

1. **Mutation gate scope.** ~~Whether the ≥70% mutation gate applies to the *entire* core or only the safety-critical crates (orchestration, provider-adapter, wire contract, checkpointing, HAR).~~ **RESOLVED (D21):** the ≥70% mutation gate applies to **all core logic across all subsystems** — orchestration, provider-adapter, wire contract, checkpointing, HAR, **and the terminal, editor, browser, and pane system**. UI-only rendering is covered by component tests. This reconciles with plan/08 §9.5 and plan/11 §10.5.
2. **E2E cadence.** ~~Whether e2e runs on every merge or on a nightly + merge-for-critical-paths schedule.~~ **RESOLVED (D32):** e2e runs on the **merge gate** (critical paths) and **nightly** (full suite). There is **no "skip e2e for small changes" path**.
3. **Mobile test toolchain.** Native unit tests depend on the mobile stack decision (open question 2: SwiftUI/Kotlin vs Expo/React Native). The contract-conformance approach holds either way, but the native test runner differs.
4. **Real-binary smoke tests.** Whether to provision a token budget / dedicated runner for real-`grok` smoke tests in CI, or keep them strictly opt-in/local. Default: opt-in local + dedicated job, pending budget.
5. **Coverage threshold on vendored code.** Whether the ≥85% line gate applies to the vendored grok-build crates or only to our own code. Default: our code only, with a documented exclusion for `third_party/` — pending confirmation.

---

*Next: `plan/16-performance.md` — cold start, input latency, memory, and how the testing gates feed the performance budget.*
