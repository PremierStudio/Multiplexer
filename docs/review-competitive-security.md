# Adversarial Review — Competitive Soundness, Security & Business Feasibility

**Reviewer:** adversarial subagent (competitive / security / business lens)
**Scope:** `plan/00`, `plan/01`, `plan/13`, `plan/14`, `plan/17`, `plan/18`, `plan/19`, `plan/20`, cross-checked against `docs/PLAN-CONTEXT.md`, the machine global secrets policy (`C:\Users\gollum\Tools\agent-policy\SECRETS.md`), and the cross-referenced `plan/04` (wire contract) and `plan/11` (browser).
**Date:** 2026-08-12

---

## (a) Summary verdict

The plan suite is **strong on architecture and engineering discipline** (TDD-at-inception, event-sourced orchestration, in-process embedding, a genuinely well-thought-out CDP security surface, and a coherent thin-client wire contract). The security posture in `plan/17` is, on the whole, the best-developed section and is largely consistent with the machine's global secrets policy.

However, the **competitive and business-feasibility claims are materially overstated and under-verified**, and there are **several concrete security gaps** that the docs either hand-wave or leave as "open questions" that are actually load-bearing. The single most important finding is that **the core differentiator — "in-process grok-build embedding" — is asserted as a moat without any evidence that (a) the grok-build crates expose a stable, embeddable library API, or (b) the Windows build is even feasible**, and the plan's own risk register rates the Windows build as the #1 risk while the competitive docs simultaneously treat it as a guaranteed win. That tension is unresolved and is the crux of the business case.

**Verdict: NOT ready to lock.** The architecture is defensible; the competitive moat and the Windows-first feasibility are not yet proven. Several security items must be resolved before Phase 4 (remote/mobile) ships.

---

## (b) Critical issues (must fix)

### C1. The "in-process embedding" moat is asserted, not verified — and it is the entire business case
`plan/01` §4.1 and `plan/00` differentiator #1 claim "Nobody else does this" and call it "our moat" (`plan/01` §6.5). But:
- PLAN-CONTEXT itself lists the grok-build crates as `xai-grok-pager-bin` (composition root → binary), `xai-grok-pager` (TUI), `xai-grok-shell`, `xai-grok-tools`, `xai-grok-workspace`. The **composition root is a binary**, and the plan's own `plan/20` §2.3 admits "the shell's public surface is not a stable, versioned contract."
- No doc demonstrates that `xai-grok-shell` can actually be consumed as a library with a stable API. The plan *assumes* it ("Reuse `xai-grok-shell` ... as libraries") without a spike.
- **This is a Phase-0-verifiable claim that the plan itself schedules first, yet the competitive docs treat it as settled fact.** If the shell is not cleanly embeddable, the differentiator collapses to "we drive the same CLI everyone else drives," and the entire §4 whitespace narrative (HAR tied to in-process runtime, orchestration insight) weakens.

**Fix:** Add an explicit Phase-0 gate: "embed `xai-grok-shell` as a library and run a headless turn in-process" as a **go/no-go** for the embedding differentiator, and reword `plan/01`/`plan/00` to present embedding as a *hypothesis to be proven*, not a fact. Do not let the competitive docs claim a moat that Phase 0 has not yet validated.

### C2. Windows grok-build build is simultaneously "the #1 risk" and "a guaranteed differentiator" — the docs contradict themselves
- `plan/20` §2.1 rates the Windows build **High / Critical** and calls it "the single largest technical risk."
- `plan/01` §4.5 and `plan/00` differentiator #9 present **Windows-first as a clean win** ("the only serious control surface that treats Windows as first-class").
- If the Windows build fails (or takes 6+ months of porting), the Windows-first positioning and the embedding differentiator both collapse, and the product is left as "a macOS/Linux app that's late." The competitive docs never price this in.

**Fix:** The roadmap (`plan/19` Phase 0) correctly identifies this as the critical path — good. But `plan/01` must carry a contingency: if Windows embedding fails, what is the fallback positioning? (`plan/20` mentions an ACP fallback but `plan/01` does not.) The competitive analysis should present Windows-first as *conditional*.

