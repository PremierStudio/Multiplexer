# Multiplexer — Plan Review Summary (consolidated)

**Date:** 2026-08-12
**Status:** 21 plan docs authored (plan/00–20), then adversarially reviewed by 3 reviewers (architecture, testing, competitive/security). This file consolidates the findings.

## What was produced

- `docs/PLAN-CONTEXT.md` — authoritative shared context (architecture, decisions, competitors, testing).
- `plan/00-vision-and-principles.md` … `plan/20-risks-and-open-questions.md` — 21 detailed plan docs, each authored by a dedicated subagent.
- `docs/review-architecture.md`, `docs/review-testing.md`, `docs/review-competitive-security.md` — three adversarial reviews.

## Headline verdict

**The architecture and engineering discipline are strong and defensible.** The server-centric single-binary runtime, thin clients over JSON-RPC/WebSocket, in-process grok-build embedding, event-sourced orchestration, and TDD-at-inception are all well-designed and internally coherent at the top level.

**But the plan is NOT ready to lock.** Three categories of issues must be resolved:

1. **The core differentiator is a hypothesis, not a fact.** "In-process grok-build embedding" and "Windows-first" both hinge on a single unverified Phase-0 question: can `xai-grok-shell` actually be consumed as a stable library, and does it build on Windows? The grok-build README documents only headless + ACP integration — **no in-process library API** — so this must be proven with a Phase-0 spike before the competitive narrative is locked.
2. **Real cross-doc inconsistencies** in the adapter↔wire↔orchestration mapping (approval-decision shape, crate layout, event vocabulary, OpenRouter adapter identity, subagent-cap ownership).
3. **Testing is uneven** — the two flagship differentiators (editor, pane system) have no mutation/CI-gate plans, and several performance targets are unquantified.

---

## A. Architecture review — critical/major findings

| # | Severity | Finding | Docs |
|---|---|---|---|
| C1 | Critical | **Approval-decision shape differs**: wire uses 4-way enum (`allow/deny/allow_once/allow_always`), adapter trait uses `bool`, orchestration uses untyped `decision`. The boolean adapter can't carry `allow_once`/`allow_always`. | 04 §4.3, 05 §2, 06 §5.1, 17 |
| M1 | Major | **Crate layout mismatch**: 02/19 use fine-grained `mx-*` split; 04/05 use consolidated `multiplexer-*` crates. Model registry location differs. | 02 §4, 04 §9.1, 05, 19 |
| M2 | Major | **"Wire events map 1:1 onto ProviderEvent, no transformation" is false** — vocabularies differ in granularity, naming, fields; wire is a superset (terminal/HAR/fs/telemetry have no ProviderEvent counterpart). | 04 §1/§5, 05 §3 |
| M3 | Major | **OpenRouter/DeepSeek adapter described 3 ways**: own crate (02/19), config variant of in-process-grok (05 §6), future HTTP adapter (05 §7). | 02, 05, 19 |
| M4 | Major | **Subagent 16-child cap tension**: 02/03 say we inherit it "for free"; 06 says our scheduler can raise it. Gates the "dozens of concurrent subagents" differentiator. | 02 §3.3, 03 §4.1, 06 §4.1 |
| m1–m5 | Minor | Event vocabulary drift (`TurnCompleted` vs `TurnFinished`), ACP adapter role ambiguity, unbounded adapter channel vs wire backpressure, `session.start` vs `start_session` params, `mx-mobile-shared` vs `multiplexer-wire` duplication. | 02, 04, 05, 06 |

## B. Testing review — critical/major findings

| # | Severity | Finding | Docs |
|---|---|---|---|
| C1 | Critical | **Editor (09) has no mutation testing and no CI-gate section** — the flagship differentiator. Grep confirms zero mutation/coverage/CI mentions. | 09 §9 |
| C2 | Critical | **Pane System (10) has no unit/mutation/CI-gate sections** despite its layout engine being "pure and serializable" (most testable logic in the product). | 10 §9 |
| C3 | Critical | **Mutation-gate scope contradiction**: 15 scopes mutation to core crates (excluding terminal/editor/browser), but 08 §9.5 and 11 §10.5 claim their subsystems are mutation-gated. | 15 §2.3, 08 §9.5, 11 §10.5 |
| C4 | Critical | **Performance gates in 16 have no home in 15's CI pipeline** (16 hedges "or a dedicated perf stage"; 15's gate list has none). | 16 §9.2, 15 §5 |
| M1 | Major | Browser security controls (11 §9) have **no tests at all**. | 11 §9 |
| M2 | Major | Windows terminal path has no Windows-specific test section. | 08 §8 |
| M3 | Major | Editor LSP integration test can silently degrade to mock (no skip-not-fail). | 09 §9.4 |
| M4 | Major | Terminal TUI smoke tests assert shallowly ("frames contain expected cells"). | 08 §9.2 |
| M5 | Major | Performance targets hand-wavy: cold-start budget no measurement basis, memory budget undefined, "dozens of subagents" unquantified. | 16 |
| M6 | Major | Input-latency measurement method in headless CI unspecified. | 16 §9.1 |

