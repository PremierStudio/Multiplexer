# 09 — Native Editor

**Status:** Draft (consistent with `docs/PLAN-CONTEXT.md`)
**Owner:** Multiplexer planning fan-out
**Scope:** Why a native editor, editor core, LSP integration, inline diff-apply, diff comments → agent, editor ↔ agent integration, performance, scope phasing, testing, open questions.

> **Locked decisions applied:** D21 (mutation scope — editor core is mutation-gated), D33 (70% is the merge floor), C1 (CI-gate section), M3 (LSP skip-not-fail). These are LOCKED per `docs/DECISIONS.md` and supersede any conflicting "open question" wording below.

---

## 1. Why a Native Editor

The editor is **differentiator #2** and the single most visible reason a developer chooses Multiplexer over every incumbent.

### The competitive gap

| Product | Built-in editor | Reality |
|---|---|---|
| **T3 Code** | ❌ | Launches external editors; no inline editing |
| **Codex Desktop** | ❌ | Chat + terminal only; no editor |
| **Superset** | ❌ | Diff review, no editing |
| **Conductor** | ❌ | macOS-only, no editor |
| **Orca** | ⚠️ | File *viewer*, not a full native editor |
| **OpenCode** | ⚠️ | Terminal/IDE hybrid; it's an *agent*, not a control surface |
| **Multiplexer** | ✅ | **Full native editor** in Rust + GPUI |

Nobody ships a real, GPU-rendered, LSP-backed editor inside an agent control surface. Multiplexer does. This is not a nice-to-have — it is a core reason to exist.

### Why it matters for the agent workflow

1. **Inline diff-apply.** The agent edits files; the user reviews and applies those edits *in place* — accept/reject hunks without leaving the surface. This is the highest-value interaction in the product.
2. **Diff comments → agent.** The user drops a comment on a diff line and routes it back to the running agent (Orca baseline bar). Requires a real editor with real line/diff geometry.
3. **Context feeding.** The user's cursor, selection, and open file are the natural "current context" for the agent. A viewer cannot express this precisely.
4. **Trust.** A developer will not let an agent rewrite their codebase from a black box. Seeing the edits land in a real editor, with LSP diagnostics and undo, is how trust is earned.
5. **Performance as a feature.** GPU-rendered text, <16ms input latency, sub-300ms cold start — the "blazing fast" promise is most tangible in the editor.

### Design posture

The editor is **not** a re-implementation of VS Code. It is a *focused, fast, keyboard-first* editor optimized for the agent loop: review diffs, apply/reject, comment, jump to definitions, rename, format. It must be excellent at those, not a kitchen sink.

---

## 2. Editor Core

### 2.1 Text buffer — rope-based

The buffer is a **rope** (persistent, O(log n) insert/delete/slice), not a flat `String` or `Vec<u8>`. This is what makes large-file handling and cheap undo feasible.

```rust
// crates/editor/src/buffer.rs
pub struct Buffer {
    rope: Rope,                       // persistent rope (e.g. xi-rope / ropey-style)
    revision: u64,                    // bumped on every edit; drives undo + LSP sync
    path: Option<PathBuf>,
    language: Option<LanguageId>,
    line_cache: LineCache,            // line-start offsets, lazily invalidated
    undo_stack: Vec<EditGroup>,
    redo_stack: Vec<EditGroup>,
}

pub struct EditGroup {
    edits: Vec<Edit>,                 // one logical user action = one group
    revision_before: u64,
    revision_after: u64,
}

pub struct Edit {
    range: Range<Point>,              // (line, column) — column in UTF-8 bytes or chars
    new_text: Rc<str>,
}
```

- **Coordinates:** `Point { line: u32, column: u32 }` with column measured in **UTF-8 bytes** internally, converted to display columns via a tab/width table. Keeps rope slicing O(log n) and avoids O(n) char scans.
- **Line cache:** lazily maintained array of line-start offsets; invalidated on edit, rebuilt on demand. Enables O(1) line → offset and O(log n) offset → line.
- **Encoding:** UTF-8 in memory; detect BOM/UTF-16 on load, transcode on save. CRLF preserved per-file.

