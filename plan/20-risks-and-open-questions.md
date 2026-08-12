# Plan 20 — Risks & Open Questions

**Status:** Planning · **Owner:** subagent fan-out → adversarial review
**Consistency:** This doc follows `docs/PLAN-CONTEXT.md` and `docs/DECISIONS.md` exactly. Decisions D1–D40 are LOCKED in `docs/DECISIONS.md` and are applied here (see "Locked decisions applied" below); this doc does not re-litigate them. Only genuinely-new open questions not covered by DECISIONS.md remain open (§6.9). Conflicts found during review are surfaced here rather than silently resolved.

**Locked decisions applied (2026-08-12):** This revision applies the LOCKED decisions from `docs/DECISIONS.md`. D-numbers applied here: **D36** (Orca baseline = match all), **D10** (embedding = Phase-0 go/no-go hypothesis), **D35** (Windows-first conditional + ACP fallback), **D30** (monetization/GTM), **D31** (track upstream). The open-questions section (§6) is updated to reflect that D1–D40 are now resolved in `docs/DECISIONS.md`.

---

## 1. Purpose & scope

This is the risk register and decision log for the Multiplexer project. It consolidates the technical, competitive, product, and business risks that cut across the other plan docs, assigns each a likelihood/impact and a mitigation owner, and restates every open question from PLAN-CONTEXT with its tradeoffs and a recommendation.

It is **not** a substitute for the per-subsystem risk tables in `plan/03` (vendoring), `plan/06` (orchestration), `plan/11` (browser), `plan/12` (HAR), `plan/09` (editor), and `plan/18` (build/release). Those docs carry the deep, subsystem-specific detail; this doc is the **consolidated, cross-cutting view** and the **de-risking priority order**.

---

## 2. Technical risks

The biggest technical risks, ordered roughly by the damage they could do to the schedule and the differentiators.

### 2.1 Windows build of the vendored grok-build crates

**The single largest technical risk in the project.** Upstream states Windows builds are *"best-effort, not currently tested from this tree."* Windows-first is a core differentiator (Superset and Conductor are macOS-only), so this risk sits directly on the product's reason to exist.

- **Likelihood:** High. Unix-only code paths (`#[cfg(unix)]`, signals, `fork`/`exec`, `termios`, Unix sockets, sandbox isolation) are pervasive in a harness built and tested on macOS/Linux.
- **Impact:** Critical. If the embedded harness cannot build and run on Windows, the #1 differentiator (in-process embedding) and the Windows-first positioning both collapse. The ACP fallback would keep the product functional but would forfeit the embedding advantage.
- **Mitigation:** Treat as a **first-class, early workstream** (Plan 03 §5), not a post-MVP afterthought. Phased bring-up: (W1) per-crate compile audit and a "Windows readiness" matrix; (W2) port `xai-grok-shell` stdio/headless drivers first (no TUI dependency — cleanest target); (W3) port `xai-grok-tools` + `xai-grok-workspace` behind small platform traits; (W4) config/MCP/markdown/sandbox, deciding sandbox disposition; (W5) a **dedicated Windows CI job** that builds all embedded crates and runs the embedding tests. **The embedding is not "done" until Windows CI is green.** Keep the ACP fallback so a Windows regression degrades gracefully rather than taking the app fully down.
- **Contingency (D35, LOCKED):** Windows-first is **conditional on the Phase-0 spike** (D10), not a guaranteed win. If the Windows grok-build build fails or is delayed, the fallback is to **ship the ACP path on Windows** (drive the installed `grok` binary over ACP) while the **in-process embedding lands on macOS/Linux first**. The competitive docs (plan/01) must frame this as "Windows-primary," not "Windows-only," and present it as conditional on the spike. This contingency is reflected in the de-risking priorities (§8.1) and the risk register (T1).
- **Owner:** Harness/platform engineer (dedicated), with CI ownership.

### 2.2 GPUI maturity / build effort

GPUI is younger and smaller than Electron's ecosystem; some widgets must be built by hand.

