# Audit 02: Left rail (icon rail + section lists)

**Surface:** `ShellView::left_rail`, `list_row`, `LeftSection`
**Spec:** `plan/30-chrome-drawers.md` §6, §7, §9, §12; `plan/32-list-rows.md` §4, §5.1
**Code:** `apps/multiplexer-desktop/src/main.rs` (`left_rail` ~988-1174, `list_row` ~1793-1855), `crates/multiplexer-shell/src/workspace.rs` (`LeftSection`, `agent_rows`, `select_file`, chrome widths)
**Helpers already in tree and unused by the left rail:** `Workspace::agent_rows`, `inspector_model::{file_rows, activity_rows, agent_rows}`, `Workspace::select_file`
**Method:** source read. No cargo.

## Verdict

The Outlook frame is present: four `LeftSection` icons, 44px collapsed width, list beside the icon column when open, Threads New/Del in the header, `SelectLeftSection` opens a closed rail. That is the scaffold.

The lists are not a control surface. Agents is a false empty state on every disconnected launch. Files and Activity clicks do not drive the workspace. Thread rows leak `thr-N`, have no hover delete, and `list_row` has no hover at all. The left list does not scroll. The 44px icon rail is a 36px hit inside a bordered 44px pane, with no tooltip and the wrong ids/glyphs.

What is already correct (do not "fix"):

- `LeftSection::all()` is `[Threads, Agents, Files, Activity]`.
- `rail_label` is Chats / Agents / Projects / Activity.
- `RAIL_COLLAPSED = 44.0`. `occupied_left()` is `left_width` when open, 44 when closed. The icon column sits *inside* the open pane.
- Only the active section's rows paint.
- `select_left_section` writes the enum only. `apply_layout_action(SelectLeftSection)` sets `left_open = true` when the rail was closed.
- Threads header still has `NewThread` / `DeleteThread`.

## Findings

| ID | Severity | Title |
|---|---|---|
| L01 | P0 | Agents list is a lie (ignores `agent_rows`, dead clicks) |
| L02 | P1 | Activity is blank or a mute log dump |
| L03 | P1 | Files clicks do not select, jump, or mark |
| L04 | P1 | Thread rows have no hover delete |
| L05 | P1 | Thread card chrome is inventory text (`status · id`) |
| L06 | P1 | Left list has no scroll region |
| L07 | P1 | `Ctrl+1..4`, palette, and catalog ids are missing |
| L08 | P2 | 44px icon rail clips, uses 36px hits, no tooltip |
| L09 | P2 | Collapse-open is implemented and untested; collapsed rail is opaque |
| L10 | P2 | Header and row spacing miss the 36 / 56 / 36 spec |
| L11 | P2 | Empty copy is wrong and the empty rows are clickable |

---

### L01. Agents list is a lie

- **severity:** P0
- **evidence:** `left_rail` Agents arm (`main.rs` ~1076-1111) reads `connection` session ids only. Default `Workspace` is `Disconnected`, so `sessions` is empty and the pane paints one row: title `No live session`, subtitle `Send a turn to start`. Click calls `term_meta("start a session from the composer")`. When session ids exist, every row is `selected: true` and click is `term_meta("session selected")`. Meanwhile `Workspace::agent_rows` (`workspace.rs` ~873-885) always maps `threads` (id, title, status, message count), and `inspector_model::agent_rows` (~264-280) already builds `agent:{id}` rows with `selected = i == ws.selected`. The left rail uses neither. Plan/30 §9.2: if session ids are empty, one row per thread; click a thread-backed row selects that thread; empty copy is `No sessions`.
- **problem:** Agents is a primary Outlook destination. On first launch it always looks empty even though a `New chat` thread exists. Connected rows cannot change selection. The section does nothing.
- **fix:** Paint `Workspace::agent_rows()` (promote the tuple to `AgentRow { id, title, status }`). If `Connected { session_ids }` is non-empty, those ids are display-only this sprint. Else each thread row click dispatches `SelectThread(i)`. Empty catalog: one muted, non-clickable `No sessions` line, not a fake `list_row`.
- **test:** `agent_rows_fall_back_to_threads_when_disconnected`

---

### L02. Activity is blank or a mute log dump

