# Multiplexer — Shared Plan Context (authoritative reference for all plan docs)

This file is the single source of truth for the Multiplexer project's approved architecture and decisions. Every `plan/XX-x.md` document MUST be consistent with this. Do not contradict it; if you find a conflict, flag it in your doc's "Open questions" rather than silently diverging.

> **Authoritative decisions:** All open questions are now **LOCKED** and resolved in **`docs/DECISIONS.md`** (D1–D40). DECISIONS.md is the **authoritative source of truth** for decisions; this file is consistent with it. If any plan doc still says "open question," that question is resolved by DECISIONS.md. When this file and DECISIONS.md conflict, DECISIONS.md wins.

## Product

- **Name:** Multiplexer (we own Multiplexer.dev and Multiplexor.dev).
- **What it is:** A beautiful, blazing-fast **desktop client / control surface** for the **Grok Build harness**, extensible to other models and harnesses (DeepSeek V4 Flash via OpenRouter, Claude, Codex, OpenCode).
- **Positioning:** "The control surface for your agents, with a real editor, real performance, and real insight."
- **Paired mobile app is required** (control/observe agents from the phone).

## Core differentiators (must all hold)

1. **In-process grok-build embedding** — vendor the grok-build crates and call the agent runtime directly (no shelling out to a CLI, no ACP protocol overhead). Nobody else does this. **This is a Phase-0 go/no-go hypothesis to prove (D10), not a settled moat** — the ACP path is the fallback if the shell is not cleanly embeddable.
2. **Native, blazing-fast editor** (Rust + GPUI, GPU-rendered) — inline diff-apply, LSP, multi-cursor, Vim mode.
3. **System-browser integration** — detect/import the user's installed browsers (Chrome, Edge, Firefox, Safari, Arc, Brave), launch/authorize, drive via CDP. NO bundled Chromium.
4. **Built-in HAR profiler/replayer** — capture network via CDP, visualize waterfalls, replay recorded sessions.
5. **Powerful pop-out pane UI** — Outlook-style left chat sidebar, center build pane, multi-purpose right bar (browser/HAR/files/diff/terminal/agent activity), optional pop-up terminal below, every pane can pop out to its own window.
6. **Multi-harness / multi-model** — Grok in-process first; DeepSeek/OpenRouter, Claude, Codex, OpenCode via provider-adapter pattern. **OpenRouter/DeepSeek (e.g. `ds-flash`) is a config variant of the in-process Grok adapter (D14), not a separate adapter crate.**
7. **Subagent orchestration at scale** — fan out many subagents on specific tasks, live orchestration dashboard. **Multiplexer owns subagent scheduling (D11)** — we fork the vendored `spawn_subagent`/workflow code to raise the 16-child cap and implement our own parallel scheduler.
8. **Paired mobile app** — same server runtime.
9. **Windows-first** — Superset and Conductor are macOS-only; we ship Windows first, then macOS/Linux. **Conditional on the Phase-0 spike (D35)**: if the Windows grok-build build fails/delays, fall back to the ACP path on Windows while in-process embedding lands on macOS/Linux first. Frame as "Windows-primary," not "Windows-only."
10. **TDD at inception** — full unit + mutation tests, component tests, integration tests, deep assertions, coverage thresholds in CI.

## Baseline bar (must match Orca, the strongest competitor)

Parallel isolated worktrees, Ghostty-class terminal with splits, Design Mode (browser element → agent), SSH remote worktrees, inline diff comments → agent, GitHub/Linear native, mobile companion, account/usage tracking, split-anything panes, native search, Orca CLI.

## Architecture (approved)

