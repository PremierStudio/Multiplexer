# 08 — Terminal

**Status:** Draft (consistent with `docs/PLAN-CONTEXT.md`)
**Owner:** Multiplexer planning fan-out
**Scope:** Server-owned PTY architecture, Ghostty embedding, rendering, PTY management, agent integration, pop-up terminal pane, Windows support, testing.

> **Locked decisions applied:** This doc applies locked decisions from `docs/DECISIONS.md`:
> **M2** (Windows-specific tests — §9.7), **M4** (deep TUI assertions — §9.2.1),
> **D21** (mutation scope incl. terminal — §9.6), **m3** (perf measurement via plan/16 — §9.8).

---

## 1. Principles & Baseline Bar

The terminal is a **baseline bar** capability, not a differentiator: we must match Orca's "Ghostty-class terminal with splits." That means a real, fast, native terminal with split panes, infinite splits, scrollback restored on restart, and full scrollback search — not a `<textarea>` pretending to be a shell.

The governing architectural rule (from `docs/PLAN-CONTEXT.md` §Architecture) is **server-centric**: the single native Rust binary **owns** terminal processes. The client is a thin shell over the JSON-RPC-over-WebSocket contract (`plan/04`). The terminal is therefore a **server-side resource** whose frames, input, and lifecycle are streamed over the wire contract — never a client-owned child process.

| Requirement | Source | Bar |
|---|---|---|
| Ghostty-class terminal with splits | Baseline bar | Must match Orca |
| Infinite splits | Baseline bar | Any pane splits arbitrarily |
| Scrollback restored on restart | Baseline bar | Session survives client reconnect |
| Full scrollback search | Baseline bar | Search the entire buffer, not just visible |
| Terminal must not block UI thread | Performance | Input latency < 16ms, 60fps+ |
| Server owns processes | Architecture | Core owns PTYs, not the client |
| Windows-first | Differentiator | ConPTY path is first-class |

---

## 2. Terminal Architecture

### 2.1 Server-owned PTYs

The core process owns every PTY. A client never spawns a shell directly; it issues a `terminal.spawn` request over the wire contract and receives a `terminal_id` handle. This gives us:

- **Survival across client reconnect** — the shell keeps running even if the desktop window closes or the mobile app disconnects; the session is resumed, not restarted.
- **Single source of truth** — the read model (SQLite) records terminal state, so the orchestration engine, agent tools, and any client observe the same reality.
- **Security** — PTY access is gated by the same authenticated contract as everything else; a rogue client cannot spawn arbitrary processes outside the contract's authorization.

### 2.2 Component layout

```
core/
  terminal/
    mod.rs            # public API: spawn, resize, write, kill, list, attach
    pty/
      mod.rs          # Pty trait (platform-agnostic)
      unix.rs         # forkpty / posix_openpt backend
      windows.rs      # ConPTY backend
    emulator/
      mod.rs          # Ghostty C ABI wrapper (ffi)
      render.rs       # frame → GPUI texture pipeline
      scrollback.rs   # ring buffer, search index
    session.rs        # TerminalSession: pty + emulator + scrollback + wire bridge
    agent_bridge.rs   # harness tool integration (x.ai/terminal/*)
    pane.rs           # UI pane model (splits, focus, pop-up)
  ui/
    terminal_pane.rs  # GPUI component
```

### 2.3 The wire contract surface

The terminal exposes a small, schema-verified set of RPC methods and events (see `plan/04` for the full contract conventions):

| Method | Direction | Purpose |
|---|---|---|
| `terminal.spawn` | client → core | Create a PTY (shell, cwd, env, cols/rows) → `terminal_id` |
| `terminal.resize` | client → core | Set cols/rows (window resize, split resize) |
| `terminal.write` | client → core | Send input bytes (keystrokes, paste) |
| `terminal.kill` | client → core | Terminate the process tree |
| `terminal.attach` | client → core | Re-attach to an existing session (reconnect) |
| `terminal.search` | client → core | Query scrollback (regex, case, direction) |
| `terminal.event` | core → client | Output frames, exit, bell, title change, cursor |

All terminal I/O flows through the same authenticated WebSocket as the rest of the contract. Frames are delivered as **binary messages** (not JSON) to avoid base64 bloat on the hot path; JSON is used for control messages.

---

## 3. Ghostty Embedding

### 3.1 Why Ghostty