**Positive:** 15 (testing strategy), 12 (HAR), and 16 (performance) are strong. 12 is the model subsystem doc (dedicated mutation section).

## C. Competitive/security review — critical/major findings

| # | Severity | Finding | Docs |
|---|---|---|---|
| C1 | Critical | **In-process embedding moat asserted, not verified** — no evidence `xai-grok-shell` has a stable embeddable library API; plan's own risk register admits the shell surface is unstable. Must be a Phase-0 go/no-go. | 01 §4.1, 20 §2.3 |
| C2 | Critical | **Windows-first is simultaneously "#1 risk" and "guaranteed win"** — docs contradict; no fallback positioning. | 20 §2.1 vs 01 §4.5 |
| C3 | Critical | **Orca baseline bar unverified** (no sources); "macOS-first in practice" vs "macOS/Windows/Linux" tension. | 01 §1.1/§3/§4.5 |
| M1 | Major | **plan/17 secrets policy contradicts the machine's global policy** — proposes runtime `op://` resolution (requires banned live `op` reads or unspecified 1Password SDK). | 17 §2.3/§2.4 |
| M2 | Major | **Relay "end-to-end encrypted, sees no plaintext" claim is false as designed** — a TLS-terminating Cloudflare Worker relay sees plaintext; DPoP/tickets authenticate but don't encrypt. | 14 §4.1/§6.2/§6.4 |
| M3 | Major | **Mobile "required" vs Phase-4 staging** — MVP definition never fixed; mobile could slip past MVP. | 13, 19 |
| M4 | Major | **Windows code-signing cost/identity reality under-specified** (OV cert $200–400+/yr, identity verification lead time). | 18 §3.2/§9 |
| M5 | Major | **"No bundled Chromium" has an unaddressed hole**: no-browser UX and CI headless-Chromium sourcing unresolved; HAR is CDP-only (Firefox/Safari degrade). | 11, 01 |
| M6 | Major | **SSH `--remote` agent trust boundary unaddressed** — no independent enforcement of permission modes on the remote host. | 14 §3.1, 17 §7 |
| F5 | Major | **No monetization/pricing/GTM anywhere** in the plan suite. | 00, 01, 18, 19, 20 |

**Positive:** CDP hardening, permission modes, HAR privacy-by-default, supply-chain controls, and the thin-client wire contract are genuinely strong.

---

## D. The single most important finding

**The entire business case rests on one unverified Phase-0 question: can we embed `xai-grok-shell` as a library and build it on Windows?**

- The grok-build README documents **only** headless mode and ACP SDK integration — there is **no documented in-process library API**.
- The plan's own risk register rates the Windows build as the #1 risk (High/Critical).
- Yet the competitive docs (01) and vision (00) present in-process embedding and Windows-first as settled moats.

**Recommendation:** Make Phase 0's first deliverable a **spike**: clone grok-build, attempt to consume `xai-grok-shell` as a library, run a headless turn in-process, and get the crates building on Windows. This is a **go/no-go** for the embedding differentiator. Until it passes, present embedding and Windows-first as *hypotheses to be proven*, not facts.

---

## E. Recommended fixes (priority order)

1. **Phase-0 embedding + Windows spike** as a go/no-go gate (resolves C1/C2 of competitive review, M4 of architecture).
2. **Reconcile the adapter↔wire↔orchestration mapping**: pick one approval-decision type (4-way enum), one crate layout, one event vocabulary, one OpenRouter-adapter identity, and add an explicit wire↔ProviderEvent↔engine-event mapping table (resolves architecture C1, M1–M3, m1–m5).
3. **Resolve who owns subagent scheduling** and how the 16-child cap is raised (architecture M4).
4. **Add mutation + CI-gate plans to the editor (09) and pane system (10)**; reconcile mutation-gate scope across 15/08/11; add a perf stage to 15's CI pipeline (testing C1–C4).
5. **Fix the security items before Phase 4**: reconcile plan/17 with the global secrets policy (M1), correct the relay E2EE claim (M2), define the remote-agent trust boundary (M6), add browser-security tests (testing M1).
6. **Define MVP explicitly** (recommend MVP = Phases 1–4, i.e., full baseline bar + core differentiators) so the required mobile app can't slip (competitive M3).
7. **Add a signing budget and a monetization/GTM section** (competitive M4, F5).

## F. Open questions still pending user decision (from PLAN-CONTEXT)

1. Stack: Rust + GPUI (recommended) vs Electron+React.
2. Mobile: native vs Expo/React Native.
3. MVP scope: Grok-only vs multi-provider from day one.
4. Editor scope: full native editor in MVP vs lighter first.
5. grok-build vendoring: submodule vs vendored copy vs `[patch]`.
6. Branding: which domain is the product brand vs redirect.
7. Orca baseline scope: match all vs subset in MVP.
8. Windows-first: confirm.
