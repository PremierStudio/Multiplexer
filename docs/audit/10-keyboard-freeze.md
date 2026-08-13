# 10. Keyboard, focus, UI-thread freeze

**Scope:** Desktop `handle_key`, `pump`, `send`, `refresh_worktrees`, `sample_cores`, `Server::with_local`, git/MCP inventory on the UI thread.
**Sources read:** `apps/multiplexer-desktop/src/{main.rs,controls.rs,inspector.rs}`, `crates/multiplexer-server/src/{server.rs,git.rs}`, `crates/multiplexer-worktree/src/git.rs`, `crates/multiplexer-resman/src/telemetry.rs`, `crates/multiplexer-mcp/src/{inventory.rs,skills.rs}`, `crates/multiplexer-client/src/{files.rs,turn.rs,command.rs}`, `crates/multiplexer-shell/src/{actions.rs,palette.rs,workspace.rs,approval_ui.rs,terminal_ui.rs}`, `crates/multiplexer-provider/src/grok.rs`, GPUI 0.2.2 `window.rs` / `windows/keyboard.rs` / `windows/events.rs`.
**Plans:** `plan/30-chrome-drawers.md` §11.2 / §12, `plan/16-performance.md` (input <16 ms p95), `plan/34` §6.3 RAF note.
**Method:** Read-only. No cargo.
**Verdict:** Keys that exist mostly work when GPUI delivers them to the root `on_key_down`. Focus is a three-state enum with no restore stack and no GPUI `FocusHandle`. Git worktree list, MCP config read, skills dirs, project tree, and `sysinfo` core samples all run on the UI thread. `request_animation_frame` is only armed while a background worker is live, so idle inspectors freeze.

---

## Findings

| ID | Severity | Title |
|---|---|---|
| F1 | High | `git worktree list` blocks the UI thread |
| F2 | High | MCP inventory, skills dirs, and project tree run on the UI thread |
| F3 | High | `sample_cores` constructs a new `sysinfo::System` on the UI thread |
| F4 | High | `request_animation_frame` only while a worker is pending |
| F5 | High | Advertised or expected keys do nothing |
| F6 | High | Focus is forgotten; help is not a trap |
| F7 | High | Tab is a two-way swap, swallowed in the palette |
| F8 | High | `Ctrl+\`` is half-wired |
| F9 | Medium | No GPUI `track_focus` / `FocusHandle` |

---

### F1. `git worktree list` blocks the UI thread

- **Severity:** High
- **Where:** `apps/multiplexer-desktop/src/main.rs` `ShellView::new` (131-147), `refresh_worktrees` (302-310), `refresh_reminder` (290-300), `pump` (583); `crates/multiplexer-server/src/server.rs` `Server::with_local` (54-64), `git_worktrees` (309-325); `crates/multiplexer-worktree/src/git.rs` `ProcessGit::run` (82-94)
- **Evidence:** `Server::with_local` installs `WorktreeService::new(ProcessGit::new())`. That catalog's `list_worktrees` is `git worktree list --porcelain` via `std::process::Command::output()` (spawn, wait, buffer stdout). Desktop calls that synchronously through `server.handle_frame(GIT_WORKTREES)`:
  1. `new()` then `refresh_worktrees()`
  2. `new()` then `refresh_reminder()` (a second identical spawn; only the second listed path is kept)
  3. Inspector / palette `RefreshGit`
  4. `pump` after every finished `pending_turn` (success, error, or empty stdout)
- `Command::output()` has no timeout. A hung `git` (lock, credential helper, huge repo) freezes the GPUI frame that called it. `grok -p` itself is off-thread (`spawn_grok_turn`); the post-turn refresh puts git back on the UI thread on the first paint after the worker returns.
- `Server::with_local` itself does not run git. It only wires the real runner. Every later `handle_frame` on `git.worktrees` is the freeze.
- **Spec:** plan/16 input latency <16 ms p95. `multiplexer-client` already documents the pattern: I/O off the UI thread (`turn.rs`, `command.rs`).
- **Impact:** Cold start pays two blocking git processes before the first frame is useful. Every turn hitchs the next keystroke. Refresh Git can freeze the window for the duration of git.
- **Fix:** Spawn `git.worktrees` like `spawn_command`. Cache the last list. Do not refresh on every turn unless the Git tab is visible. Merge reminder into the same list so startup is one call, not two.

