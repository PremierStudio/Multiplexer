# Multiplexer — Upstream Trajectory & Incoming Work (research)

**Date:** 2026-08-12
**Purpose:** Understand what's coming down the line from the projects we embed/fork/reference, so the Multiplexer plan accounts for upstream trajectory — not just today's snapshot.

---

## 1. grok-build (github.com/xai-org/grok-build) — the harness we embed

### How the repo works (critical for our fork strategy)
- **It's a read-only mirror, synced ~daily from the SpaceXAI monorepo by a bot** (`grokkybara[bot]`). Every commit is titled "Synced from monorepo."
- **Open-sourced on 2026-07-16** (commit `c68e39f` "Publish harness and TUI open-source"). The repo is ~1 month old.
- **No open issues, no PRs (creation restricted), no discussions.** External contributions are not accepted (per CONTRIBUTING.md). So there is **no community signal** — the only signal is the daily sync + the changelog.
- `SOURCE_REV` records the upstream monorepo commit SHA (currently `5d08d7e4…`). This is our sync anchor.
- **Implication for us:** we maintain our own fork; we track upstream via `SOURCE_REV` + periodic merge. The daily sync means upstream moves fast — we must budget for frequent merge churn.

### Version cadence (from the changelog)
- **Very fast:** 1.0.0 on 2026-08-07, 1.0.1 on 2026-08-10. Before that, ~0.2.x with multiple releases **per day** (0.2.100–0.2.120 span just ~2 weeks). This is a hyper-active project.
- The changelog is the **best forward-looking signal** since there are no issues/PRs/discussions.

### What's coming / recent direction (from 1.0.0 + 1.0.1 + recent 0.2.x)
**Directly relevant to Multiplexer:**
- **Subagent spawning is now bounded** (1.0.1): "wide fan-outs queue instead of exhausting file descriptors." → This is the **16-child cap** our plan flagged. Upstream is actively working on fan-out behavior — we must track this closely (it gates our "dozens of concurrent subagents" differentiator).
- **Tools now report whether they only read data** (1.0.1) → enables safer restricted agents/subagents. Relevant to our permission modes + read-only subagent routing.
- **`grok du`** shows disk usage of ~/.grok including worktrees and sessions (1.0.1) → worktree management is a first-class concern upstream.
- **Sandbox workspace sessions** can limit which bundled skills are advertised (1.0.1) → sandboxing is maturing.
- **`/rewind` now only truncates conversation history** (breaking, 1.0.1) → checkpoint/rewind semantics are changing; our checkpointing design must track this.
- **Managed MCP servers only via gateway catalog** (breaking, 1.0.1) → MCP management is centralizing server-side.
- **`grok agent stdio` Windows hangs fixed** (0.2.70/0.2.71) + **external auth providers now work on Windows** (0.2.115) + **Windows path handling cleaned up** (0.2.52) → **Windows support is actively improving upstream.** Good sign for our Windows-first bet, but the repo README still says Windows builds are "best-effort, not tested from this tree."
- **`grok worktree ls`** alias (0.2.96), **worktree sessions show branch in status bar** (1.0.1) → worktrees are a growing focus.
- **LSP integration** (lsp.json, passive diagnostics, `lsp` tool) → upstream is building LSP into the harness. Relevant to our native editor + LSP plan.
- **`--json-schema`** for headless (0.2.67) → structured output is maturing.
- **OpenTelemetry export** (0.2.52) → telemetry hooks exist.
- **`grok wrap ssh`** clipboard/terminal restore over SSH (0.2.70+) → SSH support is a focus (relevant to our SSH worktrees).
- **Session resume across hosts / mirroring to S3** (0.2.107) → remote/multi-machine sessions are coming.
- **`/goal` mode, goal evaluation** (0.2.6x–1.0.1) → goal-driven agentic mode is maturing.
- **`/code-review` slash command ships by default** (0.2.51) → built-in review workflow.
- **`/undo` = restore files+chat to earlier turn** (0.2.116) → rewind/undo is being refined.

**Performance direction (relevant to our perf targets):**
- Cold start shows UI instantly while models/settings load in background (0.2.113) — matches our <300ms cold-start goal.
- Large session fork/resume memory reductions (0.2.113, 0.2.112), grep early-stop (0.2.84), idle CPU/memory reductions (0.2.84), file-watching resource cuts (0.2.95), git status/diff CPU fixes (1.0.1).
- **Parallel tool calls on same path now execute concurrently** (0.2.46) → upstream is adding concurrency.