### C3. No evidence the "Orca baseline bar" is complete or current
`plan/01` §3 and PLAN-CONTEXT §"Baseline bar" enumerate Orca's features from a static snapshot. The review found **no primary-source verification** (no Orca docs/site fetch, no release notes) in any doc. Specific concerns:
- Orca is described as "macOS/Windows/Linux" in `plan/01` §1.1 but "macOS-first in practice" in §4.5 — an unverified and slightly contradictory characterization.
- The claim that Orca "drives CLIs (no in-process embedding)" and "bundles Chromium" is asserted as fact with no citation.
- **Competitors move fast.** A quarterly re-validation is mentioned in `plan/20` §3.5 but the baseline bar itself is treated as frozen.

**Fix:** Add a dated, sourced competitive snapshot (with URLs) to `plan/01`, and mark every competitor capability as "verified <date>" vs "assumed." This is cheap and materially de-risks the positioning.

---

## (c) Major issues (should fix)

### M1. `plan/17` §2.4 explicitly diverges from the machine's global secrets policy — and the divergence is a real security regression
The global policy (`SECRETS.md`) mandates the hot-path session cache (`%LOCALAPPDATA%\mcp-session\*.env`, user-only ACL, this-boot-only) as the *only* agent-usable secret path, and bans live `op` reads. `plan/17` §2.4 says Multiplexer "uses the keychain directly and does not write `.env` caches," and §2.3 says configs reference `op://` and are "resolved at runtime through `mx-auth`'s `SecretStore`."

This is a **direct contradiction of the machine's standing orders**, which the doc itself claims to follow (§1: "It follows the machine's global secrets policy"). Two problems:
1. **Policy violation:** The policy's model is `op://` in configs → `refresh-session-secrets` → session `.env` → wrappers. `plan/17` replaces the session-cache step with a runtime `op://` resolver that would require live `op` reads (banned) or a new keychain sync mechanism that doesn't exist.
2. **Feasibility:** A native app resolving `op://Vault/Item/field` at runtime requires either the 1Password CLI (banned for reads) or the 1Password SDK/Connect — neither is specified, costed, or approved.

**Fix:** Reconcile `plan/17` with the global policy. Either (a) adopt the session-cache model for the app's config surface, or (b) explicitly flag this as a deliberate product-level divergence requiring user approval, with a concrete resolution mechanism (e.g., 1Password SDK) and its licensing/cost. As written, the doc is internally inconsistent with the policy it cites.

### M2. The relay tunnel's "end-to-end encrypted, relay sees no plaintext" claim is not achievable with the described design
`plan/14` §4.1 and §6.2 claim the relay "never sees plaintext application data" and is "end-to-end protected (TLS at the transport + DPoP/ticket auth at the application layer)." But the described relay is a **Cloudflare Worker + Durable Object that terminates TLS** and forwards WebSocket frames. A TLS-terminating relay **does** see plaintext application bytes (it decrypts at the edge). The only way the relay sees no plaintext is if the client↔core traffic is additionally encrypted end-to-end (e.g., a session key established out-of-band) — which is **not specified anywhere** in `plan/14` or `plan/17`.

This is a security claim that is currently false as designed. If the intent is "the relay operator (Cloudflare) could read traffic," that must be stated honestly; if the intent is true E2EE, the key-agreement mechanism must be designed (and it interacts with DPoP/ticket auth, which is not currently an encryption mechanism).

**Fix:** Either (a) downgrade the claim to "relay is a TLS-terminating pipe; Cloudflare (or a self-hosted operator) can see plaintext — mitigated by ticket/DPoP auth and short-lived scoped sessions," or (b) specify a real E2EE layer (e.g., per-tunnel session keys via the pairing handshake) and add it to the threat model. As written, §6.4's "Relay compromise → relay is a dumb pipe; end-to-end ticket+DPoP+TLS" mitigation is **incorrect** — DPoP and tickets authenticate; they do not encrypt.

