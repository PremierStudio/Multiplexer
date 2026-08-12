# Adversarial Review — Architecture Consistency

**Scope:** `plan/02-architecture.md`, `plan/03-vendored-grok-build.md`, `plan/04-wire-contract.md`, `plan/05-provider-adapter-layer.md`, `plan/06-orchestration-engine.md`, cross-checked against `docs/PLAN-CONTEXT.md`.
**Reviewer focus:** architecture consistency (crate layout, embedded-harness strategy, server-centric runtime, adapter↔wire↔orchestration mapping, concurrency, feasibility).
**Date:** 2026-08-12

---

## (a) Summary verdict

The five docs are **broadly coherent and mutually consistent on the big architectural commitments**: single native Rust binary owning everything, thin clients over JSON-RPC-over-WebSocket, in-process embedding of `xai-grok-shell`/`xai-grok-tools`/`xai-grok-workspace` with the pager TUI replaced by GPUI, event-sourced orchestration with a pure decider + projector, and the `ProviderAdapter` trait as the seam. The server-centric runtime is applied consistently across 02/04/05/06, and the embedded-harness strategy in 03 matches 02's diagram.

However, there are **real inconsistencies in the middle layer** — the exact place where the docs claim to be "1:1" and "mechanical." The most serious are: (1) the approval-decision shape differs across the wire contract, the adapter trait, and the orchestration command model; (2) the claimed "wire events map 1:1 onto ProviderEvent with no transformation" is false; (3) the crate layout in 02/19 does not match the crate layout implied by 04/05; (4) the OpenRouter/DeepSeek adapter is described three different ways; and (5) there is an unresolved tension over who owns subagent scheduling and whether the grok-build 16-child cap can actually be raised — which directly threatens the "dozens of concurrent subagents" differentiator.

**Verdict: NOT READY.** The top-level architecture is sound, but the adapter↔wire↔orchestration mapping and the crate layout must be reconciled before implementation. These are consistency defects, not style nits.

---

## (b) Critical issues

### C1. Approval-decision representation is inconsistent across 04, 05, 06 (and 17)
- **04 §4.3:** `approval.respond` takes `decision: allow/deny/allow_once/allow_always`.
- **05 §2 (`ProviderAdapter::approval_respond`):** takes `approve: bool` + `reason: Option<String>`.
- **06 §5.1 (`ApprovalRespond` command):** takes an untyped `decision` + `reason`.
- **17-security-and-secrets.md §225:** user responds `allow / deny / allow_once / allow_always` via `approval.respond`, and `allow_always` records a scoped rule.

The adapter trait (05) is the odd one out: it can only express a boolean, so the wire contract's `allow_once`/`allow_always` decisions **cannot flow through the adapter**. This directly breaks 04 §1's stated invariant that the wire contract "deliberately mirrors the Provider Adapter trait" so the mapping is "mechanical." Either the adapter must carry a 4-way decision enum, or the wire contract must be reduced to a boolean — the docs cannot both be true. This is the single most concrete cross-doc contradiction.

---

## (c) Major issues

### M1. Crate layout in 02 does not match the crate layout implied by 04 and 05
- **02 §4** defines a split, `mx-*`-prefixed workspace: `mx-provider-adapter`, `mx-provider-grok`, `mx-provider-acp`, `mx-provider-openrouter`, `mx-model-registry`, `mx-wire-contract`, etc. **19-roadmap** uses the same `mx-*` names.
- **05** places everything in a single crate `crates/multiplexer-provider/` — `adapter.rs`, `event.rs`, `grok/in_process.rs`, `grok/acp.rs`, `registry.rs` (05 §2, §4, §5, §6) — i.e. the adapter trait, the canonical event enum, both Grok adapters, **and the model registry** all live in one crate.
- **04 §9.1** names the shared schema crate `multiplexer-wire` (not `mx-wire-contract`).

So 02/19 describe a fine-grained `mx-*` split (with `mx-model-registry` separate from the provider crates), while 04/05 describe consolidated `multiplexer-*` crates (with the registry inside the provider crate). These are two different workspace layouts. The naming convention (`mx-` vs `multiplexer-`) and the crate boundaries must be reconciled; as written, a reader cannot tell whether `mx-model-registry` is a separate crate (02) or part of `multiplexer-provider` (05).

### M2. "Wire events map 1:1 onto ProviderEvent, no transformation" is false (04 §1, §5)
04 §1 and §5 claim the wire events map "1:1 onto the canonical `ProviderEvent` stream … no transformation beyond adding `stream`/`seq`." The two vocabularies are **not** 1:1:

| 05 `ProviderEvent` | 04 wire event |
|---|---|
| `TextDelta` | `agent_message_chunk` **and** `agent_thought_chunk` (two events) |
| `ToolCallStarted` / `ToolCallFinished` | `tool_call` / `tool_call_update` |
| `TurnFinished` / `TurnFailed` / `TurnInterrupted` | `turn_status` (running/completed/failed) |
| `PermissionRequested {request_id,…}` | `permission_request {approval_id,…}` (field name differs) |
| `UserInputRequested` | `user_input_request` |

