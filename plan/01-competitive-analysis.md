# 01 — Competitive Analysis

> **Status:** Authoritative plan doc. Consistent with `docs/PLAN-CONTEXT.md` (the shared plan context). If any statement here conflicts with PLAN-CONTEXT, PLAN-CONTEXT wins and the conflict should be flagged in `plan/20-risks-and-open-questions.md`.
>
> **Purpose:** Define the competitive set, quantify where Multiplexer wins/ties/lags, identify the strategic whitespace, and derive a defensible positioning statement. This doc is the input to `plan/02-architecture.md` and the roadmap in `plan/19-roadmap-and-milestones.md`.

> **Locked decisions applied (2026-08-12):** This doc has been updated to reflect the locked decisions in `docs/DECISIONS.md`. Applied here: **D10** (in-process embedding is a Phase-0 go/no-go hypothesis, not a settled moat), **D28** (HAR is CDP-only — Chromium-family caveat), **D30** (freemium monetization/GTM), **D34** (plan-docs removed from the differentiator list), **D35** (Windows-first is conditional on the Phase-0 spike, framed as Windows-primary). The competitive snapshot is **sourced as of 2026-08-12** (see §8) and is re-validated quarterly per **D31**/plan/20.

---

## 1. The competitive set

Multiplexer is a **desktop client / control surface for the Grok Build harness** — a native, GPU-rendered front end over an in-process agent runtime, with a real editor, real performance, and real insight (HAR, orchestration, mutation-gated quality). The competitive set is every product that a user might reach for instead of Multiplexer to drive coding agents.

We classify the set into three tiers:

| Tier | Products | Relationship to Multiplexer |
|------|----------|---------------------------|
| **Direct control surfaces** | Orca, T3 Code, Superset, Conductor | Build a UI over coding agents; closest substitutes |
| **Agent CLIs / IDEs** | OpenCode, Codex Desktop | Agents with a thin shell; overlap on "drive an agent," but not control surfaces |
| **Excluded** | OpenChamber.io | Not a coding tool at all |

### 1.1 Orca (onorca.dev) — the strongest competitor

MIT-licensed, cross-platform (macOS/Windows/Linux), native UI with a WebGL Ghostty-class terminal and an **embedded Chromium** for browser work. Orca is the baseline bar we must match (see §3). Its feature set is the most complete of the set, and it is the only competitor that ships a mobile companion, account/usage tracking, and a CLI alongside a polished desktop app.

### 1.2 T3 Code

Open source-available, Electron + React + Effect, multi-provider (Grok via ACP, plus others), server-centric with thin clients over Effect RPC WebSocket, remote/relay, mobile, and source control. Architecturally the closest to us (server-centric, event-sourced orchestration, Ghostty terminal, hidden-git-ref checkpoints) — but it is a **reference, not a fork** (see §3.2 of PLAN-CONTEXT). Its gaps are the ones we exploit.

### 1.3 Superset (superset.sh)

ELv2-licensed, **macOS-only** (no Windows). Parallel worktrees, diff review, persistent terminals, automations, an MCP server, and OAuth 2.1 + PKCE. Polished and opinionated, but platform-locked to macOS and missing editor, mobile, browser, and HAR.

### 1.4 Conductor (conductor.build)

Closed-source, **macOS-only**. Runs parallel Claude/Codex/Cursor agents in isolated workspaces. Strong on parallel-agent orchestration, but closed, macOS-only, and lacks an editor and mobile.

### 1.5 OpenCode (opencode.ai)

MIT, ~195K stars, terminal/IDE/desktop. 75+ providers via Models.dev, LSP support, multi-session, share links. OpenCode is an **agent**, not a control surface — it has no multi-harness orchestration, no worktree fleet, no HAR, and no mobile companion.

### 1.6 Codex Desktop

Closed-source, Electron, the official Codex chat + terminal. Single-provider (Codex), no editor, no mobile. A reference point for "official vendor shell," not a serious control-surface competitor.

