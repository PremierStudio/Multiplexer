# Multiplexer — Locked Decisions (authoritative)

**Date:** 2026-08-12
**Status:** These decisions are LOCKED and supersede the "open questions" in the plan docs. Every plan doc must be consistent with this file. If a plan doc still says "open question," it is now resolved by this file.

---

## D1. Stack — Rust + GPUI (LOCKED)
- **Decision:** Rust core + GPUI (GPU-rendered) UI. NOT Electron.
- **Rationale:** Blazing-fast performance (<300ms cold start, <16ms input latency), single static binary, true parallelism for dozens of concurrent subagents, and — critically — the grok-build harness is Rust, so in-process embedding is natural and zero-overhead. Zed proves the Rust+GPUI bar.
- **Trade-off accepted:** younger ecosystem, more build effort than Electron.

## D2. Mobile — Expo / React Native (LOCKED)
- **Decision:** Expo / React Native for the mobile app (iOS + Android), NOT native SwiftUI/Kotlin.
- **Rationale:** The mobile app is a **thin client** over the same JSON-RPC-over-WebSocket contract — the contract is the real asset, not the native UI. Expo/RN gives us one codebase for both platforms, faster shipping, and the shared contract + mock-server testing gives offline determinism. Native would only matter if we needed heavy platform-specific rendering, which we don't (the terminal is rendered server-side / via the contract).
- **Note:** The desktop UI stays Rust+GPUI. Only the mobile thin client is RN.

