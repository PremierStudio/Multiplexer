# 12. Workspace model audit

**Scope:** `crates/multiplexer-shell` `Workspace` vs what the desktop paints and mutates.
**Read:** `crates/multiplexer-shell/src/workspace.rs`, `actions.rs`, `bindings.rs`.
**Callers checked (read-only):** `apps/multiplexer-desktop/src/main.rs`, `inspector.rs`; `crates/multiplexer-shell/src/inspector_model.rs`, `palette.rs`, `slash.rs`, `approval_ui.rs`, `integrations.rs`.
**Rule:** source only. No cargo.

FINDINGS: 16

---

## F01 `selected_worktree` is write-never

- **Severity:** P1
- **Kind:** unused field / missing mutation
- **Evidence:**
  - Field: `crates/multiplexer-shell/src/workspace.rs:257`, init `None` at `:294`.
  - Read: `git_detail` star mark (`:676`) and `git_rows` `row.selected` (`inspector_model.rs:132`).
  - Write: only the unit test `git_terminal_skills_detail_copy` (`workspace.rs:1250`). Desktop never assigns it.
  - There is no `select_worktree` method and no `ClientAction::SelectWorktree`.
  - Inspector row click only calls `toggle_right_row` (`main.rs:1884`).
- **Gap:** Git rows can never show a selected worktree in the live app. `RefreshGit` replaces `worktrees` and leaves the index stale if anyone ever set it.
- **Fix:** Add `Workspace::select_worktree(Option<usize>) -> bool` and a local `ClientAction::SelectWorktree(usize)`. Wire Git row click to that instead of (or before) accordion expand. Clear on `refresh_worktrees`.

## F02 Composer caret is a `|` splice, hidden when the draft is empty

- **Severity:** P1
- **Kind:** paint gap
- **Evidence:**
  - Model: `Workspace.cursor` (`workspace.rs:248`) is a char index. Mutators exist (`set_draft`, `type_char`, `backspace`, `send_draft`, `move_cursor_*`).
  - Paint: `center()` builds `draft_display(&draft, cursor, focus == Composer)` (`main.rs:1268-1272`) then throws it away when `draft.is_empty()` and paints a placeholder instead (`:1393-1399`).
  - `draft_display` (`main.rs:1958-1974`) inserts a `|` rune at the char index. No I-beam, no blink, no click-to-place.
- **Gap:** An empty focused composer shows no caret. A non-empty draft shows a character, not a caret. Mouse down on `#composer` only sets `Focus::Composer`. The field is live for keys, half-shown for eyes.
- **Fix:** Always run `draft_display` (or a GPUI caret) when composer-focused, including empty draft. Stop encoding the caret as text if a real cursor style is available.

## F03 `term_draft` has no cursor

- **Severity:** P1
- **Kind:** half field / missing mutation
- **Evidence:**
  - Field: `workspace.rs:253`. Edits are append/pop only: `type_term_char`, `backspace_term`, `set_term_draft`, `take_term_draft` (`:537-560`).
  - Desktop: `terminal_key` (`main.rs:817-831`) never left/right/home/end. Paste is `term_draft.push_str` (`:773`).
  - Paint: `terminal_strip` prints `{TERM_PROMPT} {term_draft}` with no caret (`:1475-1478`).
- **Gap:** Terminal draft is a stack, not a cursor-aware buffer. Mid-line edit is impossible. Contrast the composer, which already has `cursor`.
- **Fix:** Either add `term_cursor: usize` and reuse `composer` helpers, or document Term as append-only and stop implying it is an input field.

## F04 `Workspace::move_cursor_*` is unused by the desktop

- **Severity:** P2
- **Kind:** unused method / dual API
- **Evidence:**
  - Methods: `move_cursor_left/right/home/end` (`workspace.rs:388-409`).
  - Callers: only `cursor_insert_and_backspace_middle` (`:1146-1177`).
  - Desktop keys call `move_left` / `move_right` / `move_home` / `move_end` from `composer.rs` and assign `self.workspace.cursor` (`main.rs:741-756`).
- **Gap:** Two cursor APIs. Tests cover the unused wrappers. Desktop can drift (word-kill already inlines its own range math at `:725-734`).
- **Fix:** Make desktop call the `Workspace` methods, or delete the wrappers and keep `composer` helpers as the single surface.

## F05 Inspector `Files` / `Activity` are tabs without actions