---

### F2. MCP inventory, skills dirs, and project tree run on the UI thread

- **Severity:** High
- **Where:** `main.rs` `new` (88-124), `refresh_mcp` (278-288); `crates/multiplexer-mcp/src/inventory.rs` `load_user_mcp_inventory` (57-76); `crates/multiplexer-mcp/src/skills.rs` `list_dir_entry_names` (108-119); `crates/multiplexer-client/src/files.rs` `list_project_tree` (1-4, 51-68)
- **Evidence:** `new()` on the window-open path (GPUI `cx.new(|_| ShellView::new())`) does all of:
  - `load_user_mcp_inventory()`: `std::fs::read_to_string` of `%USERPROFILE%/.grok/config.toml`
  - `list_dir_entry_names` on user and project `.grok/skills` (`std::fs::read_dir`)
  - `list_project_tree(cwd, ListOptions::default())`: recursive `read_dir` depth 2, cap 80
- Inspector **Reload** on the MCP tab calls `refresh_mcp()` on the same thread (same `read_to_string`).
- The libraries already forbid this. `files.rs`: "Call off the UI thread." `skills.rs`: "Call this off the UI thread, then pass the names to `parse_skill_names`." Desktop ignores both comments.
- **Spec:** plan/16 cold start <300 ms and input <16 ms. plan/26 inventory is a host refresh, not a paint-time walk.
- **Impact:** First window frame waits on home-dir config, two skill directories, and a project walk. MCP Reload hitches typing. A missing file is cheap; a large or networked home/project is not.
- **Fix:** Move the three reads onto a worker (same `mpsc` + `pump` pattern as `pending_cmd`). Paint empty rows until the result arrives. Keep parse (`parse_mcp_inventory`, `parse_skill_names`) on the result, not the `read_dir`.

---

### F3. `sample_cores` constructs a new `sysinfo::System` on the UI thread

- **Severity:** High
- **Where:** `main.rs` `new` (80-87), `refresh_cores` (265-276), `pump` (549-561); `crates/multiplexer-resman/src/telemetry.rs` `sample_cores` (34-38)
- **Evidence:** `sample_cores` does `System::new(); sys.refresh_cpu_usage();` every call. Desktop calls it at startup, on inspector Reload, and from `pump` whenever `inspector == Resources` and `last_core_sample` is older than 1.5 s.
- A new `System` cannot compute a delta, so the first (and every) sample is often 0%. The function's own comment says the first call may report 0% and that the caller should reuse an interval, not sleep.
- `refresh_cores` samples `(0..8)` and forces `reserved || index < 2`. `pump` samples only `[0, 1]`. The Resources tab therefore shows a different core set depending on which path last ran.
- **Spec:** plan/16 resource monitor is a sidecar / off-paint sample. Inspector Reload may notify; it must not hitch the frame.
- **Impact:** Opening Cores, clicking Reload, or sitting on Cores during a live turn (F4 keeps frames coming) pays a sysinfo construct on the UI thread. Bars also lie (0% or a 2-core subset).
- **Fix:** Keep one `System` (or the resman sidecar) off the UI thread. Sample on a timer. `pump` only applies the last `Vec<CoreSample>`. Use one reserved-index policy.

---

### F4. `request_animation_frame` only while a worker is pending

- **Severity:** High
- **Where:** `main.rs` `pump` (548-641), `send` (437-466), `render` (835-837)
- **Evidence:** `pump` is called at the top of every `Render`. The only RAF arm is:

```
if self.pending_turn.is_some() || self.pending_cmd.is_some() {
    window.request_animation_frame();
}
```

  Missing cases:
  - Resources tab auto-sample (1.5 s). If no grok/shell worker is live, no RAF, so `last_core_sample.elapsed()` is only checked on the next user event. Cores freeze while idle.
  - `flash` (`copied last message`, `copied session id`) has no timer and never clears.
  - plan/34 §6.3 elapsed `Grok is working · Ns` needs a per-frame tick. RAF exists only for `pending_turn`, which is enough *after* the first post-send frame. `send()` itself does not request a frame; it relies on the click/key `cx.notify()`. That is OK for send. It is not OK for anything that must move while the user is idle.
  - After a turn completes, `pump` runs `refresh_worktrees()` (F1) on that frame, then drops RAF unless `pending_cmd` is also set.
- **Spec:** plan/34 §6.3: RAF while `pending_turn` is live is the working-label tick. Cores and flash need their own idle tick or they must not claim to update.
- **Impact:** The one place that pretends to be live telemetry is dead unless a child process is running. Status flash sticks until the next unrelated paint.
- **Fix:** RAF while `inspector == Resources`, while `flash` is `Some`, or while `workspace.busy`. Cap that loop (1.5 s cores, ~2 s flash). Do not RAF forever.

---

### F5. Advertised or expected keys do nothing

