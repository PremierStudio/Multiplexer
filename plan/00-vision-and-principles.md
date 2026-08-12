# 00 — Vision & Principles

**Status:** Approved (consistent with `docs/PLAN-CONTEXT.md`)
**Owner:** Multiplexer planning fan-out
**Scope:** Product vision, differentiators, baseline bar, design principles, success criteria, MVP non-goals, branding.

**Locked decisions applied:** D10 (embedding as Phase-0 hypothesis), D35 (Windows-first conditional), D30 (monetization/GTM), D6 (branding), D8 (MVP definition). See `docs/DECISIONS.md`.

---

## 1. Product Vision

**Multiplexer is a beautiful, blazing-fast desktop client / control surface for the Grok Build harness — extensible to other models and harnesses.**

It is the place where a developer *lives* while their agents work. Today, running agents means juggling a terminal, a browser, a diff viewer, a chat window, and a phone — none of which talk to each other. Multiplexer collapses that sprawl into one native, GPU-rendered surface that **owns** the agent runtime in-process, gives you a **real editor**, **real performance**, and **real insight** into what your agents are doing.

### Who it's for

- **Power users of agentic coding harnesses** — developers already running Grok Build, Claude Code, Codex, or OpenCode daily and hitting the ceiling of terminal-only workflows.
- **Multi-agent orchestrators** — engineers who fan out many subagents on parallel tasks and need a live dashboard, not a wall of scrolling logs.
- **Windows-first developers** — the largest desktop OS, underserved by the macOS-only incumbents (Superset, Conductor).
- **Mobile operators** — developers who want to observe and steer long-running agent sessions from their phone.

### The problem it solves

| Problem today | Multiplexer's answer |
|---|---|
| Agents run in a terminal TUI with no real editor | Native GPU editor with inline diff-apply, LSP, multi-cursor, Vim mode |
| Harness is a black box — you can't see *why* it acted | Event-sourced read model + live orchestration dashboard + HAR profiler |
| Browser automation bundles a 100MB+ Chromium | Import the user's installed browser and drive it via CDP |
| No visibility into network behavior | Built-in HAR capture, waterfalls, and session replay |
| One agent at a time, serialized | Dozens of concurrent subagents with a real scheduler |
| Desktop-bound | Paired mobile app over the same server runtime |
| Electron bloat (hundreds of MB, slow) | Native Rust + GPUI, cold start < 300ms |
| macOS-only incumbents | Windows-first, then macOS/Linux |

**Positioning statement:** *"The control surface for your agents, with a real editor, real performance, and real insight."*

---

## 2. The 10 Core Differentiators

These are the ten commitments that make Multiplexer distinct. **All ten must hold** — none is optional.

### 1. In-process grok-build embedding *(Phase-0 go/no-go hypothesis)*
We intend to vendor the `xai-org/grok-build` crates (Apache 2.0) and call the agent runtime **directly as a library** — no shelling out to a CLI, no ACP protocol overhead. The agent loop, tools, and workspace would run in our process. **Nobody else does this** (competitors drive CLIs or spawn `grok agent stdio`). This is the foundation of our performance and insight advantage.

**This is a hypothesis to be proven, not a settled moat.** The first Phase-0 deliverable is a spike: clone grok-build, consume `xai-grok-shell` as a library, run a headless turn in-process, and get the crates building on Windows. That spike is the **go/no-go gate** for this differentiator. If the shell is not cleanly embeddable, we fall back to the **ACP path** (drive `grok agent stdio`/`serve`), which is fully supported and documented — the plan keeps both paths open until the spike resolves.

### 2. Native, blazing-fast editor
Rust + GPUI, GPU-rendered. Inline diff-apply, LSP, multi-cursor, Vim mode. This is the "real editor" that no competitor ships (T3, Superset, Conductor, Codex all lack one).

### 3. System-browser integration
Detect and import the user's installed browsers (Chrome, Edge, Firefox, Safari, Arc, Brave), launch/authorize, and drive them via CDP. **No bundled Chromium** — saves ~100MB+ and respects the user's real browser profile.

### 4. Built-in HAR profiler/replayer
Capture network traffic via CDP, visualize waterfalls, and replay recorded sessions. No competitor has this.