- **severity:** P1
- **evidence:** `left_rail` Activity arm (`main.rs` ~1150-1171) maps `terminal_log.rev().take(20).enumerate()` to `act-{i}` with empty subtitle/meta. If the log is empty, `children()` is an empty iterator: a blank pane. Click only sets `Focus::Terminal`. `inspector_model::activity_rows` already has `act:empty` (`No activity`), `act:status` (busy/idle badge), and `act:{original_index}`. Plan/30 §9.2: always paint `act:status` as `{connection.status_label()} · {busy? running : idle} · {threads.len()} chats`, then last 40 log lines; click jumps to the right Activity tab and expands that id; empty copy is `No activity`.
- **problem:** Two of four sections fail on a fresh workspace (Agents + Activity). Re-enumerating after `rev().take` makes `act-0` the newest line, not a stable log index, so a later right-rail jump cannot share ids. Click does not open the inspector.
- **fix:** Reuse `activity_rows` (or the same id rules). Always insert `act:status`. Empty: muted `No activity`, not clickable. Click: `select_inspector(Activity)`, `toggle_right_row(id)`, open the right rail. Cap 40, keep the source index in the id.
- **test:** `activity_rows_include_status_and_empty`

---

### L03. Files clicks do not select, jump, or mark

- **severity:** P1
- **evidence:** Left Files (`main.rs` ~1113-1148): empty row click sets `inspector = Files` and does not open the right rail; path click is `term_meta(&format!("file {p}"))`. Every path uses `ChromeGlyph::Folder`, full path as title, empty subtitle, `selected: false`. `Workspace::select_file` (`workspace.rs` ~850) and `selected_file` exist and are never called from GPUI. `inspector_model::file_rows` already uses `file:{path}` and a folder-vs-file glyph (`ends_with('/')`). `cycle_file` in `main.rs` ~402 still rotates the `Vec` (hostile to row identity, plan/32 §5.7). Plan/30 §9.2: click `select_inspector(Files)`, `toggle_right_row(format!("file:{path}"))`, open right. Plan/32: `select_file`, title is the last component, subtitle is the parent.
- **problem:** Projects looks like a file list and behaves like a log. Selection cannot stick. Empty state tells the user to reload a tab it does not open.
- **fix:** Click path: `select_file(p)`, `select_inspector(Files)`, `toggle_right_row(format!("file:{p}"))`, `chrome.right_open = true`. Title `short_path`, subtitle parent, folder/file glyph, selected wash from `selected_file`. Empty: muted `No files yet`, not clickable. Stop rotating `files` in `cycle_file`.
- **test:** `left_file_click_selects_and_jumps_inspector`

---

### L04. Thread rows have no hover delete

- **severity:** P1
- **evidence:** `list_row` (`main.rs` ~1793-1855) has no `.hover()`, no trailing action, no `hover_delete` flag. Threads pass click = `SelectThread(i)` only. Delete is the header `⌫` (`DeleteThread` on `ws.selected`). Plan/32 §4.1 Hover and §5.1: reveal a Delete chip; call `delete_thread(i)` (already refuses the last thread); keep header Del as delete-selected.
- **problem:** You cannot delete a non-selected chat from the row. The only delete target is whatever is selected, including when the pointer is over another card.
- **fix:** Extract `rows.rs` `RowChrome { hover_delete }` / `list_row`. On thread hover, fade in a Delete chip that calls `workspace.delete_thread(i)`. Header Del stays. Empty last-thread refuse is already tested headless.
- **test:** `thread_row_hover_delete_refuses_last`

---

### L05. Thread card chrome is inventory text

- **severity:** P1
- **evidence:** Threads arm (`main.rs` ~1063-1074) paints `ChromeGlyph::Chat` (`☰`) for every row, title, `thread_preview`, and meta `format!("{} · {}", t.status, t.id)`. `busy` is hard-coded `false`, so the running pulse never shows. `Thread` (`workspace.rs` ~23-28) is `{ id, title, messages, status }` with no `model`. Plan/32 §5.1: glyph is the first alphanumeric of the title (or `•`); model badge from `Thread.model` (else workspace model); pulse from `idle` / `running` / `error`; **do not** show raw `thr-N` on the card. Plan/30 §9.2: 56px card, status chip via Theme muted / accent / danger.
- **problem:** The list is a status dump. Ids belong in Session detail. There is no model badge and no running/error pulse, so a live turn is invisible in the rail that is supposed to scan chats.
- **fix:** Add `Thread.model`; `new_thread` copies `workspace.model`; `cycle_model` updates the selected thread. Project avatar glyph, model badge, status pulse. Drop id from the card.
- **test:** `new_thread_copies_workspace_model`

