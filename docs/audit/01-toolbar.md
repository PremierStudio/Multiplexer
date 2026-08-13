# Audit: desktop title toolbar and native caption

Scope: `apps/multiplexer-desktop/src/main.rs` (`title_bar`, `WindowOptions`), helpers `icon_btn` / `pill` / `glass_bar`, `controls.rs` TitleBar catalog, `crates/multiplexer-shell` title/branch APIs, checked against `plan/30-chrome-drawers.md` and `plan/35-competitor-visual-bar.md`. Caption contract also read from `plan/28-glass-windows.md` (do not flip `appears_transparent` without `WindowControlArea`).

---

### TB-01
- severity: P1
- evidence: apps/multiplexer-desktop/src/main.rs:957
- problem: The idle title-bar Play control is a dead button. When `workspace.busy` is false it paints `ChromeGlyph::Play` with hint `Idle`, then only calls `term_meta("start a turn from the composer")`. It does not dispatch `ClientAction::Send`, does not call `send()`, and does not disable when the composer draft is empty. plan/30 §8 requires this slot to be Run (send draft, same as composer Enter) with a disabled look when draft is empty. Users who click Play believe a turn started.
- fix: apps/multiplexer-desktop/src/main.rs + `title_bar_run_dispatches_send_when_idle`

### TB-02
- severity: P1
- evidence: apps/multiplexer-desktop/src/main.rs:912
- problem: The chats toggle steals `ChromeGlyph::Layout` (`▦`). plan/30 §7/§8 requires `≡` for ToggleLeft and a separate Layout control (`▦`) that calls `Workspace::reset_outlook_chrome()` (Ctrl+Shift+L). That method does not exist on `Workspace`. There is no layout button in the right cluster, no `layout_reset` catalog id, and no palette/key binding. Clicking the left-most mark looks like a layout reset and instead toggles the chats rail.
- fix: crates/multiplexer-shell/src/workspace.rs + `reset_outlook_chrome_reopens_rails_and_collapses_bottom`

### TB-03
- severity: P1
- evidence: apps/multiplexer-desktop/src/main.rs:922
- problem: The branch pill does not show a git branch. It reads `workspace.reminder.0`, else the literal `main`. `refresh_reminder` writes `set_reminder("existing", path)` whenever a second worktree exists, so the pill often says `existing`. Dismissing the reminder bar (same `reminder` field) snaps the pill to `main` even when `git_status` is empty or later set (desktop does call `set_git_status` on git output at main.rs:631). `Workspace::branch_label()` from plan/30 §8 is missing. The toolbar invents a branch.
- fix: crates/multiplexer-shell/src/workspace.rs + `branch_label_uses_git_status_else_no_branch`

### TB-04
- severity: P1
- evidence: apps/multiplexer-desktop/src/controls.rs:115
- problem: TitleBar is pinned at five ids (`chats_toggle`, `inspector_toggle`, `stop`, `command_palette`, `help`) and `surfaces_nonempty` asserts `controls_on(TitleBar).len() == 5`. plan/30 §12 requires TitleBar to also catalog `run`, `layout_reset`, `project_pill`, `branch_pill`, and `model_pill`. The painted model pill dispatches `CycleModel` but the catalog still parks `cycle_model` on RightRail only. The headless catalog no longer matches the painted toolbar, so `no_dead_labels` cannot catch the dead Play/pills.
- fix: apps/multiplexer-desktop/src/controls.rs + `title_bar_catalog_includes_run_layout_and_pills`

### TB-05
- severity: P1
- evidence: apps/multiplexer-desktop/src/main.rs:918
- problem: Project and branch look like controls (28px bordered chips) and have no click handler, no `SharedString` id, and no catalog entry. plan/30 allows project click to be a host hint or copy-path this sprint, but it still requires live catalog ids. As painted they are inert chrome that the user will try to open. Branch has no picker and no jump to the Git inspector tab.
- fix: apps/multiplexer-desktop/src/main.rs + `project_and_branch_pills_are_live_title_bar_controls`