## D3. MVP scope — Grok-only, single-provider (LOCKED)
- **Decision:** The MVP is **Grok Build only** (in-process embedding). Other providers (DeepSeek/OpenRouter, Claude, Codex, OpenCode) are added AFTER the MVP, via the provider-adapter pattern.
- **Rationale:** Fastest path to a great product. The provider-adapter seam is designed from day one so adding providers later is mechanical. Multi-provider from day one would triple the MVP surface and delay the differentiators (editor, panes, HAR, browser).
- **Exception:** The model registry must support custom models (like the user's `ds-flash` via OpenRouter) from day one, because that's a config feature of the embedded harness, not a separate provider adapter.

## D4. Editor scope — full native editor in MVP (LOCKED)
- **Decision:** Build the **full native editor** (rope buffer, multi-cursor, undo/redo, tree-sitter syntax highlighting, Vim mode, LSP) as part of the MVP.
- **Rationale:** The native editor is differentiator #2 and the trust/context mechanism for the agent loop. A "lighter editor first" would be throwaway work and would ship a weak product. The editor core (§2 of plan/09) is shared and not throwaway.
- **Note:** LSP server discovery/launch is included, but we do NOT bundle language servers (user installs them, matching grok-build's approach). The editor is a **right-rail pane or pop-out**, never the Outlook center. Grok edits. Multiplexer reviews. `$VISUAL` / Open external is allowed until the crate lands.
- **Pager:** Multiplexer **hosts** the real `grok` TUI. Do not rebuild `xai-grok-pager` in GPUI. Chat log is `grok -p` until a named Engine C milestone.

## D5. grok-build vendoring — vendored fork under `third_party/` + `[patch]` (LOCKED)
- **Decision:** Clone `xai-org/grok-build` into `third_party/grok-build` as a vendored fork, wired via `[patch]` in our workspace Cargo.toml. Maintain our own fork (upstream doesn't accept contributions).
- **Rationale:** Gives us full control over Windows build fixes and any changes we need, while keeping upstream syncable via `SOURCE_REV` + periodic merge. `[patch]` lets us depend on the crates by their real names.
- **Sync:** Track `SOURCE_REV`; merge (not rebase) from upstream on a cadence; budget for fast churn (daily sync, multiple releases/day).

## D6. Branding — Multiplexer.dev is the product brand; Multiplexor.dev redirects (LOCKED)
- **Decision:** **Multiplexer.dev** is the product brand and primary domain. **Multiplexor.dev** is a redirect to it (defensive registration).
- **Rationale:** "Multiplexer" is the cleaner, more standard spelling and matches the product name. Multiplexor.dev is kept to prevent squatting and to catch typos.

## D7. Orca baseline scope — match ALL baseline features across Phases 1–5 (LOCKED)
- **Decision:** Commit to matching the **full Orca baseline** (parallel worktrees, Ghostty-class terminal with splits, Design Mode, SSH worktrees, inline diff comments → agent, GitHub/Linear native, mobile companion, account/usage tracking, split-anything panes, native search, CLI) across Phases 1–5.
- **Rationale:** These are table stakes now (Orca has them). Shipping a subset would leave us behind on day one. The roadmap already spreads them across phases; we commit to all of them.
- **Note:** This is a large commitment — the roadmap must carry an explicit effort estimate (see D8).

## D8. Effort estimate & MVP definition (LOCKED)
- **Decision:** **MVP = Phases 1–4** (Core MVP + Editor/Panes + Browser/HAR + Mobile/Remote). Phase 5 (multi-provider + scale) and Phase 6 (GA) are post-MVP.
- **Rationale:** MVP must include the full baseline bar + core differentiators, including the **required mobile app** (which lives in Phase 4). This prevents the mobile app from slipping past the MVP.
- **Effort:** Add an explicit person-month estimate to plan/19 (see plan/19 fix). Rough order: this is a multi-quarter, multi-engineer effort; the roadmap must state it.

## D9. Windows-first — CONFIRMED (LOCKED)
- **Decision:** Ship **Windows first**, then macOS, then Linux.
- **Rationale:** Superset and Conductor are macOS-only — a real gap. We build on Windows and ship Windows first.
- **Contingency (from review C2):** If the Windows grok-build build fails or is delayed, the fallback is: ship the **ACP path** (drive the installed `grok` binary over ACP) on Windows while the in-process embedding lands on macOS/Linux first. The competitive docs must present Windows-first as **conditional on the Phase-0 spike**, not a guaranteed win.

## D10. In-process embedding — PROVEN VIABLE by Phase-0 spike (LOCKED)
- **Decision:** Treat "in-process grok-build embedding" as a **settled technical approach**: the Phase-0 spike proved `xai-grok-shell` (v1.0.1) builds on Windows and is consumable as a library from an independent binary (see `docs/SPIKE-REPORT.md`). The ACP path is retained as resilience/redundancy, not as the primary path.
- **Phase-0 gate:** **PASSED (GO)** on 2026-08-12. The spike cloned grok-build, built `xai-grok-shell` on Windows (after one small `xai-proto-build` fork patch for protoc device paths), and linked it into an external crate that loaded the real user config (including `ds-flash` / `openrouter`, D14).
- **Fallback:** ACP (`grok agent stdio`/`serve`) remains fully supported and kept as the documented fallback for resilience, remote/thin clients, and testing.
- **Remaining:** a live authenticated headless turn in-process is Phase 1 work (needs a real session/key); the spike proved build, API surface, and config loading, not a live model round-trip.

## D11. Subagent scheduling ownership — OUR scheduler, fork the cap (LOCKED)
- **Decision:** **Multiplexer owns subagent scheduling.** We do NOT rely on grok-build's built-in 16-child cap. We fork the vendored `spawn_subagent`/workflow code as needed to raise the cap and implement our own parallel scheduler.
- **Rationale:** The "dozens of concurrent subagents" differentiator requires it. We can't both "inherit the cap for free" and "raise it" — we choose to own it. Upstream default is now **32** (was 16); we still fork to own 1-100 and depth.
- **Note:** Track upstream's fan-out changes (1.0.1 "bounded fan-out") closely; our fork may need to reconcile with upstream's approach.

## D12. Approval-decision model — 4-way enum everywhere (LOCKED)
- **Decision:** Use the **4-way decision enum** (`allow` / `deny` / `allow_once` / `allow_always`) consistently across the wire contract (plan/04), the ProviderAdapter trait (plan/05), the orchestration command model (plan/06), and security (plan/17).
- **Rationale:** `allow_once`/`allow_always` are real product features (permission modes). The adapter trait must carry them, not a boolean.

## D13. Crate layout — consolidated `multiplexer-*` crates (LOCKED)
- **Decision:** Use the **consolidated `multiplexer-*` crate naming** (as in plan/04 and plan/05), NOT the fine-grained `mx-*` split (plan/02/19).
- **Concretely:**
  - `multiplexer-wire` — the shared wire-contract crate (single source of truth, codegen for Swift/Kotlin/TS clients).
  - `multiplexer-provider` — the provider-adapter crate (adapter trait, canonical event enum, Grok in-process + ACP adapters, model registry).
  - `multiplexer-core` — orchestration engine, decider, projector, read model.
  - `multiplexer-server` — the single composition root binary.
  - `multiplexer-ui` — GPUI desktop UI.
  - `multiplexer-terminal`, `multiplexer-browser`, `multiplexer-har`, `multiplexer-mobile-shared` — subsystem crates.
- **Note:** plan/02 and plan/19 must be updated to use these names (no `mx-*`).

## D14. OpenRouter/DeepSeek adapter identity — config variant of in-process Grok (LOCKED)
- **Decision:** OpenRouter/DeepSeek (e.g., `ds-flash`) is a **config variant of the in-process Grok adapter** — same embedded runtime, different model config (`[model.ds-flash]` + `[auth_provider.openrouter]`). It is NOT a separate adapter crate, and NOT a future HTTP adapter.
- **Rationale:** The embedded grok-build harness already supports custom models via `[model.*]`/`[auth_provider.*]` config. Routing `ds-flash` through the in-process Grok adapter gives it the full tool loop for free. A separate HTTP adapter (plan/05 §7) is deferred and only considered if we need a model WITHOUT the harness tool loop.
- **Note:** plan/02 and plan/19 must remove the `mx-provider-openrouter` crate.

## D15. Wire↔ProviderEvent mapping — explicit mapping table, not "1:1" (LOCKED)
- **Decision:** Replace the false "wire events map 1:1 onto ProviderEvent, no transformation" claim with an **explicit mapping table** in plan/04 (and referenced in plan/05). The wire event set is a superset (terminal/HAR/fs/telemetry events have no ProviderEvent counterpart); a real transformation layer exists.
- **Action:** plan/04 must include the mapping table; plan/05 must reference it.

## D16. Event vocabulary — single canonical set (LOCKED)
- **Decision:** Use ONE canonical event vocabulary across plan/05 (ProviderEvent) and plan/06 (engine events). Standardize on the plan/05 names: `TurnFinished`, `ToolCallFinished`, `PermissionRequested`, `TextDelta`. plan/06 must stop using `TurnCompleted`/`ToolCallCompleted`/`ApprovalRequested`/`MessageAppended` and use the canonical names (or provide an explicit mapping table).

## D17. ACP adapter role — generic multi-provider ACP machinery (LOCKED)
- **Decision:** The ACP adapter is **generic ACP machinery** (plan/02's view), used by Grok-over-ACP (fallback) AND by Claude/Codex/OpenCode (future). plan/05's `AcpGrokAdapter` is a Grok-specific instance of the generic ACP adapter, not a separate concept.

## D18. Adapter channel — bounded, with backpressure (LOCKED)
- **Decision:** The ProviderAdapter event channel is **bounded** with backpressure, consistent with plan/04's window-based flow control. NOT `mpsc::UnboundedReceiver`. The provider-ingestion worker (plan/06) is the bounding point.

## D19. Session-start params — unified (LOCKED)
- **Decision:** Unify the session-start parameters across plan/04 (`session.start`) and plan/05 (`start_session`): `{provider, model, workspace, initial_prompt, resume, config}`. Both sides use the same shape.

## D20. Shared-contract crate — `multiplexer-wire` is the single source (LOCKED)
- **Decision:** `multiplexer-wire` is the single source of truth for the shared contract. There is NO separate `mx-mobile-shared` crate; mobile consumes `multiplexer-wire` via codegen. plan/02 must remove `mx-mobile-shared`.

## D21. Mutation-testing scope — ALL core logic, including editor/terminal/browser (LOCKED)
- **Decision:** Mutation testing (cargo-mutants) applies to **all core logic across all subsystems**, including the editor (buffer, diff-apply, undo, selection), terminal (PTY, scrollback, backpressure), browser (detection, launch, port-parsing, security controls), and pane system (layout engine). plan/15's scope list must be corrected to include these.
- **Thresholds (unchanged):** ≥85% line, ≥80% branch, ≥70% mutation score killed.

## D22. Performance gates — dedicated perf stage in CI (LOCKED)
- **Decision:** Add a **dedicated performance stage** to plan/15's CI pipeline (between integration and component, or as its own stage). plan/16's hard gates (cold start <300ms, input latency <16ms p95, memory under budget, dozens of subagents) are enforced there. plan/15 must name where perf lives.

## D23. Secrets policy — session-cache model, NOT runtime op:// (LOCKED)
- **Decision:** Multiplexer follows the machine's global secrets policy: **OS keychain for local secrets + session-cache model** (like `%LOCALAPPDATA%\mcp-session\*.env`), and `op://Vault/Item/field` **references only** in configs (never raw values, never runtime live `op` reads). plan/17 must be corrected: NO runtime `op://` resolution via live `op` reads or an unspecified 1Password SDK.
- **Concretely:** Multiplexer's `SecretStore` reads from the OS keychain and the session cache; configs may reference `op://` but resolution happens via the session-cache/refresh mechanism, not live `op`.

## D24. Relay E2EE — honest claim (LOCKED)
- **Decision:** The relay is a **TLS-terminating pipe**; the relay operator (Cloudflare or self-hosted) **can see plaintext**. We do NOT claim end-to-end encryption. Mitigations: ticket/DPoP auth, short-lived scoped sessions, and (optionally) a per-tunnel session-key E2EE layer via the pairing handshake — but the default claim is honest TLS-terminating, not E2EE.
- **Action:** plan/14 must correct the false "relay sees no plaintext" claim.

## D25. Remote-agent trust boundary — independent enforcement on remote (LOCKED)
- **Decision:** The SSH `--remote` agent **independently enforces** permission modes, worktree confinement, and approval gating on the remote host. It is NOT a dumb executor that trusts the local core implicitly. plan/17's threat model must include this.

## D26. Browser security tests — mandatory (LOCKED)
- **Decision:** The browser security controls (random port, localhost-only bind, origin allow-list, per-launch token, short-lived session, process hygiene) are **mutation-gated and tested** (unit + integration). plan/11 must add security-focused tests.

## D27. CI headless browser — pinned download in CI only (LOCKED)
- **Decision:** CI obtains a headless browser via a **pinned `playwright`/`chromium` download in CI only** (not shipped to users). This resolves the "no bundled Chromium" vs "CI needs a browser" contradiction. plan/11 must specify this.

## D28. HAR is CDP-only — honest caveat (LOCKED)
- **Decision:** HAR capture is **CDP-only** (Chromium-family). Firefox (BiDi) and Safari (WebDriver) get reduced or no HAR. plan/01 must present HAR with this caveat, not as a universal win.

## D29. Signing — Azure Trusted Signing + budget (LOCKED)
- **Decision:** Use **Azure Trusted Signing** (cheaper, no hardware token, ~$10/mo + per-signature) for Windows code signing, with a **budget line item** and identity-verification lead time treated as a schedule risk. plan/18 must add the budget and the OV-vs-EV-vs-Azure decision.

## D30. Monetization — freemium (LOCKED)
- **Decision:** **Freemium**: free tier (local, single-provider, core features) + paid tier (multi-provider, remote/relay, mobile advanced, usage analytics, priority support). plan/00/01/18/19/20 must add a monetization/GTM section.

## D31. Track upstream — standing task (LOCKED)
- **Decision:** Add a **standing "track upstream" task** to the roadmap: monitor grok-build changelog (daily sync, only signal) + T3 Code issues/commits. Re-validate the competitive snapshot quarterly. This is a recurring task, not one-time research.

## D32. E2E cadence — merge gate + nightly (LOCKED)
- **Decision:** E2E runs on the **merge gate** (critical paths) and **nightly** (full suite). No "skip e2e for small changes" path. plan/15's inconsistency is resolved to this.

## D33. Mutation floor vs gate — 70% is the merge floor (LOCKED)
- **Decision:** 70% mutation score is the **merge floor** (minimum to merge), and the bar may rise over time. plan/15 wording is clarified.

## D34. Plan-docs as differentiator — REMOVE from whitespace list (LOCKED)
- **Decision:** Remove "plan/00-x.md orchestration docs" from plan/01's whitespace/differentiator list. It's a process artifact, not a customer-facing differentiator. Keep it as an internal process note only.

## D35. Windows-first framing — DE-RISKED by Phase-0 spike (LOCKED)
- **Decision:** plan/01 presents Windows-first as **de-risked**: the Phase-0 spike proved grok-build builds and runs on Windows (`docs/SPIKE-REPORT.md`). The ACP fallback is retained as resilience, not as a first-class contingency. Frame as "Windows-primary," not "Windows-only."

## D36. Orca baseline default — match all (LOCKED, consistent with D7)
- **Decision:** plan/20's "subset in MVP" recommendation is overridden: the default is **match all** (D7). plan/20 must be updated to match plan/00/01.

## D37. Pairing credential model — reconcile (LOCKED)
- **Decision:** Reconcile plan/13 and plan/14 pairing: QR encodes a one-time code → exchange → issues a **long-lived device credential** (device id + stored secret in OS keychain), which is then minted into **short-lived tickets** for actual use (per plan/17). No long-lived bearer secret used directly on the wire.

## D38. Local tickets — keychain only (LOCKED)
- **Decision:** Local tickets are written to the **OS keychain only**, NOT a plaintext local token file. plan/17 corrected.

## D39. Auto-update — no live-swap (LOCKED)
- **Decision:** Auto-update swaps on **next launch** (native Rust cannot live-swap the running binary). Remove the "live-swap" phrasing from plan/18.

## D40. Roadmap dependency spine — Phase 4 depends on Phase 1 (LOCKED)
- **Decision:** plan/19's dependency-spine diagram is corrected: Phase 4 (mobile+remote) depends on Phase 1 (wire contract), not Phase 3. Phase 3 and Phase 4 can run in parallel (both depend on Phase 1).

## D41. Extension mechanism — plugins, not PRs (LOCKED)
- **Decision:** Everything deployment-specific — credential vaults, session sandboxes, harness adapters, approval policies, future panes — is a **plugin** behind stable, capability-scoped extension seams (`multiplexer-plugin-api`). Users extend Multiplexer by writing plugins, NOT by forking or PR-ing core.
- **Rationale:** Keeps the security-critical core small and reviewable while the ecosystem grows without our bottleneck. A vault integration (1Password), a sandbox backend, or a new harness (claude-code, codex, opencode, zcode) each become a plugin instead of core surface area. This is also a moat: the extension seam is a product.
- **Trade-off accepted:** API stability discipline and versioning burden on the seams (D20 discipline applies). Third-party plugins are out-of-process against a versioned sidecar protocol in v1 (no in-process WASM yet).
- **Action:** plan/21 defines the seams, manifest, lifecycle, and testing bar.

## D42. Plugin capability model — declared, least-privilege, enforced (LOCKED)
- **Decision:** Every plugin ships a manifest declaring `kind` (credential | sandbox | harness | approval | pane) and **capabilities** (e.g., `credential-read = ["1pass://automation-vault/*"]`, an egress network allow-list — deny by default). The plugin host enforces capabilities; plugins get **only** the API handles their capabilities grant — no ambient authority.
- **Rationale:** A malicious or compromised plugin must be bounded to its declared slice. This is the blast-radius guarantee that makes third-party plugins acceptable at all.
- **Trade-off accepted:** capability enforcement code is itself security-critical core (mutation-gated per D21) and the install-time consent UX must be honest about what each capability grants.

## D43. Credential plane — CredentialProvider plugin; 1Password is first-party flagship (LOCKED)
- **Decision:** A `CredentialProvider` plugin trait bridges external vaults into the **existing session-cache model (D23)**. The first-party `plugin-1password` authenticates as a **1Password service account scoped to a single automation vault** and resolves references into the server-side session cache at session start — values are then injected, task-scoped, into the session sandbox. **No live user-session `op` reads (D23 unchanged); no credentials ever exist client-side.**
- **Rationale:** Extends D23 rather than amending it: the core SecretStore stays keychain + session cache; the plugin is the external-vault bridge with its own narrowly-scoped auth. Service-account scoping means a compromised agent (or agent host) can reach exactly the automation vault and nothing else — human vaults are structurally invisible.
- **Trade-off accepted:** operators must maintain an automation vault + service account; the plugin documents this as the canonical deployment.

## D44. Session isolation — SandboxProvider plugin; containers first (LOCKED)
- **Decision:** Each agent session executes inside a sandbox provisioned by a `SandboxProvider` plugin. The first-party default is container-based isolation on the server host: per-session container, workspace bind-mount scoped to the session worktree, no host home exposure, per-session egress policy, teardown shreds injected secrets. The **independent-enforcement rule of D25 is implemented at this layer** — confinement is enforced by the sandbox, never trusted from a client.
- **Rationale:** Sessions must be isolated from each other, from the server host, and from sibling agents' state (`~/.claude` et al.). The plugin seam lets deployments choose OS-user, container, or microVM backends without core changes.
- **Trade-off accepted:** container runtime is a server-host requirement; the plugin must degrade loudly (refuse to run unsandboxed) rather than silently.

## D45. Harness admission — HarnessAdapter plugin (LOCKED)
- **Decision:** Grok stays the core in-process adapter (D10). **All external harnesses (Claude Code, Codex, OpenCode, ZCode, …) are admitted as `HarnessAdapter` plugins** riding the generic ACP machinery (D17). Adding a harness is a plugin install, never a core PR.
- **Rationale:** The process boundary at ACP is a security feature here, not the introspection limitation plan/01 frames: external runtimes are closed and untrusted-by-default, so admission through an adapter + sandbox (D44) is the correct trust posture.
- **Trade-off accepted:** external-harness sessions get wire-level introspection only (no in-process internals) — acceptable; full internals remain a Grok-only differentiator.

## D46. Approval policy — pluggable on the D12 4-way enum (LOCKED)
- **Decision:** The approval decision path is an **`ApprovalPolicy` plugin chain** over the D12 enum: local prompt, mobile push, and declarative policies ("reads auto-allow; writes/egress always gate") compose in order, first non-defer wins. Because this gate is a security boundary (extending D12/D25), policy plugins require explicit user consent at install and are held to core testing bars (D21/D33).
- **Rationale:** The approval gate is where a hijacked session's *use* of its access is stopped; making the policy pluggable lets users tune strictness per deployment without forking, while keeping enforcement in core.

---

## Summary of doc-level fixes required (mapped to decisions)

| Doc(s) | Fix | Decisions |
|---|---|---|
| plan/00 | Present embedding as hypothesis; add monetization; Windows-first conditional | D10, D30, D35 |
| plan/01 | Embedding hypothesis; Windows-first conditional; HAR CDP caveat; remove plan-docs differentiator; add monetization; sourced competitive snapshot | D10, D28, D34, D35, D30, C3 |
| plan/02 | Crate layout → `multiplexer-*`; remove `mx-provider-openrouter`/`mx-mobile-shared`; ACP generic | D13, D14, D17, D20 |
| plan/03 | (mostly fine) confirm vendored fork + `[patch]`; add track-upstream | D5, D31 |
| plan/04 | 4-way approval enum; explicit wire↔ProviderEvent mapping table; session.start params; bounded backpressure | D12, D15, D19, D18 |
| plan/05 | 4-way approval enum; bounded channel; OpenRouter = config variant; ACP generic; canonical event vocab; session params | D12, D18, D14, D17, D16, D19 |
| plan/06 | Canonical event vocab; 4-way approval; our scheduler owns subagent cap (fork); bounded ingestion | D16, D12, D11, D18 |
| plan/07 | (mostly fine) | — |
| plan/08 | Windows-specific tests; deep TUI assertions; mutation targets; reference perf suite | M2, M4, C3, m3 |
| plan/09 | Add mutation + CI-gate section; LSP skip-not-fail | C1, M3 |
| plan/10 | Add unit/mutation/CI-gate for layout engine; direct detach/re-dock property tests | C2, m4 |
| plan/11 | Browser security tests; CI headless browser sourcing; HAR CDP caveat | M1, D27, D28 |
| plan/12 | (strong; fine) | — |
| plan/13 | Reconcile pairing; MVP timing (mobile in MVP) | D37, D8 |
| plan/14 | Relay E2EE honest; remote-agent trust boundary; reconcile pairing | D24, D25, D37 |
| plan/15 | Mutation scope → all core; add perf stage; e2e cadence; floor vs gate | D21, D22, D32, D33 |
| plan/16 | Quantify targets; measurement method; reference machine | M5, M6 |
| plan/17 | Secrets session-cache model; relay E2EE; remote trust; keychain-only tickets; browser opt-in human; HAR redaction | D23, D24, D25, D38, S4, S5 |
| plan/18 | Azure Trusted Signing + budget; no live-swap; monetization | D29, D39, D30 |
| plan/19 | Crate names; MVP=Phases 1-4; effort estimate; dependency spine; track-upstream task | D13, D8, D40, D31 |
| plan/20 | Orca baseline match-all; embedding hypothesis; Windows contingency; monetization; risk register updates | D36, D10, D35, D30 |
| plan/21 | NEW — plugin architecture: seams, manifest/capabilities, first-party plugins, threat-model mapping, milestones | D41, D42, D43, D44, D45, D46 |
