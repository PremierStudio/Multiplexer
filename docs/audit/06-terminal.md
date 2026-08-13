# 06. Bottom terminal drawer

**Scope:** Desktop `terminal_strip`, `pump` / `pending_cmd`, `multiplexer-client` `command.rs`, vs `plan/30-chrome-drawers.md` §11 and `plan/08-terminal.md` §7.
**Sources read:** `apps/multiplexer-desktop/src/main.rs`, `crates/multiplexer-client/src/command.rs`, `crates/multiplexer-shell/src/{workspace.rs,terminal_ui.rs,palette.rs,actions.rs,bindings.rs}`, `apps/multiplexer-desktop/src/controls.rs`, `crates/multiplexer-terminal/src/capture.rs`.
**Not run:** cargo.
**Verdict:** The headless drawer model exists (`bottom_open`, `bottom_height`, `toggle_bottom`, `set_bottom_height`). The painted UI is still the prototype strip from plan/30 §0: always on, no chrome, no drag, 4-line cap, no toggle control. `Ctrl+\`` is only half-wired.

---

## Findings

### F1. Not a real drawer (still a fixed strip)

- **Severity:** High
- **Where:** `apps/multiplexer-desktop/src/main.rs` `terminal_strip` (1469), `Render` (876-890)
- **Evidence:** The strip is a `glass_bar()` child under the three-column row. It has no 28px header, no `>_ Terminal` label, no grab handle, no chevron, no `overflow_y_scroll` log. Desktop never reads `workspace.bottom_open`. Height is `px(workspace.occupied_bottom())` only.
- **Spec:** plan/30 §11.1: grab handle, header (`>_`, `Terminal`, Clear, chevron), log `flex-1` + scroll, draft + Run. Collapsed 120 / expanded 280. plan/08 §7: slide-up pop-up over the pane, session persists, toggle focuses the strip.
- **Impact:** Expanding height (if `ToggleBottom` fires) just stretches empty muted text. It still re-lays out the center column. It is not a slide-up drawer.
- **Fix:** Paint §11.1 chrome. Key log line count and header chevron off `bottom_open`. Keep the strip always painted (never 0px this sprint). Overlay-without-relayout stays plan/08 / later.

### F2. No drag resize (`DragRail::Bottom` missing)

- **Severity:** High
- **Where:** `apps/multiplexer-desktop/src/main.rs` `DragRail` (42-45), mouse move (849-864), `resize_handle` (1660-1685)
- **Evidence:** `DragRail` is `Left | Right` only. Mouse move sets left/right width. `resize_handle` early-returns unless that side rail is open. `set_bottom_height` exists on `Workspace` (workspace.rs 798) and is never called from desktop.
- **Spec:** plan/30 §6: 5px top-edge handle, `DragRail::Bottom`, `set_bottom_height(win_h - mouse_y - status_h)`, clamp 120..=420, `bottom_open = height > 120.5`.
- **Impact:** User cannot grow the strip to the 200/280/420 band. Headless clamp tests pass while the shipping window cannot resize the drawer.
- **Fix:** Add `DragRail::Bottom`, a 5px `cursor` NS handle on the top of `terminal_strip`, and a mouse-move arm that calls `set_bottom_height`.

### F3. `Ctrl+\`` wiring is incomplete

- **Severity:** High
- **Where:** `main.rs` `handle_key` (696-698); `palette.rs` `default_items`; `controls.rs` `SHORTCUTS` / `REQUIRED_IDS`; `help_overlay` (1654-1656)
- **Evidence:**
  - Bound: `if key == "\`" && mods.control` dispatches `ToggleBottom`. That is before palette / terminal focus, so the chord is global if GPUI emits `` ` ``.
  - Not bound: `oem_3` (plan/30 §11.2 says accept it and pin the real 0.2.2 key). No comment that the live key was pinned.
  - Palette has `run-terminal` and `term-tab`, not `{ id: "toggle-terminal", hint: "Ctrl+\`", action: ToggleBottom }`.
  - `controls.rs` has no `toggle_bottom` id and no `ctrl-\`` shortcut. TermStrip catalog is only `term_run` / `term_clear`.
  - Help overlay still says `Ctrl+[ / ] rails`. It does not say `Ctrl+\` terminal drawer`.
  - Empty-center hint (1724) does mention `Ctrl+\` terminal`.
- **Spec:** plan/30 §11.2, §12, §13 steps 5-6.
- **Impact:** On Windows, if 0.2.2 reports `oem_3`, the only working path is dead. Even when `` ` `` works, palette / help / catalog do not teach or expose the gesture.
- **Fix:** Bind `` ` `` and `oem_3`. Add palette `toggle-terminal`. Add `toggle_bottom` + `ctrl-\`` to `controls.rs` (update `REQUIRED_IDS` in the same change). Replace help copy.

### F4. Builtins are inconsistent and half-implemented

- **Severity:** Medium
- **Where:** `crates/multiplexer-shell/src/terminal_ui.rs` `parse_builtin` / `help_text`; `main.rs` `run_shell` (514-529)
- **Evidence:** Three catalogs disagree.

  | Surface | List |
  |---|---|
  | Empty-strip hint (1471) | `clear, help, cores, mcp, git` |
  | Help overlay (1655) | `clear  help  cores  mcp  git  points` |
  | `help_text()` (87-89) | `clear, help, cores, mcp` |

  `parse_builtin` also maps `points`/`checkpoint` to `Checkpoint` and `skills` to `BuiltinCmd::Unknown` (then desktop prints `unknown builtin`). `cores` / `mcp` / `git` / `checkpoint` only write `workspace.inspector = ...`. They do not call `select_inspector` (accordion not cleared), do not open the right rail, and do not refresh data. `help` prints `help_text()` and also `toggle_help()`.