### 5. Powerful pop-out pane UI
Outlook-style left chat sidebar, center build pane, multi-purpose right bar (browser / HAR / files / diff / terminal / agent activity), optional pop-up terminal below. **Every pane can pop out to its own window.**

### 6. Multi-harness / multi-model
Grok in-process first; DeepSeek/OpenRouter, Claude, Codex, OpenCode via a provider-adapter pattern. One surface, many harnesses.

### 7. Subagent orchestration at scale
Fan out many subagents on specific tasks with a live orchestration dashboard — not a serialized queue.

### 8. Paired mobile app
The same server runtime, controlled and observed from the phone.

### 9. Windows-first *(conditional on the Phase-0 spike)*
Superset and Conductor are macOS-only — a real gap we exploit. We build on Windows and ship **Windows-primary** first, then macOS/Linux. This is **conditional on the Phase-0 spike**: if the Windows grok-build build fails or is delayed, the contingency is to ship the **ACP path** (drive the installed `grok` binary over ACP) on Windows while in-process embedding lands on macOS/Linux first. Frame as "Windows-primary," not "Windows-only."

### 10. TDD at inception
Full unit + mutation tests, component tests, integration tests, deep assertions, and coverage thresholds in CI — from day one, not retrofitted.

---

## 3. The Baseline Bar (Orca's feature set)

We must **match** Orca (onorca.dev), the strongest competitor. These are table stakes — not differentiators, but the floor we cannot ship below.

| Baseline capability | What it means |
|---|---|
| Parallel isolated worktrees | Multiple independent agent workspaces running concurrently |
| Ghostty-class terminal with splits | A real, fast terminal with split panes |
| Design Mode | Click a browser element → agent acts on it |
| SSH remote worktrees | Work on remote machines over SSH |
| Inline diff comments → agent | Comment on a diff line and route it back to the agent |
| GitHub / Linear native | First-class integrations with both |
| Mobile companion | Paired mobile app (also a differentiator) |
| Account / usage tracking | Usage metering and account management |
| Split-anything panes | Any pane can be split arbitrarily |
| Native search | Fast, native search across the workspace |
| Orca CLI | A command-line entry point |

**Note:** The exact MVP scope of the Orca baseline is an open question (see §7 and `plan/20`). The default position is to match the full set, but the fan-out may propose a defensible subset for the MVP.

---

## 4. Design Principles

These principles govern every decision across the product.

### Beautiful
- GPU-rendered UI (GPUI), not web-in-a-box. Smooth 60fps+ animations, crisp typography, deliberate spacing.
- Aesthetic is a feature: a control surface you *want* to keep open.

### Clean
- Progressive disclosure: the surface is calm by default; complexity is revealed on demand.
- Every pane earns its place. No clutter, no redundant chrome.

### Blazing fast
- Cold start to usable editor **< 300ms**.
- Input latency **< 16ms** (60fps+).
- Memory far below Electron competitors.
- Performance is a design constraint, not an afterthought (see `plan/16`).

### Powerful UI
- A real editor, real terminal, real browser, real HAR tooling — not stubs.
- Deep keyboard-first workflows (Vim mode, command palette) alongside mouse-driven panes.

### TDD at inception (non-negotiable)
- Unit + property tests, mutation tests (cargo-mutants), component tests, integration tests, e2e.
- CI gates: fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e → coverage. **All green before merge. No blind CI.**

### Windows-first
- Windows is the primary shipping target; macOS/Linux follow. Windows build support for the vendored grok-build is **our responsibility** (upstream is best-effort/untested).
- **Conditional on the Phase-0 spike** (D35): if the Windows grok-build build fails or is delayed, fall back to the **ACP path** on Windows while in-process embedding lands on macOS/Linux first. Windows-primary, not Windows-only.

### Privacy & security
- OS keychain for local secrets; `op://Vault/Item/field` references only in configs (never raw values).
- OAuth for providers; passkeys/DPoP for remote.
- No bundled Chromium means no hidden data collection; the user's browser stays the user's browser.

---

## 5. Success Criteria / North Star Metrics

"Winning" is measurable. These are the gates we hold ourselves to.