- **Stack:** Rust core + GPUI (GPU-rendered) UI. NOT Electron. (D1)
- **Server-centric runtime:** a single native Rust binary owns agent processes, terminals, git, filesystem, checkpoints, HAR capture. Clients (desktop/mobile/web) are thin shells over one authenticated **JSON-RPC over WebSocket** contract.
- **Embedded harness:** fork/vendor `xai-org/grok-build` (Apache 2.0) under `third_party/` (or `[patch]`). Reuse `xai-grok-shell` (agent runtime), `xai-grok-tools`, `xai-grok-workspace` as libraries. Replace `xai-grok-pager` (TUI) with our GPUI UI. **Windows build support is our responsibility** (upstream says Windows is best-effort/untested). **In-process embedding is a Phase-0 go/no-go hypothesis (D10)** — the ACP path is the fallback.
- **Crate layout (D13):** consolidated `multiplexer-*` crates — `multiplexer-wire` (shared contract, single source of truth, codegen for Swift/Kotlin/TS), `multiplexer-provider` (adapter trait, canonical event enum, Grok in-process + ACP adapters, model registry), `multiplexer-core` (orchestration engine, decider, projector, read model), `multiplexer-server` (composition root binary), `multiplexer-ui` (GPUI desktop UI), plus `multiplexer-terminal`, `multiplexer-browser`, `multiplexer-har`, `multiplexer-mobile-shared` subsystem crates. NOT the `mx-*` split.
- **Provider Adapter contract** (Rust trait): `start_session`, `send_turn`, `interrupt_turn`, `approval_respond`, `user_input_respond`, `checkpoint_revert`, `session_stop` + canonical `ProviderEvent` stream. **OpenRouter/DeepSeek (e.g. `ds-flash`) is a config variant of the in-process Grok adapter (D14)** — same embedded runtime, different `[model.*]`/`[auth_provider.*]` config — not a separate adapter crate.
- **Model registry:** manage `[model.*]`/`[auth_provider.*]` config; select per thread.
- **Event-sourced orchestration** (Rust): serialized command queue per thread + parallel scheduler for cross-thread/subagent work. Pure decider + projector into a SQLite read model in one transaction. **Multiplexer owns subagent scheduling (D11)** — fork the vendored `spawn_subagent`/workflow code to raise the 16-child cap and implement our own parallel scheduler.
- **Checkpointing:** hidden Git refs per turn; diff queries.
- **Terminal:** embed Ghostty.
- **Resource monitor:** Rust sidecar (NDJSON over stdio), power-adaptive sampling.
- **Remote/relay:** local + paired + relay tunnel + SSH; WebSocket ticket auth (5-min TTL); Tailscale serve.
- **Auth:** OS keychain for local secrets; OAuth for providers; passkeys/DPoP for remote.

## Performance targets

- Cold start to usable editor: **< 300ms**.
- Input latency: **< 16ms** (60fps+).
- Subagent fan-out: **dozens of concurrent subagents** without serialization bottleneck.
- Memory: far below Electron competitors.

## Testing (TDD at inception — non-negotiable)

- **Unit:** co-located `#[cfg(test)]`; **property-based** with proptest for state machines/deciders/projectors/serializers.
- **Mutation:** cargo-mutants; CI gates: ≥85% line, ≥80% branch, ≥70% mutation score killed. **Scope = ALL core logic across all subsystems (D21)**, including the editor (buffer, diff-apply, undo, selection), terminal (PTY, scrollback, backpressure), browser (detection, launch, port-parsing, security controls), and pane system (layout engine). **70% mutation score is the merge floor (D33)** — the bar may rise over time.
- **Integration:** real core + mock ACP agent (fake `grok agent stdio`); assert on read model; real-binary smoke tests when available.
- **Contract:** JSON-RPC wire contract schema-verified on both sides.
- **Component (GPUI):** element/component tests, snapshot tests for pane layouts.
- **E2E:** drive the real app/headless — this beats T3 Code (no e2e). **Runs on the merge gate (critical paths) and nightly (full suite) (D32)** — no "skip e2e for small changes" path.
- **Mobile:** native unit + integration against shared contract; mock server for offline determinism.
- **CI gates:** fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e → coverage. All green before merge. No blind CI. **A dedicated performance stage (D22)** enforces the hard perf gates (cold start <300ms, input latency <16ms p95, memory under budget, dozens of subagents).

## Plan doc structure (each authored by a dedicated subagent)