### 1.7 OpenChamber.io — explicitly NOT a competitor

OpenChamber.io is a **business-formation / roadmap tool**, not a coding agent or control surface. It does not drive agents, edit code, or manage worktrees. It is excluded from the competitive set entirely; we do not allocate analysis or roadmap budget to it. (Included here only to pre-empt confusion.)

---

## 2. Detailed comparison table

| | **Multiplexer** | **Orca** | **T3 Code** | **Superset** | **Conductor** | **OpenCode** | **Codex Desktop** |
|---|---|---|---|---|---|---|---|
| **What it is** | Control surface + native editor over in-process Grok Build | Control surface over coding agents | Server-centric multi-provider client | Worktree/diff agent client | Parallel-agent workspace | Agent CLI/IDE | Official Codex shell |
| **License** | Apache 2.0 (core) + proprietary UI | MIT | Source-available | ELv2 | Closed | MIT | Closed |
| **Platforms** | **Windows-primary** (conditional on Phase-0 spike), then macOS/Linux | macOS/Windows/Linux | macOS/Windows/Linux (Electron) | **macOS only** | **macOS only** | macOS/Windows/Linux | macOS/Windows/Linux |
| **Stack** | Rust + GPUI (GPU) | Native + WebGL + embedded Chromium | Electron + React + Effect | Native (Swift) | Native (Swift) | Terminal/IDE (TS) | Electron |
| **In-process harness** | **Yes (vendored grok-build)** | No (drives CLIs) | No (spawns `grok agent stdio`) | No | No | No | No |
| **Native editor** | **Yes (GPUI)** | No | No | No | No | IDE plugin | No |
| **HAR profiler/replayer** | **Yes (built-in)** | No | No | No | No | No | No |
| **System-browser import** | **Yes (CDP, no bundled Chromium)** | No (bundles Chromium) | No | No | No | No | No |
| **Parallel worktrees** | Yes | Yes | Yes | Yes | Yes | No | No |
| **Terminal** | Ghostty (embedded) | Ghostty-class (WebGL) | Ghostty | Persistent terminals | Yes | Terminal | Terminal |
| **Design Mode** | Yes | Yes | Partial | No | No | No | No |
| **SSH remote worktrees** | Yes | Yes | Yes | No | No | No | No |
| **Inline diff comments → agent** | Yes | Yes | Yes | Diff review | No | No | No |
| **GitHub / Linear native** | Yes | Yes | Yes | Partial | No | No | No |
| **Mobile companion** | **Yes (required)** | Yes | Yes | No | No | No | No |
| **Account / usage tracking** | Yes | Yes | Yes | Yes | No | No | No |
| **CLI** | Yes (Orca-class) | Yes | Yes | No | No | Yes | No |
| **Native search** | Yes | Yes | Yes | No | No | No | No |
| **Split-anything panes** | Yes | Yes | Yes | No | No | No | No |
| **Multi-harness orchestration** | **Yes (provider adapters)** | Partial | Yes | No | Claude/Codex/Cursor | No | No |
| **Mutation-testing CI gates** | **Yes (cargo-mutants)** | No | No | No | No | No | No |
| **E2E tests** | **Yes** | Partial | **No** | Partial | No | No | No |
| **Subagent fan-out dashboard** | **Yes** | No | No | No | Yes (parallel) | No | No |

**Reading the table:** Multiplexer is the only product that is simultaneously (a) a control surface, (b) a native editor, (c) an in-process harness embedder, and (d) an insight tool (HAR + orchestration + mutation gates). Every competitor wins on at most one or two of those axes.

---

## 3. Deep-dive: Orca (the strongest competitor)

Orca is the baseline bar. PLAN-CONTEXT states we **must match** its feature set. This section enumerates each Orca capability, what it means, and what Multiplexer must do to match or exceed it.

### 3.1 Parallel isolated worktrees
Orca lets an agent work in an isolated worktree per task, so multiple agents can operate in parallel without stepping on each other. **We must match:** our checkpointing/VCS layer (`plan/07`) must provide isolated worktrees per thread/subagent, with clean merge-back semantics. This is a hard requirement, not optional.

