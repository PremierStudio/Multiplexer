# Plan 18 — Build, Release & Distribution

**Status:** Planning · **Owner:** subagent fan-out → adversarial review
**Consistency:** This doc follows `docs/PLAN-CONTEXT.md` exactly. Where the shared context leaves a decision open (e.g. branding/domain, Windows-first confirmation), this doc makes a **recommendation** and flags it as an open question rather than silently diverging.

**Locked decisions applied:** This doc is consistent with the following locked decisions from `docs/DECISIONS.md`:
- **D29 (Signing):** Windows code signing uses **Azure Trusted Signing** (no hardware token, ~$10/mo + per-signature), with a **budget line item** and identity-verification lead time treated as a **schedule risk** (§3.2, §9, §10, §11).
- **D39 (Auto-update):** Auto-update swaps on **next launch** — native Rust cannot live-swap the running binary; all "live-swap" phrasing removed (§4.1).
- **D30 (Monetization):** Freemium model — free local single-provider tier + paid multi-provider/remote/mobile-advanced tier; distribution channels support this (§7.5).

---

## 1. Purpose & scope

Multiplexer is a **single native Rust binary** (server-centric runtime) with thin desktop/mobile/web clients. Because there is no Node runtime, no Chromium, and no interpreter to ship, the artifact is small and the distribution story is comparatively simple — but the bar for **trust, signing, and update safety** is high: this is a tool that drives the user's real agents, terminals, git, filesystem, and browsers. A broken or tampered update is not a cosmetic failure; it is a security incident.

This doc is the implementation plan for how we **build, sign, package, update, and ship** Multiplexer across Windows (first), macOS, and Linux. It covers:

1. Packaging (Windows NSIS/MSI, macOS dmg, Linux AppImage/deb).
2. Code signing (Windows, macOS notarization, Linux) and why it matters.
3. Auto-update (native mechanism, update server, staged rollouts, rollback).
4. CI/CD (build matrix, the full test-gate chain, release smoke tests).
5. The grok-build vendored build (building embedded crates; Windows build fixes).
6. Versioning (semver, changelog).
7. Distribution channels (Multiplexer.dev, winget/brew, direct downloads, update server).
8. Release process (cut → smoke → ship).
9. Open questions (branding/domain, Windows-first confirmation, signing budget).

---

## 2. Packaging

Windows-first is a core differentiator (Superset and Conductor are macOS-only). We ship **Windows first**, then macOS, then Linux. All three targets are produced from the **same CI pipeline** (§5) so packaging is reproducible and auditable.

### 2.1 Artifact matrix

| Platform | Target triple | Installer format | Notes |
|----------|---------------|------------------|-------|
| **Windows** | `x86_64-pc-windows-msvc` | **NSIS** (primary, per-user) + **MSI** (enterprise/IT-managed) | Windows-first; both produced from one build |
| **macOS** | `aarch64-apple-darwin` + `x86_64-apple-darwin` | **dmg** (universal or per-arch) | Apple Silicon + Intel; notarized |
| **Linux** | `x86_64-unknown-linux-gnu` | **AppImage** (primary) + **deb** (Debian/Ubuntu) | AppImage for portability; deb for apt users |

### 2.2 Windows packaging