### 2.2 Cursor, selections, multi-cursor

- **Cursor:** a set of selections, each `{ anchor: Point, head: Point }`. A single cursor is one selection with `anchor == head`.
- **Multi-cursor:** an ordered `Vec<Selection>`; every editing command applies to all selections simultaneously. Add cursor with `Ctrl+Click` / `Alt+Click` (Windows), `Cmd+D` next-occurrence, `Alt+Shift+↑/↓` add line.
- **Edit semantics:** edits are applied in reverse order (bottom selection first) so earlier positions stay valid; each selection's edit is computed against the pre-edit buffer, then applied as one `EditGroup` → one undo step.

```rust
pub struct SelectionSet {
    selections: Vec<Selection>,
    primary: usize,                   // the "main" cursor (where the caret blinks)
}
```

### 2.3 Undo / redo

- **Grouping:** coalesce consecutive edits within a time window (e.g. 300ms) and same "edit kind" into one `EditGroup` — so typing a word is one undo, not one per keystroke.
- **Granularity:** undo restores the exact `revision_before` state by replaying the inverse of the group's edits against the rope. Because the rope is persistent, undo is a pointer swap, not a copy.
- **Redo:** symmetric; cleared when a new edit is made after an undo.
- **Checkpoint tie-in:** the VCS/checkpoint subsystem (see `plan/07`) snapshots at *turn* boundaries, not keystroke boundaries; the editor's undo is the fine-grained layer beneath it.

### 2.4 Syntax highlighting

- **Tree-sitter** for tokenization (fast, incremental, robust to partial edits). Grammar set is a curated, versioned list (Rust, TypeScript/JS, Python, Go, JSON, YAML, TOML, Markdown, SQL, shell, HTML/CSS) — the languages a coding agent actually touches.
- **Incremental:** on each edit, re-parse only the affected subtree; the syntax tree is persistent and shares unchanged nodes.
- **Themes:** a small set of hand-tuned GPUI themes (light/dark/high-contrast) with a token-color mapping table. No theme marketplace in MVP.

### 2.5 Line numbers, minimap, folding

- **Line numbers:** rendered in a gutter; click to select line, drag to select range, click on fold marker to fold.
- **Minimap:** a right-edge, GPU-rendered overview of the whole file (code + diff markers + diagnostic markers). Toggleable; hidden on small panes.
- **Folding:** fold ranges derived from the tree-sitter tree (function/block boundaries) plus manual fold markers. Fold state is per-file, persisted in a sidecar or the read model.

### 2.6 Vim mode

- A **modal layer** on top of the core: Normal / Insert / Visual / Command-line modes.
- Motions (`w`, `b`, `e`, `f/t`, `%`, `gg`, `G`, `{`, `}`), operators (`d`, `c`, `y`, `>`, `<`), text objects (`iw`, `aw`, `i(`, `a"`), registers, macros, marks, search (`/`, `?`, `n`, `N`), and `:` commands (write, quit, substitute).
- **Multi-cursor + Vim:** operators apply to all selections; visual-block mode maps to a rectangular selection set.
- **Scope:** a faithful, fast subset — not a full Neovim clone. The command palette and mouse still work; Vim is an input mode, not a separate program.
- **Config:** a `vim` section in the app config (leader key, relative line numbers, etc.), not a `.vimrc` interpreter in MVP.

---

## 3. LSP Integration

### 3.1 Client architecture

A single **LSP client** per language server, owned by the server runtime (not the UI), so the same client serves desktop and mobile. The client is a Rust crate (`crates/lsp/`) implementing the JSON-RPC LSP protocol over stdio.

```rust
// crates/lsp/src/client.rs
pub struct LspClient {
    server: ChildProcess,             // stdio transport
    capabilities: ServerCapabilities,
    documents: HashMap<PathBuf, DocumentState>,  // open buffers + versions
    pending: HashMap<RequestId, PendingRequest>,
    diagnostics: HashMap<PathBuf, Vec<Diagnostic>>,
}
```