### 3.2 Ghostty-class terminal with splits
Orca ships a WebGL-rendered, Ghostty-class terminal with split panes. **We must match:** we embed Ghostty directly (`plan/08`), which gives us a real native terminal rather than a WebGL emulation — a performance and fidelity advantage, not just parity.

### 3.3 Design Mode (browser element → agent)
Orca lets the user click an element in a rendered browser and hand it to the agent as context. **We must match:** our system-browser integration (`plan/11`) must support element selection via CDP and feed the selected element (with its DOM/accessibility tree) into the agent turn.

### 3.4 SSH remote worktrees
Orca can run agents against worktrees on a remote host over SSH. **We must match:** our remote/relay layer (`plan/14`) covers SSH plus local, paired, and relay-tunnel modes — a superset of Orca's SSH support.

### 3.5 Inline diff comments → agent
Orca lets the user comment on a diff line and route it back to the agent as feedback. **We must match:** our editor's inline diff-apply (`plan/09`) must support line-anchored comments that become agent turns.

### 3.6 GitHub / Linear native
Orca has first-class GitHub and Linear integrations (issues, PRs, tickets). **We must match:** native GitHub and Linear integration is in the baseline bar.

### 3.7 Mobile companion
Orca pairs with a mobile app to control/observe agents. **We must match:** PLAN-CONTEXT makes the paired mobile app **required** (`plan/13`), sharing the same server runtime over the JSON-RPC/WebSocket contract.

### 3.8 Account / usage tracking
Orca tracks account and usage. **We must match:** account/usage tracking is in the baseline bar (see `plan/17` for how this is done without leaking secrets).

### 3.9 Orca CLI
Orca ships a CLI for headless/scripted control. **We must match:** we ship an Orca-class CLI, which also serves as our headless/e2e driver.

### 3.10 Native search
Orca has fast native search across code and history. **We must match:** native search is in the baseline bar.

### 3.11 Split-anything panes
Orca's panes can split arbitrarily and pop out. **We must match:** our pop-out pane system (`plan/10`) is explicitly designed for split-anything + pop-out-to-window, matching and exceeding Orca's layout flexibility.

### 3.12 What Orca does NOT have (our edge)
Orca **bundles Chromium** (no system-browser import), has **no HAR profiler/replayer**, does **no mutation testing**, and **drives CLIs** rather than embedding the harness in-process. These are the four structural gaps we exploit (§4, §5).

---

## 4. The strategic whitespace — the 6 things nobody has

These six capabilities are the core differentiators from PLAN-CONTEXT. Each is something **no competitor** ships today. They are the reason Multiplexer exists. (A seventh candidate — the plan-docs suite — is an internal process artifact, not a customer-facing differentiator; see the note at the end of this section.)

