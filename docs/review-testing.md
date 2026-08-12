# Adversarial Review — Testing Rigor & Performance (docs 08–12, 15, 16)

**Reviewer:** adversarial subagent (testing/performance focus)
**Scope reviewed:** `plan/08-terminal.md`, `plan/09-editor.md`, `plan/10-ui-pane-system.md`,
`plan/11-system-browser-integration.md`, `plan/12-har-profiler-replayer.md`,
`plan/15-testing-strategy.md`, `plan/16-performance.md`, against `docs/PLAN-CONTEXT.md`.
**Mandate checked:** "TDD at inception with full unit + mutation, full component, full
integration, deep assertions, not shallow. Check everything."

---

## (a) Summary verdict

The testing strategy (**15**) is genuinely strong and is the best-written doc in the set: it
operationalizes the mandate, defines "deep assertions" concretely (read model / event stream /
invariants / cross-layer), and specifies a strict ordered CI gate. **12 (HAR)** is the model
subsystem doc — it has a dedicated mutation section with mandatory high-mutation-score areas.
**16 (Performance)** has a real measurement/enforcement plan (criterion benchmarks + hard/soft CI
gates), which is more than most plan docs attempt.

However, the subsystem docs are **uneven**, and the two most important subsystems for the
product's differentiators are the **weakest on testing**:

- **09 (Editor)** — the flagship differentiator — has **no mutation section and no CI-gate
  section at all**, despite its own text calling the editor "a state machine — ideal for unit +
  property + mutation" (§9). This is the single biggest gap.
- **10 (Pane System)** — the shell of the product — has **no mutation, no CI-gate section, and no
  explicit unit/property tests for its pure layout-engine transformations** (detach/re-dock),
  which are the most testable logic in the whole product.

There are also **real cross-doc inconsistencies** between 15's mutation-gate scope and what 08/11
claim, and between 15's CI gate list and 16's performance gates. Several performance targets are
hand-wavy (no concrete numbers, no measurement basis). Verdict: **the plan is directionally right
but not yet "check everything" — it must be tightened before it can be called TDD-at-inception
with full rigor.**

---

## (b) Critical issues (must fix)