Ghostty is a fast, native terminal emulator written in Rust with a **C ABI** (`libghostty`). Embedding it gives us a battle-tested terminal emulator (VT/ANSI parsing, Unicode, fonts, scrollback) without writing an emulator from scratch — and it is the same class of terminal Orca ships. Because it is Rust with a C ABI, it integrates cleanly with our Rust core via FFI, with no Electron/DOM in the frame path.

### 3.2 Vendoring

Following the grok-build vendoring decision (`plan/03`), we vendor Ghostty under `third_party/ghostty` (or via `[patch]` to a pinned commit) and build `libghostty` as a static library linked into our core. We pin a specific release and track upstream for security/emulation fixes. Ghostty is MIT-licensed; we keep its license header intact.

### 3.3 C ABI surface

We wrap the Ghostty C ABI in a thin, unsafe-free Rust module (`emulator/mod.rs`) behind a safe `TerminalEmulator` trait. The key calls:

```c
// Ghostty C ABI (abridged — actual names per pinned version)
ghostty_app_t*      ghostty_app_new(const ghostty_app_config_t*);
ghostty_surface_t*  ghostty_surface_new(ghostty_app_t*, const ghostty_surface_config_t*);
void                ghostty_surface_set_size(ghostty_surface_t*, uint32_t cols, uint32_t rows);
void                ghostty_surface_write(ghostty_surface_t*, const uint8_t* data, size_t len);
void                ghostty_surface_key(ghostty_surface_t*, const ghostty_key_t*);
void                ghostty_surface_mouse(ghostty_surface_t*, const ghostty_mouse_t*);
void                ghostty_surface_paste(ghostty_surface_t*, const uint8_t* data, size_t len);
void                ghostty_surface_scrollback(ghostty_surface_t*, int64_t lines);
```

The app/surface model maps cleanly onto our split model: **one Ghostty surface per terminal pane**. A split is just another surface sharing the same app instance and font cache.

### 3.4 Rendering integration

Ghostty's renderer is OpenGL-based; we do **not** use its GL renderer directly. Instead we use Ghostty's **headless / offscreen** mode: the emulator produces a grid of cells (text + attributes) which we rasterize ourselves into a GPU texture via GPUI. This keeps the frame path 100% native GPUI (no GL context ownership fights, no DOM), and lets us reuse GPUI's font/shader pipeline for crisp text at any DPI.

```rust
// emulator/render.rs — conceptual
struct TerminalFrame {
    cols: u32,
    rows: u32,
    cells: Vec<Cell>,          // char, fg, bg, attrs (bold/italic/underline/url)
    cursor: Option<(u32, u32)>,
    dirty: bool,
}

impl TerminalFrame {
    // Rasterize into a GPUI texture; only re-upload dirty regions.
    fn to_texture(&self, gpu: &Gpu) -> Texture { /* ... */ }
}
```

Only **dirty regions** are re-rasterized and re-uploaded per frame, so a mostly-idle terminal costs almost nothing. Full-screen redraws (e.g. `vim`, `htop`, `cargo test`) are handled by a fast path that re-uploads the changed cell rows.

### 3.5 Splits & infinite splits

Splits are a **pane-system** concern (`plan/10`) layered on top of the terminal model. Each split holds one `TerminalSession`; the pane tree is a binary split tree (horizontal/vertical) that can be nested arbitrarily, giving infinite splits. Resizing a split issues a `terminal.resize` to the affected session(s). Focus follows the active pane; keyboard input routes to the focused session's `terminal.write`.

---

## 4. Rendering

### 4.1 Native frame path

There is **no React/DOM in the frame path**. The pipeline is:

```
Ghostty emulator (headless)
   → TerminalFrame (cells + attrs)
   → GPUI rasterizer → GPU texture
   → GPUI draw (blit quad)
```

This is a pure native pipeline: CPU-side cell generation in the emulator, GPU-side blit in GPUI. No HTML, no canvas-in-DOM, no per-frame JS.

### 4.2 Non-blocking the UI thread

The terminal must never block the UI thread (input latency < 16ms, 60fps+). We achieve this with a **dedicated terminal worker thread** per session (or a small thread pool):

- The PTY read loop, Ghostty emulator parsing, and scrollback indexing run on the worker thread.
- The worker produces `TerminalFrame`s and hands them to the UI thread via a lock-free channel (e.g. `crossbeam-channel` or a ring buffer).
- The UI thread only rasterizes and blits the latest frame; it never parses VT sequences or blocks on PTY I/O.
- Backpressure (see §5.4) prevents the worker from flooding the channel.