The wire event set is also a **superset** with no `ProviderEvent` counterpart: `plan`, `terminal_output`, `terminal_exit`, `har_event`, `subagent_status`, `fs_change`, `telemetry_resources`, `error`, `session_status`. And 05 has `SessionReady`, `Status`, `SessionStopped`, `TurnInterrupted` with no direct wire event. So a real transformation layer exists and is non-trivial (name mapping + granularity split + field renaming). The "mechanical / no transformation" claim is incorrect and should be replaced with an explicit mapping table.

### M3. OpenRouter / DeepSeek adapter is described three different ways
- **02 §4 / §5.2** and **19 §5.1:** a dedicated `mx-provider-openrouter` crate (DeepSeek V4 Flash / OpenRouter adapter).
- **05 §6.1:** `[model.ds-flash]` is routed with `adapter = "in-process-grok"` — "same embedded runtime, different model config" — i.e. **no** separate OpenRouter adapter; OpenRouter rides the in-process Grok adapter.
- **05 §7:** a future `DeepSeekAdapter` over HTTP (OpenAI-compatible), with the note "no agent tool loop unless we add one."

These three statements are mutually inconsistent about whether OpenRouter is (a) its own adapter crate, (b) a config variant of the in-process Grok adapter, or (c) a future HTTP adapter. This matters for the crate layout (M1) and for whether `ds-flash` gets tool execution in MVP.

### M4. Subagent orchestration ownership / 16-child cap tension (02/03 vs 06)
- **02 §3.3** and **03 §4.1:** we "inherit" grok-build's `spawn_subagent` + Rhai workflows (`agent()`, `parallel()`, `phase()`, budget caps, **max 16 concurrent children**) "for free."
- **06 §4.1:** "our scheduler" owns subagent scheduling and "can raise it (configurable) because we own the scheduling, not the vendored default."
- **06 OQ #3** then concedes: "the scheduler's ability to raise the built-in 16-child cap depends on how deeply we fork the vendored `spawn_subagent`/workflow code."

If we reuse grok-build's Rhai workflow engine as a library (02/03), its hard 16-child cap applies and "dozens of concurrent subagents" (PLAN-CONTEXT target) is capped at 16 per workflow unless we fork deeply. If we replace it with our own scheduler (06), we are not really "reusing it for free." The docs assert both simultaneously and flag the contradiction only as an open question. This is the crux of the fan-out differentiator and needs a decision, not a deferral.

---

## (d) Minor issues

### m1. Event vocabulary diverges between 06 and 05
06 §5.2 uses `TurnCompleted`, `ToolCallCompleted`, `ApprovalRequested`/`ApprovalResolved`, `MessageAppended`; 05 uses `TurnFinished`, `ToolCallFinished`, `PermissionRequested`, `TextDelta`. 06 §6.1 acknowledges a translation (provider ingestion worker maps `ProviderEvent` → engine events) but gives no mapping table, and the naming divergence (`Finished` vs `Completed`, `Permission` vs `Approval`) invites drift. A single canonical event vocabulary should be shared.

### m2. Role of the "acp" adapter differs between 02 and 05
02 §4/§5.2 defines `mx-provider-acp` as a **generic** ACP adapter for Claude/Codex/OpenCode. 05 §5 defines `AcpGrokAdapter` specifically for **Grok via ACP** (the fallback), and §7 gives Codex/OpenCode their own adapters that "reuse ACP machinery." So "acp" means "generic multi-provider ACP" in 02 but "Grok-over-ACP fallback" in 05. The crate/role boundary is ambiguous.

### m3. Unbounded event channel vs wire backpressure
05 §2 uses `mpsc::UnboundedReceiver<ProviderEvent>` at the adapter boundary, while 04 §8 mandates window-based flow control and bounded buffers on the wire. The unbounded adapter channel is a plausible source of unbounded memory growth under fan-out (a slow projector vs a fast backend), and the docs don't reconcile the two. 06 §6.1's "provider runtime ingestion" worker is the natural place to bound this, but it isn't specified as bounded.

### m4. `session.start` vs `start_session` parameter mismatch
04 §4.1 `session.start` takes `{provider, model?, worktree?, config?}`; 05 `start_session` takes `(model, workspace, initial_prompt, resume)`. The wire method has no `initial_prompt`/`resume` and the adapter has no `provider`/`config`. Minor, but it undercuts the "mirrors the adapter" claim (same issue as C1/M2).

### m5. `mx-mobile-shared` vs `multiplexer-wire` duplication
02 §4 lists `mx-mobile-shared` ("shared contract/types for the mobile client") as a separate crate, while 04 §9.1 says the single source of truth is `multiplexer-wire` and mobile consumes it via codegen. Two "shared contract" artifacts are implied; unclear which is authoritative.

---

## (e) Consistency gaps between docs (summary table)