- **Severity:** P1
- **Kind:** InspectorTab gap
- **Evidence:**
  - Tabs exist: `InspectorTab::{Files, Activity}` in `all()` (`workspace.rs:73-85`). Desktop paints all nine (`main.rs:1193, 1221`).
  - Buttons: `tab_buttons` returns `Vec::new()` for both (`inspector.rs:78-79`). Same as Term/Skills.
  - Palette: `default_items()` has Session, Cores, MCP, Points, Git, Term, Skills. No Files, no Activity (`palette.rs:58-98`). Stale module comment still says Git/Term/Skills have no SelectTab rows (`:3-4`).
  - Slash: no `/files` or `/activity` (`slash.rs:29-42`). Unknown slash copy omits them (`main.rs:501`).
  - Controls catalog: `REQUIRED_IDS` has `tab_session` … `tab_skills`, no `tab_files` / `tab_activity` (`controls.rs:74-80`).
- **Gap:** Users can click the tab glyphs. They cannot reload files, mention a path, or jump from palette/slash. Plan/36 C asked for Reveal / @ mention / Reload.
- **Fix:** `tab_buttons(Files)` = Reload + @ mention + copy path. `tab_buttons(Activity)` can stay empty if Activity stays a log. Add palette + slash + control ids.

## F06 Activity is a second view of `terminal_log`

- **Severity:** P1
- **Kind:** half field
- **Evidence:**
  - `activity_detail` joins `terminal_log` or `"No activity yet."` (`workspace.rs:721-727`).
  - `activity_rows` is `act:status` (busy/idle) plus the last 16 `terminal_log` lines (`inspector_model.rs:221-253`).
  - Left `LeftSection::Activity` clones `terminal_log` and takes 20 reversed lines (`main.rs:995, 1150-1171`). Click only sets `Focus::Terminal`.
- **Gap:** No `ActivityLine` kind (Input/Output/Meta/Error from plan/32). Approval, reminder, and turn start are not activity rows. Capping 16 vs 20 vs Term's 12 vs strip's 4 is uncoordinated.
- **Fix:** Either promote `terminal_log` to typed activity (plan/32) or drop the Activity tab and keep Term + the bottom strip as the one log.

## F07 Files have no selection mutation

- **Severity:** P1
- **Kind:** missing mutation
- **Evidence:**
  - No `selected_file` field (plan/32 and plan/36 named it). `files: Vec<String>` only (`workspace.rs:250`).
  - Left Files click: `term_meta("file {p}")` (`main.rs:1142-1144`). Empty-state click only sets `inspector = Files` (`:1124`).
  - Chip "List project files" sets inspector + left section (`:1357-1360`). Does not select a path.
  - `ClientAction::CycleFile` is a host no-op in `apply_layout_action` (`actions.rs:113`). Desktop `cycle_file` rotates the vec (`main.rs:400-407`). Palette exposes it (`palette.rs:178-181`).
  - Inspector Files rows expand only (`inspector_row_el`).
- **Gap:** Files is a flat list you can rotate or log. No open, reveal, @ mention, or selected path. `set_files` runs once at startup (`main.rs:96`).
- **Fix:** `selected_file: Option<String>` + `ClientAction::SelectFile` / `RefreshFiles` / `InsertFileMention`. Host reload via `list_project_tree`. Click selects; do not use `CycleFile` as the interaction.

## F08 `*_detail`, `inspector_body`, and `Workspace::title_bar` are dead paint

- **Severity:** P2
- **Kind:** unused method / desktop never paints
- **Evidence:**
  - Desktop right rail paints `inspector_rows` (`main.rs:1182, 1262`), not text dumps.
  - `inspector_body` is `#[allow(dead_code)]` and only used in desktop unit tests (`inspector.rs:83-97, 104-164`).
  - `session_detail` / `resource_detail` / `mcp_detail` / `checkpoint_detail` / `git_detail` / `terminal_detail` / `skills_detail` / `files_detail` / `activity_detail` have no desktop call sites.
  - `Workspace::title_bar` (`workspace.rs:305-312`) is only asserted in workspace tests. Desktop `title_bar()` is a different method (`main.rs:902`) and does not use that string.
- **Gap:** The text-dump model still encodes Files inside Cores (`resource_detail` `:631-636`) and paints models/palette/help only there (`session_detail` `:597-606`). Live UI already moved on.
- **Fix:** Keep `*_detail` as test oracles or delete them once `inspector_rows` covers the same facts. Stop growing fields that exist only in the dump.

## F09 `rename_thread` is never called

- **Severity:** P2
- **Kind:** unused method / missing mutation
- **Evidence:**
  - `rename_thread` (`workspace.rs:361-369`).
  - Callers: `delete_thread_reselects` test only (`:1084-1086`).
  - No `ClientAction::RenameThread`. Thread titles change only via `send_draft` first-40-chars heuristic (`:420-422`).
  - Left rail paints `t.title` but click is `SelectThread` (`main.rs:1064-1073`).
