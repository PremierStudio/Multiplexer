# Plan 03 — Vendoring & Embedding the grok-build Harness

**Status:** Planning · **Owner:** subagent fan-out → adversarial review
**Consistency:** This doc follows `docs/PLAN-CONTEXT.md` exactly. Where the shared context leaves a decision open (e.g. vendoring strategy), this doc makes a **recommendation** and flags it as an open question rather than silently diverging.

---

## 1. Purpose & scope

Multiplexer's core differentiator is **in-process embedding of the grok-build harness**: we call the agent runtime directly from our Rust binary — no shelling out to a CLI, no ACP protocol hop. This doc is the implementation plan for how we **fork, vendor, and embed** `github.com/xai-org/grok-build` (Apache 2.0, Rust), and — critically — how we make it **build and run on Windows**, which upstream explicitly does not support.

Scope covered here:

1. Source of truth and crate layout.
2. Vendoring strategy decision (submodule vs vendored copy vs `[patch]`).
3. Which crates we embed as libraries vs which we replace.
4. Windows build support (our responsibility — the key de-risking task).
5. Build toolchain (rust-toolchain pinning, DotSlash/protoc).
6. Keeping the fork in sync with upstream.
7. The ACP fallback path.
8. Testing the embedding.
9. Risks and mitigations.

---

## 2. Source of truth & crate layout

**Repo:** `github.com/xai-org/grok-build` — Apache 2.0, Rust. We fork it (upstream does **not** accept external contributions), so our fork is the durable source of truth for the embedded harness.

### 2.1 Crate map

| Crate | Role | Our disposition |
|-------|------|-----------------|
| `xai-grok-pager-bin` | Composition root → `xai-grok-pager` binary | **Replace** (we supply our own composition root) |
| `xai-grok-pager` | TUI (terminal UI) | **Replace** with our GPUI UI |
| `xai-grok-shell` | Agent runtime: leader / stdio / headless, session lifecycle, subagent orchestration | **Embed as library** |
| `xai-grok-tools` | Tool implementations (fs, git, terminal, search, session, auth) | **Embed as library** |
| `xai-grok-workspace` | Filesystem / VCS / execution / checkpoints | **Embed as library** |
| `xai-grok-config` | `[model.*]` / `[auth_provider.*]` config parsing | **Embed as library** |
| `xai-grok-mcp` | MCP client/server glue | **Embed as library** |
| `xai-grok-markdown` | Markdown parsing/rendering helpers | **Embed as library** |
| `xai-grok-sandbox` | Sandboxing / execution isolation | **Embed as library** (evaluate on Windows) |

### 2.2 Root `Cargo.toml` is generated & read-only

The repo's root `Cargo.toml` is **generated** (a workspace manifest produced by a build script / tooling) and must be treated as **read-only**. We do **not** hand-edit it. All of our changes go into **per-crate `Cargo.toml`** files. This is important for two reasons:

- It keeps our fork's diff surface small and reviewable (we only touch the crates we actually modify).
- It survives upstream rebases: per-crate edits are far less likely to conflict with upstream's regenerated root manifest than edits to the root itself.

**Rule:** never commit a hand-edit to the generated root `Cargo.toml`. If we need a workspace-level change, we either (a) patch the generator, or (b) express it at the per-crate level, or (c) apply it in our own workspace's `[patch]` section (see §3).

---

## 3. Vendoring strategy decision

### 3.1 Options compared

| Option | How it works | Pros | Cons |
|--------|--------------|------|------|
| **A. Git submodule** | `third_party/grok-build` is a submodule pinned to a commit | Upstream sync is a `git submodule update`; small repo footprint; upstream history preserved | Submodule + our fork is awkward (submodule points at *our* fork, not upstream); Windows checkout friction; build tooling must recurse; harder to patch crates in-place; CI needs `--recurse-submodules` |
| **B. Vendored copy under `third_party/`** | Full fork tree committed into our repo | Fully self-contained; we own every byte; no network needed at build; easy to patch; deterministic builds; CI simple | Repo size grows; upstream sync is a manual merge/rebase; must track `SOURCE_REV` |
| **C. `[patch]` dependency** | Keep upstream as a git dependency; override selected crates via `[patch]` to our fork | Minimal vendored surface; only patched crates diverge | `[patch]` only works for *path/git* overrides of the same crate name — we'd still need the fork checked out somewhere; fragile if we replace whole crates; less self-contained; harder to reason about the full tree |