| # | Topic | Doc A | Doc B | Gap |
|---|-------|-------|-------|-----|
| 1 | Approval decision | 04: 4-way enum | 05: `bool` | **Contradiction (C1)** |
| 2 | Crate layout | 02/19: `mx-*` split | 04/05: `multiplexer-*` consolidated | **Mismatch (M1)** |
| 3 | Wire↔ProviderEvent mapping | 04: "1:1, no transformation" | 05 event set | **False claim (M2)** |
| 4 | OpenRouter adapter | 02: own crate | 05 §6: in-process-grok | **3-way conflict (M3)** |
| 5 | Subagent cap | 02/03: inherit 16-child | 06: raise it | **Tension (M4)** |
| 6 | Event vocabulary | 06: `TurnCompleted` | 05: `TurnFinished` | **Naming drift (m1)** |
| 7 | ACP adapter role | 02: generic | 05: Grok fallback | **Ambiguous (m2)** |
| 8 | Backpressure | 04: bounded wire | 05: unbounded channel | **Unreconciled (m3)** |
| 9 | Session start params | 04: `provider/config` | 05: `model/workspace/prompt` | **Mismatch (m4)** |
| 10 | Shared contract crate | 02: `mx-mobile-shared` | 04: `multiplexer-wire` | **Duplication (m5)** |

---

## (f) Specific findings with doc+section references

1. **C1** — `plan/04-wire-contract.md` §4.3 (`approval.respond` decision enum) vs `plan/05-provider-adapter-layer.md` §2 (`approval_respond(approve: bool)`) vs `plan/06-orchestration-engine.md` §5.1 (`ApprovalRespond` untyped `decision`) vs `plan/17-security-and-secrets.md` §225 (`allow_once`/`allow_always`). The boolean adapter cannot carry the wire's 4-way decision.
2. **M1** — `plan/02-architecture.md` §4 (crate tree, `mx-*`) and `plan/19-roadmap-and-milestones.md` §1.x/§5.x (`mx-provider-*`, `mx-model-registry`, `mx-wire-contract`) vs `plan/05-provider-adapter-layer.md` §2/§4/§5/§6 (`crates/multiplexer-provider/…`) and `plan/04-wire-contract.md` §9.1 (`multiplexer-wire`).
3. **M2** — `plan/04-wire-contract.md` §1 ("deliberately mirrors the Provider Adapter trait … mapping mechanical") and §5 ("map 1:1 … no transformation beyond adding stream/seq") vs the actual event sets in §5 (04) and §3 (05), which differ in granularity, naming, and field names (`request_id` vs `approval_id`).
4. **M3** — `plan/02-architecture.md` §4/§5.2 (`mx-provider-openrouter`) and `plan/19-roadmap-and-milestones.md` §5.1 vs `plan/05-provider-adapter-layer.md` §6.1 (`adapter = "in-process-grok"` for `ds-flash`) vs §7 (`DeepSeekAdapter` over HTTP).
5. **M4** — `plan/02-architecture.md` §3.3 and `plan/03-vendored-grok-build.md` §4.1 ("inherit … max 16 concurrent children") vs `plan/06-orchestration-engine.md` §4.1 ("our scheduler … can raise it") and §10 OQ #3 (concedes the dependency on fork depth).
6. **m1** — `plan/06-orchestration-engine.md` §5.2/§6.1 (`TurnCompleted`, `ToolCallCompleted`, `ApprovalRequested`) vs `plan/05-provider-adapter-layer.md` §3 (`TurnFinished`, `ToolCallFinished`, `PermissionRequested`).
7. **m2** — `plan/02-architecture.md` §4/§5.2 (`mx-provider-acp` generic) vs `plan/05-provider-adapter-layer.md` §5 (`AcpGrokAdapter` Grok-only fallback) and §7 (Codex/OpenCode reuse ACP machinery).
8. **m3** — `plan/05-provider-adapter-layer.md` §2 (`mpsc::UnboundedReceiver`) vs `plan/04-wire-contract.md` §8 (window-based flow control, bounded buffers).
9. **m4** — `plan/04-wire-contract.md` §4.1 (`session.start` params) vs `plan/05-provider-adapter-layer.md` §2 (`start_session` params).
10. **m5** — `plan/02-architecture.md` §4 (`mx-mobile-shared`) vs `plan/04-wire-contract.md` §9.1 (`multiplexer-wire` as single source of truth).

---

## Recommended next steps (for the plan authors, not performed here)

1. Pick **one** approval-decision type (recommend the 4-way enum) and thread it through 04/05/06/17 consistently.
2. Reconcile the **crate layout**: either the fine-grained `mx-*` split (02/19) or the consolidated `multiplexer-*` crates (04/05), and decide where the model registry lives.
3. Replace the "1:1 / no transformation" claim with an explicit **wire↔ProviderEvent↔engine-event mapping table**.
4. Decide the **OpenRouter/DeepSeek** adapter identity and whether `ds-flash` gets a tool loop in MVP.
5. Resolve **who owns subagent scheduling** and how the 16-child cap is raised, since it gates the fan-out differentiator.