- **Severity:** High
- **Where:** `main.rs` `handle_key` (644-769), `palette_key` (785-815), `terminal_key` (817-832); `controls.rs` `SHORTCUTS` (186-199); `plan/30` §12; approval hints (1460-1465)
- **Evidence:** Global chords that fire: Esc, Ctrl+K/P, Ctrl+N, F1, Ctrl+[/], Ctrl+., Ctrl+S, Ctrl+V, and `Ctrl+\`` when the key is exactly `` ` ``. After that, unmatched keys either type a character or fall through.

  Dead or swallowed:

  | Key | What the UI implies | What happens |
  |---|---|---|
  | Ctrl+1..4 | plan/30 left sections Threads/Agents/Files/Activity | Not bound. In composer, `mods.control` skips `type_char`, so the digit is eaten. |
  | Ctrl+Shift+L | plan/30 reset Outlook chrome | Not bound. |
  | A / D | Approval card hints `"A"` / `"D"` | Types `a`/`d` into the composer. No `ClientAction::Approve`/`Deny` in `handle_key`. |
  | `?` | Palette hint for help | Types `?`. Help is F1 only. |
  | `g c` / `g m` / … | Palette hints | Not a chord parser. Types `g`. |
  | Ctrl+A/C/X/Z/Y/L/F | Standard editor | Swallowed (`!mods.control` guard). No select/copy/cut/undo. |
  | Up / Down | Composer history, palette already uses these | Composer: no-op. Terminal: no-op. |
  | Terminal Left/Right/Home/End/Delete | Draft editing | Only Backspace + type. Cursor keys do nothing. |
  | Palette Tab / Left / Right / Delete | Filter editing | `palette_key` ignores them; `cx.notify()` only. |
  | Palette Enter, empty filter | Run command | `active_item()` is `None`; palette stays open. |
  | Title-bar Idle / Play | Looks like Run | `term_meta("start a turn from the composer")` only. |

- Unbound Ctrl+letter is consumed, not passed through. The user cannot type those letters, and the chord does not run an action.
- **Spec:** plan/30 §12 (Ctrl+1..4 before the composer character path; ignore those digits in Terminal and palette). Approval card paints A/D. `controls.rs` catalog is the live chord list and omits all of the above except the existing 11 shortcuts.
- **Impact:** The help overlay and palette teach chords the window does not implement. Approval cannot be answered from the keyboard. Terminal draft is not editable except by appending and popping.
- **Fix:** Bind Ctrl+1..4 in the global branch (same place as Ctrl+N). Bind A/D only while `pending_approval()` is `Some`. Either implement the editor chords or do not swallow them. Add terminal Left/Right/Home/End/Delete.

---

### F6. Focus is forgotten; help is not a trap

- **Severity:** High
- **Where:** `main.rs` `Focus` (47-52), `dispatch` palette arms (179-194), Esc (647-664), `palette_key` Enter (786-793), palette row click (1614-1618), `help_overlay` (1626-1657), `handle_slash` `/term` (481-484)
- **Evidence:** Focus is `Composer | Terminal | Palette`. There is no previous-focus field.
  - Opening the palette sets `Focus::Palette`. Closing it (Esc, backdrop, Enter, row click, TogglePalette) always sets `Focus::Composer`, even if the user was in the terminal.
  - Esc with no overlay forces `Focus::Composer`. A terminal session is kicked back to the composer without a second Esc meaning.
  - Help is a full-window overlay but is not a focus mode. F1 and Esc toggle/close it. Every other key still goes to composer or terminal under the dimmer. Tab (F7) will flip those wells while help is on screen.
  - `/term` sets `Focus::Terminal` and does not open or expand the bottom drawer (`bottom_open` stays false). The user is typing into a 120px strip they did not ask to grow.
  - Clicking left-rail or inspector rows does not change `Focus`. The composer caret stays painted, so clicks look focused while the next key still edits the draft. Activity-row click is the exception (sets Terminal).
- **Spec:** Modal overlays (palette, help, approval) must own keys until dismissed. Restoring the previous well is the minimum after palette close.
- **Impact:** Terminal focus is easy to lose and hard to notice. Help looks modal and is not. Users type into the composer while reading the shortcut list.
- **Fix:** `prev_focus: Option<Focus>`. Palette open saves it; close restores it. Help is a fourth mode or a capture-phase filter that only allows Esc/F1. Approval A/D (F5) belong in that filter too.

---

### F7. Tab is a two-way swap, swallowed in the palette

- **Severity:** High
- **Where:** `main.rs` composer Tab (757-758), `terminal_key` Tab (822-823), `palette_key` (785-815); help copy (1654-1656); GPUI 0.2.2 `platform/windows/events.rs` (`VK_TAB => "tab"`)
- **Evidence:** GPUI on Windows reports the key as `"tab"`. Composer: `self.focus = Focus::Terminal`. Terminal: `self.focus = Focus::Composer`. That is the entire cycle.
  - No `Shift+Tab`. Shift+Tab is still `"tab"` and takes the same arm.
  - Palette: `palette_key` has no `tab` arm. Tab does not move the selection, does not insert a tab, and does not close the palette.
  - Help says "Tab focuses the terminal". It does not say Tab returns, and it does not mention inspector or left rail.
  - Tab does not expand the drawer (correct per plan/30 §11.2). Combined with F6 `/term` and a collapsed 120px strip, Tab can put keys into a well the user is not looking at.
  - There is no GPUI `tab_stop` chain (see F9). Rails, inspector rows, Send, and chips are mouse-only.
- **Spec:** plan/30 §11.2: focusing the draft must not auto-expand. It does not define Tab as the only focus tool, and it does not say Tab is a dead key in the palette.
- **Impact:** Keyboard users cannot reach chats, inspector, or palette rows. Palette Tab feels broken. Shift+Tab is not a reverse cycle.
- **Fix:** Keep Tab as composer <-> terminal only if that is the product, and document it. In the palette, Tab should `move_down` (Shift+Tab `move_up`). Do not send Tab to the composer while the palette or help is open.

---

### F8. `Ctrl+\`` is half-wired