- **Gap:** Users cannot rename. "New chat" stays until the first send.
- **Fix:** Local `RenameThread` (or inline edit on the selected row) if this wave wants it. Otherwise leave it, but do not count the method as product surface.

## F10 `pending` is never populated by the desktop

- **Severity:** P1
- **Kind:** half field / missing mutation
- **Evidence:**
  - Field: `pending: Option<PendingApproval>` (`workspace.rs:247`).
  - `set_pending_approval` lives in `approval_ui.rs:37-39`. Callers: tests in that file only.
  - Desktop paints `approval_bar` when `pending_approval()` is `Some` (`main.rs:1441-1466`) and can Allow/Deny (`host_action` `:221-222`).
  - Nothing in `pump` / `send` / RPC handling calls `set_pending_approval`.
- **Gap:** The card, `ClientAction::{Approve,Deny}`, and `approval.respond` binding (`bindings.rs:78-79`) are dead until a host writes `pending`.
- **Fix:** Map server approval requests into `set_pending_approval`. Until then the field is unused in production.

## F11 `models` catalog is not painted

- **Severity:** P2
- **Kind:** field desktop never paints
- **Evidence:**
  - Field: `models: Vec<String>` (`workspace.rs:249`). Desktop seeds three ids (`main.rs:78`).
  - Live paint shows only `workspace.model` (title pill `:938`, session row subtitle).
  - Catalog is listed in unused `session_detail` and in `integration_tiles` (`integrations.rs:20-38`). Desktop never calls `integration_tiles` / `filter_tiles`.
  - Mutation is `cycle_model` / `set_models` only. No `SelectModel(usize)`.
- **Gap:** Users cannot see the other models without cycling. Plan/36 H wanted a picker.
- **Fix:** Paint `models` as session rows or tiles. Add `SelectModel`. Or drop the unused tile helper.

## F12 Inspector click never selects a checkpoint

- **Severity:** P1
- **Kind:** missing mutation
- **Evidence:**
  - `selected_checkpoint` is set by `select_checkpoint` (`workspace.rs:521-523`).
  - Desktop sets it on create/revert (`main.rs:332, 356`), and copies it into `ActionContext.checkpoint_id` (`:164`).
  - `checkpoint_rows` marks `row.selected` from the field (`inspector_model.rs:118`).
  - Row click is only `toggle_right_row` (`main.rs:1884`). No `ClientAction::SelectCheckpoint`.
- **Gap:** Revert uses last checkpoint if none selected (`main.rs:338-342`). The `*` / selected chrome cannot be chosen from the list.
- **Fix:** Click a Points row → `select_checkpoint(Some(id))`. Accordion can stay for the expanded caption.

## F13 Bottom height and rail nudges have no desktop callers

- **Severity:** P2
- **Kind:** unused method
- **Evidence:**
  - `set_bottom_height` / `occupied_bottom` (`workspace.rs:748-755`). Desktop only `ToggleBottom` (`main.rs:697`) and paints `occupied_bottom()` (`:1481`). No NS drag.
  - `occupied_bottom` ignores `bottom_open` and always returns `bottom_height`. The strip never hides; toggle jumps 120 ↔ 280.
  - `ChromeLayout::nudge_left` / `nudge_right` (`workspace.rs:203-209`) are test-only. Live resize uses `set_left_width` / `set_right_width` from mouse (`main.rs:852-860`).
  - `collapse_right_row` (`workspace.rs:766-768`) is test-only. Tab change already clears expand via `select_inspector` (`:770-777`).
- **Gap:** Half a bottom-drawer model (open flag + height + collapse constants) with only a toggle. Keyboard nudge helpers are unused.
- **Fix:** Add a bottom grab that calls `set_bottom_height`, or delete `bottom_open` if height is the source of truth. Drop `nudge_*` if mouse resize is enough.

## F14 Dual `palette_open`

- **Severity:** P2
- **Kind:** half field
- **Evidence:**
  - `Workspace.palette_open` (`workspace.rs:254`) plus a separate `PaletteState` on `ShellView` (`main.rs:65`).
  - Desktop overlay keys off `self.palette.open` (`:891`). It mirrors into `workspace.palette_open` on toggle/slash (`:181, 488`).
  - `status_from` reads the workspace flag for the `palette ·` prefix (`status.rs:21, 48-49`).