- **Document sync:** `textDocument/didOpen`, `didChange` (incremental, per edit batch), `didClose`. Each buffer carries a `version`; the client sends `version` with every change so the server's state stays in lockstep with the editor's `revision`.
- **Requests:** `textDocument/definition`, `hover`, `completion`, `rename`, `formatting`, `documentSymbol`, `references`, `codeAction`.
- **Notifications:** `textDocument/publishDiagnostics` → routed to the editor's diagnostic overlay and the minimap.
- **Cancellation:** out-of-date requests are cancelled (`$/cancelRequest`) when the user moves on; results are tagged with the buffer `revision` they were computed against and dropped if stale.

### 3.2 Server discovery & launch

- **Per-language resolver:** a table mapping `LanguageId → { command, args, env }`. Defaults for the common toolchain:
  - Rust → `rust-analyzer`
  - TypeScript/JS → `typescript-language-server` (or `vtsls`)
  - Python → `pyright-langserver` / `basedpyright`
  - Go → `gopls`
  - JSON/YAML/TOML → `vscode-json-languageserver` / `yaml-language-server`
- **Discovery order:** (1) explicit user config, (2) `PATH` lookup, (3) well-known install locations (e.g. `%USERPROFILE%\.rustup\toolchains\...\bin\rust-analyzer.exe`), (4) bundled fallback for a minimal set where licensing permits.
- **Launch policy:** lazy — start a server only when a file of that language is opened; shut down after an idle timeout. Never block editor startup on LSP.
- **Failure handling:** if a server is missing, the editor degrades gracefully (no diagnostics/completion) and surfaces a one-time, dismissible hint with an install command — never a hard error.

### 3.3 Features surfaced

| Feature | Trigger | Notes |
|---|---|---|
| Diagnostics | on `publishDiagnostics` | gutter markers + minimap + problems list in right bar |
| Go-to-definition | `F12` / `Ctrl+Click` | opens target in editor, respects multi-file |
| Hover | mouse hover / `K` (Vim) | type + doc, GPU-rendered tooltip |
| Completion | typing / `Ctrl+Space` | async, debounced; snippet support where server provides |
| Rename | `F2` | workspace-wide via `textDocument/rename` |
| Format | `Shift+Alt+F` | whole-document or selection via `textDocument/formatting`/`rangeFormatting` |

---

## 4. Inline Diff-Apply

This is the editor's signature feature and the connective tissue to the checkpointing/VCS subsystem (`plan/07`).

### 4.1 Data flow

1. The VCS subsystem produces a **diff** (unified or custom structured) between two revisions — typically the agent's working tree vs the last checkpoint, or `HEAD` vs working tree.
2. The diff is parsed into **hunks** (`{ old_range, new_range, lines }`), each with a status: added / removed / modified / context.
3. The editor renders the diff as an overlay on the buffer (or a dedicated diff view), with per-hunk **accept / reject / edit** affordances.

### 4.2 Apply model

```rust
// crates/editor/src/diff.rs
pub struct DiffHunk {
    id: HunkId,
    old_range: Range<Point>,
    new_range: Range<Point>,
    lines: Vec<DiffLine>,             // each tagged added/removed/context
    status: HunkStatus,               // Pending | Accepted | Rejected | Edited
}

pub enum HunkAction {
    Accept,                           // apply hunk's new text to the buffer
    Reject,                           // keep the old text (drop the hunk)
    Edit,                             // open the hunk in the buffer for manual fix
}
```

- **Accept:** apply the hunk's `new` lines into the buffer at the hunk position as one `EditGroup` (one undo step). Update the VCS subsystem so the applied change is reflected in the working tree.
- **Reject:** leave the buffer at `old` text; mark the hunk rejected so it is excluded from the next apply pass.
- **Edit:** materialize the hunk into the buffer, let the user edit freely, then re-diff the region to produce a new hunk.
- **Partial accept:** select a subset of lines within a hunk and accept only those (line-level granularity).

### 4.3 Consistency with the agent

- Applying a hunk **does not** silently diverge the agent's view. The editor sends the applied edit back through the same edit stream the agent sees (see §6), so the agent's next turn operates on the user-accepted state.
- If the buffer has been edited since the diff was computed, the hunk is **re-validated** against the current text before apply; a stale hunk is flagged rather than blindly applied (three-way merge semantics).

