# 19 — Roadmap & Milestones

**Status:** Draft for adversarial review
**Author:** Roadmap subagent
**Source of truth:** `docs/PLAN-CONTEXT.md` (this doc is consistent with it; conflicts are flagged in §11 Open questions)
**Scope:** Phased delivery plan from MVP → GA, per-phase deliverables/exit criteria/test gates, dependency & sequencing, timeline risks, open questions.

> **Locked decisions applied: D13, D8, D40, D31, D10.** Crate names consolidated to `multiplexer-*` (no `mx-*`, no `mx-provider-openrouter`, no `mx-mobile-shared`); **MVP = Phases 1–4** (Phase 5 and Phase 6 are post-MVP, with the mobile app a hard MVP gate); explicit effort estimate added for matching the full Orca baseline. **D40:** dependency spine corrected — Phase 4 depends on Phase 1 (wire contract), not Phase 3; Phase 3 and Phase 4 run in parallel. **D31:** standing "track upstream" task added (§9.5). **D10:** Phase 0's first deliverable framed as the embedding + Windows spike (go/no-go), not a settled fact.

---

## 1. Delivery philosophy

Multiplexer ships in **seven phases (0–6)**, each a shippable, testable milestone with a hard **exit criterion** and a **test gate** that must be green before the phase is declared done. We do **not** do "big bang" integration: every phase lands behind the CI gates from `docs/PLAN-CONTEXT.md` (fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e → coverage), and every phase is **demoable** at its end.

Two structural commitments shape the whole roadmap:

1. **TDD at inception is non-negotiable.** The test harness and CI gates are built in **Phase 0**, before any product feature. No phase ships without its gates green. This is the "no blind CI" rule from PLAN-CONTEXT.
2. **Windows grok-build build is the critical path.** Everything downstream (embedding, orchestration, UI) depends on the vendored harness compiling and running on Windows. We de-risk this **first**, in Phase 0, because it is the single biggest unknown and the one thing we cannot design around.

The phases are sequenced so that each one **unlocks** the next:

```
Phase 0 Foundation ─► Phase 1 Core MVP ─► Phase 2 Editor+Panes ─► Phase 3 Browser+HAR
      │                                                              │
      └──────────────► Phase 4 Mobile+Remote ─► Phase 5 Multi-provider ─► Phase 6 GA
```

Phases 2 and 4 can partially overlap with Phase 3 (they touch different subsystems), but the **dependency spine** (0 → 1 → 2 → 3 → 5 → 6) is strict: each phase's exit criteria assume the prior spine phase is green.

### MVP definition (D8 — LOCKED)

**MVP = Phases 1–4** (Core MVP + Editor/Panes + Browser/HAR + Mobile/Remote). **Phase 5** (multi-provider + scale) and **Phase 6** (GA) are **post-MVP**. The **mobile app (Phase 4.1) is a hard MVP gate**: the MVP does not ship without the paired mobile thin client over `multiplexer-wire`. This prevents the mobile app from slipping past the MVP.

### Effort estimate (D8 — LOCKED)

Matching the **full Orca baseline** (D7) across **Phases 1–5** is a **multi-quarter, multi-engineer** effort. Rough order: **~6–9 months of calendar time with a team of 3–5 engineers** (≈ **20–40 person-months**), driven by the Windows grok-build build (Phase 0), the full native editor (Phase 2), and the browser/HAR + mobile/remote subsystems (Phases 3–4). Phase 5 (multi-provider + scale) adds provider adapters and subagent orchestration at scale. This is a large commitment and is the primary driver of the timeline risk in §10.

### New differentiators (2026-08-12)

Six additional differentiators are being authored in parallel and slotted into the existing phases below. They do **not** change the MVP scope (Phases 1–4) or the dependency spine; they add deliverables and exit criteria to the phases where they naturally land.