### C1. Editor (09) has no mutation testing and no CI-gate section
`plan/09-editor.md` §9 covers unit, property, component, integration, e2e — but **never mentions
mutation testing, coverage thresholds, or CI gates**. Its own §9 opening says the editor "is a
state machine — ideal for unit + property + **mutation** + component + integration coverage," yet
no mutation plan follows. The editor is the flagship differentiator and the most safety-relevant
UI logic (diff-apply mutates the user's working tree). A grep for `mutation|mutants|70%|85%|80%`
in 09 returns **zero matches**. This must be fixed: define which editor logic is mutation-gated
(buffer, diff-apply, undo, selection) and state the CI gate order.

### C2. Pane System (10) has no mutation, no CI-gate section, and no unit/property tests for the layout engine
`plan/10-ui-pane-system.md` §9 is titled "Component Testing" and covers only component + snapshot
tests. The layout engine is repeatedly described as **pure and serializable** (§3.1, §4.1, §4.2)
— "detach is a pure tree transformation," "re-dock is the inverse transformation" — which makes
it the ideal target for unit + property + mutation tests. Yet there is **no unit-test section, no
mutation section, and no CI-gate section**. The only property test mentioned is a round-trip
snapshot test (§9.2), which is shallow relative to what the pure engine warrants. The layout
engine is the product's shell; its correctness (focus routing, detach/re-dock, split collapse)
should be mutation-gated.

### C3. Mutation-gate scope in 15 contradicts 08 and 11
`plan/15-testing-strategy.md` §2.3 and §9 Q1 scope the mutation gate to **"core crates
(orchestration, provider-adapter, wire contract, checkpointing, HAR)"** — explicitly **excluding
terminal, editor, and browser**. But:
- `plan/08-terminal.md` §9.5 says "Terminal changes must pass the full ladder: … mutation
  (cargo-mutants; ≥85% line, ≥80% branch, ≥70% mutation score)."
- `plan/11-system-browser-integration.md` §10.5 says "Mutation tests target the
  detection/launch/port-parsing logic."

So 08 and 11 claim their subsystems are mutation-gated, while 15's authoritative scope omits
them. Either the scope list in 15 is wrong (it should include terminal/editor/browser core logic)
or 08/11 overclaim. This must be reconciled — the reader cannot tell which subsystems are
actually mutation-gated. Same ambiguity applies to the coverage gate ("enforced on the core
crates," §8).

### C4. Performance gates in 16 are not integrated into 15's CI pipeline
`plan/16-performance.md` §9.2 says performance is "gated in CI (in the standard gate order, with
perf checks as part of integration/coverage **or a dedicated perf stage**)" — the "or" is a hedge,
and **15's CI gate list (§5) contains no performance stage at all**. The hard gates 16 promises
("cold start < 300 ms, input latency < 16 ms (p95), memory under budget … fails CI") have **no
home in the authoritative gate definition**. If performance is a core differentiator and a hard
CI gate, 15 must name where it lives (a dedicated perf stage between integration and component,
or within coverage). As written, the two docs disagree about whether a perf gate exists.

---

## (c) Major issues (should fix)

### M1. Security controls in 11 have no testing plan
`plan/11-system-browser-integration.md` §9 defines a real attack surface (remote-debugging port,
cookies, profile access) with controls: random port, localhost-only bind, origin allow-list,
per-launch token, short-lived session, no remote exposure, process hygiene. **None of these
controls has a test.** §10 tests detection/launch/port-parsing but nothing asserts "the port is
bound to 127.0.0.1 only," "a connection without the token is rejected," "the debugging session is
killed on pane close," or "no orphaned debugging browser after a panic." For a security-sensitive
capability this is a gap. Add security-focused unit/integration tests (and consider them
mutation-gated, since a mutant that opens the port or drops the token check must be killed).

### M2. Windows terminal path is "first-class" but has no dedicated test section
`plan/08-terminal.md` §8.4 says "Windows is the harder one and gets the most test attention," and
§8.3 says "add integration tests for the common TUIs" for ConPTY quirks — but there is **no
Windows-specific test section** and no enumeration of which TUIs, which ConPTY quirks, or how
resize/full-screen behavior is asserted. Given Windows-first is a differentiator and ConPTY is
the riskiest backend, this needs a concrete test list (e.g. `vim`, `htop`, `cargo test`
full-screen redraw, resize storms, job-object tree-kill verification).

### M3. Editor LSP integration test can silently degrade to mock-only
`plan/09-editor.md` §9.4: "launch a real language server (e.g. `rust-analyzer` if present, **else
a scripted mock LSP server**)." Unlike 11's explicit "skipped (not silently passed)" policy
(§10.5), 09 does not specify skip semantics. In CI without `rust-analyzer`, the "else" branch
means the real-LSP path is never exercised and the test still passes — a silent coverage gap for
the LSP integration that is a core editor feature. Specify: skip-not-fail with an explicit
marker, or a CI job that installs a real server.

### M4. Terminal "real-binary smoke tests" assert shallowly
`plan/08-terminal.md` §9.2: "run `vim`, `htop`-style TUIs … and assert the resulting frames
contain expected cells." "Contain expected cells" is vague and shallow — no golden-frame
comparison, no assertion on cursor position, scrollback, or full-screen redraw correctness. For
an emulator integration this is the kind of shallow assertion the mandate warns against. Specify
golden cell-grid snapshots or structural assertions (e.g. specific cells at specific
row/col, cursor state).