- **Gap:** Two sources of truth. `ClosePalette` applies layout then the host closes `PaletteState` again (`main.rs:188-194`). Easy to desync.
- **Fix:** One owner. Either Workspace is the flag and `PaletteState` is query/selection only, or drop `workspace.palette_open`.

## F15 `ConnectionState::Connecting` is never assigned on `Workspace`

- **Severity:** P2
- **Kind:** unused variant / field desktop never paints
- **Evidence:**
  - `Workspace.connection` starts `Disconnected` (`workspace.rs:271`) then `connect(...)` jumps to `Connected` (`:482-484`). Desktop does that at boot with an empty session list (`main.rs:79`) and again after `session.start` (`:429`).
  - `Connecting` exists on `ConnectionState` (`lib.rs:58`) and `DesktopChrome::mark_connecting`, not on `Workspace`.
  - Session row subtitle is `status_label()` (`inspector_model.rs:45`). Agents rail treats anything not `Connected` as empty (`main.rs:996-998`).
- **Gap:** The model can say "connecting". The product never does. Boot already lies "connected" with zero sessions.
- **Fix:** Set `Connecting` around `ensure_session`, or delete the unused state from the Workspace path.

## F16 Action catalog lags the Workspace API

- **Severity:** P2
- **Kind:** missing mutation / catalog gap
- **Evidence:**
  - `ClientAction` has `SelectLeftSection` and `ToggleBottom` (`actions.rs:31-32`). `apply_layout_action` and `host_call` handle them (`actions.rs:60-72`, `bindings.rs:45-46`).
  - `bindings.rs` `layout_is_local` test omits both (`:245-257`).
  - Palette has neither, nor Files/Activity tabs (see F05).
  - No actions for: select worktree, select file, select checkpoint, rename thread, refresh files, insert mention, set bottom height.
- **Gap:** The headless model grew rails and tabs faster than the action map the desktop is supposed to dispatch through. Several clicks mutate `Workspace` fields directly (`inspector = Files`, `left_section = Files` at `main.rs:1358-1359`) and skip `apply_layout_action`.
- **Fix:** Every inspector/rail click goes through `ClientAction`. Extend `default_items` and the bindings local test in the same change.

---

## Field / method matrix (Workspace)

| Item | Mutated live | Painted live | Notes |
|---|---|---|---|
| `project` | boot | yes | title pill, session row |
| `model` | cycle | yes | pill + session row |
| `models` | set/cycle | no | F11 |
| `connection` | connect | partial | never `Connecting` (F15) |
| `threads` / `selected` | yes | yes | |
| `draft` / `cursor` | yes | partial | F02 |
| `inspector` | yes | yes | Files/Activity hollow (F05) |
| `worktrees` | refresh | yes | |
| `selected_worktree` | no | mark only | F01 |
| `chrome` | yes | yes | `nudge_*` unused (F13) |
| `cores` / `mcp` / `checkpoints` | yes | yes | |
| `selected_checkpoint` | create/revert | mark only | F12 |
| `reminder` | yes | yes | |
| `terminal_log` | yes | yes | also Activity (F06) |
| `busy` | yes | yes | |
| `pending` | no | bar exists | F10 |
| `files` | boot + cycle | yes | no selection (F07) |
| `skills` | boot | yes | |
| `git_status` | shell pump | yes | |
| `term_draft` | yes | partial | F03 |
| `palette_open` | mirrored | status prefix | F14 |
| `help_open` | yes | overlay | |
| `left_section` | yes | yes | |
| `right_expanded_id` | click | yes | |
| `bottom_open` / `bottom_height` | toggle | height only | F13 |
| `title_bar()` | n/a | no | F08 |
| `rename_thread` | no | n/a | F09 |
| `move_cursor_*` | no | n/a | F04 |
| `*_detail` / `files_detail` / `activity_detail` | n/a | no | F08 |
| `set_bottom_height` | no | n/a | F13 |
| `collapse_right_row` | no | n/a | F13 |
| `set_pending_approval` | no | n/a | F10 |

---

## Suggested TDD slices (parent)

1. `select_worktree_sets_index_and_git_row_selected` in `workspace.rs` + desktop Git click.
2. `select_checkpoint_from_row_not_only_create` in `workspace.rs` + Points click.
3. `draft_display_shows_caret_when_empty_and_focused` next to `draft_display`.
4. `tab_buttons_files_is_not_empty` in `inspector.rs`; palette/slash include Files.
5. `select_file_sets_selected_and_inserts_mention_at_cursor` (new field + action).
6. Host test: approval request → `set_pending_approval` → bar visible.

P0 is not used here. Nothing crashes. Several fields pretend to be product state and are not.