| Plan doc | Differentiator | Lands in |
|---|---|---|
| `plan/21-mcp-lifecycle-supervisor.md` | MCP process reuse/teardown (lifecycle supervisor) | Phase 1 (server composition) |
| `plan/22-remote-delegation.md` | Control on A, execute on B over the existing JSON-RPC/WS contract | Phase 4 |
| `plan/23-tailscale-integration.md` | MagicDNS, local API discovery, Serve for private relay | Phase 4 |
| `plan/24-resource-manager.md` | CPU/RAM pin, Job Objects, fleet 1–100, live visual (**KILLER feature**) | Phase 1 (local) + Phase 4 (fleet/distributed) |
| `plan/25-worktree-hooks.md` | Auto create/remove worktrees, pre-existing reminder, lifecycle hooks | Phase 1 (basics) |
| `plan/26-mcp-skills-ui.md` | MCP registry + skills/hooks GPUI UI | Phase 2 (UI) |

The local resource manager is pulled into **Phase 1** because it fixes orphaned agent processes **now**; the distributed fleet scheduler stays in **Phase 4** (its natural home with Mobile+Remote) and is **not** pulled forward. Tailscale is an **optional degrade** (see §10 risk note).

---

## 2. Phase 0 — Foundation (de-risk everything)

**Goal:** Prove the two hardest unknowns — *the vendored grok-build builds and runs on Windows*, and *the test/CI harness works* — before writing any product feature. Everything after this phase is de-risked by construction.

**Framing (D10):** In-process grok-build embedding is a **Phase-0 go/no-go hypothesis**, not a settled fact. The first deliverable (0.3) is a **spike** — clone grok-build, consume `xai-grok-shell` as a library, run a headless turn in-process, and get the crates building on Windows. This spike is the **go/no-go gate** for the embedding differentiator. If the shell is not cleanly embeddable, we fall back to the **ACP path** (drive `grok agent stdio`/`serve`), which is fully supported and documented; the plan keeps both paths open until the spike resolves.

### Deliverables