- **Severity:** High
- **Where:** `main.rs` `handle_key` (696-698), `help_overlay` (1654-1656), `empty_center` (1724); `crates/multiplexer-shell/src/palette.rs` `default_items`; `controls.rs` `SHORTCUTS` / `REQUIRED_IDS`
- **Evidence:** Global (before palette/terminal routing): `if key == "\`" && mods.control { dispatch(ToggleBottom) }`. Headless `toggle_bottom` is implemented and tested.
  - Not bound: `oem_3`. plan/30 §11.2 says accept it and pin the live 0.2.2 key in a comment. No pin comment exists.
  - GPUI 0.2.2 Windows maps `VK_OEM_3` through `MapVirtualKeyW` to the layout character (`keyboard.rs` `get_key_from_vkey`). On US QWERTY that is `` ` ``. On other layouts `VK_OEM_3` is not grave, so the chord is dead. The plan's `oem_3` fallback is exactly for that uncertainty.
  - Palette has `run-terminal` and `term-tab`, not `{ id: "toggle-terminal", hint: "Ctrl+\`", action: ToggleBottom }`.
  - `controls.rs` has no `toggle_bottom` id and no `ctrl-\`` shortcut. TermStrip catalog is `term_run` / `term_clear` only.
  - Help overlay lists rails / stop / checkpoint / Tab / Esc. It does not list `Ctrl+\`` . Empty-center copy does.
  - `terminal_strip` has Run and Clear, no chevron. If the key name is wrong, ToggleBottom is unreachable from the keyboard and from the chrome.
- **Spec:** plan/30 §11.2, §12, §13 steps 5-6. Related drawer paint is `docs/audit/06-terminal.md` F3 / F7.
- **Impact:** Windows-first: a non-US layout, or a 0.2.2 build that emits `oem_3`, cannot toggle the drawer. Even on US QWERTY the gesture is undocumented in help and missing from the palette.
- **Fix:** Bind `` ` `` and `oem_3` (and log the live key once). Add palette `toggle-terminal`. Add `toggle_bottom` + `ctrl-\`` to `controls.rs` in the same change as the `REQUIRED_IDS` bump. Teach help the same line as empty-center.

---

### F9. No GPUI `track_focus` / `FocusHandle`

- **Severity:** Medium
- **Where:** `main.rs` `Render` root (838-848), `fn main` (2050-2072). Repo-wide `track_focus` / `FocusHandle`: none under `apps/`.
- **Evidence:** The window attaches `.on_key_down` to the root `div` and never calls `.track_focus`. `ShellView` holds a custom `Focus` enum only.
- GPUI 0.2.2 dispatches keys along the path from the root to the focused node (`window.rs` `dispatch_key_event` / `focus_node_id_in_rendered_frame`). With `focus == None` it falls back to `root_node_id()`, so today's root listener still fires. That is accidental, not designed.
- GPUI examples (`input.rs`, `tab_stop.rs`) require `track_focus` for IME, `tab_stop`, and `on_action` keymaps. This shell uses none of those. `handle_key` reads `event.keystroke.key`, not `key_char`, so composed input is out.
- The first child that later adds `track_focus` (real editor, Ghostty, a text input) will become the focused node. If it stops propagation, `handle_key` goes silent: every global chord in F5/F7/F8 dies.
- **Spec:** GPUI 0.2.2 `key_dispatch.rs` and `InteractiveElement::track_focus`. Product focus (F6) must sit on a real `FocusHandle` per well.
- **Impact:** Keys work only because nothing is focusable. Adding a real input will look like "keyboard broke." IME and OS tab cycle are unavailable.
- **Fix:** One `FocusHandle` for the window (or one per Composer / Terminal / Palette). `track_focus` on the root. `window.focus(&handle)` on open and on well click. Keep the enum as a projection of which handle is focused.

---

## What already works

- `grok -p` and `cmd.exe /C` are off the UI thread (`spawn_grok_turn`, `spawn_command`). `pump` uses `try_recv`, not `recv`.
- `CliGrokFactory::start` (used by `ensure_session` inside `send`) only stores cwd/program. It does not spawn. The turn spawn is the worker.
- Global chords that are implemented (Esc, Ctrl+K/P/N/[ / ]/. /S /V, F1, Shift+Enter, composer Backspace/Delete/Left/Right/Home/End, Ctrl+Backspace word delete) run before well routing when they should.
- `Ctrl+\`` is in the global branch, so it is not eaten by terminal or palette *when* the key is `` ` ``.
- Composer click and term-input click set the matching `Focus` and paint an accent border.
- GPUI reports Tab as `"tab"` on Windows (`VK_TAB => "tab"`), so the Tab arms are not a naming miss.

---

## FINDINGS

9