- **Spec:** plan/30 §11.1: builtins stay (clear, help, cores, mcp, git, plus draft Run). Inspector tab changes should go through `select_inspector`.
- **Impact:** Typed `skills` looks broken. `help` lies about `git`/`points`. Tab builtins do not match palette `SelectTab` (no accordion clear, rail may stay closed).
- **Fix:** One catalog. Map `skills` to `SelectTab(Skills)` or drop it from copy. Route tab builtins through `select_inspector` + open-right. Stop toggling the help overlay from the `help` builtin, or document that as the only help path.

### F5. Freeze / wedge risk in `spawn_command`

- **Severity:** High
- **Where:** `crates/multiplexer-client/src/command.rs` `run_command` (80-86); `main.rs` `run_shell` (531-537), `pump` (595-641), `interrupt` (253-264)
- **Evidence:** `windows_cmd` is `cmd.exe /C <line>`. The worker calls `Command::output()` with no timeout, no `Stdio::null()` on stdin, no `CREATE_NO_WINDOW`, no child handle for kill. `output()` waits for process exit and buffers all stdout/stderr. Desktop keeps one `pending_cmd`. A second Run prints `a shell command is already running`. `interrupt` only hits the grok session + `ignore_turn`. It does not drop or kill `pending_cmd`. `pump` then `body.lines().take(40)` after the full body is already in memory. `multiplexer-terminal::ProcessCapture` already spawns piped + `CREATE_NO_WINDOW` + line drain and is unused by the strip.
- **Spec:** plan/30 §11.3: this sprint is log+draft, not Ghostty/PTY. That still requires a command that cannot wedge the strip. plan/08: job-object kill, streaming read.
- **Impact:** `pause`, `timeout /t 99999`, `ping -t`, `python`, or a huge `type` permanently occupy the worker. The UI stays painted (`try_recv` + animation frames) but the terminal is frozen: no new command, no cancel, unbounded worker memory. Interactive `cmd` can wait on inherited stdin forever.
- **Fix:** Timeout + stdin null + `CREATE_NO_WINDOW`. Hold a killable child (or use `ProcessCapture`). Wire Stop / `Ctrl+.` to kill `pending_cmd`. Stream lines into `push_capped` instead of one `output()` blob.

### F6. Four-line cap even when expanded

- **Severity:** Medium
- **Where:** `main.rs` `terminal_strip` (1470-1474, 1489-1494)
- **Evidence:** Log paint is always `visible_tail(&self.workspace.terminal_log, 4)` (or the empty hint). The joined string is one `div` child. No `overflow_y_scroll`. `bottom_open` / `occupied_bottom()` do not change the tail size. History cap is 80 (`TERM_HISTORY_MAX`). `pump` already keeps up to 40 output lines per command, then the strip shows four.
- **Spec:** plan/30 §11.1: collapsed last **4** lines, expanded last **16**. Prefer scroll; else a clamped `visible_tail` window.
- **Impact:** `Ctrl+\`` to 280px (or a drag to 420) wastes the extra height. Users cannot read command output in the drawer they just opened.
- **Fix:** `let n = if self.workspace.bottom_open { 16 } else { 4 }; visible_tail(..., n)` and/or `overflow_y_scroll` on the log pane.

### F7. Missing toggle affordance

- **Severity:** Medium
- **Where:** `main.rs` `terminal_strip` (1526-1533); title bar (965-985); `controls.rs` TermStrip; `palette.rs`
- **Evidence:** Strip actions are Run and Clear only. No header chevron, no `` ` `` button, no title-bar layout reset that collapses the drawer (`reset_outlook_chrome` is specified in plan/30 §8 and is not in desktop). Clicking the draft focuses `Focus::Terminal` and does not expand (correct per §11.2). There is still no explicit on-screen toggle. Catalog and palette omit `toggle_bottom`.
- **Spec:** plan/30 §11.1 header: Clear + chevron -> `ToggleBottom`. §11.2: `Ctrl+\`` is the explicit gesture; do not auto-expand on focus.
- **Impact:** If the key is dead (F3), the drawer cannot be opened or closed at all. Users who do not memorize `Ctrl+\`` have no target to click.
- **Fix:** 28px header with Clear and a chevron/`\` control that dispatches `ToggleBottom`. Keep no auto-expand on draft focus.

---

## What already works

- `Workspace::{toggle_bottom,set_bottom_height,occupied_bottom}` and `ClientAction::ToggleBottom` are local and tested (`left_section_and_bottom_drawer`).
- `handle_key` does dispatch `ToggleBottom` on `` Ctrl+` `` when the keystroke is literally `` ` ``.
- `spawn_command` is off the UI thread (`mux-shell-cmd`); `pump` uses `try_recv`, not `recv`.
- Clear button, `clear`/`cls` builtin, draft + Enter / Run, `push_capped` 80-line history.
- Second in-flight command is refused instead of overlapping workers.

---

## FINDINGS

7