- **NSIS installer (primary).** Per-user install (no admin elevation) by default — matches the "control surface for your agents" positioning and keeps the update story simple (see §4). Options: Start Menu shortcut, desktop shortcut (opt-in), `Add/Remove Programs` entry, file-association for a `.mx` project file, and a `--silent` flag for scripted installs.
- **MSI (secondary).** For IT-managed fleets and enterprise policy. Produced via WiX (or the `cargo-wix` toolchain) from the same build artifacts. MSI gives us per-machine install, GPO/Intune deployment, and proper uninstall/repair semantics. MSI is a **stretch** for MVP; NSIS is the MVP installer.
- **Architecture:** x64 only for MVP (the overwhelmingly common Windows target). ARM64 Windows is tracked as a future target (see §9).
- **No bundled Chromium** (differentiator #3) — the installer stays lean. We detect/import the user's installed browsers at runtime instead (Plan 11).

### 2.3 macOS packaging

- **dmg** containing a `.app` bundle. Produce **both** `aarch64` and `x86_64` builds; ship either a universal binary or per-arch dmg (recommend per-arch dmg for MVP to keep the binary lean, with a universal build as a later optimization).
- The `.app` must be **notarized** (see §3.2) or Gatekeeper will block it.
- Code-sign the app bundle, the dmg, and the embedded helper binaries.

### 2.4 Linux packaging

- **AppImage (primary).** Self-contained, runs on most distros without installation, ideal for direct downloads and the update flow. Requires `appimagetool` and an AppImage runtime; we pin the tool version in CI.
- **deb (secondary).** For Debian/Ubuntu users who prefer a system package. Produced with `cargo-deb` or a hand-rolled `dpkg-deb` layout. Recommend **not** shipping a `.rpm` in MVP (smaller audience); track as a follow-up.
- **Portability note:** GPUI/wgpu needs a working Vulkan/GL stack; we document minimum graphics requirements and test on a headless CI runner with a software renderer (lavapipe) for smoke tests.

### 2.5 Packaging tooling & reproducibility

- **Reproducible builds:** pin every tool (NSIS, WiX, `appimagetool`, `cargo-deb`, `hdiutil`, `codesign`, `notarytool`) to exact versions in CI. Record build metadata (commit SHA, toolchain, source tree hash) into a `build-info` embedded in the binary and surfaced in the app's About dialog.
- **`build.rs` bootstrap:** a single bootstrap script verifies toolchain + protoc presence (per Plan 03 §6.3) and fails fast with a clear message instead of a cryptic linker error.
- **Artifact naming:** `multiplexer-<version>-<platform>-<arch>.<ext>` (e.g. `multiplexer-0.4.0-windows-x64.exe`), plus a `latest.yml`/`latest-mac.yml`/`latest-linux.yml` update manifest (see §4).

---

## 3. Code signing

Signing is **not optional** for a tool like this. It is the mechanism by which the OS and the user establish that the binary they run is genuinely ours and has not been tampered with. For a product that drives agents, terminals, git, and browsers, an unsigned or compromised binary is a direct path to full machine compromise.

### 3.1 Why signing matters (trust)

- **Windows SmartScreen:** unsigned or untrusted-signer executables trigger "Windows protected your PC" warnings that scare away users and erode trust. A valid code-signing certificate from a trusted CA (we use **Azure Trusted Signing**, §3.2) clears this.
- **macOS Gatekeeper:** unsigned apps are blocked outright ("cannot be opened because the developer cannot be verified"); notarization is mandatory for distribution outside the App Store.
- **Linux:** no mandatory signing, but we still sign for integrity (see §3.3) and to let users verify provenance.
- **Auto-update integrity:** every update payload is signed; the client verifies the signature before applying (see §4). This is the single most important trust property of the update system.

### 3.2 Windows code signing

- **Certificate & service (LOCKED — D29):** use **Azure Trusted Signing** for Windows code signing. It is **cheaper** than a traditional OV/EV certificate (no hardware token, no per-certificate hardware purchase), priced at roughly **~$10/mo + per-signature**, and the key is held by Microsoft — it is **never** on a CI runner or a developer laptop. This resolves the OV-vs-EV-vs-Azure question in favor of Azure Trusted Signing.
- **Signing method:** sign with `signtool` (Windows SDK) against the Azure Trusted Signing endpoint using **timestamping** (RFC 3161) so signatures remain valid after the cert expires. Sign the main `.exe`, the NSIS installer, and the MSI.
- **Key custody:** the signing key is held by **Azure Trusted Signing** (Microsoft-managed HSM) — **never** on a CI runner or a developer laptop. CI calls the signing service over an authenticated, audited channel (Azure AD / managed identity). This is a security requirement (Plan 17) and a release-process requirement.
- **Identity verification lead time (schedule risk — D29):** Azure Trusted Signing requires **organization identity verification** (Org ID validation) before the signing account is provisioned. This is a **schedule risk** — it can take days to weeks and must be started early (see §9, §10, §11). Budget for it in the release timeline; do not assume signing is available on day one.
- **SmartScreen reputation:** reputation builds over time as users run our signed binaries. We track SmartScreen reputation as a release-health metric.

### 3.3 macOS notarization

- **Developer ID Application certificate** for signing the `.app` and dmg.
- **Notarization:** submit the dmg to Apple's notary service (`xcrun notarytool submit`), staple the ticket (`xcrun stapler staple`), and verify with `spctl --assess`. Notarization is **mandatory** — without it, Gatekeeper blocks the app.
- **CI integration:** notarization runs in CI as a release-only step (it requires Apple credentials and network). Credentials stored in the CI secret store / keychain, never in the repo.

### 3.4 Linux signing

- No OS-enforced signing, but we **sign** the AppImage and deb with a **GPG key** and publish the public key on Multiplexer.dev. Users (and our own update client) can verify integrity.
- **Reproducible-build verification:** publish the build's source-tree hash so independent parties can verify the binary matches the source (a trust differentiator for a security-sensitive tool).

### 3.5 Signing budget & sequencing

Signing certificates and notarization cost money and require identity verification, so they are a **release-readiness** item, not an MVP-build item. Sequence:

1. **Dev builds:** unsigned, local, for development and CI test gates.
2. **Beta builds:** signed with a **self-signed / test** cert for internal smoke testing of the signing + update pipeline.
3. **Release builds:** production certs + notarization + GPG, gated behind a release-only CI job.

**Budget line item (LOCKED — D29):** Windows code signing via **Azure Trusted Signing** is a recurring cost that must be budgeted:

| Item | Cost | Notes |
|------|------|-------|
| Azure Trusted Signing subscription | ~$10/mo | Fixed monthly fee for the signing account |
| Per-signature fee | ~$0.005–0.01 / signature | Billed per signature; release + update payloads |
| Identity verification (Org ID) | one-time effort | Days–weeks lead time; **schedule risk** (see §9, §11) |
| macOS Developer ID + notarization | Apple Developer Program (~$99/yr) | Separate from Azure; required for Gatekeeper |
| Linux GPG | ~$0 | Self-managed key; no CA cost |

The Azure Trusted Signing line is a **recurring operating cost** (monthly + per-signature), not a one-time purchase — fold it into the release budget and track it as a release-health metric. Because identity verification has real lead time, **start the Azure Trusted Signing Org ID verification early** (it is a critical-path item for the first signed release).

---

## 4. Auto-update

Multiplexer must update itself **safely and automatically**. The reference pattern is `electron-updater` (used by T3 Code and countless Electron apps), but we are native Rust — so we implement the same semantics natively, without Electron.

### 4.1 Update mechanism (native)

- **Design:** a small **updater module** in the Rust server binary. On a schedule (and on launch, with jitter), it:
  1. Fetches the update manifest (`latest.yml`) from the update server.
  2. Compares the remote version to the local version (semver-aware, §6).
  3. If newer, downloads the target-platform artifact **and its signature**.
  4. **Verifies the signature** (Windows Authenticode / macOS notarization / GPG) against the embedded public key / cert chain.
  5. Stages the new binary to a temp location, then swaps it in on **next launch** (LOCKED — D39: native Rust **cannot live-swap** the running binary, so the swap always happens on the next launch, never in-place while running).
  6. Rolls back automatically if the new version fails to start (see §4.4).
- **No Electron dependency:** this is a native implementation (Rust + `reqwest`/`hyper` for download, `ring`/`aws-lc` for signature verification, atomic file rename for swap). We can reuse the `electron-updater` *protocol* (the `latest.yml` format) so our update server and tooling are familiar, but the client is native.
- **Delta updates:** optional and later. MVP ships full-artifact updates (small binary, so full downloads are cheap). Track binary-diff/delta updates as a bandwidth optimization.

### 4.2 Update server

- **Static-hosted manifest + artifacts** (S3 / CloudFront / any object store) — no bespoke server needed for MVP. The client only needs:
  - `latest.yml` (or per-channel manifests) listing version, artifact URLs, SHA-256 hashes, and signatures.
  - The artifacts themselves.
- **Channels:** `stable`, `beta`, `canary` (see §4.3). Each channel has its own manifest. The client is pinned to a channel and only ever sees that channel's manifest.
- **Integrity:** every manifest entry carries the artifact's SHA-256 and a signature over the manifest. The client verifies both. The update server is served over HTTPS with HSTS.

### 4.3 Staged rollouts

- **Canary → Beta → Stable.** A new release first goes to `canary` (internal + opt-in users), then `beta` (wider opt-in), then `stable` (all users).
- **Progressive rollout within stable:** the manifest can carry a **rollout percentage** (e.g. "offer to 10% of clients"). The client uses a stable hash of its install ID to deterministically decide whether it is in the rollout cohort. This lets us catch regressions before 100% exposure.
- **Kill switch:** a flag in the manifest ("hold this version") lets us stop offering a bad version instantly without redeploying artifacts.

### 4.4 Rollback

- **Automatic rollback on failed start:** the updater writes a "pending update" marker; on next launch, if the app fails to start within a timeout (or the user force-quits during startup), the launcher restores the previous known-good binary and reports the failure.
- **Manual rollback:** the About/Settings UI exposes "revert to previous version" for users who hit a regression that still launches.
- **Version pinning:** enterprise users (MSI) can pin a version and disable auto-update via policy.

### 4.5 Update safety properties

| Property | Mechanism |
|----------|-----------|
| Authenticity | Signature verification of manifest + artifact before apply |
| Integrity | SHA-256 hash check on every downloaded artifact |
| Availability | Staged rollouts + kill switch + per-channel manifests |
| Recoverability | Automatic rollback on failed start; manual revert |
| Privacy | Update checks are anonymous (install ID only, no PII); no telemetry by default |

---

## 5. CI/CD

CI is the enforcement point for **TDD at inception** (PLAN-CONTEXT §Testing) and for reproducible packaging. We use **GitHub Actions** (the natural home for a GitHub-hosted Rust project; alternatives like GitLab CI are equivalent).

### 5.1 Pipeline overview

```
push / PR
   │
   ▼
[1] fmt ──► [2] clippy (deny warnings) ──► [3] unit + property
   │
   ▼
[4] mutation (cargo-mutants) ──► [5] integration ──► [6] component (GPUI)
   │
   ▼
[7] e2e ──► [8] coverage gates ──► [9] package (per-OS) ──► [10] release smoke
```

Every gate must be **green before merge** — no blind CI (per the workspace guide). The gate chain is the same one defined in PLAN-CONTEXT §Testing and Plan 15; this doc focuses on how it runs in CI and how it feeds release.

### 5.2 Build matrix

| Job | Runner | Gates | Artifacts |
|-----|--------|-------|-----------|
| **fmt** | ubuntu | `cargo fmt --check` | — |
| **clippy** | ubuntu + windows | `cargo clippy -- -D warnings` | — |
| **unit + property** | ubuntu + windows + macos | unit + proptest | — |
| **mutation** | ubuntu | cargo-mutants (≥85% line, ≥80% branch, ≥70% killed) | — |
| **integration** | ubuntu + windows | real core + mock ACP agent | — |
| **component** | ubuntu | GPUI element/snapshot tests | — |
| **e2e** | windows (primary) + ubuntu | drive real app / headless | — |
| **coverage** | ubuntu | coverage thresholds | coverage report |
| **package** | windows / macos / linux | build + package | installers + manifests |
| **release smoke** | windows / macos / linux | install + launch + sign verify | — |

- **Windows is a first-class CI citizen** (Windows-first). The Windows runner runs the full gate chain, not just a build — this is what enforces the grok-build Windows port (Plan 03 §5) continuously.
- **macOS runner** is needed for notarization and for the macOS-specific gates; it is a paid runner, so macOS jobs are limited to what's necessary (unit/integration + package + smoke).
- **Caching:** cache the Cargo registry, target dir, and the vendored grok-build build artifacts across jobs to keep CI fast. Pin the toolchain via `rust-toolchain.toml` (Plan 03 §6.1).

### 5.3 Release smoke tests

Before any artifact is published, a **release smoke job** runs on each OS:

1. **Install** the produced installer (NSIS silent install / dmg mount / AppImage run) on a clean runner.
2. **Launch** the app and assert it reaches a usable state (server up, UI renders, JSON-RPC responds).
3. **Verify signatures** (Authenticode / notarization / GPG) on the installed binary.
4. **Update check** against the staging update server returns the expected version.
5. **Embedded harness smoke:** one real turn through the embedded grok-build runtime (opt-in, no live credentials — a mock agent suffices; see Plan 03 §9.3).

These smoke tests are the **gate between "built" and "shipped"** — a release is not cut until the smoke job is green on all three OSes.

### 5.4 Release-only vs every-PR

- **Every PR / push:** gates 1–8 (fmt → coverage). Fast, no signing, no packaging.
- **On merge to `main`:** gates 1–8 + **package** (unsigned dev artifacts) so `main` is always installable.
- **On release tag (`v*`):** gates 1–8 + package + **signing/notarization** + **release smoke** + **publish** to channels (§7). Signing credentials are only available to this job.

---

## 6. Versioning

### 6.1 Semantic versioning

- **SemVer 2.0.0.** `MAJOR.MINOR.PATCH` with pre-release tags for channels:
  - `1.4.0` — stable.
  - `1.4.0-beta.1` — beta channel.
  - `1.4.0-canary.20260812` — canary (date-stamped).
- **MAJOR:** breaking changes to the wire contract, storage format, or user-visible behavior that requires migration.
- **MINOR:** new features, backward-compatible.
- **PATCH:** bug fixes, backward-compatible.
- **Wire-contract versioning:** the JSON-RPC wire contract (Plan 04) has its **own** version, independent of the app version, so server and thin clients can negotiate compatibility. The app version and contract version are both recorded in `build-info`.

### 6.2 Changelog

- **`CHANGELOG.md`** maintained in the repo, following **Keep a Changelog** conventions, with sections per release (`Added` / `Changed` / `Deprecated` / `Removed` / `Fixed` / `Security`).
- **Generated from PRs:** each merged PR must carry a conventional-commit prefix (`feat:`, `fix:`, `chore:`, etc.); a release tooling step aggregates them into the changelog draft, which a human reviews and edits before tagging.
- **User-facing:** the changelog is shown in-app (What's New dialog on update) and on Multiplexer.dev.

### 6.3 Version source of truth

- Version lives in a single place (e.g. `Cargo.toml` workspace version + a `version` file), and CI derives the tag, the artifact names, and the update manifest from it. No hand-edited version strings scattered across packaging files.

---

## 7. Distribution channels

| Channel | Purpose | MVP? |
|---------|---------|------|
| **Multiplexer.dev** | Primary landing + direct downloads (per-OS installers) + changelog + docs | Yes |
| **Auto-update server** | In-app updates (manifest + artifacts) | Yes |
| **winget** | Windows package manager install/update | Yes (stretch) |
| **Homebrew (brew)** | macOS install | Later |
| **Microsoft Store** | Windows Store distribution | Later (optional) |
| **App Store** | macOS App Store | Later (optional) |

### 7.1 Website (Multiplexer.dev)

- Primary download page with per-OS installers, checksums, and signature verification instructions.
- Changelog, docs, and a "verify this build" page (source-tree hash + GPG public key).
- **Branding note:** we own both Multiplexer.dev and Multiplexor.dev; which is the product brand vs a redirect is an **open question** (see §9) — this doc assumes Multiplexer.dev as the primary brand but does not decide unilaterally.

### 7.2 winget

- Publish the Windows NSIS/MSI to **winget** (Windows Package Manager). This gives users `winget install Multiplexer` and automatic updates via winget.
- Requires a manifest PR to the `microsoft/winget-pkgs` repo; we automate the manifest generation in the release job.
- **Stretch for MVP** — direct download + auto-update is the primary Windows path; winget is a convenience add-on.

### 7.3 Homebrew

- Publish a **cask** for macOS (`brew install --cask multiplexer`). Requires a PR to `homebrew-cask`. Track as a later item (macOS ships after Windows).

### 7.4 Direct downloads + update server

- Direct downloads are served from the same object store as the update server (or the website CDN). The update server is the **authoritative** source for in-app updates; the website is the source for first-time installs.

### 7.5 Monetization / GTM (LOCKED — D30)

Multiplexer uses a **freemium** model. The distribution channels in this doc support it directly: the free tier is a **free download** (direct download, winget, auto-update), and the paid tier is gated behind an **account** (usage/entitlement tracking, Plan 20 / account infrastructure).

- **Free tier (local, single-provider, core features):** the full local desktop experience with the **in-process Grok adapter** (single provider), native editor, panes, terminal, browser integration, HAR, and local orchestration. This is the primary acquisition path — a free download with no account required for local use.
- **Paid tier (multi-provider / remote / mobile-advanced):** unlocked via an account and subscription. Includes **multi-provider** (Claude, Codex, OpenCode, OpenRouter adapters), **remote/relay** (SSH + relay tunnel, Plan 14), **mobile advanced** (the paired mobile app's advanced features), **usage analytics**, and **priority support**.
- **Channel fit:** the free download channels (§7.1–7.4) carry the free tier; the paid tier is provisioned through the account/entitlement system rather than a separate paid binary. The auto-update pipeline (§4) serves both tiers from the same manifest — entitlement is enforced server-side / via the account, not by shipping different artifacts.
- **GTM note:** the free tier is the wedge (matches the "control surface for your agents" positioning and the Windows-first gap); the paid tier monetizes the multi-provider, remote, and mobile-advanced differentiators. Full GTM detail lives in plan/00, plan/01, plan/19, and plan/20 (D30).

---

## 8. Release process

A release is a **scripted, gated, auditable** procedure — not a manual scramble. The full sequence:

### 8.1 Pre-release (cut)

1. **Freeze `main`** for the release window; only release-blocking fixes merge.
2. **Run the full gate chain** on the release candidate commit (fmt → clippy → unit+property → mutation → integration → component → e2e → coverage). All green.
3. **Draft changelog** from conventional commits; human review.
4. **Bump version** (single source of truth, §6.3); update `CHANGELOG.md`.
5. **Tag** `v<version>` (e.g. `v1.4.0`). Tagging triggers the release CI job.

### 8.2 Build & sign (CI, release job)

6. **Build matrix** produces installers for Windows (NSIS+MSI), macOS (dmg), Linux (AppImage+deb).
7. **Sign** (Windows Authenticode + timestamp, macOS notarize + staple, Linux GPG).
8. **Release smoke tests** on all three OSes (install → launch → verify → update-check → harness smoke). **Gate: all green or the release is blocked.**

### 8.3 Ship (staged)

9. **Canary:** publish to the canary channel; internal + opt-in users update.
10. **Beta:** publish to beta channel after canary is healthy (no crash/rollback spike).
11. **Stable:** publish to stable channel with a **progressive rollout** (start at 10%, ramp to 100% over days as health metrics hold).
12. **Publish** website downloads, winget manifest, changelog, and release notes.

### 8.4 Post-release

13. **Monitor** rollout health (crash rate, rollback rate, update success rate) for the ramp window.
14. **Hotfix path:** a `PATCH` release follows the same pipeline but skips canary/beta if the fix is critical (goes straight to stable with a fast ramp).
15. **Rollback trigger:** if health degrades, use the manifest kill switch to hold the version and/or push the previous version.

### 8.5 Release checklist artifact

The release job emits a **release checklist** (a generated markdown/JSON) recording: commit SHA, toolchain, artifact hashes, signature status, smoke-test results, and rollout plan. This is the audit trail for every release.

---

## 9. Open questions (flag, don't decide)

1. **Branding / domain** (PLAN-CONTEXT #6): which of Multiplexer.dev / Multiplexor.dev is the product brand vs a redirect? This doc assumes **Multiplexer.dev** as primary but does not decide unilaterally.
2. **Windows-first confirmation** (PLAN-CONTEXT #8): this doc assumes Windows-first is confirmed and macOS/Linux follow. Confirm before investing in the macOS notarization + Linux packaging pipeline.
3. **Signing budget (partially LOCKED — D29):** the OV-vs-EV-vs-Azure decision is **resolved** in favor of **Azure Trusted Signing** (see §3.2, §3.5). Remaining to confirm: the specific Azure subscription/tier, the per-signature volume estimate, and **budget approval** for the recurring ~$10/mo + per-signature cost. **Identity-verification lead time is a schedule risk** — start the Org ID verification early (see §11).
4. **MSI in MVP:** NSIS is the MVP installer; is MSI (enterprise) in-scope for MVP or a follow-up?
5. **ARM64 Windows / Linux:** x64-only for MVP; confirm ARM64 is a later target.
6. **Delta updates:** full-artifact updates for MVP; confirm binary-delta updates are a later optimization.
7. **winget / brew / Store:** which package-manager channels are in MVP scope vs later?
8. **Update channel policy:** confirm canary → beta → stable with progressive rollout is the desired model (vs simpler stable-only for MVP).
9. **Telemetry for rollout health:** staged rollouts benefit from anonymous crash/update telemetry; confirm the privacy stance (Plan 17) before enabling any.

---

## 10. Milestones

| Milestone | Deliverable | Exit criteria |
|-----------|-------------|---------------|
| R1 — CI gate chain | GitHub Actions running fmt → coverage on every PR | All gates green on Windows + Linux |
| R2 — Windows packaging | NSIS installer from CI | Installer installs + launches on clean runner |
| R3 — Windows signing | Azure Trusted Signing + signtool + timestamp in release job | Signed installer passes SmartScreen review path |
| R4 — Auto-update MVP | Native updater + static manifest server + staged rollout | Client updates, verifies signature, rolls back on failed start |
| R5 — macOS packaging + notarization | dmg + notarized | Gatekeeper-clean install on Apple Silicon + Intel |
| R6 — Linux packaging | AppImage + deb + GPG signing | AppImage runs on clean runner; deb installs |
| R7 — Release process live | Scripted cut → smoke → staged ship + checklist | First stable release shipped via the full pipeline |
| R8 — Package-manager channels | winget (and brew) manifests automated | `winget install Multiplexer` works |

---

## 11. Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **Signing key compromise** | Low | Critical | HSM/cloud signing; key never on runners; audited signing channel; rotation plan |
| **Update pushed with a regression** | Medium | High | Staged rollouts, kill switch, automatic rollback, release smoke tests |
| **Windows SmartScreen reputation slow to build** | Medium | Medium | Azure Trusted Signing + timestamping; track reputation; consider EV later |
| **Azure Trusted Signing identity-verification lead time** (D29) | Medium | High | Start Org ID verification early (critical path); it can take days–weeks; budget for it in the release timeline; ACP/unsigned dev builds keep development unblocked |
| **macOS notarization friction / cost** | Medium | Medium | Automate notarytool in CI; paid macOS runner; per-arch dmg for MVP |
| **grok-build Windows build breaks packaging** | High (upstream untested) | High | Windows CI gate on every change (Plan 03); ACP fallback; smoke test embedded harness |
| **Reproducibility drift** (builds differ across machines) | Medium | Medium | Pin toolchain + all packaging tools; record build-info; source-tree hash |
| **Update server availability** | Low | Medium | Static object store + CDN; HTTPS/HSTS; client fails safe (no update, no break) |
| **Rollout health without telemetry** | Medium | Medium | Opt-in anonymous telemetry (Plan 17); manual canary/beta cohorts as fallback |

---

## 12. Related plans

- **Plan 02 (Architecture):** single-binary server-centric runtime — the reason distribution is simple.
- **Plan 03 (Vendored grok-build):** Windows build support, toolchain pinning, protoc/DotSlash — the embedded build this doc packages.
- **Plan 15 (Testing strategy):** the gate chain this doc runs in CI.
- **Plan 17 (Security & secrets):** signing key custody, update integrity, telemetry privacy.
- **Plan 20 (Risks & open questions):** cross-cutting open questions referenced here.