### 3.2 Recommendation: **B (vendored fork under `third_party/`) + C (`[patch]` wiring)**

We recommend a **hybrid**: a **vendored fork committed under `third_party/grok-build/`** (option B) as the durable source of truth, **wired into our workspace via `[patch]`** (option C) so our crates depend on the vendored crates by name while Cargo resolves them to our local fork.

Rationale:

- **Self-contained & deterministic.** Multiplexer builds from a single checkout with no network dependency on GitHub at build time. This matters for CI, offline builds, and reproducible releases.
- **We own the fork.** Upstream rejects external contributions, so our fork *is* the product's harness. Committing it makes our divergence explicit, reviewable, and auditable.
- **`[patch]` gives clean wiring.** Our own crates declare `xai-grok-shell = { path = "..." }` or we use `[patch.crates-io]` / `[patch."https://github.com/xai-org/grok-build"]` to redirect the crate names to `third_party/grok-build/crates/*`. This keeps our dependency declarations readable while resolving to the vendored code.
- **Submodule rejected** because it adds Windows checkout friction, complicates our fork (a submodule pointing at our own fork is redundant), and makes patching in-place awkward. We already need a fork; committing it is simpler than nesting a submodule.

**Open question (flag, don't decide):** PLAN-CONTEXT open question #5 lists "submodule vs vendored copy vs `[patch]`" with the note "recommend vendored fork under `third_party/` + `[patch]`". This doc recommends exactly that. The user should confirm the hybrid before we commit the fork tree.

### 3.3 Fork mechanics

- Create `third_party/grok-build/` as a **normal directory** (not a submodule) containing our fork.
- Keep the fork's own `.git` history **or** flatten it? Recommendation: keep a shallow upstream history imported once, then commit our changes on top. This preserves blame and makes rebases tractable. (If repo size becomes a concern, we can re-import shallowly at each sync — see §6.)
- Add a `SOURCE_REV` file (see §6) recording the exact upstream commit our fork is based on.

---

## 4. Which crates we embed vs replace

### 4.1 Embed as libraries (reuse)

- **`xai-grok-shell`** — the agent runtime. This is the heart of the embedding: leader loop, session lifecycle, subagent orchestration (`spawn_subagent`, Rhai workflows), stdio/headless drivers. We call into it directly.
- **`xai-grok-tools`** — tool implementations. Reuse the fs/git/terminal/search/session/auth tools; we surface them through our own UI and provider-adapter layer.
- **`xai-grok-workspace`** — fs/VCS/execution/checkpoints. Reuse for worktree management and checkpointing (see Plan 07).
- **`xai-grok-config`** — config parsing for `[model.*]` / `[auth_provider.*]`. Reuse so our model registry (Plan 05) speaks the same config dialect.
- **`xai-grok-mcp`**, **`xai-grok-markdown`**, **`xai-grok-sandbox`** — reuse where they fit; sandbox is evaluated on Windows (see §5).

### 4.2 Replace

- **`xai-grok-pager`** (TUI) — replaced by our GPUI UI. We do **not** link the TUI crate.
- **`xai-grok-pager-bin`** (composition root) — replaced by our own composition root in the Multiplexer binary. We own process startup, the GPUI event loop, and the JSON-RPC server.

### 4.3 Embedding boundary

The embedding boundary is the **`xai-grok-shell` public API** (session start, turn send, interrupt, approval, user-input, checkpoint revert, stop — mirroring the Provider Adapter contract in Plan 05). We treat the shell's public surface as a **stable seam**: our provider-adapter layer (Plan 05) wraps it, so if upstream's API shifts, only the adapter changes, not the rest of Multiplexer.

---

## 5. Windows build support (our responsibility — critical de-risking)

Upstream states Windows builds are **"best-effort, not currently tested from this tree."** Making the crates build and run on Windows is **our** job and is a **first-class, early** workstream — not a post-MVP afterthought. Windows-first is a core differentiator (Superset and Conductor are macOS-only), so this is a strategic de-risking task.

### 5.1 Likely challenge areas

| Area | Likely issues on Windows | Mitigation plan |
|------|--------------------------|-----------------|
| **Platform-specific code** | `#[cfg(unix)]` blocks, Unix-only syscalls (signals, `fork`, `exec`, `poll`, `termios`, `unistd`), Unix socket paths | Audit every crate for `cfg(unix)`; add `#[cfg(windows)]` equivalents; gate Unix-only modules behind `cfg`; use `std::os::windows` / `windows-sys` where needed |
| **Terminal / TTY** | TUI code assumes a Unix PTY; `termios`/`ioctl` for raw mode; ANSI handling differs | We replace the TUI anyway; ensure the *shell* crate's stdio/headless drivers don't require a PTY on Windows; use `windows-sys` console APIs or a cross-platform PTY crate (e.g. `portable-pty`) for any terminal we do spawn |
| **Process spawning** | `Command` differences, job objects, `CREATE_NEW_PROCESS_GROUP`, signal emulation, exit-code/`kill` semantics | Abstract process control behind a small trait; use `windows-sys` Job Objects for process-tree cleanup; map Unix signals to Windows equivalents (e.g. Ctrl-Break / TerminateProcess) |
| **Paths** | `\` vs `/`, drive letters, UNC paths, case-insensitivity, reserved names (`CON`, `NUL`), long-path (>260) opt-in | Use `std::path`/`PathBuf` everywhere (never string concat); enable long paths via manifest + registry guidance; normalize separators at boundaries; test with spaces/Unicode in paths |
| **Filesystem / VCS** | Git on Windows (line endings, symlinks, file locking), `xai-grok-workspace` assumptions | Rely on Git-for-Windows; set `core.autocrlf` policy; handle symlink/`core.symlinks` differences; test checkpoints on NTFS |
| **Sandbox** | `xai-grok-sandbox` may rely on Unix isolation (namespaces, seccomp, `chroot`) | Evaluate; if it's Unix-only, gate it behind `cfg(unix)` and provide a no-op/limited Windows sandbox or skip it in MVP (flag in Plan 20) |
| **Build tooling** | DotSlash / protoc availability, MSVC toolchain, `cc`/`cmake` for native deps | See §5.2 and §6 toolchain plan |

### 5.2 Windows bring-up plan (phased)

1. **Phase W1 — Compile audit.** Clone fork; attempt `cargo build` on Windows for each crate in isolation. Catalog every `cfg(unix)` and every Unix-only dependency. Produce a per-crate "Windows readiness" matrix.
2. **Phase W2 — Port the shell core.** Make `xai-grok-shell` (stdio/headless drivers) compile and run on Windows first — it has no TUI dependency, so it's the cleanest target. This unblocks the embedding.
3. **Phase W3 — Port tools & workspace.** Bring `xai-grok-tools` and `xai-grok-workspace` up; abstract process/fs/VCS differences behind traits.
4. **Phase W4 — Config/MCP/markdown/sandbox.** Port the remaining crates; decide sandbox disposition.
5. **Phase W5 — CI.** Add a Windows runner to CI that builds all embedded crates and runs the embedding tests (§8). **Windows must be green in CI before we call the embedding done.**

**Key principle:** we do **not** fork-and-forget. Windows support is maintained continuously in our fork, with CI enforcing it on every change.

---

## 6. Build toolchain

### 6.1 `rust-toolchain.toml` pinning

- Add a `rust-toolchain.toml` at the Multiplexer workspace root pinning the **exact toolchain** (channel + components + targets) that upstream grok-build is known to build with, plus the MSVC host target for Windows.
- Components: `rustfmt`, `clippy`, `rust-src` (for `cargo-mutants`/`cargo-expand`), and the `x86_64-pc-windows-msvc` target.
- Rationale: deterministic builds across dev machines and CI; avoids "works on my machine" drift; matches the TDD-at-inception gates (fmt → clippy → tests).

### 6.2 DotSlash / protoc requirements

The grok-build README lists **DotSlash** and **protoc** as build prerequisites:

- **protoc** (Protocol Buffers compiler) — needed to generate Rust bindings from `.proto` files used by the harness (MCP / IPC / wire types). On Windows: install via `choco install protoc` or download the official `protoc-<ver>-win64.zip`; ensure `protoc` is on `PATH` for builds. We pin the protoc version in CI.
- **DotSlash** — a launcher used by the repo's tooling to run pinned tool versions. On Windows this is a `.cmd`/`.exe` shim. We either install DotSlash or bypass it by invoking the underlying tools directly; the goal is that **our build does not hard-depend on DotSlash** — it's an upstream convenience, not a hard requirement for the crates themselves.

### 6.3 Wiring into our workspace

- Our root workspace `Cargo.toml` lists our own crates **plus** the vendored crates via `[patch]` (or direct `path` deps).
- A `build.rs` or a small bootstrap script verifies toolchain + protoc presence and prints a clear error if missing (fail fast, not a cryptic linker error).
- CI installs the pinned toolchain, protoc, and (on Windows) the MSVC build tools, then runs the full gate chain.

---

## 7. Keeping the fork in sync with upstream

### 7.1 `SOURCE_REV`

- Commit a `third_party/grok-build/SOURCE_REV` file containing the **exact upstream commit SHA** our fork is based on, plus the date and a short changelog of our local deltas.
- This is the single source of truth for "how far are we behind upstream?".

### 7.2 Sync cadence & strategy

- **Periodic rebase/merge** (recommend: on a schedule, e.g. every 2–4 weeks, and before any major feature that touches the shell). We **merge upstream into our fork** (not rebase-and-force-push) so our fork history is stable and our own commits are preserved.
- **Sync procedure:**
  1. `git fetch upstream` in the fork.
  2. Create a `sync/upstream-<sha>` branch; merge `upstream/main`.
  3. Resolve conflicts — expected in the crates we patched (shell, tools, workspace) and in generated files (root `Cargo.toml` — regenerate, don't hand-merge).
  4. Run the full Windows build + embedding test suite (§8) on the merge.
  5. Update `SOURCE_REV`; commit.
- **Conflict policy:** because we keep our edits per-crate and minimal, conflicts should be localized. If a conflict is large, we re-review the upstream change for API drift (see §9) before merging.
- **When to NOT sync:** if upstream introduces a breaking API change that would ripple through our adapter layer, we may pin to the last-known-good `SOURCE_REV` and schedule the migration deliberately rather than absorbing it mid-sprint.

---

## 8. The ACP fallback path

Even though we embed in-process, we **keep** the ACP path (`grok agent stdio` / `serve` / `headless`) as a fallback. This is a deliberate redundancy, not a contradiction of the embedding differentiator.

### 8.1 When each path is used

| Path | When used | Notes |
|------|-----------|-------|
| **In-process embedding** | **Default** for the desktop app (local, interactive, performance-critical) | The differentiator; lowest latency; no protocol hop |
| **`grok agent stdio` (ACP)** | Remote/headless clients, mobile companion, CI, and as a **fallback** if embedding is broken on a given platform | Drives the *installed* `grok` binary; matches T3 Code's approach; useful for the mobile app which can't embed a full harness |
| **`grok agent serve`** | Long-lived remote sessions / relay | Exposes the harness over a socket for the relay/SSH path (Plan 14) |
| **`grok agent headless`** | Scripted / non-interactive batch runs | For automation and tests |

### 8.2 Why keep it

- **Resilience:** if a Windows build regression breaks the embedded shell, the app can degrade to ACP rather than being fully down.
- **Remote/mobile:** thin clients (mobile companion, web) cannot reasonably embed the harness; ACP/serve is the natural transport.
- **Testing:** our integration tests use a **mock ACP agent** (fake `grok agent stdio`) per PLAN-CONTEXT §Testing — the ACP path is the test seam.
- **Provider breadth:** ACP lets us drive other harnesses (Claude, Codex, OpenCode) via the same contract, aligning with multi-harness extensibility.

**Design rule:** the Provider Adapter layer (Plan 05) must be able to back onto **either** the embedded runtime **or** ACP behind the same trait. The embedding is the fast path; ACP is the portable path. Both must pass the same contract tests.

---

## 9. Testing the embedding

TDD at inception is non-negotiable (PLAN-CONTEXT §Testing). The embedding gets its own test layers:

### 9.1 Unit tests (in-crate)

- Co-located `#[cfg(test)]` in our adapter and in the vendored crates we touch.
- **Property-based (proptest):** test the session state machine and the adapter's command serialization (start → turn → interrupt → approval → revert → stop) for all valid orderings; test that the adapter never deadlocks or double-sends.
- **Mutation (cargo-mutants):** the adapter and the embedding seam must hit the CI gates (≥85% line, ≥80% branch, ≥70% mutation killed).

### 9.2 Integration tests — mock agent

- Spin up the **real embedded runtime** with a **mock agent** (a fake model backend that returns scripted tool calls and turns). Assert on the resulting read model / event stream.
- This proves the embedding actually drives a session end-to-end without needing a live model or network.
- Also run the same suite against the **mock ACP agent** (fake `grok agent stdio`) to prove the adapter is path-agnostic.

### 9.3 Real-binary smoke tests

- When a real `grok` binary / model is available, run a small smoke test (one turn, one tool call) through the embedded path and the ACP path. Marked `#[ignore]` / opt-in so CI doesn't require live credentials.

### 9.4 Windows CI gate

- A dedicated Windows CI job builds all embedded crates and runs the unit + integration suites. **The embedding is not "done" until this is green.**

### 9.5 Contract tests

- The JSON-RPC wire contract (Plan 04) is schema-verified on both the embedded and ACP paths, ensuring the fallback is a true drop-in.

---

## 10. Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **Upstream API changes** break our adapter | Medium | High | Stable adapter seam (Plan 05); pin `SOURCE_REV`; deliberate sync cadence; contract tests catch drift |
| **Windows build breakage** in vendored crates | High (upstream untested) | High | Early, phased Windows bring-up (§5); Windows CI gate; keep ACP fallback so we're never fully down |
| **Fork drift / merge conflicts** on sync | Medium | Medium | Per-crate minimal edits; regenerate root manifest; `SOURCE_REV` tracking; scheduled merges |
| **Licensing obligations (Apache 2.0)** | Low | High | Preserve upstream copyright/license headers; ship `THIRD-PARTY-NOTICES`; comply with Apache 2.0 notice requirements; document our fork's provenance |
| **Sandbox crate Unix-only** | Medium | Medium | Gate behind `cfg(unix)`; no-op/limited Windows sandbox or defer (flag in Plan 20) |
| **Repo size growth** from vendored fork | Medium | Low | Shallow import at sync; exclude upstream `.git`; consider sparse/partial history |
| **protoc/DotSlash tooling friction on Windows** | Medium | Low | Pin protoc; make DotSlash optional; fail-fast bootstrap checks |
| **Embedding blocks the main event loop** (perf) | Medium | High | Run the shell on a dedicated thread/async runtime; keep the GPUI loop responsive (Plan 16 perf targets: <16ms input latency) |

### 10.1 Licensing (Apache 2.0) obligations

- grok-build is Apache 2.0. We must:
  - Retain the license and copyright notices in the vendored source.
  - Include a `THIRD-PARTY-NOTICES` file in our distribution listing grok-build and its license.
  - Not misrepresent provenance; document that our fork is derived from `xai-org/grok-build`.
- This is a legal/compliance item to confirm with the user (flag in Plan 20), not something we decide unilaterally.

---

## 11. Milestones

| Milestone | Deliverable | Exit criteria |
|-----------|-------------|---------------|
| M1 — Fork & vendor | `third_party/grok-build/` committed; `SOURCE_REV`; `[patch]` wiring builds on Linux | Workspace compiles with vendored crates |
| M2 — Windows compile audit | Per-crate Windows readiness matrix | All crates catalogued; blockers identified |
| M3 — Shell core on Windows | `xai-grok-shell` builds & runs on Windows | Windows CI job green for shell |
| M4 — Tools/workspace on Windows | `xai-grok-tools` + `xai-grok-workspace` ported | Windows CI green |
| M5 — Adapter seam | Provider Adapter wraps embedded runtime + ACP | Both paths pass contract tests |
| M6 — Embedding test suite | Unit + property + mutation + integration (mock agent) | CI gates met; Windows green |
| M7 — Sync cadence live | Scheduled upstream merges; `SOURCE_REV` updated | First sync merge clean |

---

## 12. Open questions (flag, don't decide)

1. **Vendoring strategy** (PLAN-CONTEXT #5): confirm the recommended **vendored fork under `third_party/` + `[patch]`** hybrid.
2. **Sandbox disposition on Windows:** gate out, no-op, or limited port in MVP?
3. **Fork history:** keep full upstream history vs shallow import (repo-size tradeoff).
4. **Sync cadence:** 2-week vs 4-week upstream merge schedule.
5. **Licensing/notices:** confirm `THIRD-PARTY-NOTICES` approach and any legal review needed.
6. **ACP fallback scope:** is ACP required for MVP (mobile/remote) or is embedding-only acceptable initially?