| Metric | Target |
|---|---|
| Cold start → usable editor | **< 300ms** |
| Input latency | **< 16ms** (60fps+) |
| Concurrent subagents | **Dozens** without serialization bottleneck |
| Memory footprint | **Far below** Electron competitors |
| Mutation-testing gates | **Green** in CI (≥85% line, ≥80% branch, ≥70% mutation score killed) |
| Mobile companion | **Shipped** and paired to the same server runtime |
| E2E coverage | Real app/headless driven — beats T3 Code (which has none) |
| Windows-first | Windows is the first-class shipping platform (conditional on the Phase-0 spike; ACP fallback as contingency) |

**North Star:** *A developer can start Multiplexer, fan out a dozen subagents, watch them work in a real editor with real insight, and steer them from their phone — all under 300ms to first keystroke and 16ms of input latency.*

---

## 6. Non-Goals for the MVP

**MVP definition (D8):** The MVP is **Phases 1–4** — Core MVP + Editor/Panes + Browser/HAR + Mobile/Remote — and **includes the required mobile app** (which lives in Phase 4). Phase 5 (multi-provider + scale) and Phase 6 (GA) are post-MVP. The mobile app cannot slip past the MVP.

Explicitly **out of scope** for the first release. These are not abandoned — they are deferred.

- **Full multi-provider parity from day one.** Grok in-process is the MVP harness; DeepSeek/OpenRouter, Claude, Codex, OpenCode arrive via the provider-adapter pattern after the core is proven. *(Open question 3 — default is Grok-only MVP.)*
- **Full native editor feature-completeness.** The editor ships with the core (inline diff-apply, LSP, multi-cursor, Vim), but a lighter editor may precede the full one. *(Open question 4.)*
- **macOS/Linux shipping in the MVP.** Windows-primary; other platforms follow (conditional on the Phase-0 spike).
- **Web client in the MVP.** The server runtime supports it, but the desktop + mobile clients ship first.
- **Bundled browser.** We never bundle Chromium — system-browser integration only.
- **Third-party plugin marketplace.** Extensibility comes via the provider-adapter and pane system, not a plugin store, in the MVP.
- **Enterprise SSO / admin console.** Account/usage tracking is baseline, but enterprise identity is deferred.
- **Non-coding agent domains.** Multiplexer is a coding-agent control surface; it is not a general business tool.

---

## 7. Branding Note

**Branding decision (D6, LOCKED):** **Multiplexer.dev** is the product brand and primary domain. **Multiplexor.dev** is a defensive registration that **301-redirects** to Multiplexer.dev (catches typos, prevents squatting).

- **Multiplexer.dev** is the canonical brand for all product surfaces, docs, and packaging.
- **Multiplexor.dev** redirects to it — never split branding across both.
- This decision is resolved; it is no longer an open question.

---

## 7b. Monetization / GTM (D30)

**Freemium (LOCKED):** a free local tier plus a paid tier.

- **Free tier:** local, single-provider (Grok in-process), core features.
- **Paid tier:** multi-provider, remote/relay, mobile advanced, usage analytics, priority support.

The free tier is the wedge that gets developers using the product locally; the paid tier monetizes the multi-provider, remote, and mobile-advanced surface. Detailed pricing/GTM lives in `plan/18` and `plan/19`/`plan/20`.

---

## 8. Open Questions Referenced

Per PLAN-CONTEXT, these are pending user decisions and must not be decided unilaterally. This doc references them where relevant; they are tracked in `plan/20-risks-and-open-questions.md`.

1. Stack: Rust + GPUI (recommended) vs Electron+React — **assumed Rust + GPUI throughout this doc.**
2. Mobile: native (SwiftUI/Kotlin) vs Expo/React Native.
3. MVP scope: Grok-only vs multi-provider from day one.
4. Editor scope: full native editor vs lighter editor first.
5. grok-build vendoring: submodule vs vendored copy vs `[patch]` (recommend vendored fork under `third_party/` + `[patch]`).
6. Branding: which domain is the product brand vs redirect — **resolved (D6): Multiplexer.dev is the brand; Multiplexor.dev redirects.**
7. Orca baseline scope: match all Orca features in MVP vs subset.
8. Windows-first: confirm — **resolved (D35): Windows-primary, conditional on the Phase-0 spike, ACP fallback as contingency.**

---

*Next: `plan/01-competitive-analysis.md` — deep dive on Orca, T3 Code, Superset, Conductor, OpenCode, Codex Desktop.*
