# Audit 07: Command palette overlay

**Surface:** `ShellView::palette_overlay`, `palette_key`, `insert_text` (palette branch)
**Spec:** `plan/36-feature-gap-ui.md` §4.11 (wave 11), §6 Palette row, test #11; `plan/10-ui-pane-system.md` §6; `plan/28-glass-windows.md` §6.8; `plan/35-competitor-visual-bar.md` §2.4 / §2.5
**Code:** `apps/multiplexer-desktop/src/main.rs`, `crates/multiplexer-shell/src/palette.rs`
**Helpers:** `filter_items`, `default_items`, `PaletteItem`, `PaletteState`. Adjacent and unused by the overlay: `crates/multiplexer-shell/src/search.rs` (`search_workspace`)
**Method:** source read of `palette_overlay`, `palette_key`, `palette.rs`, and plan/36 wave 11. No cargo.

## Verdict

The palette is still the command runner plan/36 already named: a static `'static` vec, case-insensitive substring, no namespaces. Wave 11 did not land. `filter_items` still takes only `query`. There is no `PaletteHit`, no `PaletteNs`, no `filter_items(ws, q)`, and no test `palette_filter_includes_files_and_threads`.

The overlay paints a dim + 520px card (plan/28 geometry is present). The query well is a label, not an input. The dim is not occluded, so mouse hits reach chrome under the glass. Rows are `label   hint` with no `cmd` / `pane` / `file` / `thread` prefix. Workspace already holds `files` and `threads` (and `search_workspace` already walks both). The palette never reads them.

## Findings

| ID | Severity | Title |
|---|---|---|
| C07-01 | High | Static catalog |
| C07-02 | High | No files or threads |
| C07-03 | High | No fuzzy |
| C07-04 | High | Overlay click-through |
| C07-05 | High | Query field is fake |
| C07-06 | High | Missing namespaces |
| C07-07 | Medium | List capped at 12, no scroll |

---

### C07-01: Static catalog

- **Severity:** High
- **Spec:** plan/36 §4.11: "Dynamic rows are rebuilt from `Workspace` on each query. Static catalog stays the command source." Headless: `filter_items(ws, query) -> Vec<PaletteHit>`. §6: catalog also adds create worktree, settings, search, pop out, show usage, start/stop MCP.
- **Code:** `crates/multiplexer-shell/src/palette.rs` `default_items()` (19-189), `filter_items` (195-209). Overlay calls `multiplexer_shell::filter_items(&self.palette.query)` at `main.rs` 1695.
- **Expected:** `default_items()` remains the command list. `filter_items` takes `&Workspace` and rebuilds file / thread hits from `ws.files` / `ws.threads` every keystroke. New static rows for settings, search, pop out, worktree, usage, start/stop MCP.
- **Actual:** `filter_items` ignores the workspace. It clones `default_items()` (28 `'static` rows) and substring-filters. `PaletteItem` is `Copy` with `&'static str` fields only. `ClientAction::ToggleSettings`, `StartMcp`, `StopMcp` exist and are not in the catalog. No Search / PopOut / CreateWorktree / ShowUsage rows. Module comment still claims InspectorTab has no Git / Terminal / Skills variants; those tabs already have SelectTab rows. Files / Activity / Agents tabs exist on `InspectorTab` and have no palette rows.
- **Impact:** The palette cannot grow with the session. New inspector tabs and wave-11 actions are unreachable from Ctrl+K.
- **Headless gap:** no `PaletteHit`. Signature is still `filter_items(query: &str)`.

---

### C07-02: No files or threads