---

### L06. Left list has no scroll region

- **severity:** P1
- **evidence:** Open list (`main.rs` ~1036-1061) is `div().flex_1().flex().flex_col().min_w_0()` with the 36px-ish header as the first child, then rows appended as siblings. There is no `.id("left-list")`, no inner `flex_1().min_h_0()`, no `overflow_y_scroll`. A repo search of `apps/multiplexer-desktop` finds no `overflow_y` / `overflow_x` at all. Plan/30 §9.2 and §14.2: `div().id("left-list").flex_1().min_h_0().overflow_y_scroll()` then one child per row. Fallback if GPUI 0.2.2 lacks overflow: a clamped visible window (Activity already fakes this with `take(20)`; Threads/Files do not).
- **problem:** More threads or files than the pane height clip. The user cannot reach them. Header New/Del stay put only if the header is *outside* the scroller; today everything is one column, so the header can scroll away or the rows clip under the bottom drawer.
- **fix:** Keep the uppercase header + actions outside the scroller. Wrap rows in `id("left-list")` + `flex_1` + `min_h_0` + `overflow_y_scroll`. If overflow is missing on 0.2.2, paint a `visible_tail`-style window and pin that in a comment.
- **test:** `left_list_is_named_scroll_region` (projection helper or component)

---

### L07. `Ctrl+1..4`, palette, and catalog ids are missing

- **severity:** P1
- **evidence:** `handle_key` (`main.rs` ~644-769) binds `Ctrl+N`, `Ctrl+[`, `Ctrl+]`, `Ctrl+\``, not digits 1-4. After the global chords, Terminal and palette swallow keys, then a single character goes to the composer. `palette.rs` `default_items` has no `SelectLeftSection` rows. `controls.rs` `REQUIRED_IDS` / `SHORTCUTS` have `new_thread`, `select_thread`, `delete_thread` only; plan/30 §12 required `left_section_threads|agents|files|activity`. Help overlay (~1654) still says `Ctrl+[ / ] rails` and never mentions `Ctrl+1..4`.
- **problem:** The icon rail is mouse-only. Keyboard-first (plan/10, plan/30 acceptance item 5) cannot switch Outlook sections. Digits would also type into the composer if bound in the wrong branch.
- **fix:** In the global branch, before the composer path, map `Ctrl+1..4` to `SelectLeftSection` (Threads, Agents, Files, Activity). Ignore when `Focus::Terminal` or the palette is open. Add palette items and catalog ids; update `REQUIRED_IDS` in the same change. Help line: `Ctrl+1..4 left sections`.
- **test:** `ctrl_1_to_4_select_left_section`

---

### L08. 44px icon rail clips, uses 36px hits, no tooltip

- **severity:** P2
- **evidence:** Collapsed and open icon column is `w(px(44.0))` with children `h(px(36.0)).mx_1()` (`main.rs` ~1000-1031). `glass_pane()` (~1688) adds `border_1` + `rounded(Theme::panel_radius())` (12px) + shadow, then `.w(px(occupied_left()))`. When closed, occupied width is 44 *including* the 2px of border, so the inner box is 42px and a 36+4+4 icon overflows. Plan/30 §6 / §9.1: 44px column = 32px glyph + 6px pad each side; 32x32 hit; tooltip / id `left-rail-threads` etc.; `pt_2`, `gap_1`, `justify_start`. Actual ids are `rail_label()` (`Chats`, `Agents`, `Projects`, `Activity`). Glyphs are `☰ ⚡ ▤ ●` (`LeftSection::glyph`, `workspace.rs` ~124-130), not plan/30 `💬 ⚡ 🗂 ⏱`. No `.hover()` on the icons (`icon_btn` already has hover; these divs do not).
- **problem:** The collapsed strip is the only navigator when the list is hidden. Icons clip, the selected wash sits 4px from the glass edge, and there is no tooltip, so `☰` and `▤` are unnamed. 12px radius on a 44px pane reads as a pill, not a rail.
- **fix:** Keep `RAIL_COLLAPSED = 44` as the *content* column (32 hit + 6 pad). Do not let `glass_pane` border eat that 44 (border on the outer pane after the column is measured, or size the pane to 44 + borders). Ids: `left-rail-threads` … `left-rail-activity`. Tooltip = `rail_label()`. Hover wash. Either ship plan/30 glyphs or lock ChromeGlyph marks in a test so they stop drifting.
- **test:** `left_rail_icon_ids_and_hit_are_32_in_44`