- `plan/00-vision-and-principles.md`
- `plan/01-competitive-analysis.md`
- `plan/02-architecture.md`
- `plan/03-vendored-grok-build.md`
- `plan/04-wire-contract.md`
- `plan/05-provider-adapter-layer.md`
- `plan/06-orchestration-engine.md`
- `plan/07-checkpointing-and-vcs.md`
- `plan/08-terminal.md`
- `plan/09-editor.md`
- `plan/10-ui-pane-system.md`
- `plan/11-system-browser-integration.md`
- `plan/12-har-profiler-replayer.md`
- `plan/13-mobile-app.md`
- `plan/14-remote-and-relay.md`
- `plan/15-testing-strategy.md`
- `plan/16-performance.md`
- `plan/17-security-and-secrets.md`
- `plan/18-build-release-distribution.md`
- `plan/19-roadmap-and-milestones.md`
- `plan/20-risks-and-open-questions.md`
- `plan/21-mcp-lifecycle-supervisor.md`
- `plan/22-remote-delegation.md`
- `plan/23-tailscale-integration.md`
- `plan/24-resource-manager.md`
- `plan/25-worktree-hooks.md`
- `plan/26-mcp-skills-ui.md`

> **New differentiators (2026-08-12):** `plan/21`–`plan/26` add six differentiators (MCP lifecycle supervisor, remote delegation, Tailscale integration, resource manager, worktree hooks, MCP/skills UI). They are slotted into the existing phases in `plan/19` without changing MVP scope: local resource manager + MCP supervisor + worktree-hook basics in Phase 1, MCP/skills UI + resource visual pane in Phase 2, and remote delegation + Tailscale + distributed fleet in Phase 4.

## Key facts about grok-build (verified)

- Repo: `github.com/xai-org/grok-build`, Apache 2.0, Rust.
- Crates: `xai-grok-pager-bin` (composition root → `xai-grok-pager` binary), `xai-grok-pager` (TUI), `xai-grok-shell` (agent runtime + leader/stdio/headless), `xai-grok-tools` (tool impls), `xai-grok-workspace` (fs/VCS/execution/checkpoints), plus config/MCP/markdown/sandbox crates.
- Root `Cargo.toml` is generated (read-only); edit per-crate `Cargo.toml`.
- Builds to `xai-grok-pager`; official installs ship it as `grok`.
- Windows builds are "best-effort, not currently tested from this tree."
- External contributions not accepted → we maintain our own fork.
- Model plug-in via `[model.<id>]` + `[auth_provider.<id>]` in `config.toml` (user already runs `ds-flash` = DeepSeek V4 Flash via OpenRouter).
- Subagent/workflow orchestration built in: `spawn_subagent` (depth 1) + Rhai workflows (`agent()`, `parallel()`, `phase()`, budget caps, max 16 concurrent children).
- ACP integration: `grok agent stdio` / `serve` / `headless`; xAI extensions `x.ai/fs/*`, `x.ai/git/*`, `x.ai/terminal/*`, `x.ai/search/*`, `x.ai/session/*`, `x.ai/auth/*`.

## Key facts about T3 Code (reference, do NOT fork)

- Server-centric: a `t3` server owns everything; clients are thin over Effect RPC WebSocket.
- Provider abstraction: `ProviderDriver` → `ProviderAdapter` (startSession, sendTurn, interruptTurn, approval.respond, userInput.respond, checkpoint.revert, session.stop).
- Grok via ACP: spawns `grok agent stdio`; `AcpSessionRuntime`; `XAiAcpExtension`.
- Event-sourced orchestration: serialized command queue → decider → projector into SQL read model.
- Checkpointing via hidden Git refs.
- Terminal via Ghostty (WASM web / native mobile).
- Rust `resource-monitor` sidecar (NDJSON over stdio).
- Remote: local/bearer/relay/SSH + Tailscale; WebSocket ticket auth.
- **Gaps we exploit:** no built-in editor, no e2e tests, no mutation tests, Electron/web-perf ceiling, single serialized queue, no HAR, no system-browser import.

## Key facts about competitors (verified)