- **Severity:** High
- **Spec:** plan/36 §4.11 user-visible: Files from the tree, Threads from the rail. Enter on a file runs `SelectFile` and opens the Files tab. Enter on a thread runs `SelectThread`. Test `palette_filter_includes_files_and_threads`: workspace with file `src/lib.rs` and thread title "Fix MCP"; query `lib` is a file hit; query `fix` is a thread hit; query `mcp` is the MCP command **and** the thread; empty query is commands + panes, not every file. plan/10 §6.1 Files and Agents / threads namespaces.
- **Code:** `filter_items` (palette.rs 195-209) never mentions `Workspace`. `PaletteState::active_item` (273-280) calls the same. Overlay maps only those items. `ClientAction::SelectThread(usize)` exists; `SelectFile` does not. Desktop already fills `workspace.files` via `list_project_tree` (`main.rs` 97-108) and keeps `workspace.threads`.
- **Expected:** Dynamic rows owned as `String` labels plus a `ClientAction`. Empty query does not dump the tree.
- **Actual:** No file rows, no thread rows. Jumping to a chat is left-rail only. Opening a path is `CycleFile` / inspector dump, not a palette hit. `search_workspace` in `crates/multiplexer-shell/src/search.rs` already matches threads, files, and `default_items()` by substring, and the overlay does not call it. Gate test `palette_filter_includes_files_and_threads` is not in the tree (only named in plan/36 §7).
- **Impact:** Ctrl+K cannot jump to a conversation or a project file. Wave 11's named test would fail to compile against today's API.

---

### C07-03: No fuzzy

- **Severity:** High
- **Spec:** plan/10 §6.2 fuzzy rank across namespaces. plan/36 §4.11: "substring still works; add a simple fuzzy (contiguous subsequence) so `mcp` still hits MCP and `wt` can hit Create worktree". plan/35: Warp / Linear overlay is a fuzzy list.
- **Code:** `filter_items` (palette.rs 192-209): `query.to_lowercase()` then `id` / `label` / `hint` `.contains(&needle)`. Tests `filter_narrows` and `filter_matches_id_label_or_hint` pin substring only (`"NEW-CHAT"`, `"ctrl+n"`, `"toggle inspector"`).
- **Expected:** Contiguous subsequence (and keep substring). Ranked results. `wt` hits a worktree command once that row exists.
- **Actual:** `"wt"` does not match `"Create worktree"` even if the row is added. `"chk"` does not match `"Create checkpoint"` (`checkpoint`). No score, no sort. Empty query returns the catalog in declaration order.
- **Impact:** Typo-tolerant and acronym queries fail. The filter is a command grep, not a palette.

---

### C07-04: Overlay click-through

- **Severity:** High
- **Spec:** plan/28 §6.8: dim is absolute, full window; click dim to close; card is the glass overlay. plan/35: dimmed canvas. GPUI overlays that cover chrome must occlude hit testing so siblings under the dim do not receive the same mouse-down.
- **Code:** `palette_overlay` (`main.rs` 1694-1760). Dim: `id("palette")`, `absolute`, `size_full`, `bg(...)`, `on_mouse_down` -> `ClosePalette` (1707-1712). Card: `on_mouse_down(MouseButton::Left, |_, _, _| {})` (1722). Rows dispatch the item (1749-1756). Render attaches the overlay as a later sibling of title bar, rails, center, terminal (`main.rs` 936-937). No `.occlude()` anywhere in `apps/multiplexer-desktop`. Card handler never calls `stop_propagation`. Query well has no mouse handler of its own. Help and settings overlays copy the same dim pattern.
- **Expected:** Dim click closes. Card / query / row clicks stay inside the overlay. Chrome under the glass (thread rows, composer, title buttons) does not fire.
- **Actual:** Empty card closure does not consume the event. Without `occlude`, GPUI still hit-tests the rails and composer under the dim. A click on the card can close the palette (bubble to dim) and also activate whatever is painted underneath. A click on a row can run the command **and** select a thread or focus the composer.
- **Impact:** The overlay is not a modal. Mouse use is racy with the chrome it is supposed to cover.

---

### C07-05: Query field is fake