---

## 5. Diff Comments → Agent

Orca baseline bar: *inline diff comments → agent*. This is a first-class feature.

### 5.1 Interaction

- In the diff view, the user clicks a line's gutter to open a comment thread anchored to that line (old or new side).
- The comment is stored against a stable **line anchor** (survives edits via the rope's line tracking), not a raw line number.
- The user can reply to existing threads; threads are shown as gutter markers and in a comments list.

### 5.2 Routing to the agent

- A comment is packaged as a **structured message** and sent to the running agent session through the provider-adapter layer (`plan/05`) — e.g. a `user_input_respond` with a `diff_comment` payload, or an injected tool result.
- The agent receives: the file path, the anchored line range, the comment text, and the surrounding context (the hunk's old/new text).
- **Feedback loop:** the agent's next edit to that region updates the diff; the thread stays anchored and the user sees the resolution. A comment can be marked resolved.

```rust
// wire contract (see plan/04)
{
  "type": "diff_comment",
  "session_id": "...",
  "file": "src/main.rs",
  "anchor": { "line": 142, "side": "new" },
  "text": "This branch never returns; add an early return.",
  "context": { "old": "...", "new": "..." }
}
```

- **Multi-harness:** the same comment payload is adapted per provider; for Grok in-process it is delivered directly, for ACP/CLI providers via their input channel.

---

## 6. Editor ↔ Agent Integration

The editor and the agent share one source of truth: the **server runtime's workspace state**. The editor is a view over that state, not an independent copy.

### 6.1 Real-time agent edits

- When the agent edits a file, the VCS/workspace layer (`plan/07`) emits an **edit event** (`file_changed`, with the new content or an incremental edit).
- The editor receives the event and updates the buffer **without** moving the user's cursor or clobbering unsaved local edits. If the user has unsaved changes in the same region, the editor shows a conflict affordance rather than overwriting silently.
- The user sees agent edits land live, with a subtle visual pulse on changed lines and a diff badge in the file tab.

### 6.2 User context → agent

- The editor publishes a lightweight **context snapshot** on change: active file, cursor position, selection, visible range, and any open diff hunks.
- This feeds the agent's "current context" (e.g. the `x.ai/fs/*` / workspace context the harness uses) so the agent can act on what the user is looking at — without sending the whole buffer on every keystroke (throttled, e.g. on cursor idle or selection change).
- **Explicit send:** the user can also pin a selection and say "use this as context" — a first-class action, not just implicit telemetry.

### 6.3 Single edit stream

Both the user's edits and the agent's edits flow through the **same buffer revision stream**. This is what makes undo, diff-apply, and LSP sync coherent: there is one buffer, one revision counter, one LSP document version — regardless of who edited.

---

## 7. Performance

Performance targets from PLAN-CONTEXT apply directly to the editor: **cold start < 300ms**, **input latency < 16ms**, memory far below Electron.

### 7.1 GPU-rendered text

- Text is rendered as **GPU glyph atlases** (GPUI's text stack): glyphs are rasterized once per font/size/weight into an atlas texture, then drawn as textured quads. No per-frame CPU text shaping for static regions.
- **Shaping:** only the visible viewport is shaped (via a shaping cache); off-screen lines are not shaped until scrolled into view.
- **Layers:** the editor is composed of GPUI layers (text, gutter, minimap, diff overlay, diagnostics, selection) so only dirty layers re-render.

### 7.2 Input latency

- Keystroke → screen in **< 16ms**: the edit is applied to the rope, the affected viewport region is invalidated, and the frame is presented on the next vsync. No synchronous LSP, no synchronous disk I/O on the input path.
- **Async everything else:** LSP requests, file save, diff computation, and syntax re-parse all happen off the input path; results arrive as events.

### 7.3 Large files

- Rope buffer → O(log n) edits regardless of file size.
- **Viewport virtualization:** only visible lines are laid out and rendered; scrolling is O(viewport), not O(file).
- **Syntax highlighting** is incremental (tree-sitter) and only the visible + dirty regions are re-tokenized.
- **Minimap** is a downsampled GPU texture, rebuilt lazily.
- Target: a 100k-line file opens and scrolls smoothly; a 1M-line file opens without freezing (may defer full syntax tree).

### 7.4 Memory

- Persistent rope shares unchanged nodes across undo/redo and revisions → undo of large edits is cheap.
- Glyph atlases and shaping caches are bounded and evicted by LRU.
- No per-line `String` allocations for the visible window beyond the viewport.

---

## 8. Scope Phasing

**Open question 4** (PLAN-CONTEXT): *full native editor in MVP vs lighter editor first.* This doc does **not** decide — it lays out both paths and the trade-offs, and defers the call to the user (tracked in `plan/20`).

### 8.1 Path A — Full native editor (big effort)

Everything in §2–§7: rope buffer, multi-cursor, Vim mode, tree-sitter highlighting, LSP, inline diff-apply, diff comments, minimap, folding.

- **Pros:** ships the complete differentiator; matches the vision and the Orca baseline in one release; no rework later.
- **Cons:** largest single workstream; LSP + Vim + multi-cursor are each substantial; delays MVP.

### 8.2 Path B — Lighter editor first

A focused editor with: rope buffer, single/multi-cursor basics, **syntax highlighting**, **inline diff-apply**, **diff comments → agent**, line numbers, undo/redo. **Deferred:** LSP, Vim mode, minimap, folding, multi-cursor polish.

- **Pros:** ships the *signature* features (diff-apply + comments) fast; the editor is usable for the agent loop early; LSP/Vim land as follow-ups.
- **Cons:** not yet a "real editor" for standalone coding; some rework risk if the lighter core's buffer/selection model doesn't generalize.

### 8.3 Recommendation framing (not a decision)

The **buffer, selection, undo, and diff-apply core is identical in both paths** — it is not throwaway. The question is purely *how much of the surface layer (LSP, Vim, minimap, folding) ships in the MVP*. A defensible default is **Path B for the MVP, with the §2 core built to the full spec** so Path A features bolt on without rework. The final call is the user's.

---

## 9. Testing

TDD at inception is non-negotiable (PLAN-CONTEXT §Testing). The editor is a state machine — ideal for unit + property + component + integration coverage.

### 9.1 Unit tests (co-located `#[cfg(test)]`)

- **Buffer:** insert/delete at boundaries (start, end, middle, multi-byte chars, CRLF), line/column ↔ offset conversions, line cache invalidation.
- **Cursor/selections:** movement, selection expansion, multi-cursor edit application order (bottom-first), primary cursor tracking.
- **Undo/redo:** grouping, coalescing window, redo clearing, undo across multi-cursor edits, revision correctness.
- **Diff:** hunk parsing, accept/reject/edit, partial accept, stale-hunk re-validation.
- **LSP client:** request/response correlation, cancellation, document version tracking, diagnostics routing.

### 9.2 Property tests (proptest)

- **Rope invariants:** for arbitrary edit sequences, `buffer.text() == model.text()` where `model` is a naive `String` reference implementation; line/offset round-trips hold; undo then redo returns to the original text.
- **Selection invariants:** after any edit, all selections are within bounds and non-overlapping in the intended order.
- **Diff round-trip:** applying all accepted hunks of a diff to the old text yields the new text.

### 9.3 Component tests (GPUI)

- Editor pane element/component tests: rendering a buffer, scrolling, gutter, minimap, diff overlay, diagnostic markers.
- **Snapshot tests** for pane layouts (per PLAN-CONTEXT component strategy).

### 9.4 Integration tests

- **Real core + mock agent:** drive the editor against the server runtime with a fake agent (`grok agent stdio` mock per PLAN-CONTEXT); assert that agent edits appear in the buffer and that diff-apply updates the read model.
- **LSP integration (skip-not-fail):** launch a real language server (e.g. `rust-analyzer`) and assert definition/hover/completion round-trips and diagnostics flow. This test uses **skip-not-fail semantics** (per `plan/11`): if a real server is not present on the machine, the test is **explicitly SKIPPED** (marked via `#[ignore]` / a skip marker), **not** silently passed through a mock fallback. A mock LSP server may be used only in a *separate, clearly-named* unit/integration test that exercises the client protocol — never as a silent stand-in for the real-server test. In CI, a dedicated job installs a real server (e.g. `rustup component add rust-analyzer`) so the real-LSP test actually runs on the merge gate.
- **Wire contract:** the editor's edit events and diff-comment payloads are schema-verified against the JSON-RPC contract (`plan/04`) on both sides.

### 9.5 Mutation tests (cargo-mutants)

The editor's **core logic is mutation-gated** (LOCKED, D21): the buffer, diff-apply, undo, and selection are safety-critical core and must survive the cargo-mutants gate with the standard thresholds — **≥85% line, ≥80% branch, ≥70% mutation score killed** (D33: 70% is the merge floor, not a target; the bar may rise over time). This is consistent with `plan/15` §3.3 and the corrected mutation-scope list (D21).

- **Buffer:** mutants in insert/delete/slice, line-cache invalidation, and line/column ↔ offset conversion must be killed.
- **Diff-apply:** mutants in hunk parsing, accept/reject/edit, partial accept, and stale-hunk re-validation must be killed.
- **Undo/redo:** mutants in grouping, coalescing, redo-clearing, and revision correctness must be killed.
- **Selection:** mutants in multi-cursor edit application order (bottom-first) and primary-cursor tracking must be killed.
- **Operational rules (per `plan/15`):** survived mutants are a code smell — fix the test or the dead code, never silence a survivor without a written justification in the PR. Incremental mutation (`--in-place`) limits blast radius to changed files on PRs. The editor's unit+property suite (§9.1–§9.2) is kept fast so the mutation gate stays tractable in CI.

### 9.6 CI-gate section

Editor changes must pass the **full gate chain** before merge, in the exact order from `plan/15` §5 and PLAN-CONTEXT §Testing — **no blind CI**:

```
fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e → coverage
```

- **No blind CI:** the local dev loop runs the same gates before pushing; CI is a second confirmation.
- **Deny warnings:** `clippy -D warnings` is a hard gate — a single warning fails the build.
- **Mutation gate** (D21) runs on the editor core (buffer, diff-apply, undo, selection) at ≥70% killed; **coverage gate** enforces ≥85% line / ≥80% branch on top.
- **E2E cadence** (D32): e2e runs on the merge gate (critical paths) and nightly (full suite) — there is no "skip e2e for small changes" path.
- **LSP integration** is skip-not-fail (see §9.4): a CI job installs a real server so the real-LSP test runs on the merge gate; otherwise it is explicitly skipped, never silently passed.

### 9.7 E2E

- Drive the real app/headless: open a file, type, undo, apply a diff hunk, drop a diff comment, verify it reaches the agent and the agent's reply updates the diff. This is a differentiator over T3 Code (no e2e).

---

## 10. Open Questions

Per PLAN-CONTEXT, these are pending user decisions; this doc references them and does **not** decide unilaterally. Tracked in `plan/20-risks-and-open-questions.md`.

1. **Editor scope (Open Q4):** full native editor vs lighter editor first in the MVP — see §8. **The single most consequential decision for this doc.**
2. **LSP server bundling:** which servers, if any, we bundle vs require from `PATH` (licensing and size trade-offs).
3. **Vim fidelity:** how faithful the Vim mode must be in the MVP (subset vs near-complete).
4. **Tree-sitter grammar set:** the exact curated language list for MVP highlighting.
5. **Diff-apply semantics:** whether applying a hunk should immediately rewrite the agent's working tree or stay as a pending overlay until the user confirms a batch.
6. **Editor ↔ agent context:** how aggressively the editor's cursor/selection feeds the agent (implicit vs explicit-only), balancing usefulness against surprise.
7. **Windows input handling:** IME/composition support for CJK input in the editor (Windows-first makes this relevant for a subset of users).

---

*Next: `plan/10-ui-pane-system.md` — the pop-out pane UI that hosts the editor, terminal, browser, HAR, and diff panes.*