### 4.1 In-process harness embedding — a hypothesis to be proven in Phase 0
We intend to vendor the `xai-grok-build` crates and call the agent runtime **directly** — no shelling out to a CLI, no ACP protocol overhead, no process boundary. Every competitor (including Orca and T3 Code) drives agents as external processes. In-process embedding would give us lower latency, richer introspection (we see the runtime's internals, not just its stdout), and the ability to instrument the agent loop for HAR/orchestration. **Nobody else does this.**

**This is a hypothesis, not a settled moat (D10).** The first Phase-0 deliverable is a go/no-go spike: clone grok-build, consume `xai-grok-shell` as a library, run a headless turn in-process, and get the crates building on Windows. If the shell is not cleanly embeddable, we fall back to the **ACP path** (drive the installed `grok` binary over ACP), which is fully supported and documented. We do not claim structural permanence for the embedding differentiator until the spike passes; the plan keeps both paths live.

### 4.2 HAR profiler / replayer
We capture network traffic via CDP, visualize waterfalls, and replay recorded sessions. This is a debugging/insight superpower that no competitor ships. It turns Multiplexer from "a place to run agents" into "a place to *understand* what the agent's code actually did on the network."

**Caveat — HAR is CDP-only (D28):** HAR capture works only on the **Chromium family** (Chrome, Edge, Brave, Arc, and Chromium-based browsers we import). Firefox (BiDi) and Safari (WebDriver) get **reduced or no HAR**. This is not a universal win across all imported browsers; we should scope the HAR differentiator to Chromium-based browsers and be explicit about the reduced coverage elsewhere.

### 4.3 System-browser import (no bundled Chromium)
We detect and import the user's installed browsers (Chrome, Edge, Firefox, Safari, Arc, Brave), launch/authorize them, and drive them via CDP. **No bundled Chromium.** This is lighter, faster to start, respects the user's existing profiles/logins, and avoids the multi-hundred-MB binary bloat that Orca's embedded Chromium carries.

### 4.4 Native GPU editor
A real native editor (Rust + GPUI, GPU-rendered) with inline diff-apply, LSP, multi-cursor, and Vim mode. No competitor in the control-surface set has a built-in editor at all — they all punt editing to an external IDE. We own the edit loop end-to-end.

### 4.5 Windows-primary — conditional on the Phase-0 spike
Superset and Conductor are **macOS-only**. Orca and T3 Code are cross-platform and **do ship Windows**, though they are macOS-first in practice. We ship **Windows first**, then macOS/Linux — making us **Windows-primary**, not Windows-only. For a large Windows developer population, Multiplexer is a serious control surface that treats Windows as a primary platform.

**This is conditional on the Phase-0 spike (D35).** If the Windows grok-build build fails or is delayed, the fallback is to ship the **ACP path** (drive the installed `grok` binary over ACP) on Windows while in-process embedding lands on macOS/Linux first. Windows-first is a bet to be proven in Phase 0, not a guaranteed win.

### 4.6 Mutation-testing CI gates
We gate CI on cargo-mutants mutation scores (≥85% line, ≥80% branch, ≥70% mutation score killed). No competitor enforces mutation testing. This is a *quality* differentiator: our codebase is measurably more robust, and it signals engineering discipline that enterprise buyers care about.

---

> **Internal process note (not a differentiator, D34):** We author a full plan-doc suite (`plan/00` … `plan/20`) that documents the orchestration, architecture, and testing strategy up front. This is an **internal process artifact** that de-risks execution — it is **not** a customer-facing differentiator and is removed from the whitespace list above. It should not be marketed or counted among the six whitespace items.

---

## 5. Feature-by-feature gap analysis

Legend: **Win** = we exceed every competitor · **Tie** = we match the best · **Lag** = a competitor is ahead and we must close.

| Feature | Multiplexer | Orca | T3 Code | Superset | Conductor | OpenCode | Verdict |
|---|---|---|---|---|---|---|---|
| In-process harness | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | **Win** |
| Native editor | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | **Win** |
| HAR profiler/replayer | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | **Win** |
| System-browser import | ✅ | ❌ (bundles) | ❌ | ❌ | ❌ | ❌ | **Win** |
| Windows-primary (conditional) | ✅ | ⚠️ | ⚠️ | ❌ | ❌ | ⚠️ | **Win** (conditional on Phase-0 spike) |
| Mutation-testing CI | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | **Win** |
| Subagent fan-out dashboard | ✅ | ❌ | ❌ | ❌ | ⚠️ | ❌ | **Win** |
| Parallel worktrees | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | Tie |
| Ghostty-class terminal | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | Tie |
| Design Mode | ✅ | ✅ | ⚠️ | ❌ | ❌ | ❌ | Tie |
| SSH remote worktrees | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | Tie |
| Inline diff comments | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | Tie |
| GitHub/Linear native | ✅ | ✅ | ✅ | ⚠️ | ❌ | ❌ | Tie |
| Mobile companion | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | Tie |
| Account/usage tracking | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | Tie |
| Orca-class CLI | ✅ | ✅ | ✅ | ❌ | ❌ | ✅ | Tie |
| Native search | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | Tie |
| Split-anything panes | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | Tie |
| Multi-provider breadth | ✅ | ⚠️ | ✅ | ❌ | ⚠️ | ✅ (75+) | **Lag** (OpenCode/T3 breadth) |
| Ecosystem maturity | ⚠️ | ✅ | ⚠️ | ⚠️ | ⚠️ | ✅ (195K★) | **Lag** (Orca/OpenCode) |

**Where we lag and how we close it:**
- **Multi-provider breadth:** OpenCode (75+ via Models.dev) and T3 Code lead. We close via the provider-adapter pattern (`plan/05`) — Grok in-process first, then DeepSeek/OpenRouter, Claude, Codex, OpenCode. We don't need 75 providers; we need the *right* ones, in-process and first-class.
- **Ecosystem maturity:** Orca and OpenCode have larger communities today. We close with a greenfield, Windows-first, insight-rich product and a documented plan — we are not playing catch-up on an existing codebase.

**Where we tie:** the entire Orca baseline bar (§3). Ties are the *floor*, not the goal — we must match them to be credible, then differentiate on the six whitespace items.

---

## 6. Risks from competitors

Competitors will move. Here is how each credible threat could close our gaps, and how we stay ahead.

### 6.1 Orca adds HAR
Orca already embeds Chromium and drives browsers, so a HAR/waterfall feature is a plausible, low-friction addition. **How we stay ahead:** our HAR is built into the *in-process* runtime and the native editor, not bolted onto a browser tab. We ship HAR as a first-class pane with replay, tied to agent turns and orchestration events — a depth Orca would have to rebuild to match.

### 6.2 Superset adds Windows
Superset is macOS-only today; adding Windows is a large port but a natural roadmap item. **How we stay ahead:** we are Windows-primary from day one, with the architecture (`plan/02`) built for it — conditional on the Phase-0 spike (D35), with the ACP fallback as contingency. We are not retrofitting; we are native. If Superset ships Windows later, we already own the Windows developer mindshare and the native-perf story.

### 6.3 OpenCode adds orchestration
OpenCode is an agent with a huge community; it could grow a worktree fleet or orchestration layer. **How we stay ahead:** orchestration is our *core* (event-sourced, in-process, `plan/06`), not a bolt-on. We also pair it with the editor, HAR, and mutation gates that OpenCode's architecture (agent CLI/IDE) would struggle to add coherently.

### 6.4 T3 Code adds e2e / mutation tests
T3 Code has no e2e and no mutation tests; it could add them. **How we stay ahead:** TDD-at-inception is a *process* advantage we hold now, and it compounds — our codebase is mutation-gated from the first commit, so we never accumulate the untested surface T3 would have to retrofit.

### 6.5 Orca / T3 add in-process embedding
This is the hardest for them to copy: Orca and T3 are architected around driving external CLIs/processes. **How we stay ahead:** in-process embedding is a deep architectural commitment (vendored fork, `[patch]`, provider-adapter contract) that cannot be bolted on. **But it is a hypothesis to be proven in Phase 0, not a settled moat (D10)** — if the Phase-0 spike fails, we fall back to the ACP path, and this differentiator collapses to parity with Orca/T3. We must treat the spike as the go/no-go for this edge, and keep the ACP fallback as a first-class contingency rather than assuming the moat holds.

### 6.6 General risk: feature-copying our whitespace
Any competitor could attempt any of the six whitespace items. **How we stay ahead:** the whitespace is *interlocking* — in-process embedding enables HAR and orchestration; the native editor and system-browser import are complementary; mutation gates are process. Copying one item without the architecture underneath yields a shallow imitation. We defend by shipping the integrated whole, fast, Windows-primary.

---

## 7. Positioning statement

> **Multiplexer is the control surface for your agents — the one with a real native editor, real in-process performance, and real insight (HAR, orchestration, mutation-gated quality) — and the one built Windows-primary.**

In one sentence: **"The control surface for your agents, with a real editor, real performance, and real insight."**

The differentiation is structural, not cosmetic: we aim to embed the harness in-process (a hypothesis to be proven in Phase 0, D10), own the edit loop natively, and instrument the agent loop for insight — capabilities no competitor can bolt on. We ship Windows-primary to a developer population the macOS-only competitors ignore, and we gate our own quality with mutation testing no one else enforces.

---

## 8. Monetization / GTM (D30)

Multiplexer follows a **freemium** model (locked decision D30):

- **Free tier:** local, single-provider (Grok in-process), core features — the full baseline bar and core differentiators, usable locally with one provider at no cost. This is the wedge that gets the product into a developer's daily loop.
- **Paid tier:** multi-provider (DeepSeek/OpenRouter, Claude, Codex, OpenCode via provider adapters), remote/relay (paired mobile + relay tunnel + SSH), mobile-advanced features, usage analytics, and priority support.

**GTM implications for the competitive analysis:** the free tier is the on-ramp that undercuts the paid-only competitors (Orca, Conductor) and matches the open-source-available T3 Code on price while differentiating on editor/perf/insight. The paid tier monetizes the multi-provider and remote/mobile surface that the free tier deliberately excludes. Full pricing/GTM detail lives in `plan/18` and `plan/19`; this section only records the freemium structure that shapes how we position against the set.

---

## 9. Sourced snapshot & re-validation (C3)

**This competitive snapshot is as of 2026-08-12.** Capabilities are marked **verified** (confirmed against PLAN-CONTEXT's verified competitor facts and `docs/UPSTREAM-TRAJECTORY.md`) vs **assumed** (reasonable inference, not directly confirmed):

- **Verified:** Orca's platform coverage (macOS/Windows/Linux), bundled Chromium, no HAR, no system-browser import, drives CLIs; T3 Code's Electron stack, no editor, no e2e, no mutation tests, Grok-via-ACP; Superset/Conductor macOS-only; OpenCode's provider breadth and agent-not-control-surface nature; grok-build's Windows "best-effort" status and the unproven in-process library API.
- **Assumed:** the precise current feature parity of each competitor's roadmap (e.g., whether Orca has since added HAR, whether T3 has added e2e) — these are moving targets and are **not** re-verified in this snapshot.

**Trajectory:** `docs/UPSTREAM-TRAJECTORY.md` (2026-08-12) documents the incoming work from grok-build and T3 Code — including upstream's active improvement of Windows support, subagent fan-out, worktrees, and LSP, all of which de-risk our embedding/Windows bets but keep the Phase-0 spike the go/no-go.

**Re-validation:** per **D31** and `plan/20`, this snapshot is **re-validated quarterly** against the grok-build changelog and T3 Code issues/commits. Do not treat the Orca/T3 baseline as static.

---

## Open questions (carried from PLAN-CONTEXT, not decided here)

- **Orca baseline scope:** match all Orca features in MVP vs a subset (PLAN-CONTEXT Q7). This doc assumes we match the full baseline bar; the MVP cut is decided in `plan/19`.
- **Multi-provider breadth:** MVP Grok-only vs multi-provider from day one (Q3). Affects how aggressively we close the §5 provider-breadth lag.
- **Windows-first confirmation:** **resolved** — Windows-first is confirmed (D9) but **conditional on the Phase-0 spike** (D35), with the ACP fallback as contingency. See §4.5.

## References

- `docs/DECISIONS.md` — authoritative locked decisions (D10, D28, D30, D34, D35 applied here).
- `docs/PLAN-CONTEXT.md` — authoritative shared plan context (source of all competitor facts).
- `docs/UPSTREAM-TRAJECTORY.md` — upstream trajectory research (grok-build + T3 Code), dated 2026-08-12.
- `plan/02-architecture.md` — how the whitespace items are built.
- `plan/19-roadmap-and-milestones.md` — sequencing of the baseline bar + whitespace.
- `plan/20-risks-and-open-questions.md` — consolidated risk register and open decisions (quarterly re-validation).