- **Severity:** High
- **Spec:** plan/36 §4.11: query drives `filter_items(ws, query)`. plan/28 §6.8: query well is a real well (low-alpha fill). `controls.rs` catalogs `palette_filter` on `Surface::Palette`. Composer already shows a caret via `draft_display`.
- **Code:** Query well (`main.rs` 1723-1735) is a `div` whose only child is `SharedString`: placeholder `"Type to filter commands…"` or `self.palette.query.clone()`. No `.id("palette_filter")`. No caret, no `cursor` index on `PaletteState` (palette.rs 213-217: `open`, `query`, `selected` only). `palette_key` (829-858) appends single chars, space, and `pop` on backspace. `insert_text` (820-825) appends paste when `palette.open`. Arrows move the row, not a caret. No Delete, Home, End, word-delete. Clicking the well has no listener (C07-04).
- **Expected:** A focusable filter field with a caret, click-to-focus, and the `palette_filter` id. Typing filters. Clicking the well does not dismiss.
- **Actual:** The well is a painted label. Insertion point is invisible. Clicking it can dismiss via the dim (C07-04). Catalog id `palette_filter` is not on any element. `palette_run` is not on the rows (`pal-{i}` only).
- **Impact:** Users cannot see or edit the query like a field. Mouse users have no honest target. Screen / test automation cannot find `palette_filter`.

---

### C07-06: Missing namespaces

- **Severity:** High
- **Spec:** plan/10 §6.1 four namespaces: Commands, Panes, Files, Agents / threads. plan/36 §4.11: results grouped under those headings; each row shows prefix `cmd`, `pane`, `file`, `thread` plus the existing hint. plan/35: shortcut hint on the right, muted 11px.
- **Code:** `PaletteItem` (palette.rs 11-16) is `id`, `label`, `hint`, `action`. No `namespace` / `PaletteNs`. Overlay row (`main.rs` 1758): `format!("{}   {}", item.label, item.hint)`. No group headers. Inspector tabs are mixed into the same flat list as Send / Stop / Approve.
- **Expected:** Grouped lists. Prefix on every row. Files and threads in their own namespaces (C07-02). Panes include inspector tabs plus pop-out, settings, search.
- **Actual:** One undifferentiated list. No prefix. Hint is concatenated onto the label, not a right-aligned muted caption. Settings exists as `ToggleSettings` / `settings_overlay` and is not a pane row. Search is a separate `search_workspace` helper with no overlay and no palette item.
- **Impact:** Cannot tell a command from a pane. Cannot scan by namespace. The product promise of four searchable spaces is a single command list.

---

### C07-07: List capped at 12, no scroll

- **Severity:** Medium
- **Spec:** plan/35 §2.4: 12 listed rows as the visible window, not a hard catalog cap. plan/10 §6.2: virtualize file / agent results; keyboard up/down / Enter / Esc still reach every hit. plan/36 empty query returns commands + panes (already more than 12).
- **Code:** `main.rs` 1736: `items.into_iter().enumerate().take(12)`. `PaletteState::move_down` / `move_up` wrap over `filter_items(...).len()` (full 28, or the filtered set). No `overflow_y_scroll` on the card. Selected index can be 12..n-1 while only `i < 12` is painted.
- **Expected:** At most 12 **visible** rows, scrolled so the selected row is on screen. Or a virtualized window around `selected`.
- **Actual:** Items 13+ are unreachable by mouse and invisible when selected by keyboard. Empty query shows the first 12 of 28 (New chat through Session) and hides Refresh MCP, Help, Delete thread, Approve, and the rest.
- **Impact:** Keyboard selection and what is on screen diverge. Half the static catalog is hidden until the user types a substring that happens to land in the first 12 matches.

---

## Headless contract (blocking wave 11)

plan/36 §4.11 / §6 / §7 test 11 require:

- `PaletteNs` (or equivalent) and `PaletteHit` with owned `String` labels
- `filter_items(ws, query) -> Vec<PaletteHit>`
- `PaletteState::active_item` on that hit list
- Gate test `palette_filter_includes_files_and_threads`

None of those names exist. `search_workspace` is a parallel substring index (wave 8 shape) and is not the palette filter. `ClientAction::SelectFile` is still missing; `InsertFileMention` / `CycleFile` / `selected_file` are not a substitute.

Desktop `controls.rs` still lists only `palette_filter` and `palette_run` on `Surface::Palette`. Overlay ids are `palette` and `pal-{i}`. plan/28 also asked for `palette-card`; that id is absent.

## Out of scope for this file

Settings overlay (`settings_overlay`, F2 / `ToggleSettings`) is a different surface. Search overlay (`Ctrl+Shift+F`) is wave 8. Both are missing from the **palette catalog**, which is in scope above. Help overlay shares the click-through pattern; it is not the palette.

## FINDINGS: 7