- **Likelihood:** Medium-High. GPUI is production-proven by Zed, but for *our* specific widget set (pop-out panes, split-anything layout, terminal embedding, editor) there is real build effort and some gaps.
- **Impact:** High. A UI that is slow to build delays the whole product; a UI that under-delivers on "beautiful, blazing-fast" undermines the brand.
- **Mitigation:** Reuse Zed's open-source GPUI components and patterns where licensing permits (Zed is GPL — **must verify** license compatibility before copying code; see §5.2). Keep the UI **thin over the server contract** so a future UI rewrite is cheap and the risk is contained to the shell, not the core. Component/snapshot tests (Plan 15) catch layout regressions early. Set realistic expectations: the native UI is genuinely more work than Electron — budget accordingly in the roadmap (Plan 19).
- **Owner:** UI lead.

### 2.3 Embedding the grok-build harness in-process

The differentiator depends on a stable, embeddable API surface in a codebase we do not control. **Per D10 (LOCKED), this is a Phase-0 go/no-go hypothesis, not a settled moat** — the plan and competitive docs must present it as a hypothesis to be proven, not a guaranteed win.

- **Likelihood:** Medium. Upstream API drift is expected; the shell's public surface is not a stable, versioned contract. The README documents only headless + ACP integration — **no in-process library API** — so embeddability itself is unproven until the Phase-0 spike.
- **Impact:** High. Breaking API changes ripple through the adapter layer and can stall feature work or force a sync migration. If the shell is not cleanly embeddable, the #1 differentiator collapses and we fall back to ACP.
- **Mitigation:** Treat the `xai-grok-shell` public API as a **stable seam** (Plan 03 §4.3, Plan 05): our provider-adapter layer wraps it, so upstream shifts only touch the adapter. Pin `SOURCE_REV`; adopt a deliberate sync cadence (2–4 weeks); run contract tests on both the embedded and ACP paths so drift is caught by CI, not in production. When upstream introduces a breaking change, pin to last-known-good and schedule the migration deliberately rather than absorbing it mid-sprint.
- **Phase-0 gate (D10, LOCKED):** The first Phase-0 deliverable is a **spike**: clone grok-build, consume `xai-grok-shell` as a library, run a headless turn in-process, and get the crates building on Windows. This is the **go/no-go** for the embedding differentiator. **Fallback:** if the shell is not cleanly embeddable, we fall back to the **ACP path** (drive `grok agent stdio`/`serve`), which is fully supported and documented. The plan keeps both paths.
- **Owner:** Harness/platform engineer + orchestration lead.

### 2.4 System-browser CDP integration across browsers

Differentiator #3 (drive the user's real installed browsers, **no bundled Chromium**) depends on a fragmented browser-automation landscape.

- **Likelihood:** High. Browsers differ: Chrome/Edge/Arc/Brave speak CDP; **Firefox uses WebDriver BiDi, not CDP**; Safari's WebDriver is a separate beast. Version churn and per-browser auth flows add surface.
- **Impact:** High. If browser support is flaky or browser-specific, the browser pane, Design Mode, and HAR capture all suffer — three differentiators at once.
- **Mitigation:** Abstract the browser driver behind a trait with two implementations: a **CDP driver** (Chrome/Edge/Arc/Brave) and a **WebDriver BiDi driver** (Firefox), with Safari handled via WebDriver or deferred. Scope MVP to the CDP browsers (the majority) and treat Firefox/Safari as follow-on. Detect/import via a per-browser discovery layer (registry/plist/known paths). Share the CDP connection between the browser pane and HAR capture (Plan 11/12). Fail gracefully: if a browser can't be driven, show a clear message rather than a broken pane.
- **Owner:** Browser integration lead.

### 2.5 HAR capture/replay complexity

HAR capture via CDP is straightforward; **replay** is the hard part.

- **Likelihood:** Medium-High. CDP `Network.enable` + `Network.getResponseBody` gives capture; replay requires re-serving recorded responses, handling dynamic content, auth, WebSockets, and timing.
- **Impact:** Medium-High. Capture is table stakes; replay is the differentiator. A replay that only works on trivial static pages is a demo, not a feature.
- **Mitigation:** Phase it: (1) capture + waterfall visualization first (high value, low risk); (2) deterministic replay of recorded responses with a local proxy/interceptor; (3) advanced replay (WebSocket, auth, timing) as follow-on. Reuse the CDP connection from the browser subsystem. Set explicit scope boundaries in Plan 12 so "replay" doesn't silently balloon.
- **Owner:** HAR lead.

### 2.6 Native editor scope

The "real editor" differentiator (multi-cursor, LSP, Vim mode, inline diff-apply) is a large, well-known-hard problem.