### 4.3 Fonts & text

Text is rendered with GPUI's font pipeline (system fonts, subpixel/grayscale AA, ligatures where the font supports them). We reuse the same font cache as the editor (`plan/09`) so terminal and editor share glyph atlases — one less memory/startup cost. Monospace detection and fallback chains are configured per theme.

---

## 5. PTY Management

### 5.1 Spawning

`terminal.spawn` creates a PTY and execs the configured shell (default: `pwsh` on Windows, `$SHELL`/`bash` on Unix, configurable per session). The spawn request carries: shell path, working directory, environment (merged with the session's env, minus secrets), and initial cols/rows. The core sets the process group / job object so the whole tree can be killed reliably.

### 5.2 Resize

`terminal.resize` updates the PTY window size and the Ghostty surface size. On Windows this is a `ResizePseudoConsole` call; on Unix a `TIOCSWINSZ` ioctl. Resize is cheap and idempotent; the UI debounces rapid window-resize events.

### 5.3 Input & output streaming

- **Input:** client keystrokes/paste → `terminal.write` → PTY master. Ghostty translates key events to the correct escape sequences before writing (so the client sends semantic keys, not raw bytes — this keeps mobile/web clients correct).
- **Output:** PTY master → worker thread → Ghostty emulator → `TerminalFrame` → UI. Output is also appended to the scrollback ring buffer and indexed for search.

### 5.4 Backpressure

PTY output can be unbounded (e.g. `yes`, `cat /dev/urandom`, a runaway build). We enforce backpressure at two levels:

1. **PTY level:** when the worker's frame channel is full, we stop draining the PTY master (Unix: stop reading; Windows: stop `ReadFile` on the ConPTY output). The OS-level pipe buffer then fills and the child blocks on write — this is the correct, kernel-enforced backpressure that prevents unbounded memory growth.
2. **Scrollback level:** the scrollback ring buffer has a hard cap (configurable, default e.g. 100k lines). When full, the oldest lines are evicted and the search index is pruned. The live frame is never evicted.

This mirrors how real terminals behave and keeps memory bounded regardless of what the child does.

### 5.5 Lifecycle & cleanup

When a session's process exits, the worker emits a `terminal.event` (exit code) and the session transitions to a "closed" state. The core guarantees the process tree is reaped (job object on Windows, process group kill on Unix) — no orphaned shells. Sessions are listed in the read model so the orchestration engine and clients can enumerate live terminals.

---

## 6. Integration with Agents

### 6.1 The terminal as the harness's shell

The embedded grok-build harness (`plan/03`) runs shell commands. By default the harness uses its own execution primitives (`xai-grok-workspace`), but we surface those commands **in a visible terminal** so the user sees exactly what the agent is doing — the "watch the agent work" experience.

Two integration modes:

| Mode | Description | Use |
|---|---|---|
| **Mirror** | Harness runs the command; we tee its output into a terminal session | Default: user watches agent actions live |
| **Interactive** | The agent's command runs *in* a real PTY the user can take over | Debugging, long-running interactive tools |

In **Mirror** mode the terminal is read-mostly (the user can interrupt via the agent, not by typing into the PTY). In **Interactive** mode the user can type directly and the input is fed to the harness's running command.

### 6.2 Terminal output as agent context

The harness's `x.ai/terminal/*` tools (from the vendored grok-build ACP extensions) read terminal output. We make the **scrollback of any session available as agent context**: the agent can query recent output, search the buffer, and read exit codes. This is how the agent "sees" what a long-running build printed, even if it wasn't captured at the time.

```rust
// agent_bridge.rs — conceptual
impl TerminalAgentBridge {
    // Feed a terminal's recent output into the agent's context window.
    fn recent_output(&self, id: TerminalId, tail: usize) -> String { /* scrollback tail */ }
    fn search(&self, id: TerminalId, pattern: &Regex) -> Vec<Match> { /* scrollback search */ }
}
```

### 6.3 Agent-initiated terminals

The agent can also spawn terminals (e.g. "open a shell in the worktree and run the test suite") via the same `terminal.spawn` path. These appear in the terminal pane and the orchestration dashboard (`plan/06`), so the user sees agent-created shells alongside their own.

---

## 7. The Pop-up Terminal Pane

From the pane-system spec (`plan/10`): an **optional terminal that slides up from the bottom** of the UI — the "pop-up terminal" (like VS Code's integrated terminal / a Quake-style dropdown).

Design:

- **Trigger:** a global hotkey (e.g. `` Ctrl+` ``) toggles it; it slides up over the current pane without re-laying-out the whole window.
- **Behavior:** it is a normal `TerminalSession` hosted in a dedicated bottom strip. It can be split (horizontal splits inside the pop-up), popped out to its own window, or promoted to a full right-bar pane.
- **State:** the pop-up's session persists across toggles — closing it does not kill the shell; it just hides the strip. This matches the "scrollback restored on restart" bar.
- **Focus:** toggling focuses the pop-up; toggling again returns focus to the previous pane.
- **Mobile:** the pop-up is a desktop affordance; on mobile the terminal is a full pane in the right bar instead.

The pop-up is implemented as a pane-system layout node, so it inherits split/pop-out behavior for free (`plan/10`).

---

## 8. Windows Support

Windows-first is a differentiator, so the Windows PTY path is **first-class, not an afterthought**.

### 8.1 ConPTY

We use **ConPTY** (Windows Pseudo Console, the modern Win32 API) — the same underlying mechanism node-pty uses, but accessed directly via the Win32 API (no node-pty dependency, no Node runtime). ConPTY gives us:

- A real console host with proper VT/ANSI emulation (so `vim`, `htop`-style TUIs, and colored output work).
- `CreatePseudoConsole`, `ResizePseudoConsole`, `ReadFile`/`WriteFile` on the ConPTY pipes.
- Job objects (`CreateJobObject` + `AssignProcessToJobObject`) for reliable process-tree kill.

### 8.2 Windows architecture

```
Windows PTY backend (windows.rs)
  CreatePseudoConsole(cols, rows, in_pipe, out_pipe)
  → spawn shell (CreateProcessW with EXTENDED_STARTUPINFO_PRESENT)
  → assign to job object
  → worker thread: ReadFile(out_pipe) → emulator; WriteFile(in_pipe) ← input
  → ResizePseudoConsole on resize
  → TerminateJobObject on kill
```

### 8.3 Windows-specific concerns

| Concern | Handling |
|---|---|
| Default shell | `pwsh` (PowerShell 7) if present, else `powershell.exe`; configurable |
| Paths & env | Use UTF-16 APIs (`CreateProcessW`); normalize `%PATH%`; never shell-escape blindly |
| ConPTY quirks | ConPTY has known quirks around resize and certain full-screen apps; we pin a Ghostty version that handles them and add integration tests for the common TUIs |
| Job objects | Every session gets a job object so `terminal.kill` reaps the whole tree (no orphaned `node`/`cargo` children) |
| Unicode | ConPTY is UTF-16 internally; we convert to UTF-8 at the emulator boundary |

### 8.4 Unix backend

The Unix backend (`unix.rs`) uses `posix_openpt`/`grantpt`/`unlockpt` + `forkpty` (or `openpty` + `fork`/`exec`), `TIOCSWINSZ` for resize, and process-group kill. This is the well-trodden path; Windows is the harder one and gets the most test attention.

---

## 9. Testing

TDD at inception is non-negotiable (`docs/PLAN-CONTEXT.md` §Testing). The terminal gets the full ladder.

### 9.1 Unit tests (co-located `#[cfg(test)]`)

- **PTY backend:** spawn/resize/write/kill round-trips against a real PTY; verify exit codes and process-tree reaping. Property tests (proptest) for resize sequences and byte-stream chunking (arbitrary splits of a byte stream must produce identical emulator output).
- **Scrollback ring buffer:** eviction at cap, search correctness, tail extraction. Property tests for "append N lines, read tail M" invariants.
- **Backpressure:** bounded memory under a synthetic firehose; verify the worker stops draining when the channel is full.
- **Wire contract:** `terminal.*` RPC serialization/deserialization round-trips; schema-verified against the contract (`plan/04`).

### 9.2 Integration tests (real core + real shell)

- Spawn a real shell (`pwsh` on Windows CI, `bash` on Unix CI), run commands, assert on the read model and emitted events.
- **Real-binary smoke tests:** run `vim`, `htop`-style TUIs, and colored output through the emulator and assert the resulting frames contain expected cells.
- **Reconnect test:** spawn a long-running process, detach the client, re-attach, and assert the session survived and scrollback is intact.
- **Agent integration:** run a harness command in Mirror mode and assert its output appears in the terminal session and is queryable as agent context.

#### 9.2.1 Deep TUI assertions (M4)

The real-binary smoke tests above must **not** stop at "the resulting frames contain expected
cells." Per locked decision **M4**, TUI output is asserted with **golden cell-grid snapshots or
structural assertions** — specific cells at specific row/col, cursor position, scrollback, and
full-screen redraw correctness — not a shallow "some expected cells are present" check.

- **Golden cell-grid snapshots.** For a fixed terminal size (e.g. 80×24) and a pinned TUI version,
  run the TUI to a deterministic state and compare the full cell grid (char + fg + bg + attrs per
  cell) against a committed golden snapshot (via `insta`). A change in any cell is a visible,
  reviewed diff — not a silent regression. This is the terminal analogue of the pane-layout
  snapshots in `plan/15` §3.2.
- **Structural assertions** (used where a full golden grid is brittle, e.g. live-updating TUIs):
  - **Specific cells at specific row/col:** assert exact cell content at named coordinates
    (e.g. `vim`'s status line at row 24, `htop`'s header row at row 0).
  - **Cursor position:** assert the reported cursor row/col after a known sequence (e.g. cursor
    at the prompt after a command, at the `vim` insert position after `i`).
  - **Scrollback:** assert the ring buffer contains the expected lines and that search returns
    the expected matches at the expected offsets.
  - **Full-screen redraw correctness:** after a full-screen repaint (e.g. `vim` toggling
    alternate screen, `htop` redrawing), assert the entire grid matches the expected state —
    no stale cells, no residue from the previous screen.
- **Property tests** complement the golden/structural checks: arbitrary splits of a byte stream
  must produce identical emulator output, and resize sequences must leave the grid in a
  consistent state (see §9.1).

### 9.3 Component tests (GPUI)

- **Terminal pane:** render a `TerminalFrame` into the GPUI component; snapshot tests for pane layouts (`plan/10`).
- **Pop-up terminal:** toggle open/close, assert the strip appears/disappears and the session persists.
- **Splits:** split/unsplit/resize a pane tree; snapshot the layout; assert focus routing.

### 9.4 E2E

Drive the real app/headless: open the app, spawn a terminal, type a command, assert the output renders. This beats T3 Code (which has no e2e) and is part of the baseline bar.

### 9.5 CI gates

Terminal changes must pass the full ladder: fmt → clippy (deny warnings) → unit+property → mutation (cargo-mutants; ≥85% line, ≥80% branch, ≥70% mutation score) → integration → component → e2e → coverage. **No blind CI.**

### 9.6 Mutation testing (D21)

Per locked decision **D21**, the terminal is **explicitly in scope** for mutation testing — it is
not excluded as "UI-adjacent." `plan/15`'s corrected scope list includes the terminal (PTY,
scrollback, backpressure), and this doc confirms it with concrete targets.

- **Scope (what is mutated):** the PTY backend (`pty/unix.rs`, `pty/windows.rs`), the scrollback
  ring buffer + search index (`emulator/scrollback.rs`), and the backpressure logic (§5.4) — the
  correctness-critical terminal logic. The Ghostty C-ABI FFI wrapper is thin and covered by
  integration tests; the vendored Ghostty emulator itself is excluded (third-party, covered by
  upstream tests), matching `plan/15`'s vendored-code exclusion.
- **Targets:** the terminal's mutation score must be **≥70% mutants killed** (the merge floor per
  D33), with ≥85% line and ≥80% branch coverage — the same thresholds as the rest of the core.
  Survived mutants are a code smell: fix the missing test or the dead branch, never silence a
  survivor without a written justification in the PR.
- **Concrete mutation targets to guard:**
  - **PTY:** operator flips in resize/read/write paths, dropped exit-code propagation, missed
    process-tree reaping, wrong backend branch selection.
  - **Scrollback:** eviction-at-cap boundary, search-index pruning, tail-extraction off-by-ones,
    ring-buffer wrap-around.
  - **Backpressure:** the "stop draining when the channel is full" branch, the resume-after-drain
    branch, and the bounded-memory invariant under a firehose.
- **Mechanism:** `cargo mutants --in-place` on the terminal crate, gated in CI (mutation stage of
  §9.5), with incremental `--in-place` on PRs to limit blast radius (per `plan/15` §2.3).

### 9.7 Windows-specific tests (M2)

Windows-first is a differentiator and **ConPTY is the riskiest backend**, so Windows gets a
dedicated, mandatory test section (locked decision **M2**) — not just "the same tests run on
Windows CI." These run on Windows CI and gate the merge like every other layer.

- **Common TUIs under ConPTY:** run `vim`, `htop`, and `cargo test` (a long, colored, scrolling
  build) through the ConPTY backend and assert the resulting frames via the deep assertions in
  §9.2.1 (golden cell-grid snapshots / structural assertions).
- **Full-screen redraw:** assert full-screen redraw correctness (alternate-screen entry/exit,
  no stale cells) for `vim` and `htop` under ConPTY.
- **Resize storms:** drive rapid `terminal.resize` sequences (window drag, split resize) and
  assert the grid stays consistent and no resize deadlocks or dropped output occur — ConPTY's
  known resize quirks are the specific target.
- **Job-object tree-kill verification:** spawn a process tree (e.g. a shell that spawns `node` /
  `cargo` children), call `terminal.kill`, and assert the **entire tree** is reaped — no orphaned
  children. This verifies `CreateJobObject` + `AssignProcessToJobObject` + `TerminateJobObject`.
- **ConPTY quirks asserted:** the specific quirks we pin a Ghostty version for (§8.3) are each
  covered by a named test — resize behavior, full-screen app handling, UTF-16→UTF-8 conversion at
  the emulator boundary, and `pwsh` default-shell spawning.

### 9.8 Performance measurement (m3)

Per locked decision **m3**, terminal-specific performance is measured via **plan/16's benchmark
suite** rather than ad-hoc checks. `plan/16` §9.1 already defines `terminal_throughput`
(coalesced terminal output under burst → bounded frames, no flood); this doc adds the two
terminal-specific benchmarks that are not yet in plan/16's table:

- **`terminal_frame_throughput`:** full-screen redraws per second (the §10 "Frame throughput"
  target) — a `vim`/`htop`-style full repaint must sustain 60fps.
- **`terminal_idle_cost`:** CPU/frame cost when the terminal is idle (the §10 "Idle cost ~0"
  target) — no re-rasterization when nothing changes.

Both use the same **measurement mechanism** as plan/16: a **criterion** benchmark suite run on a
**pinned reference machine** (a representative Windows desktop), with recorded baselines and
regression flags. These feed the **dedicated perf stage** in plan/15's CI (D22), where the hard
gates (input latency < 16ms p95, frame throughput 60fps, idle cost ~0) are enforced.

---

## 10. Performance Targets

| Metric | Target | How |
|---|---|---|
| Terminal input latency | < 16ms (60fps+) | Worker thread + lock-free frame channel; UI only rasterizes/blits |
| Frame throughput | Full-screen redraws at 60fps | Dirty-region re-upload; fast path for changed rows |
| Idle cost | ~0 | No re-rasterization when nothing changes |
| Scrollback memory | Bounded (hard cap) | Ring buffer + eviction + kernel backpressure |
| Cold start | < 300ms total | Terminal lazily initializes; Ghostty surface created on first open |

---

## 11. Open Questions

These reference pending decisions from `docs/PLAN-CONTEXT.md` and `plan/20`; we do **not** decide them unilaterally.

1. **Ghostty vendoring mechanism** — submodule vs vendored copy vs `[patch]` (mirrors open question 5 for grok-build; recommend vendored fork + `[patch]`).
2. **Ghostty version pinning cadence** — how aggressively we track upstream for emulation/security fixes vs stability.
3. **Default shell on Windows** — `pwsh` vs `powershell.exe` vs user-configured; recommend `pwsh` with fallback, but confirm.
4. **Scrollback cap default** — 100k lines is a starting point; confirm against memory budget.
5. **Mirror vs Interactive default** for harness commands — whether agent commands default to visible-mirror or user-takeover.
6. **Pop-up terminal hotkey** — default binding (`` Ctrl+` ``) and whether it's configurable in MVP.
7. **Orca baseline scope** (open question 7) — whether full terminal parity (infinite splits, scrollback search) is in MVP or a subset.

---

*Next: `plan/09-editor.md` — the native GPU editor.*