### M5. Performance targets lack concrete numbers and measurement basis
- **Cold start < 300 ms** (`16` §3.5): the budget table ("~50 ms", "~100 ms", "~100 ms") is
  hand-wavy with no measurement basis, and it accounts for "first frame" but the target is a
  **usable editor** — editor init is lazy (§3.2) and not in the budget. Also, GPUI-on-Windows
  cold start is unproven (Zed's precedent is macOS/Linux-first); this is a real risk that should
  be flagged, not assumed.
- **Memory "far below Electron"** (`16` §7.4): no concrete number and no defined budget — §9.2
  says "memory under budget" but the budget is never specified. A gate with an undefined
  threshold is not a gate.
- **"Dozens of concurrent subagents"** (`16` §5, §9.1): "dozens" is undefined (24? 48? 96?), the
  fan-out benchmark is a **soft** gate ("scales ~linearly to dozens"), and raising the built-in
  16-child cap depends on vendoring depth (open question §10.3). The headline differentiator has
  no hard, quantified gate.

### M6. Input-latency measurement methodology is under-specified
`16` §9.1 lists an `input_latency` benchmark ("Keystroke → rendered frame, < 16 ms p95") but does
not say how keystroke-to-frame latency is measured in a headless/CI environment (GPUI frame-time
instrumentation? synthetic input injection? hardware vsync?). Without a defined measurement
method, the hard gate is not enforceable. Specify the instrumentation and the reference-machine
class (which is itself an open question, §10.6).

---

## (d) Minor issues

### m1. E2E cadence is internally inconsistent in 15
`15` §3.3 says e2e runs "on a **merge gate** (and nightly), not on every commit," but §5 says
"Merge requires all green … There is no 'skip e2e for this small change' path." §9 Q2 then lists
the cadence as an **open question** (merge vs nightly+merge-for-critical). These three statements
don't fully agree. Pick one and state it.

### m2. 15's mutation "floor" vs "gate" wording
`15` §2.3 says "The 70% kill threshold is a floor, not a target" and "New core modules must reach
it before merge." Fine — but §1.1's table and §2.3's gate table present 70% as the gate. Minor
wording tension; clarify that 70% is the merge floor and the bar may rise.

### m3. 08's performance "How" column is hand-wavy
`08` §10 lists targets (input latency, frame throughput, idle cost, scrollback memory, cold start)
with "How" descriptions ("Worker thread + lock-free frame channel", "Dirty-region re-upload") but
**no measurement mechanism and no benchmark** — unlike 16 §9. Terminal-specific perf (frame
throughput, idle cost) has no way to be enforced. Reference 16's benchmark suite or add
terminal-specific benchmarks.

### m4. 10's snapshot tests vs pure-engine property tests
`10` §9.2's round-trip test is "serialize → deserialize → re-render must be identity … property-
based with proptest over random split/resize/detach sequences." Good — but it's the *only*
property test for the engine, and it's framed as a snapshot concern. The pure detach/re-dock
transformations (§4.1, §4.2) deserve direct unit/property tests of the transformation functions
themselves (e.g. detach then re-dock is identity; focus_path is always a valid root→leaf path).

### m5. 12 is strong but its mutation section is the only dedicated one
`12` §8.2 is the only subsystem doc with a dedicated mutation section (mandatory high score on
redaction + timing — excellent). The other docs should follow this pattern rather than burying
mutation in a one-line CI-gate mention (08 §9.5, 11 §10.5) or omitting it entirely (09, 10).

---

## (e) Testing-coverage gaps per subsystem

| Doc | Unit | Property | Mutation | Component | Integration | E2E | CI gates | Coverage thresholds | Verdict |
|---|---|---|---|---|---|---|---|---|---|
| **08 Terminal** | ✅ | ✅ (in unit) | ⚠️ (1 line, no targets) | ✅ | ✅ | ✅ | ✅ | ✅ (in gate line) | Good, but mutation under-specified; no Windows-specific tests; shallow TUI assertions |
| **09 Editor** | ✅ | ✅ | ❌ **none** | ✅ | ✅ | ✅ | ❌ **none** | ❌ **none** | **Weakest — flagship has no mutation/CI-gate plan** |
| **10 Pane System** | ❌ (no unit section) | ⚠️ (1 round-trip) | ❌ **none** | ✅ | ⚠️ (via 15) | ⚠️ (via 15) | ❌ **none** | ❌ **none** | **Weak — pure layout engine untested at unit/mutation level** |
| **11 Browser** | ✅ | ✅ (in unit) | ⚠️ (1 line, targets named) | ✅ | ✅ | ✅ | ✅ | ✅ (in gate line) | Good, but **security controls (§9) untested** |
| **12 HAR** | ✅ | ✅ | ✅ **dedicated** | ✅ | ✅ | ✅ | ✅ | ✅ | **Best — model for the others** |
| **15 Strategy** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | Strong; scope/consistency issues (C3, m1) |
| **16 Performance** | n/a | n/a | n/a | n/a | n/a | n/a | ⚠️ (not in 15) | ⚠️ (budget undefined) | Good structure; targets under-specified (M5, M6) |

**Subsystems with NO testing plan at all:** none of 08–12 is entirely without a plan, but three
critical areas have **no plan**: (1) editor mutation + CI gates (09), (2) pane-engine unit/mutation
(10), (3) browser security controls (11 §9). These are the gaps that violate "check everything."

---

## (f) Specific findings (doc + section references)

1. **09 §9** — no mutation, no CI-gate, no coverage-threshold content; contradicts its own §9
   opening ("ideal for unit + property + mutation"). (C1)
2. **10 §9** — titled "Component Testing" only; no unit/mutation/CI-gate sections despite the
   engine being "pure and serializable" (§3.1, §4.1, §4.2). (C2)
3. **15 §2.3 / §9 Q1** mutation scope = "orchestration, provider-adapter, wire contract,
   checkpointing, HAR" — omits terminal/editor/browser, contradicting **08 §9.5** and **11 §10.5**.
   (C3)
4. **16 §9.2** "perf checks as part of integration/coverage **or** a dedicated perf stage" vs
   **15 §5** gate list with no perf stage. (C4)
5. **11 §9** security controls (random port, localhost-only, origin allow-list, token, short-lived,
   process hygiene) — no tests anywhere in §10. (M1)
6. **08 §8.3/§8.4** "Windows … gets the most test attention" + "add integration tests for the
   common TUIs" — no Windows-specific test section, no TUI enumeration. (M2)
7. **09 §9.4** LSP integration "else a scripted mock LSP server" — no skip-not-fail semantics,
   unlike **11 §10.5**. (M3)
8. **08 §9.2** "assert the resulting frames contain expected cells" — shallow; no golden-frame or
   structural assertion. (M4)
9. **16 §3.5** cold-start budget "~50/~100/~100 ms" — no measurement basis; "usable editor" not in
   budget; GPUI-on-Windows cold start unproven. (M5)
10. **16 §7.4 / §9.2** memory "far below Electron" / "under budget" — no concrete number, undefined
    budget. (M5)
11. **16 §5 / §9.1** "dozens of concurrent subagents" undefined; fan-out is a soft gate; raising the
    16-child cap depends on vendoring depth (§10.3). (M5)
12. **16 §9.1** `input_latency` benchmark — measurement method in headless CI unspecified. (M6)
13. **15 §3.3 vs §5 vs §9 Q2** — e2e cadence (merge gate vs nightly vs open question) inconsistent.
    (m1)
14. **08 §10** — terminal perf targets have no measurement/enforcement mechanism; should reference
    16's benchmark suite. (m3)
15. **10 §9.2** — only property test is a snapshot round-trip; pure detach/re-dock transformations
    deserve direct unit/property tests. (m4)
16. **12 §8.2** — the only dedicated mutation section; the pattern other docs should adopt. (m5)

---

## Bottom line

The plan's *intent* is right and 15/12/16 are strong, but the mandate "check everything" is not
yet met: the **editor (09)** and **pane system (10)** — two of the product's core differentiators —
have no mutation/CI-gate plans, the **mutation-gate scope in 15 contradicts 08/11**, the
**performance hard gates in 16 have no home in 15's CI pipeline**, and several performance targets
are unquantified. Fix C1–C4 first; they are the difference between "we have a testing strategy"
and "we enforce it everywhere."
