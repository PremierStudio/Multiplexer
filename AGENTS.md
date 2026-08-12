# Multiplexer — Workspace Guide

**Multiplexer** is a beautiful, blazing-fast desktop client / control surface for the **Grok Build harness**, extensible to other models and harnesses. We own Multiplexer.dev and Multiplexor.dev.

## Read first

- `docs/PLAN-CONTEXT.md` — the authoritative shared plan context (architecture, decisions, competitors, testing strategy). All work must be consistent with it.
- `plan/00-x.md` … `plan/20-x.md` — the detailed implementation plan docs (authored by subagent fan-out).

## Status

- **Phase:** Planning. The workspace is greenfield (empty except `docs/` and `plan/`).
- The plan docs in `plan/` are being authored by a fan-out of subagents, then adversarially reviewed.

## Key decisions (approved)

- **Stack:** Rust core + GPUI (GPU-rendered) UI. NOT Electron.
- **Server-centric runtime:** single native Rust binary owns agent processes/terminals/git/fs/checkpoints/HAR; thin clients (desktop/mobile/web) over one authenticated JSON-RPC-over-WebSocket contract.
- **Embedded harness:** fork/vendor `xai-org/grok-build` (Apache 2.0) and embed its crates in-process.
- **Windows-first** shipping.
- **TDD at inception:** unit + property + mutation (cargo-mutants) + component + integration + e2e, with CI coverage gates.

## Conventions

- Rust workspace. Follow the grok-build crate conventions where we embed it.
- All changes must pass: fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e → coverage. No blind CI.
- Do not commit secrets. Use OS keychain for local secrets; `op://Vault/Item/field` references only in configs (never raw values).