| # | Deliverable | Detail |
|---|---|---|
| 0.1 | Repo scaffold | Rust workspace per `plan/02` §4: `crates/multiplexer-*`, `apps/multiplexer-desktop`, `third_party/`. Root `Cargo.toml` with `[patch]` wiring. |
| 0.2 | Vendored grok-build | Fork `xai-org/grok-build` under `third_party/grok-build/` (mechanism per open question #5; recommended vendored fork + `[patch]`). |
| 0.3 | **Windows build proof** | `xai-grok-shell` / `xai-grok-tools` / `xai-grok-workspace` compile **and run a headless smoke turn** on Windows. Add `cfg(windows)` fixes upstream lacks. |
| 0.4 | GPUI shell | Minimal GPUI window that renders a blank pane and a hello-world frame at 60fps. Proves the UI toolchain on Windows. |
| 0.5 | Wire contract skeleton | `multiplexer-wire` types + JSON-RPC codec + schema, with contract tests on both encode and decode sides. |
| 0.6 | Test harness | Unit + property (proptest) + mutation (cargo-mutants) + integration scaffolding wired into CI. |
| 0.7 | CI gates | GitHub Actions (or equivalent) pipeline: fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e → coverage. Windows runner primary. |

> **Note (2026-08-12):** Phase 0 adds **no** new product features. The 0.6 test harness should, however, include fixtures for the later **affinity / Job Object unit tests** (resource manager, `plan/24`) so those tests are not retrofitted.

### Exit criteria

- [ ] `cargo build --release` on a clean Windows machine succeeds for the whole workspace.
- [ ] A headless `xai-grok-shell` smoke turn (a trivial prompt → tool call → response) completes in-process on Windows.
- [ ] GPUI shell renders a window and holds 60fps on Windows.
- [ ] CI gates are green on a trivial commit; mutation gate thresholds configured (≥85% line, ≥80% branch, ≥70% mutation score killed).
- [ ] A contract round-trip test (encode → decode → validate) passes for the core message types.

### Test gate

Full CI chain green on the scaffold. **Mutation gate is configured and enforced from day one** — not retrofitted later.

### Why this phase is the critical de-risk

PLAN-CONTEXT states Windows builds of grok-build are "best-effort, not currently tested from this tree," and upstream does not accept external contributions. If the harness cannot be made to build and run on Windows, the entire in-process-embedding differentiator is at risk on our primary platform. We therefore **prove it in the first phase** — as a **spike / go-no-go**, not an assumed win — before investing in orchestration, UI, or any feature that assumes it. If it fails, we surface the blocker immediately (see §10) and fall back to the ACP path rather than discovering it mid-MVP.

---

## 3. Phase 1 — Core MVP (the Grok control surface)

**Goal:** A Windows desktop app that embeds the Grok harness in-process, runs a real agent session through the orchestration engine, and lets the user chat, watch the build, and use a terminal. This is the **first demoable product**.

### Deliverables

| # | Deliverable | Detail |
|---|---|---|
| 1.1 | Embedded Grok adapter | `multiplexer-provider`: in-process `xai-grok-shell` adapter implementing the `ProviderAdapter` trait. The fast path. |
| 1.2 | Orchestration engine | `multiplexer-core` + `multiplexer-orchestration`: per-thread serialized command queue, pure decider, projector into SQLite read model, parallel scheduler. |
| 1.3 | Provider adapter trait | `multiplexer-provider`: `start_session`, `send_turn`, `interrupt_turn`, `approval_respond`, `user_input_respond`, `checkpoint_revert`, `session_stop` + canonical `ProviderEvent` stream. |
| 1.4 | Wire contract | Full JSON-RPC-over-WebSocket contract for the MVP surface: sessions, turns, events, terminal, approvals. Schema-verified both sides. |
| 1.5 | Server composition root | `multiplexer-server`: owns the harness, orchestration, read model, and serves the WebSocket contract. |
| 1.6 | Desktop shell | `apps/multiplexer-desktop`: thin GPUI shell — left chat sidebar, center build pane, terminal pane. |
| 1.7 | Terminal | `multiplexer-terminal`: Ghostty embedding with basic splits; agent terminal tool wired through. |
| 1.8 | Checkpointing (basic) | `multiplexer-checkpoint`: hidden git refs per turn, diff query, revert. |
| 1.9 | Model registry (basic) | `multiplexer-model-registry`: `[model.*]`/`[auth_provider.*]` config; select Grok per thread. |
| 1.10 | Auth (local) | `multiplexer-auth`: OS keychain for provider secrets; OAuth for Grok provider. |
| 1.11 | Resource manager (local) | `multiplexer-resource-manager` (per `plan/24`): CPU/RAM pinning, Windows Job Objects, kill-on-close, sysinfo telemetry. Fixes orphaned agent processes **now**. |
| 1.12 | MCP lifecycle supervisor | `multiplexer-mcp-supervisor` (per `plan/21`): MCP process reuse/teardown, part of the server composition root. |
| 1.13 | Worktree hooks (basics) | `multiplexer-worktree-hooks` (per `plan/25`): auto create/remove worktrees, pre-existing worktree reminder, lifecycle hooks, where they fit orchestration. |

### Exit criteria

- [ ] A user can start Multiplexer, create a thread, send a prompt, and watch the agent run tools and edit files **in-process** (no CLI, no ACP round-trip).
- [ ] The user can interrupt a turn, respond to an approval, and revert to a checkpoint.
- [ ] The terminal pane runs a real PTY with splits; the agent's terminal tool output appears live.
- [ ] Two independent threads run concurrently without serializing through one queue.
- [ ] Cold start to usable editor **< 300 ms**; input latency **< 16 ms** (measured, see `plan/16`).
- [ ] Desktop and (stub) mobile client both render the same live state from the read model over the wire contract.
- [ ] Killing a thread closes its agent processes (Job Object kill-on-close); affinity and memory limits are applied and reported via sysinfo telemetry (`plan/24`).
- [ ] MCP servers are reused across sessions and torn down cleanly on exit (`plan/21`).
- [ ] Worktrees are auto-created/removed with lifecycle hooks firing; a pre-existing worktree is surfaced as a reminder rather than clobbered (`plan/25`).

### Test gate

Full CI chain green, **plus**:
- **Integration:** real core + mock ACP agent (fake `grok agent stdio`); assert on the SQLite read model.
- **Unit:** affinity / Job Object behavior against the Phase 0 fixtures (`plan/24`); MCP supervisor reuse/teardown (`plan/21`).
- **Contract:** wire-contract schema-verified on both sides.
- **Component (GPUI):** pane layout snapshot tests.
- **E2E:** drive the real app/headless — a full prompt → tool → response cycle.

### Windows-first note

Phase 1 is **Windows-only**. macOS/Linux builds are not a Phase 1 deliverable (open question #8 assumes Windows-first; revisit if it flips).

---

## 4. Phase 2 — Editor + Panes (the "real editor" differentiator)

**Goal:** Replace the placeholder center pane with the native GPUI editor and ship the full pop-out pane system. This is where we beat every competitor that lacks an editor.

### Deliverables

| # | Deliverable | Detail |
|---|---|---|
| 2.1 | Native editor | `multiplexer-editor`: multi-cursor, LSP, Vim mode, incremental rendering (Zed-proven model). |
| 2.2 | Inline diff-apply | Diff view over checkpoint refs; apply/reject hunks inline; route diff-line comments back to the agent. |
| 2.3 | Pane system | `multiplexer-ui`: Outlook-style layout — left chat sidebar, center build pane, multi-purpose right bar (browser/HAR/files/diff/terminal/agent activity), pop-up terminal below. |
| 2.4 | Pop-out windows | Every pane can pop out to its own window. |
| 2.5 | Split-anything | Any pane can be split arbitrarily (Orca baseline). |
| 2.6 | Design system | Shared GPUI theme, typography, spacing, component library; snapshot-tested. |
| 2.7 | Native search | Fast native search across the workspace (Orca baseline). |
| 2.8 | MCP/Skills/Hooks Customize UI | `multiplexer-ui` (per `plan/26`): MCP registry + skills/hooks management UI. |
| 2.9 | Resource visual pane | `multiplexer-ui` (per `plan/24`): live visual of CPU/RAM per agent/thread. |

### Exit criteria

- [ ] Editor opens files, runs LSP, supports multi-cursor and Vim mode at < 16 ms input latency.
- [ ] Inline diff-apply works against checkpoint refs; a diff-line comment routes back to the agent and the agent responds.
- [ ] All panes render in the layout and each can pop out to its own window and split arbitrarily.
- [ ] Design system is consistent and snapshot-tested; no ad-hoc styling.
- [ ] Native search returns results across the workspace in real time.
- [ ] The MCP registry and skills/hooks management UI work end to end (`plan/26`).
- [ ] The resource visual pane renders live CPU/RAM per agent/thread from the resource manager telemetry (`plan/24`).

### Test gate

Full CI chain green, **plus**:
- **Component:** editor element tests, pane-layout snapshot tests, pop-out window tests; MCP registry and resource visual pane snapshot tests (`plan/26`, `plan/24`).
- **Integration:** diff-apply against real checkpoint refs; LSP against a fixture project.
- **E2E:** drive the editor headlessly — open, edit, diff-apply, comment → agent.

### Scope note

Whether the **full** editor ships in the MVP or a lighter editor precedes it is open question #4. This phase describes the full editor; if the user chooses a lighter MVP editor, Phase 2 is split into "lighter editor in MVP" + "full editor post-MVP" without changing the architecture (`plan/02` §9).

---

## 5. Phase 3 — Browser + HAR (the "real insight" differentiator)

**Goal:** System-browser integration and the built-in HAR profiler/replayer — capabilities **no competitor has**. This is where Multiplexer stops being "another agent UI" and becomes a control surface with real insight.

### Deliverables

| # | Deliverable | Detail |
|---|---|---|
| 3.1 | Browser detection/import | `multiplexer-browser`: detect installed browsers (Chrome, Edge, Firefox, Safari, Arc, Brave), import profile, launch/authorize. **No bundled Chromium.** |
| 3.2 | CDP driver | Drive the browser via CDP; browser pane in the right bar. |
| 3.3 | Design Mode | Click a browser element → route it to the agent as an actionable target (Orca baseline). |
| 3.4 | HAR capture | `multiplexer-har`: capture network via CDP. |
| 3.5 | HAR waterfall | Visualize captured requests as a waterfall in the HAR pane. |
| 3.6 | HAR replay | Replay a recorded session deterministically. |
| 3.7 | GitHub / Linear native | First-class integrations (Orca baseline). |

### Exit criteria

- [ ] Multiplexer detects and imports the user's installed browsers and drives them via CDP without bundling Chromium.
- [ ] Design Mode: clicking a browser element produces an actionable agent target.
- [ ] HAR capture produces a valid HAR; the waterfall renders; a recorded session replays.
- [ ] GitHub and Linear integrations work natively (auth, issue/PR surfacing).

### Test gate

Full CI chain green, **plus**:
- **Integration:** CDP driver against a headless browser fixture; HAR capture/replay round-trip.
- **Component:** waterfall and browser-pane snapshot tests.
- **E2E:** headless browser → Design Mode → agent action cycle.

### Sequencing note

Phase 3 depends on the pane system (Phase 2) for the browser/HAR panes and on the orchestration engine (Phase 1) for Design Mode routing. It can run **in parallel** with Phase 4 (different subsystems) but not before Phase 2's pane system lands.

---

## 6. Phase 4 — Mobile + Remote (the paired companion)

**Goal:** The required paired mobile app and the remote/relay layer, so agents can be observed and steered from the phone and from anywhere.

### Deliverables

| # | Deliverable | Detail |
|---|---|---|
| 4.1 | Mobile app | `apps/multiplexer-mobile` (thin client over `multiplexer-wire` via codegen; no separate `multiplexer-mobile-shared` crate). Expo/React Native (D2). |
| 4.2 | Remote/relay | `multiplexer-remote`: local + paired + relay tunnel + SSH; WebSocket ticket auth (5-min TTL); Tailscale serve. |
| 4.3 | SSH worktrees | SSH remote worktrees (Orca baseline). |
| 4.4 | Account/usage tracking | Usage metering and account management (Orca baseline). |
| 4.5 | Remote auth | Passkeys/DPoP for remote; OAuth for providers. |
| 4.6 | Remote delegation | `multiplexer-remote` (per `plan/22`): control on A, execute on B over the existing JSON-RPC/WS contract. |
| 4.7 | Tailscale integration | `multiplexer-remote` (per `plan/23`): MagicDNS, local API discovery, Serve for private relay. Optional degrade. |
| 4.8 | Fleet scheduler (distributed) | `multiplexer-resource-manager` (per `plan/24`): distributed fleet scheduler, fleet 1–100. |

### Exit criteria

- [ ] Mobile app pairs to the desktop server and observes/controls a live agent session over the same wire contract.
- [ ] Remote access works over relay tunnel and SSH; ticket auth (5-min TTL) is enforced.
- [ ] SSH worktrees run agent sessions on a remote machine.
- [ ] Account/usage tracking is functional.
- [ ] A session is controlled on machine A and executed on machine B over the existing wire contract (`plan/22`).
- [ ] Tailscale MagicDNS discovery and Serve private relay work; the feature degrades gracefully when Tailscale is absent (`plan/23`).
- [ ] The distributed fleet scheduler runs 1–100 agents across machines with live resource visuals (`plan/24`).

### Test gate

Full CI chain green, **plus**:
- **Mobile:** native unit + integration against the shared contract; mock server for offline determinism.
- **Integration:** relay/SSH transport round-trip; ticket-auth expiry test.
- **E2E:** mobile → remote → agent cycle.

### Sequencing note

Phase 4 depends on the wire contract (Phase 1) and the server composition root (Phase 1). It can run in parallel with Phase 3, but the mobile app needs a stable contract, so it should not start before Phase 1's contract is frozen.

---

## 7. Phase 5 — Multi-provider + Scale (the "many harnesses" differentiator)

**Goal:** Open the surface to other models/harnesses via the provider-adapter pattern and scale subagent orchestration to the "dozens of concurrent subagents" target.

### Deliverables

| # | Deliverable | Detail |
|---|---|---|
| 5.1 | OpenRouter/DeepSeek | Config variant of the in-process Grok adapter (`[model.ds-flash]` + `[auth_provider.openrouter]`) — **not** a separate crate (D14). |
| 5.2 | ACP adapter | `multiplexer-provider` (generic ACP machinery): Claude, Codex, OpenCode (the universal path). |
| 5.3 | Model registry (full) | Per-thread model selection across all providers; `[model.*]`/`[auth_provider.*]` management. |
| 5.4 | Subagent orchestration at scale | Inherit grok-build's `spawn_subagent` + Rhai workflows (`agent()`, `parallel()`, `phase()`, budget caps, 16-concurrent-children); layer the parallel scheduler for cross-thread fan-out. |
| 5.5 | Orchestration dashboard | Live dashboard of subagent activity (the "real insight" control surface). |
| 5.6 | Orca CLI | A command-line entry point (Orca baseline). |

### Exit criteria

- [ ] A thread runs on DeepSeek/OpenRouter, Claude, Codex, and OpenCode via the adapter layer — one surface, many harnesses.
- [ ] Dozens of concurrent subagents run without a serialization bottleneck (measured, see `plan/16`).
- [ ] The orchestration dashboard shows live subagent activity.
- [ ] Orca CLI works as a command-line entry point.

### Test gate

Full CI chain green, **plus**:
- **Integration:** each provider adapter against a mock provider; ACP adapter against a fake `grok agent stdio`.
- **Property:** decider/projector state machines under concurrent fan-out (proptest).
- **E2E:** a multi-subagent fan-out completes and is observable in the dashboard.

### Sequencing note

Phase 5 depends on the provider-adapter trait (Phase 1) and the orchestration engine (Phase 1). It is deliberately **after** the Grok-only MVP so the core is proven before multi-provider complexity lands (open question #3 default: Grok-only MVP).

---

## 8. Phase 6 — GA (ship it)

**Goal:** A polished, signed, auto-updating, distributed Windows product with marketing and support in place.

### Deliverables

| # | Deliverable | Detail |
|---|---|---|
| 6.1 | Packaging | Installer (MSI/NSIS), portable build, per-platform artifacts. |
| 6.2 | Code signing | Windows code-signing cert; macOS/Linux signing when those platforms ship. |
| 6.3 | Auto-update | Signed, atomic auto-update channel (stable/beta). |
| 6.4 | Distribution | Website (Multiplexer.dev / Multiplexor.dev — brand decision per open question #6), download pages, release notes. |
| 6.5 | Marketing | Positioning, docs, onboarding, changelog, community channels. |
| 6.6 | Telemetry (privacy-safe) | Opt-in crash reporting + usage analytics (respecting the no-bundled-Chromium privacy posture). |
| 6.7 | Support | Issue triage, docs site, feedback loop. |

### Exit criteria

- [ ] A user can download, install, sign in, and run Multiplexer on Windows with no dev toolchain.
- [ ] Auto-update rolls out a patch and a minor release cleanly.
- [ ] All CI gates green on the release branch; release artifacts are reproducible and signed.
- [ ] Marketing site, docs, and onboarding are live; brand is unambiguous (one canonical domain).

### Test gate

Full CI chain green on the release branch, **plus**:
- **E2E:** install → launch → update cycle on a clean Windows VM.
- **Coverage:** release-branch coverage thresholds met.
- **Manual QA:** a release-candidate checklist covering the full baseline bar.

---

## 9. Dependencies & sequencing

### 9.1 The dependency spine

```
Phase 0 ─► Phase 1 ─► Phase 2 ─► Phase 3 ─► Phase 5 ─► Phase 6
   │          │          │          │          │
   │          │          │          └──► Phase 3 needs Phase 2 panes
   │          │          └──► Phase 2 needs Phase 1 orchestration + read model
   │          └──► Phase 4 (parallel with Phase 3, needs Phase 1 contract)
   └──► everything needs Phase 0 Windows grok-build build + CI gates
```

### 9.2 Key dependency rules

| Dependency | Why | Blocks |
|---|---|---|
| Windows grok-build build (0.3) | The in-process-embedding differentiator is meaningless if the harness won't run on our primary platform. | Everything (0 → 6). |
| CI gates (0.7) | "No blind CI" is non-negotiable; every phase's gate assumes the harness exists. | Every phase's exit. |
| Wire contract (1.4) | Mobile (4.1) and remote (4.2) are thin shells over it. | Phase 4. |
| Orchestration + read model (1.2) | Editor diff-apply (2.2), Design Mode (3.3), dashboard (5.5) all read from it. | Phases 2, 3, 5. |
| Pane system (2.3) | Browser/HAR panes (3.x) render inside it. | Phase 3. |
| Provider-adapter trait (1.3) | Multi-provider (5.x) implements it. | Phase 5. |

### 9.3 What can run in parallel

- **Phase 3 and Phase 4** touch disjoint subsystems (browser/HAR vs mobile/remote) and both depend on Phase 1, so they can run **in parallel** — Phase 4 needs only Phase 1's contract (not Phase 2's panes), while Phase 3 additionally needs Phase 2's panes.
- **Phase 5's OpenRouter adapter (5.1)** can be prototyped early (the user already runs `ds-flash`) but is only *shipped* in Phase 5.
- **Phase 6 packaging (6.1–6.3)** can be scaffolded in parallel with Phase 5, but signing/auto-update only finalize at GA.

### 9.4 What is strictly serial

The dependency spine (0 → 1 → 2 → 3 → 5 → 6) is strict. Do not start Phase 2 before Phase 1's orchestration and read model are green; do not start Phase 3 before Phase 2's panes; do not start Phase 5 before Phase 1's provider-adapter trait.

### 9.5 Standing task — track upstream (recurring)

A **recurring** task, not one-time research, running for the life of the project:

- **Daily sync (signal only):** monitor the grok-build changelog (upstream releases frequently — multiple releases/day) and T3 Code issues/commits. Only surface meaningful changes (breaking API, new capabilities, fan-out changes, Windows fixes); do not act on every release.
- **Quarterly:** re-validate the competitive snapshot against current upstream reality.
- **Reference:** `docs/UPSTREAM-TRAJECTORY.md` is the canonical record of upstream tracking and trajectory.

This feeds the vendored-fork sync cadence (D5) and the subagent-scheduling fork reconciliation (D11).

---

## 10. Risks to timeline

| # | Risk | Impact | De-risk (early) |
|---|---|---|---|
| R1 | **grok-build won't build/run on Windows** | Critical — kills the core differentiator on our primary platform. | Prove in Phase 0 (0.3) before any feature work. If it fails, escalate immediately; fallback is ACP path (slower, still viable). |
| R2 | **GPUI on Windows maturity** | UI toolchain gaps slow the editor/pane work. | Phase 0 GPUI shell (0.4) proves the toolchain; reuse Zed's open-source GPUI components. |
| R3 | **LSP / editor complexity** | The "real editor" is the hardest UI deliverable. | Ship a lighter editor in MVP if needed (open question #4); keep UI thin over the contract so a rewrite is cheap. |
| R4 | **Multi-provider scope creep** | Delays the Grok-only MVP. | Default is Grok-only MVP (open question #3); multi-provider is Phase 5. |
| R5 | **Orca baseline scope** | Matching all Orca features in MVP is large. | Default is full baseline, but a defensible subset is allowed (open question #7); subset must not drop the differentiators. |
| R6 | **CDP / browser fragmentation** | Browser detection/import across Chrome/Edge/Firefox/Safari/Arc/Brave is fiddly. | Start with Chrome/Edge on Windows (highest share); add others incrementally. |
| R7 | **Mutation-test gate friction** | cargo-mutants on a large workspace is slow; gate may be flaky. | Configure thresholds in Phase 0; run mutation on changed crates in CI, full suite nightly. |
| R8 | **Remote/relay security** | Ticket auth, DPoP, and relay are security-sensitive. | Build against `plan/17`; security review before Phase 4 ships. |
| R9 | **Vendoring drift** | Upstream grok-build changes break our fork. | Pin the fork; treat it as our own crate; document the upgrade path. |
| R10 | **Branding ambiguity** | Shipping with dual branding confuses users. | Resolve open question #6 before Phase 6 marketing. |
| R11 | **Tailscale dependency** | Remote discovery/relay depends on Tailscale being installed. | Treat Tailscale as an **optional degrade**: fall back to the relay tunnel / SSH path when Tailscale is absent (`plan/23`). |
| R12 | **Fleet scope creep** | Distributed fleet scheduling is large and could bloat the MVP. | Keep the fleet scheduler in **Phase 4**; the local resource manager ships in **Phase 1** because it fixes orphaned processes now (`plan/24`). |

**The single biggest timeline risk is R1.** Everything else is a scoping or quality risk; R1 is a feasibility risk on the core differentiator. That is why Phase 0 exists and why it is the first thing we do.

**New-differentiator risk note (2026-08-12):** Tailscale is an **optional degrade** (R11); the distributed fleet is **Phase 4** (R12); the **local resource manager is Phase 1** because it fixes orphaned agent processes **now**, not later.

---

## 11. Open questions

These are the pending decisions from PLAN-CONTEXT that this roadmap touches but does **not** decide unilaterally. They are tracked in `plan/20-risks-and-open-questions.md`.

1. **MVP scope (Grok-only vs multi-provider from day one).** This roadmap assumes Grok-only MVP (Phase 1) with multi-provider in Phase 5. If the user wants multi-provider from day one, Phase 1 grows and Phase 5 moves earlier.
2. **Editor scope (full native editor in MVP vs lighter editor first).** This roadmap describes the full editor in Phase 2. A lighter MVP editor would split Phase 2 without changing the architecture.
3. **Orca baseline scope (match all Orca features in MVP vs subset).** This roadmap assumes the full baseline across phases. A defensible subset is allowed but must not drop the differentiators.
4. **Mobile stack (native SwiftUI/Kotlin vs Expo/React Native).** Phase 4 is stack-neutral; the concrete choice is pending.
5. **grok-build vendoring (submodule vs vendored copy vs `[patch]`).** Phase 0 assumes vendored fork + `[patch]` (recommended); the exact mechanism is pending.
6. **Branding (which domain is the product brand vs redirect).** Must be resolved before Phase 6 marketing; this roadmap does not decide.
7. **Windows-first (confirm).** This roadmap assumes Windows-first throughout; revisit if it flips.

**Flagged consistency note:** none found — this doc is consistent with PLAN-CONTEXT. If any decision above flips, the affected phases (§2–§8) and the dependency spine (§9) must be revisited.

---

*Next: `plan/20-risks-and-open-questions.md` — consolidated risk register and the pending user decisions referenced throughout.*