---

### L09. Collapse-open is implemented and untested; collapsed rail is opaque

- **severity:** P2
- **evidence:** `apply_layout_action` (`actions.rs` ~60-67) sets `left_open = true` when `SelectLeftSection` fires on a closed rail, even if the section did not change. `select_left_section` itself does not touch chrome (`workspace.rs` ~729). Collapsed paint returns `rail.child(icons)` only (`main.rs` ~1033-1034). There is no test named `select_left_section_action_opens_closed_rail` (plan/30 §5.6). `left_section_and_bottom_drawer` never asserts `chrome.left_open` is unchanged by the headless method (`select_left_section_does_not_toggle_chrome` is also missing as a named test). Combined with L08 (no tooltip), a collapsed rail is four mystery marks.
- **problem:** The important collapse contract (icon rail is a real navigator, not a no-op) can regress without a red test. Users who collapse with `Ctrl+[` cannot read the remaining 44px.
- **fix:** Add the two plan/30 tests. Pair with L08 tooltips so the collapsed rail is labeled.
- **test:** `select_left_section_action_opens_closed_rail`

---

### L10. Header and row spacing miss the 36 / 56 / 36 spec

- **severity:** P2
- **evidence:** Section header (`main.rs` ~1037-1060) is `px_3 py_2` with two `icon_btn` children. `icon_btn` is 32x32 with a border (`~1759-1760`). Plan/30 §6 / §9.2: header 36px; Threads actions `+` and `⌫`; row height 56 (threads with preview) / 36 (files, activity). Plan/32 §4.1: 8px vertical pad, 12px horizontal, 4px gap, collapsed ~44px. Actual `list_row` is `mx_2 mb_1 px_3 py_2 rounded_xl` with one, two, or three text lines and no fixed height. Files/Activity often render as a single title line (weaker than 36px spec, not denser on purpose).
- **problem:** The THREADS header is taller than the spec (32px buttons + 16px padding) and crowds the first card. Thread cards are an unbounded stack of title + preview + `status · id`. Files and Activity do not share a 36px rhythm, so switching sections jumps the scan line.
- **fix:** Header `h(36)` with 24-28px ghost actions (or a slimmer `ghost_btn`, not 32px `icon_btn`). Threads `h(56)`, files/activity `h(36)`, `mb_1` gap. Do not put three text lines in a 36px row.
- **test:** `left_row_heights_match_plan30` (constants on the row spec)

---

### L11. Empty copy is wrong and the empty rows are clickable

- **severity:** P2
- **evidence:** Agents empty: `No live session` / `Send a turn to start` (clickable). Files empty: `No files listed` / `Reload from the Files tab` (clickable, sets inspector, does not open the rail). Activity empty: no row (L02). Plan/30 §9.2 empty strings: Agents `No sessions`, Files `No files yet`, Activity `No activity`. Plan/32 §4.1 / §4.2: empty catalog is one muted line, not a fake row, not clickable.
- **problem:** Empty states look like real items. They steal a click and, for Files, only half-navigate. Copy does not match the plan strings tests should pin.
- **fix:** One muted, non-interactive line per empty catalog. Pin the three plan/30 strings. Threads stay never-empty (`new` keeps one).
- **test:** `left_empty_copy_matches_plan30`

---

## Out of scope here

Title toolbar (`≡` / project / branch) is the title-bar audit. Right inspector lists are the right-rail audit. `InspectorTab::Agents` (10th right tab) is not the left Agents section. Plan/35 48px rail is a later visual bar, not this sprint (plan/30 locked 44).

## FINDINGS: 11 P0: 1 P1: 6 P2: 4