### TB-06
- severity: P1
- evidence: apps/multiplexer-desktop/src/main.rs:2061
- problem: Native caption is correctly `appears_transparent: false` with title `Multiplexer` (plan/28 §2, plan/30 §2). plan/35 §3.2 and §4 item 4 tell the parent to flip `appears_transparent: true`, draw a 48px glass caption, and reserve 140px on the right for system min/max/close. The live toolbar has no 140px trailing reserve, no drag region on the `flex_1` spacer, and no `WindowControlArea` hit-testing. Applying plan/35 on this code hides the OS caption and leaves move / minimize / maximize / close dead (the caption bug plan/28 §1 exists to prevent).
- fix: apps/multiplexer-desktop/src/main.rs + `window_options_keep_native_caption`

### TB-07
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:946
- problem: Stop is a plain `icon_btn` with default `glass_ultra` fill, not `Theme::danger()`. `left_on` / `right_on` are computed only for hint strings. They never apply the selected 32x32 accent wash plan/30 §8 and plan/35 §4 item 6 require. Idle Play stays visible (plan/35 item 15: Stop only when busy). The run/stop slot does not look like a primary or danger control.
- fix: apps/multiplexer-desktop/src/main.rs + `title_stop_uses_danger_and_toggles_use_accent_wash`

### TB-08
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:1749
- problem: Toolbar marks are mismatched Unicode, and hints never paint. Chats uses Layout `▦`, inspector uses Settings `⚙` (reads as Settings, not `⋮`), palette uses `⌘` instead of `ChromeGlyph::Search` (`⌕`), help is a raw `"?"` string. `icon_btn` takes `hint` only to build `id("icon-{hint}")`. There is no tooltip, no `aria` text, and `ButtonSpec::icon` in `crates/multiplexer-shell/src/widgets.rs` is unused. The bar is a row of unlabeled box-drawing characters.
- fix: crates/multiplexer-shell/src/icons.rs + `title_bar_glyphs_match_plan30_table`

### TB-09
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:1776
- problem: `pill()` is 28px tall, `rounded_lg` (8px), muted text, `glass_ultra` fill. plan/35 §4 items 10 and 15 require 20px stadium pills (radius 999, 11px type, pad 8). The model pill is not an accent-wash chip and has no tooltip listing `workspace.models`. Root chrome is `.text_sm()` (main.rs:845). `TypeScale::UI` is 13px in `multiplexer-theme` and is never applied on the toolbar. This is the 16px-ish web header look plan/35 forbids.
- fix: apps/multiplexer-desktop/src/theme.rs + `title_pill_is_20px_and_chrome_type_is_ui_13`

### TB-10
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:906
- problem: Layout is weak. `glass_bar()` uses `gap_3`; `title_bar` adds `px_3` and height 44. plan/30 §8 asks `px_2`, `gap_2`, `items_center`. Pills have no `min_w_0`, no max width, and no ellipsis, so a long project or model name shoves Palette / Help / inspector off the right edge. The `flex_1` spacer is an empty `div` with no id. Density is a loose prototype row, not a compact 4px-grid tool strip.
- fix: apps/multiplexer-desktop/src/main.rs + `title_bar_uses_gap2_px2_and_clamps_pill_width`

### TB-11
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:2062
- problem: The OS caption title is a hardcoded `"Multiplexer".into()`. It never tracks project, model, or connection. `Workspace::title_bar()` already formats `Multiplexer  ·  {project}  ·  {model}  ·  {status}` (workspace.rs:305) and `DEFAULT_WINDOW_TITLE` exists on `multiplexer_shell`, but `main()` uses neither. Alt-Tab, the taskbar, and the native caption stay generic while the in-window pills try to carry context. `DesktopChrome.title` is also unused by the desktop binary.
- fix: apps/multiplexer-desktop/src/main.rs + `caption_title_tracks_workspace_title_bar`

### TB-12
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:2054
- problem: `WindowOptions` is inlined in `main()`. plan/28 §2 and §8 require an extracted `window_options(bounds)` so tests can lock `appears_transparent == false`, `titlebar.is_some()`, `window_background == Blurred`, and movable/minimizable/resizable without opening an HWND. There is no `window_options_keep_native_caption` test. A one-line flip of `appears_transparent` or `titlebar: None` (GPUI Windows treats None as hide caption) has no regression tripwire.
- fix: apps/multiplexer-desktop/src/main.rs + `window_options_keep_native_caption`

---

FINDINGS: 12  P0:0 P1:6 P2:6