- **Likelihood:** High (that it's a big effort). Zed proves it's *possible* on GPUI, but it's a multi-person-year class of work to do well.
- **Impact:** High. Editor scope is the biggest single scope-creep risk in the product (see §4.1) and can delay MVP substantially.
- **Mitigation:** Reuse Zed's editor architecture/patterns where licensing permits. The MVP editor scope is **resolved (D4): full native editor** (rope buffer, multi-cursor, undo/redo, tree-sitter, Vim mode, LSP) in the MVP — the earlier "lighter editor first" recommendation is overridden. Keep the editor thin over the server contract so scope can be trimmed without re-architecture. Sequence the full editor build deliberately (Plan 09) rather than letting it grow organically.
- **Owner:** Editor lead.

### 2.7 Concurrency / parallel scheduler correctness

The event-sourced orchestration with per-thread queues + a parallel scheduler is the correctness heart.

- **Likelihood:** Medium. Concurrency bugs (races, deadlocks, lost updates, out-of-order events) are subtle and hard to find.
- **Impact:** High. A corrupt read model or a deadlocked scheduler breaks every client and erodes trust in the "control surface" promise.
- **Mitigation:** This is exactly where **TDD-at-inception pays off**: pure decider + projector in one transaction (Plan 06), property-based tests (proptest) over the state machine and command orderings, mutation testing, and integration tests against the real embedded runtime with a mock agent. The read model is the only shared mutable structure and is written transactionally; concurrent work serializes only on the short projector transaction. No global lock on the hot path.
- **Owner:** Orchestration lead.

### 2.8 Secondary technical risks (summary)

| Risk | Likelihood | Impact | Mitigation / owner |
|------|-----------|--------|--------------------|
| **Ghostty terminal embedding** on Windows (PTY differences) | Medium | Medium | Use `portable-pty`/`windows-sys` console APIs; abstract PTY behind a trait (Plan 08) |
| **Sandbox crate is Unix-only** (namespaces/seccomp/chroot) | Medium | Medium | Gate behind `cfg(unix)`; no-op/limited Windows sandbox or defer in MVP (Plan 03 §5) |
| **Checkpointing on NTFS** (symlinks, line endings, file locking) | Medium | Medium | Rely on Git-for-Windows; set `core.autocrlf` policy; test checkpoints on NTFS (Plan 07) |
| **protoc / DotSlash tooling friction on Windows** | Medium | Low | Pin protoc; make DotSlash optional; fail-fast bootstrap checks (Plan 03 §6) |
| **Embedding blocks the main event loop** (perf) | Medium | High | Run the shell on a dedicated thread/async runtime; keep GPUI loop responsive (<16 ms, Plan 16) |
| **SQLite read-model contention** under dozens of subagents | Low-Medium | Medium | Transactional writes, snapshot reads; benchmark fan-out early (Plan 16) |
| **Remote/relay security** (ticket auth, DPoP, tunnel) | Medium | High | WebSocket ticket auth (5-min TTL), passkeys/DPoP, Tailscale serve; security review (Plan 14/17) |

### 2.9 Upstream churn / merge burden (D31, LOCKED)

grok-build is a **read-only mirror synced ~daily from the SpaceXAI monorepo**, with a hyper-active release cadence (1.0.0 on 2026-08-07, 1.0.1 on 2026-08-10; before that multiple 0.2.x releases **per day**). See `docs/UPSTREAM-TRAJECTORY.md`. Our vendored fork must absorb this churn.

- **Likelihood:** High. Daily sync + multiple releases/day means near-constant upstream movement; the changelog is the only signal (no issues/PRs/discussions).
- **Impact:** Medium-High. Fast merge churn consumes engineering time, risks breaking our fork's Windows fixes and `[patch]` wiring, and can stall feature work if we fall behind.
- **Mitigation:** Adopt a **standing "track upstream" task** (D31, LOCKED) on the roadmap: monitor the grok-build changelog (daily sync, only signal) + T3 Code issues/commits, and **re-validate the competitive snapshot quarterly** (see §3.5). This is a **recurring task, not one-time research**. Budget for fast merge churn on the fork (D5); pin `SOURCE_REV` and merge (not rebase) on a deliberate cadence; keep the ACP fallback so a bad upstream merge degrades gracefully. Watch the subagent fan-out cap closely — upstream is actively changing it (1.0.1 "bounded fan-out"), which gates our "dozens of concurrent subagents" differentiator (D11).
- **Owner:** Harness/platform engineer + orchestration lead.

---

## 3. Competitive risks

The market is moving fast. The main risk is not that competitors copy us — it's that they **close their specific gaps** and erode our differentiators.

### 3.1 Orca adds HAR / system-browser import

Orca is the strongest competitor and already bundles Chromium. If Orca ships HAR capture and/or drops its bundled-Chromium approach for system-browser import, two of our differentiators (#3, #4) lose their edge.

- **How we stay ahead:** Our **in-process embedding** (#1) and **native editor** (#2) are structural advantages Orca cannot quickly match (it drives CLIs; it has no editor). We should ship HAR and system-browser **early and well** (they're in the MVP differentiator set) so we're the reference implementation, and lean on the editor + embedding as the moat. Speed matters: these are not "someday" features.
- **Owner:** Product lead.

### 3.2 Superset adds Windows support

Superset is macOS-only; Windows-first is our wedge. If Superset ships Windows, we lose the "only Windows option" positioning.

- **How we stay ahead:** Windows-first is a *shipping* decision, not a feature — we must actually ship Windows before they do. Our editor, mobile companion, and in-process embedding remain gaps for Superset regardless. Treat the Windows build (risk §2.1) as the competitive clock.
- **Owner:** Product lead + platform engineer.

### 3.3 OpenCode adds orchestration / control-surface features

OpenCode is an *agent*, not a control surface, but it's huge (195K stars) and could grow a multi-harness orchestration layer or a worktree fleet.

- **How we stay ahead:** Our differentiators are the **control surface** (multi-harness orchestration, worktree fleet, HAR, browser, mobile, real editor) — the layer *above* the agent. OpenCode becoming a better *agent* doesn't threaten that; it could even become a provider we drive via ACP. Keep the provider-adapter layer open so we ride improvements in any agent rather than competing with all of them.
- **Owner:** Product lead.

### 3.4 T3 Code / Codex Desktop close their gaps

T3 Code lacks an editor, e2e, and mutation tests; Codex Desktop is closed and single-provider. Both could improve.

- **How we stay ahead:** Our **TDD-at-inception** (e2e + mutation) is a durable quality moat that's hard to bolt on late. The native editor and in-process embedding are structural. Keep shipping the differentiators that are *architectural*, not just feature-list items.
- **Owner:** Product lead.

### 3.5 General competitive posture

- **Differentiators are architectural, not cosmetic.** Embedding, native editor, event-sourced orchestration, and TDD are hard to copy quickly. Feature-level differentiators (HAR, browser import) are copyable — ship them first and best.
- **Revisit `plan/01-competitive-analysis.md` on a cadence** (e.g., quarterly) to re-validate the gap table against reality.

---

## 4. Product risks

### 4.1 MVP scope creep

The vision is enormous (editor + terminal + browser + HAR + mobile + remote + multi-harness). The biggest product risk is trying to ship all of it at once and shipping nothing well.

- **Mitigation:** Ruthless MVP scoping. The MVP must hold the **core differentiators** (embedding, editor, browser, HAR) but can defer the long tail (Vim mode, Firefox/Safari, advanced replay, multi-provider). The Orca-baseline scope is **resolved (D36/D7): match ALL baseline features** across Phases 1–5, with MVP = Phases 1–4 (D8). Use Plan 19's milestones to sequence and cut.
- **Owner:** Product lead.

### 4.2 Mobile stack choice

The paired mobile app is **required**, but the stack is undecided (native SwiftUI/Kotlin vs Expo/React Native).

- **Risk:** Choosing native doubles mobile effort (two codebases) and slows the required mobile companion; choosing Expo/React Native risks performance and native-feel on a "blazing-fast" brand.
- **Mitigation:** The architecture is stack-agnostic (thin client over the wire contract; mobile consumes `multiplexer-wire` via codegen). Because the mobile app is *observe/control* (not the editor), a cross-platform stack is a much better fit than it would be for the desktop. **Resolved (D2): Expo/React Native** for the mobile companion (see §6.2).
- **Owner:** Mobile lead.

### 4.3 Editor scope

The editor is the largest single scope item and the most likely to balloon (see §2.6).

- **Mitigation:** MVP editor scope is **resolved (D4): full native editor** (rope buffer, multi-cursor, undo/redo, tree-sitter, Vim mode, LSP) in the MVP. The earlier "lighter editor first" recommendation is overridden. Keep the editor thin over the contract so scope can be trimmed without re-architecture, but plan for the full editor as the MVP bar.
- **Owner:** Editor lead + product lead.

### 4.4 "No bundled Chromium" UX risk

Driving the user's real browsers is a differentiator, but it's also a UX risk: browser version drift, missing browsers, and auth friction can make the browser pane feel less reliable than a bundled engine.

- **Mitigation:** Clear detection/import UX, graceful degradation, and a well-scoped MVP browser set (CDP browsers first). Document the tradeoff honestly in the UI.
- **Owner:** Browser lead.

---

## 5. Business risks

### 5.1 Branding / domain choice

We own **Multiplexer.dev** and **Multiplexor.dev**. The brand decision is **resolved (D6)**.

- **Risk:** Low technical, but a wrong or late brand decision is costly to reverse (marketing, domains, naming in code/artifacts).
- **Mitigation:** **Resolved (D6): Multiplexer.dev** is the product brand; **Multiplexor.dev** is a redirect/defensive registration. Lock the brand name into the binary/product naming before wide distribution.
- **Owner:** Founder/user.

### 5.2 Licensing (Apache 2.0 obligations)

We vendor `xai-org/grok-build` (Apache 2.0) and may reuse Zed GPUI components (GPL).

- **Risk:** Medium-High if mishandled. Apache 2.0 requires retaining license/copyright notices and a notice file; **GPL reuse is a serious concern** — copying Zed code into our (presumably proprietary or differently-licensed) product could impose GPL obligations.
- **Mitigation:** Preserve upstream license/copyright headers in the vendored fork; ship a `THIRD-PARTY-NOTICES` file listing grok-build and its license; document fork provenance. **Before reusing any Zed GPUI code, verify license compatibility** — prefer Apache/MIT-licensed GPUI components or write our own where GPL is a problem. This is a legal/compliance item to confirm with the user, not something we decide unilaterally.
- **Owner:** Founder/user + legal review.

### 5.3 Distribution / code signing

Windows-first means dealing with Windows distribution realities: code signing (SmartScreen), installer/updater, and store vs direct-download.

- **Risk:** Medium. An unsigned or poorly-distributed Windows app is a trust and adoption problem; signing certs and CI signing pipelines add cost and process.
- **Mitigation:** Plan signing (EV or OV cert), a signed installer (e.g., MSIX/NSIS), and an auto-update path from day one (Plan 18). Budget for cert costs. Decide store distribution (Microsoft Store) vs direct download.
- **Owner:** Release engineer + founder.

### 5.4 Single-vendor dependency on grok-build

Our #1 differentiator depends on a harness we don't control (upstream could change direction, slow down, or change licensing).

- **Risk:** Medium. We maintain our own fork, but the upstream project's health affects our sync burden and roadmap.
- **Mitigation:** The fork is ours and durable; the provider-adapter layer means we can back onto ACP or other harnesses if needed. Keep the ACP fallback as a strategic hedge. Monitor upstream health.
- **Owner:** Harness/platform engineer.

### 5.5 Monetization / go-to-market (D30, LOCKED)

The monetization model is **freemium** (D30): a free tier (local, single-provider, core features) plus a paid tier (multi-provider, remote/relay, mobile advanced, usage analytics, priority support). The risk is that the model, pricing, and go-to-market are not validated before launch.

- **Risk:** Medium. Freemium conversion is hard to get right; pricing the paid tier (multi-provider, remote/relay, mobile advanced) against free alternatives (Orca, T3 Code, OpenCode are all free/open) is a real challenge. A weak GTM or a mis-priced tier can stall adoption and revenue.
- **Mitigation:** Add a **monetization/GTM section** to the plan docs (plan/00, 01, 18, 19) alongside this risk register. Define the free vs paid tier boundary explicitly and early (what stays free, what gates to paid). Validate pricing and willingness-to-pay against the competitive landscape before GA. Treat GTM as a first-class workstream, not an afterthought.
- **Owner:** Founder/user + product lead.

---

## 6. Open questions (from PLAN-CONTEXT)

**All decisions D1–D40 are now LOCKED in `docs/DECISIONS.md` (2026-08-12).** The eight open questions from PLAN-CONTEXT are **resolved** — this doc no longer treats them as pending user decisions. They are listed below with their resolved outcome for reference; the authoritative statement of each is in `docs/DECISIONS.md`. Only genuinely-new open questions not covered by DECISIONS.md remain open (§6.9).

### 6.1 Stack: Rust + GPUI vs Electron+React — **RESOLVED (D1)**

- **Resolved:** **Rust + GPUI** (GPU-rendered), NOT Electron. See D1.

### 6.2 Mobile: native (SwiftUI/Kotlin) vs Expo/React Native — **RESOLVED (D2)**

- **Resolved:** **Expo / React Native** for the mobile app (iOS + Android). Desktop UI stays Rust+GPUI. See D2.

### 6.3 MVP scope: Grok-only vs multi-provider from day one — **RESOLVED (D3)**

- **Resolved:** **Grok Build only** (in-process embedding) for MVP; other providers added after MVP via the provider-adapter pattern. Custom models (e.g., `ds-flash`) supported from day one as a config feature. See D3, D14.

### 6.4 Editor scope: full native editor in MVP vs lighter editor first — **RESOLVED (D4)**

- **Resolved:** **Full native editor** (rope buffer, multi-cursor, undo/redo, tree-sitter, Vim mode, LSP) in the MVP. See D4. *(This overrides the earlier "lighter editor first" recommendation in §4.3 and §8.5 — see note below.)*

### 6.5 grok-build vendoring: submodule vs vendored copy vs `[patch]` — **RESOLVED (D5)**

- **Resolved:** **Vendored fork under `third_party/` + `[patch]` wiring**, tracking `SOURCE_REV` with periodic merge. See D5.

### 6.6 Branding: which domain is the product brand vs redirect — **RESOLVED (D6)**

- **Resolved:** **Multiplexer.dev** is the product brand; **Multiplexor.dev** is a redirect/defensive registration. See D6.

### 6.7 Orca baseline scope: match all Orca features in MVP vs subset — **RESOLVED (D36, D7)**

- **Resolved:** **Match ALL baseline features** across Phases 1–5 (D7). The earlier "subset in MVP" recommendation is **overridden by D36** — the default is match all, consistent with plan/00 and plan/01. MVP = Phases 1–4 (D8). See D7, D8, D36.

### 6.8 Windows-first: confirm — **RESOLVED (D9, D35)**

- **Resolved:** **Windows-first confirmed** (D9), but **conditional on the Phase-0 spike** (D35): if the Windows grok-build build fails or is delayed, ship the **ACP path on Windows** while in-process embedding lands on macOS/Linux first. Frame as "Windows-primary," not "Windows-only." See D9, D35, and §2.1.

### 6.9 Additional open questions raised by this review (flag, don't decide)

The following are **genuinely new** — not covered by DECISIONS.md — and remain open for the user:

1. **Sandbox disposition on Windows:** gate out, no-op, or limited port in MVP? (Plan 03 §5)
2. **Zed GPUI code reuse:** confirm license compatibility (GPL) before reusing any Zed components (§5.2).
3. **Licensing/notices:** confirm `THIRD-PARTY-NOTICES` approach and any legal review needed.
4. **Distribution channel:** Microsoft Store vs direct download (Plan 18). *(Signing itself is resolved — D29: Azure Trusted Signing + budget.)*

---

## 7. Risk register (consolidated)

| # | Risk | Likelihood | Impact | Mitigation owner |
|---|------|-----------|--------|------------------|
| T1 | Windows build of vendored grok-build crates | High | Critical | Platform engineer + CI |
| T2 | GPUI maturity / build effort | Med-High | High | UI lead |
| T3 | In-process embedding API drift | Medium | High | Harness/platform + orchestration |
| T4 | System-browser CDP/BiDi fragmentation | High | High | Browser lead |
| T5 | HAR capture/replay complexity | Med-High | Med-High | HAR lead |
| T6 | Native editor scope | High | High | Editor lead + product |
| T7 | Concurrency / scheduler correctness | Medium | High | Orchestration lead |
| T8 | Ghostty terminal embedding on Windows | Medium | Medium | Terminal lead |
| T9 | Sandbox crate Unix-only | Medium | Medium | Platform engineer |
| T10 | Checkpointing on NTFS | Medium | Medium | VCS/checkpoint lead |
| T11 | protoc/DotSlash tooling friction | Medium | Low | Platform engineer |
| T12 | Embedding blocks main event loop | Medium | High | Orchestration + perf |
| T13 | SQLite read-model contention | Low-Med | Medium | Orchestration + perf |
| T14 | Remote/relay security | Medium | High | Security lead |
| T15 | Upstream churn / merge burden (D31) | High | Med-High | Harness/platform + orchestration |
| C1 | Orca adds HAR / system-browser | Medium | High | Product lead |
| C2 | Superset adds Windows | Medium | Medium | Product + platform |
| C3 | OpenCode adds orchestration | Medium | Medium | Product lead |
| C4 | T3/Codex close gaps | Medium | Medium | Product lead |
| P1 | MVP scope creep | High | High | Product lead |
| P2 | Mobile stack choice | Medium | Medium | Mobile lead |
| P3 | Editor scope balloon | High | High | Editor + product |
| P4 | "No bundled Chromium" UX risk | Medium | Medium | Browser lead |
| B1 | Branding / domain choice | Low | Medium | Founder/user |
| B2 | Licensing (Apache 2.0 / GPL) | Med-High | High | Founder + legal |
| B3 | Distribution / code signing | Medium | Medium | Release engineer |
| B4 | Single-vendor dependency on grok-build | Medium | Medium | Harness/platform |
| B5 | Monetization / GTM (D30) | Medium | Medium | Founder + product |

---

## 8. De-risking priorities

What to de-risk **first**, in order. These are the tasks that, if they fail, invalidate the architecture or the differentiators — so they must be proven early, before broad feature investment.

### 8.1 Priority 1 — Windows grok-build build (T1)

**Prove the #1 differentiator works on the #1 platform before anything else.** Stand up the fork, run the phased Windows bring-up (Plan 03 §5), and get a **Windows CI job green** for the embedded `xai-grok-shell` + tools + workspace. This is the make-or-break technical task. If it fails, we must know before we invest in the editor, browser, and HAR. **Contingency (D35):** if the Windows build fails or is delayed, ship the **ACP path on Windows** while in-process embedding lands on macOS/Linux first — Windows-first is conditional on this spike (D10).

### 8.2 Priority 2 — GPUI shell (T2)

**Prove the UI can deliver the "beautiful, blazing-fast" promise.** Build a minimal GPUI shell with the pane system and a real editor buffer; validate cold-start < 300 ms and input latency < 16 ms on Windows. This de-risks both the UI effort and the performance targets (Plan 16). A working shell also gives the team a daily driver for everything else.

### 8.3 Priority 3 — Embedding seam + orchestration correctness (T3, T7)

**Prove the in-process embedding end-to-end with a mock agent**, and prove the event-sourced orchestration is correct under concurrency (proptest + integration). This validates the two architectural hearts (embedding, orchestration) and the TDD-at-inception gates before feature breadth.

### 8.4 Priority 4 — Browser + HAR on CDP (T4, T5)

**Prove system-browser import and HAR capture on the CDP browsers** (Chrome/Edge/Arc/Brave) early. These are copyable differentiators — ship them first and best. Defer Firefox/BiDi and advanced replay.

### 8.5 Priority 5 — Editor MVP scope (T6, P3)

**Lock the MVP editor scope** — **resolved (D4): full native editor** in the MVP — and prove inline diff-apply + basic LSP early. This is the biggest scope-creep risk; the scope is now decided (D4), so the focus is on sequencing the full editor build, not re-deciding scope.

### 8.6 Priority 6 — Business/legal decisions (B1, B2, B3, B5)

**Resolve licensing (Zed GPL check) and distribution/signing early** — they're cheap to decide now and costly to reverse later. Branding (D6), signing (D29), and monetization/GTM (D30) are resolved; the remaining open items are the Zed GPL check and the distribution channel (§6.9).

---

## 9. Consistency notes

- This doc is consistent with `docs/PLAN-CONTEXT.md` and `docs/DECISIONS.md`. No contradictions found.
- The eight open questions from PLAN-CONTEXT are **resolved by `docs/DECISIONS.md` (D1–D40)**; this doc reflects those resolutions in §6 and the affected risk/priority sections.
- If any locked decision is revisited, the affected risks and de-risking priorities (§2, §7, §8) must be updated to match.
