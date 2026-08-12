# Phase-0 Spike Report: In-Process grok-build Embedding on Windows

**Date:** 2026-08-12
**Status:** **GO** — the embedding differentiator is viable on Windows.
**Author:** Phase-0 spike (subagent fan-out contraction)

## Verdict

The single unverified question that gated the entire business case — *"can
`xai-grok-shell` be consumed as a library, and does it build on Windows?"* —
is **answered YES**. The Phase-0 go/no-go gate (D10) passes.

## What was proven (evidence)

### 1. The library API exists (contradicts the earlier "no library API" claim)

`xai-grok-shell` (v1.0.1) is a **first-class library crate** with a broad
public module surface in `crates/codegen/xai-grok-shell/src/lib.rs`:

- `pub mod session` (Session, PromptOrigin, fork, persistence)
- `pub mod agent` (Config, app::run_headless / run_leader / run_stdio_agent)
- `pub mod leader` (LeaderClient, connect_or_spawn, run_leader_server,
  `in_process::spawn_agent`)
- `pub mod config`, `pub mod models`, `pub mod auth`, `pub mod tools`,
  `pub mod workflow` (via re-exports)

The README's "headless + ACP only, no in-process library API" is a
**documentation gap, not a structural one**. The crate is built as a `[lib]`
and exposes these modules publicly.

### 2. It compiles on Windows (after one small fork patch)

`cargo +1.94.0 check -p xai-grok-shell --lib` **succeeds** on Windows. The
only blocker was a single, well-contained build-script bug:

- **Bug:** `crates/build/xai-proto-build/src/lib.rs` hardcoded Unix device
  paths: `protoc --dependency_out=/dev/stdout --descriptor_set_out=/dev/null`.
  On Windows, protoc rejects `/dev/stdout` with "No such file or directory".
- **Fix (fork patch):** use a temp file for `--dependency_out` and `NUL` for
  `--descriptor_set_out` on Windows; parse the dependency target from the
  temp file. Unix path unchanged. This is the kind of small, owned patch the
  vendored-fork strategy (D5) exists to make.

### 3. It is consumable as a library from an independent binary

A standalone spike crate (`spike/`) depends on the vendored crates via path
dependencies + the upstream `[patch.crates-io]`, and **builds and runs** on
Windows. Output:

```
xai-grok-version::VERSION      = 1.0.1
shell public API surface resolves: config, agent::config, session::PromptOrigin
load_effective_config() -> OK (toml::Value)
  [model.*] entries: glm52, glm52-or, ds-flash
  [auth_provider.*] entries: openrouter
grok home                      = C:\Users\gollum\.grok
SPIKE-OK: xai-grok-shell is consumable as a library on Windows
```

This proves the D5 `[patch]`-style wiring works on Windows **and** that the
runtime reads the real user config — including the `ds-flash` model and
`openrouter` auth provider (D14), exactly the multi-model mechanism the plan
depends on.

## Windows blockers found and resolved

| # | Severity | Blocker | Resolution |
|---|----------|---------|------------|
| 1 | High | `xai-proto-build` hardcodes `/dev/stdout` + `/dev/null` for protoc | Fork patch: temp file + `NUL` on Windows |
| 2 | Medium | protoc not on PATH; upstream `bin/protoc` is macOS/Linux-only DotSlash | Install protoc 29.3; set `PROTOC` |
| 3 | Medium | External crate needs the upstream `[patch.crates-io]` (async-openai fork) to resolve `ReasoningEffort::Max` | Replicate the `[patch]` in the consuming workspace |
| 4 | Low | External crate needs the upstream `Cargo.lock` to pin compatible dep versions for rustc 1.94.0 | Vendor the lock alongside |

No blockers were structural. All were build-configuration or platform-path
issues, each small and mechanical.

## What this means for the plan

- **D10 (embedding hypothesis):** upgraded from "hypothesis to prove" to
  **"proven viable"**. The ACP fallback remains as resilience/redundancy, not
  as the primary path.
- **D35 (Windows-first):** **de-risked**. The Windows build works; the ACP
  contingency is no longer needed for the embedding to ship on Windows.
- **D5 (vendored fork + `[patch]`):** validated in practice. The consuming
  workspace must replicate the upstream `[patch.crates-io]` and vendor the
  `Cargo.lock`.
- **D13 (crate layout):** the `multiplexer-*` crates can depend directly on
  the vendored `xai-grok-*` crates by path.

## Remaining work (not part of this spike)

- Running a real authenticated headless turn in-process requires a live
  grok.com session / API key; the spike proved the API surface, config
  loading, and build, not a live model round-trip. That is Phase 1 work.
- The `in_process::spawn_agent` path is `#[cfg(feature = "test-support")]`;
  the production in-process path is `run_leader` / `run_headless` /
  `run_stdio_agent`, which the composition root (`xai-grok-pager-bin`) shows
  how to drive. Multiplexer replaces that composition root with our own.