### M3. The mobile app's "required" status is in tension with the roadmap's own MVP staging
`plan/13` §1 correctly states the mobile app is **required** (PLAN-CONTEXT differentiator #8, baseline bar, North Star). But `plan/19` Phase 4 (mobile+remote) is the **fourth** phase, after Core MVP, Editor+Panes, and Browser+HAR. `plan/13` §8 and `plan/19` §11 both leave "MVP timing of the mobile app" open.

If mobile is truly required for the product to be credible (Orca has it), then shipping it in Phase 4 is fine **only if** the MVP definition includes Phase 4. The docs never state whether "MVP" = Phase 1 (Grok control surface) or Phase 4 (mobile+remote). This ambiguity means the "required mobile app" could silently slip past the MVP. **Fix:** Define "MVP" explicitly in `plan/19` (recommend: MVP = Phases 1–4, i.e., the full baseline bar + core differentiators), and make the mobile app's MVP inclusion a hard gate, not an open question.

### M4. `plan/18` under-specifies the Windows code-signing cost/identity reality
`plan/18` §3.2 recommends an **OV** certificate and §9 flags "signing budget" as an open question. But:
- OV code-signing certs cost **$200–$400+/yr** and require **organization identity verification** (DBA/business registration) — a real friction for a solo/founder project. EV certs cost more and require a hardware token.
- The plan does not budget for the **yearly cert renewal**, the **HSM/cloud-signing service** (Azure Trusted Signing ~$10/mo + per-signature, DigiCert KeyLocker), or the **identity-verification lead time**.
- SmartScreen reputation is correctly flagged as slow to build, but the plan offers no mitigation for the **first-release trust valley** (users see "unknown publisher" warnings for weeks regardless of OV).

**Fix:** Add a concrete signing budget line item and a decision on OV vs EV vs Azure Trusted Signing (which is cheaper and doesn't require a hardware token) to `plan/18` §9, and treat the identity-verification lead time as a schedule risk in `plan/19`.

### M5. The "no bundled Chromium" differentiator has an unaddressed UX/feasibility hole: no-browser and headless-CI cases
`plan/11` §11 open question #5 flags that when **no system browser is installed** (or none is drivable), the plan shows a "no drivable browser found" state rather than bundling. This is a real product gap:
- **Windows Server / minimal images / CI runners** frequently have no GUI browser. The plan's own browser integration tests (`plan/11` §10.2) require "headless Chromium available in CI" — but if we never bundle Chromium, where does CI's headless Chromium come from? The plan says tests "skip (not silently passed) otherwise," which means **the browser/HAR/Design-Mode differentiators could be entirely untested in CI** if no browser is present.
- This is an internal contradiction: the differentiator is "no bundled Chromium," yet the test strategy implicitly depends on a Chromium being available.

**Fix:** Specify how CI obtains a headless browser (e.g., a pinned `playwright`/`chromium` download in CI only — not shipped to users), and decide the no-browser UX explicitly. Also note: **HAR capture is CDP-only** (`plan/11` §8), so on Firefox/Safari the HAR differentiator silently degrades — `plan/01` presents HAR as a universal win without this caveat.

### M6. `plan/14` SSH worktrees and `plan/17` don't address the **remote-agent trust boundary** for the SSH `--remote` mode
`plan/14` §3.1 has the local core run the agent runtime locally but operate on a remote filesystem over SSH, with a "remote agent (`multiplexer --remote`)" exposing fs/git/process/terminal. This means:
- The remote host runs **our binary with broad filesystem/process/PTY access** over an SSH channel. If the SSH channel or the remote agent is compromised, the attacker gets arbitrary command execution on the remote host.
- `plan/17`'s permission modes (§7) are enforced **server-side (local core)** — but the remote agent executes commands on the remote host. **Who enforces scope on the remote side?** The docs never specify that the remote agent independently enforces permission modes or worktree confinement. A malicious agent prompt could drive the local core to issue a destructive command to the remote agent, and if the remote agent trusts the local core implicitly, the permission-mode mitigation is bypassed on the remote.
- Also unaddressed: **remote-side secrets** (provider tokens on the remote) and whether the remote agent holds any credentials.

**Fix:** Specify the remote agent's independent security posture (does it enforce its own permission modes / worktree confinement / approval gating, or is it a dumb executor?). Add this to `plan/17`'s threat model (it's currently absent).

---

## (d) Minor issues

### m1. `plan/01` §4.7 lists "authoring plan docs" as a differentiator
"Plan/00-x.md orchestration docs" is listed as whitespace item #7 ("a documented, adversarial-reviewed plan that competitors ... do not have"). This is a **process artifact, not a product differentiator** — it has zero customer-facing value and will not be visible to buyers. Including it in the "7 things nobody has" dilutes the credibility of the other six. Recommend removing it from the whitespace list (keep it as an internal process note).

### m2. `plan/01` §5 gap table marks "Windows-first" as a **Win** over Orca/T3/OpenCode with a ⚠️
Orca, T3, and OpenCode are all cross-platform (including Windows). Marking "Windows-first" as a Win over them is defensible only as "primary platform," not "Windows support." The table's ⚠️ partially acknowledges this, but the §4.5 narrative ("the only serious control surface that treats Windows as first-class") overstates — Orca and T3 do ship Windows today. **Fix:** soften to "we are Windows-first; competitors are cross-platform but not Windows-primary."

### m3. `plan/13` §5.2 and `plan/14` §5 describe **two different pairing flows**
- `plan/13` §5.2: "scan a QR code ... Pairing establishes a long-lived trusted identity on the phone (stored in the OS keychain) and registers the phone with the core."
- `plan/14` §5.1: QR encodes a one-time code → exchange → issues a **long-lived device credential (device id + stored bearer secret)**.
These are compatible in spirit but use different terminology ("long-lived trusted identity" vs "device credential + bearer secret") and `plan/14` introduces a **long-lived bearer secret** that `plan/17` §4.1 says should be minted into short-lived tickets. The docs should reconcile the pairing credential model so the security properties are unambiguous.

### m4. `plan/17` §4.1 says local tickets are "written to the OS keychain / a local token file"
Writing a ticket to a **local token file** on disk (even loopback) is a minor plaintext-on-disk concern that contradicts the doc's own "no raw secrets in plaintext" principle. Minor, but the doc should pick keychain-only for consistency.

### m5. `plan/18` §4.1 auto-update "live-swaps if the runtime supports it"
GPUI/native Rust apps generally cannot live-swap the running binary. The plan hedges with "or swaps in on next launch," which is fine, but the "live-swap" phrasing should be removed to avoid over-promising.

### m6. `plan/19` Phase 3 sequencing note says Phase 3 "can run in parallel with Phase 4"
But Phase 3 (browser+HAR) needs the pane system (Phase 2) and orchestration (Phase 1), while Phase 4 (mobile+remote) needs the wire contract (Phase 1). Both depend on Phase 1, so parallel is fine — but the roadmap's dependency spine diagram (§9.1) shows Phase 4 branching off Phase 3, which is slightly misleading (Phase 4 depends on Phase 1, not Phase 3). Cosmetic.

### m7. `plan/20` §6.7 recommends "subset in MVP" for the Orca baseline, but `plan/00`/`plan/01` default to "match all"
`plan/00` §3 and `plan/01` §3 both default to matching the **full** Orca baseline; `plan/20` §6.7 recommends a **subset**. These are contradictory defaults across docs (both correctly flag it as an open question, but the "default" differs). Reconcile so the roadmap has one stated default.

---

## (e) Security findings

### S1. [CRITICAL] Relay E2EE claim is false as designed (see M2)
The relay terminates TLS and forwards frames; it **can** see plaintext. DPoP/tickets authenticate but do not encrypt. `plan/14` §6.4's "relay compromise → dumb pipe, end-to-end" mitigation is incorrect. Must be fixed or honestly downgraded.

### S2. [HIGH] Remote-agent trust boundary unaddressed (see M6)
The SSH `--remote` agent executes commands on the remote host with no specified independent enforcement of permission modes/worktree confinement. A compromised local core or a malicious agent prompt could drive destructive commands on the remote. Add to `plan/17` threat model.

### S3. [HIGH] `plan/17` secrets policy contradicts the machine's global policy (see M1)
Runtime `op://` resolution requires banned live `op` reads or an unspecified 1Password SDK. The doc claims to follow the policy but replaces its session-cache mechanism. Must be reconciled.

### S4. [MEDIUM] CDP surface is well-mitigated — but one gap: `browser.cdp` raw passthrough
`plan/04` §4.9 exposes `browser.cdp` (raw CDP passthrough) and `plan/17` §5.2 correctly restricts it to `control` scope and validates against an allowlist. Good. However, `plan/17` §5.2 says "reject dangerous methods ... unless the user opts in" — the "user opts in" escape hatch is underspecified (per-session? per-call? who is the user — the human or the agent?). If the **agent** can trigger the opt-in, the mitigation is weakened. Tighten: opt-in must be a human approval, not an agent action.

### S5. [MEDIUM] HAR redaction is seeded from keychain values
`plan/17` §6.2 seeds redaction patterns "from the OS keychain (values the user has stored)." This means the app reads every stored secret to build a redaction list — a broad keychain read that expands the attack surface and the "least privilege" principle the doc itself states. Consider seeding from a curated allowlist + user-configured patterns rather than scanning all keychain items.

### S6. [MEDIUM] OAuth loopback listener — `plan/17` §3.2 is solid, but the "system browser" reuse has a subtle issue
The OAuth flow opens the provider URL in the **system browser** (reusing `mx-browser`). If the system browser is the user's real profile with remote debugging, the OAuth callback and any tokens in the redirect URL pass through the CDP-managed browser. `plan/17` §3.2 binds the callback to `127.0.0.1` and validates `state`, which is good, but the plan should confirm the OAuth flow does **not** require the CDP debugging session to be active (i.e., OAuth should use a plain browser launch, not the debugged one) to avoid coupling auth to the CDP surface. Minor but worth stating.

### S7. [LOW] `plan/17` §4.1 local ticket "token file" (see m4)

### S8. [POSITIVE] What is done well
- CDP hardening (`plan/17` §5, `plan/11` §9): loopback-only, random port, origin allow-list, token, no remote forwarding, throwaway-profile default for agent automation. This is genuinely strong and correctly identifies the real threat.
- Permission modes with server-side scope enforcement and audit trail (`plan/17` §7).
- HAR privacy-by-default with metadata-only capture and capture-time redaction (`plan/17` §6).
- Supply chain: `cargo audit`/`deny`, committed lockfile, vendored-fork provenance, secret-scanning (`plan/17` §8).
- Threat model is comprehensive (T1–T12) and maps to mitigations.

---

## (f) Competitive findings

### F1. The whitespace is real but narrower than claimed
The genuinely defensible differentiators are: **in-process embedding** (if it works — C1), **native editor** (real, no control-surface competitor has one), **HAR** (real, nobody has it), **system-browser import** (real, but see F3), **mutation-gated CI** (real but a process/quality signal, not a customer-facing feature). **Windows-first** is real only if the Windows build succeeds (C2). **Subagent fan-out dashboard** is partially claimed by Conductor (parallel agents) — `plan/01` §5 marks it a Win with a ⚠️, which is honest.

### F2. The "moat" framing overstates durability
`plan/01` §6.5 calls in-process embedding "our moat" and "cannot be bolted on." True for Orca/T3's current architecture — but **T3 Code is open-source-available** and could fork/embed grok-build the same way we do, and Orca could add a native editor. The moat is a **first-mover + execution** advantage, not a structural one. The docs should not claim structural permanence.

### F3. HAR and system-browser are CDP-only — the "nobody has this" claim is narrower than presented
`plan/11` §8 makes clear HAR capture is CDP-only (Chromium-family); Firefox (BiDi) and Safari (WebDriver) get reduced or no HAR. `plan/01` presents HAR as a universal win. If a user's primary browser is Firefox, the HAR differentiator is absent. This is a real caveat that the competitive docs omit.

### F4. The Orca baseline bar is unverified (C3) — and the "match all Orca features" default is a large, under-costed commitment
Matching parallel worktrees + Ghostty terminal + Design Mode + SSH worktrees + inline diff comments + GitHub/Linear + mobile + usage tracking + CLI + native search + split-anything panes is essentially building a second Orca. `plan/19` spreads this across Phases 1–5, but the **cumulative effort is not estimated anywhere** (no person-month or calendar estimate in any doc). For a greenfield solo/founder project, this is a serious feasibility gap. Recommend an explicit effort estimate and a hard MVP cut.

### F5. Business feasibility: no revenue/monetization model anywhere
None of the docs (00, 01, 18, 19, 20) address **how Multiplexer makes money**. Account/usage tracking is listed as baseline, and `plan/14` §7.2 defers billing/entitlements out of MVP scope, but there is no pricing, no free-vs-paid tier, no competitive pricing analysis (Orca's pricing, T3's pricing), and no go-to-market. For a "business feasibility" review this is a notable omission — the plan is entirely engineering-focused.

---

## (g) Specific findings — doc + section references

| # | Doc / Section | Finding |
|---|---|---|
| C1 | `plan/01` §4.1, §6.5; `plan/00` diff #1; `plan/20` §2.3 | Embedding moat asserted without a library-API spike; shell public surface admitted unstable |
| C2 | `plan/20` §2.1 vs `plan/01` §4.5, `plan/00` diff #9 | Windows build = High/Critical risk vs presented as guaranteed win; no fallback positioning in `plan/01` |
| C3 | `plan/01` §1.1, §3, §4.5 | Orca baseline unverified/no sources; "macOS-first in practice" vs "macOS/Windows/Linux" tension |
| M1 | `plan/17` §2.3, §2.4 vs `SECRETS.md` | Runtime `op://` resolution contradicts global session-cache policy; no resolution mechanism specified |
| M2 | `plan/14` §4.1, §6.2, §6.4 | Relay E2EE claim false: TLS-terminating relay sees plaintext; DPoP/tickets don't encrypt |
| M3 | `plan/13` §1, §8; `plan/19` §11 | "Required" mobile app vs Phase-4 staging; MVP definition never fixed |
| M4 | `plan/18` §3.2, §9 | Signing cost/identity-verification lead time under-specified; no budget line |
| M5 | `plan/11` §10.2, §11 Q5; `plan/01` §4.2 | No-browser UX and CI headless-Chromium sourcing unresolved; HAR CDP-only caveat omitted from competitive docs |
| M6 | `plan/14` §3.1; `plan/17` §7 | Remote `--remote` agent trust boundary / independent scope enforcement unspecified |
| m1 | `plan/01` §4.7 | Plan-docs listed as a product differentiator |
| m2 | `plan/01` §4.5, §5 | "Windows-first" Win over cross-platform competitors overstated |
| m3 | `plan/13` §5.2 vs `plan/14` §5.1 | Two pairing credential models not reconciled |
| m4 | `plan/17` §4.1 | Local ticket "token file" contradicts no-plaintext principle |
| m5 | `plan/18` §4.1 | "Live-swap" auto-update over-promises for native Rust |
| m6 | `plan/19` §9.1 | Dependency-spine diagram implies Phase 4 depends on Phase 3 (it depends on Phase 1) |
| m7 | `plan/20` §6.7 vs `plan/00` §3, `plan/01` §3 | Baseline default differs (subset vs full) across docs |
| S1 | `plan/14` §6.4 | Relay-compromise mitigation incorrect (see M2) |
| S2 | `plan/14` §3, `plan/17` §7 | Remote-agent trust boundary missing from threat model |
| S3 | `plan/17` §2 | Secrets policy contradicts global policy (see M1) |
| S4 | `plan/17` §5.2 | `browser.cdp` "user opts in" escape hatch underspecified (human vs agent) |
| S5 | `plan/17` §6.2 | HAR redaction seeded from all keychain values = broad keychain read |
| S6 | `plan/17` §3.2 | OAuth via system browser should not couple to CDP debugging session |
| F4 | `plan/19` all phases | No cumulative effort estimate for matching full Orca baseline |
| F5 | `plan/00`, `plan/01`, `plan/18`, `plan/19`, `plan/20` | No monetization/pricing/GTM anywhere |

---

## Bottom line

**Architecture and security engineering: strong.** The CDP hardening, permission modes, HAR privacy, supply-chain controls, and the thin-client wire contract are genuinely well-designed and internally consistent.

**Competitive moat and business feasibility: not proven.** The plan asserts a moat (in-process embedding) and a wedge (Windows-first) that both hinge on a single unverified Phase-0 feasibility question (the Windows grok-build build / library embeddability), while simultaneously rating that question as the #1 risk. The Orca baseline is unverified and under-costed, and there is no monetization model.

**Security: three must-fix items** — the false relay E2EE claim (M2/S1), the remote-agent trust boundary (M6/S2), and the secrets-policy contradiction with the machine's global policy (M1/S3).

**Recommendation:** Resolve C1/C2 (prove embedding + Windows build in Phase 0 before locking the competitive narrative), fix M1/M2/M6 before Phase 4 ships, reconcile the MVP/mobile definition (M3), and add a signing budget (M4) and a monetization section (F5) before treating the plan as business-ready.