- **Orca (onorca.dev):** MIT, macOS/Windows/Linux, native + WebGL Ghostty-class terminal + embedded Chromium. Parallel worktrees, Design Mode, SSH worktrees, inline diff comments, GitHub+Linear, mobile, account/usage tracking, Orca CLI, native search, split-anything panes. Gaps: no HAR, no system-browser import (bundles Chromium), no mutation testing, drives CLIs (no in-process embedding).
- **T3 Code:** open source-available, Electron+React+Effect, multi-provider, remote, mobile, source control. Gaps: no editor, no e2e, no mutation tests, web-perf ceiling.
- **Superset (superset.sh):** ELv2, macOS-only (no Windows), parallel worktrees, diff review, persistent terminals, automations, MCP server, OAuth 2.1+PKCE. Gaps: macOS-only, no editor, no mobile, no browser, no HAR.
- **Conductor (conductor.build):** closed, macOS-only, parallel Claude/Codex/Cursor in isolated workspaces. Gaps: macOS-only, closed, no editor, no mobile.
- **OpenCode (opencode.ai):** MIT, 195K stars, terminal/IDE/desktop, 75+ providers via Models.dev, LSP, multi-session, share links. It's an *agent*, not a control surface — no multi-harness orchestration, no worktree fleet, no HAR, no mobile companion.
- **Codex Desktop:** closed, Electron, official Codex chat+terminal. Gaps: no editor, closed, single-provider, no mobile.
- **OpenChamber.io:** NOT a coding agent (business-formation roadmap tool). Excluded.

## Decisions (LOCKED — see `docs/DECISIONS.md`)

All open questions are now **LOCKED** and resolved in **`docs/DECISIONS.md`** (D1–D40), which is the **single source of truth** for decisions. Do not decide unilaterally; reference DECISIONS.md. The previously-open questions are resolved as follows:

- **D1 — Stack:** Rust + GPUI (GPU-rendered). NOT Electron.
- **D2 — Mobile:** Expo / React Native (thin client over the shared contract). Desktop stays Rust+GPUI.
- **D3 — MVP scope:** Grok Build only (in-process embedding); other providers added after MVP via the provider-adapter pattern. Custom models (e.g. `ds-flash` via OpenRouter) supported from day one as a config feature.
- **D4 — Editor scope:** full native editor in the MVP (rope buffer, multi-cursor, undo/redo, tree-sitter, Vim mode, LSP).
- **D5 — grok-build vendoring:** vendored fork under `third_party/` + `[patch]`.
- **D6 — Branding:** Multiplexer.dev is the product brand; Multiplexor.dev redirects.
- **D7 — Orca baseline scope:** match ALL baseline features across Phases 1–5.
- **D8 — MVP definition:** MVP = Phases 1–4 (Core MVP + Editor/Panes + Browser/HAR + Mobile/Remote); Phases 5–6 are post-MVP.
- **D9 — Windows-first:** CONFIRMED (ship Windows first, then macOS, then Linux), with an ACP-path contingency if the Windows build fails.
- **D10 — In-process embedding:** a Phase-0 go/no-go hypothesis to prove (spike), not a settled moat; ACP path kept as fallback.
- **D11 — Subagent scheduling:** Multiplexer owns scheduling; fork the vendored `spawn_subagent`/workflow code to raise the 16-child cap.
- **D12 — Approval model:** 4-way enum (`allow`/`deny`/`allow_once`/`allow_always`) everywhere.
- **D13 — Crate layout:** consolidated `multiplexer-*` crates (NOT the `mx-*` split).
- **D14 — OpenRouter/DeepSeek:** a config variant of the in-process Grok adapter, NOT a separate adapter crate.
- **D15–D20 — Contract/adapter:** explicit wire↔ProviderEvent mapping table; single canonical event vocabulary; generic ACP adapter; bounded channel with backpressure; unified session-start params; `multiplexer-wire` as single shared-contract source.
- **D21–D22 — Testing/perf:** mutation scope = ALL core logic incl. editor/terminal/browser/pane; dedicated perf stage in CI.
- **D23–D29 — Security/ops:** session-cache secrets model (no runtime `op://`); honest relay TLS-terminating (not E2EE); remote-agent independent trust enforcement; browser security tests; CI-only pinned headless browser; HAR is CDP-only; Azure Trusted Signing.
- **D30 — Monetization:** freemium.
- **D31 — Track upstream:** standing roadmap task.
- **D32–D33 — E2E/floor:** e2e on merge gate + nightly; 70% mutation score is the merge floor.
- **D34–D40 — Framing/consistency:** plan-docs not a differentiator; Windows-first conditional; Orca baseline match-all default; pairing credential model; keychain-only local tickets; no live-swap auto-update; Phase 4 depends on Phase 1.

See `docs/DECISIONS.md` for the full rationale, trade-offs, and the doc-level fixes required to make every plan doc consistent.