### Key takeaway for the plan
- **The embedding hypothesis is still unproven** — the README documents only headless + ACP integration, no in-process library API. But upstream is **actively improving Windows + subagent fan-out + worktrees + LSP**, all of which de-risk our bet. The Phase-0 spike remains the go/no-go.
- **We must track the changelog continuously** (it's the only signal) and budget for fast upstream merge churn. Add a "track grok-build changelog" recurring task to the plan.

---

## 2. T3 Code (github.com/pingdotgg/t3code) — reference + competitor

### How the repo works
- **Actively developed, open to contributions** (unlike grok-build). ~2,468 commits, many contributors, a `t3-code[bot]`, and heavy use of **Claude + Codex bots** in commits (they dogfood agents heavily).
- **Very high velocity:** multiple commits per day across web/desktop/mobile/server.
- **Open issues are active** — this is a real community signal (unlike grok-build).

### What they're building (from recent commits + open issues)
**Recent commit themes (Aug 10–12, 2026):**
- **Mobile app is a major focus** — thread title regeneration, composer stabilization, Android gesture-bar fixes, App Store release guards, tablet rotation. They're polishing the mobile companion hard.
- **Web UI polish** — OKLCH theme palettes, Open VSX theme search, sidebar/footer refinements, typography, theme-aware artwork.
- **Source control** — Azure DevOps SSH remotes, self-hosted GitLab routing, unborn-HEAD VCS handling, PR page fixes.
- **Windows support** — 256-color TERM on Windows terminals, Windows window-controls in PR header, libc-detection skip on Windows.
- **Usage tracking** — hourly past-24-hour usage view (matches Orca's usage tracking).

**Open issues signal (what users are asking for / what's missing):**
- **#6257: "Make provider context continuity and compaction provenance explicit"** — context/compaction transparency is a real user need.
- **#6220: Mobile context-window usage ring around composer** — context-window visibility on mobile.
- **#6251: Attach files to messages on iOS.**
- **#6240: Surface device toolbar as preview toolbar button.**
- **#6205: "Terribly confused by the project setup workflow"** — onboarding is a pain point.
- **#6200: Branded macOS DMG installer window.**
- **#6219: Esc to pop a just-sent message back into composer.**
- **#6229: Environment filter in sidebar.**
- **#6263: Chinese (zh-cn) localization.**

### Key takeaway for the plan
- **T3 Code is a fast-moving, well-funded-feeling competitor** with a strong mobile app and heavy agent-dogfooding. They are NOT standing still — they're polishing UX, mobile, source control, and usage tracking.
- **Their gaps remain** (no built-in editor, no HAR, no mutation testing, Electron/web-perf ceiling) — our differentiators still hold. But they're closing the UX/mobile gap fast.
- **Their open issues are a goldmine of unmet user needs** we can learn from (context/compaction transparency, context-window visibility, onboarding, file-attach-on-mobile).

---

## 3. Implications for the Multiplexer plan

1. **Add a "track upstream" recurring task** — grok-build changelog (daily sync, only signal) + T3 Code issues/commits. This should be a standing item, not a one-time research.
2. **Re-validate the embedding + Windows bet against upstream momentum** — upstream is actively improving Windows + subagent fan-out + worktrees + LSP, which de-risks us. But the Phase-0 spike is still the go/no-go.
3. **Budget for fast merge churn** on the grok-build fork (daily sync, multiple releases/day).
4. **Watch the subagent fan-out cap closely** — upstream is actively changing it (1.0.1 "bounded fan-out"). This gates our "dozens of concurrent subagents" differentiator.
5. **Learn from T3's open issues** — context/compaction transparency, context-window visibility, onboarding, mobile file-attach are proven unmet needs we can address.
6. **Do not assume the competitive snapshot is static** — re-validate quarterly (already in plan/20), and treat the Orca/T3 baseline as moving.

---

## 4. Sources
- grok-build commits: https://github.com/xai-org/grok-build/commits/main/
- grok-build changelog: https://raw.githubusercontent.com/xai-org/grok-build/main/crates/codegen/xai-grok-shell/CHANGELOG.md
- grok-build SOURCE_REV: https://raw.githubusercontent.com/xai-org/grok-build/main/SOURCE_REV
- grok-build issues/PRs/discussions: none (read-only mirror, contributions not accepted)
- T3 Code commits: https://github.com/pingdotgg/t3code/commits/main/
- T3 Code issues: https://github.com/pingdotgg/t3code/issues
