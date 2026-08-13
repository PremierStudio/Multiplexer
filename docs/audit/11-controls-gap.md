# 11. Controls catalog vs ShellView

Audit of `apps/multiplexer-desktop/src/controls.rs` `REQUIRED_IDS` against what `ShellView` in `apps/multiplexer-desktop/src/main.rs` actually paints and handles. Read-only. No cargo.

**Catalog size:** 39 required ids (`REQUIRED_IDS.len() == all_controls().len()`).  
**Status rule:** `implemented` means a visible or key-bound control exists and the handler does that job. `partial` means a handler or control exists but the catalog label, surface, or behavior is wrong or incomplete. `missing` means no matching chrome and no handler for that id.

**Score:** 36 implemented, 2 partial, 1 missing.

ShellView never matches `ControlSpec::action` (the catalog says the parent must). Handlers are hardcoded `ClientAction` / `InspectorAction` arms and a few direct method calls. Status below is functional, not string-equal.

---

## Required ids

| id | surface | status | ShellView evidence |
|---|---|---|---|
| `chats_toggle` | TitleBar | implemented | Title-bar layout icon dispatches `ClientAction::ToggleLeft`. Chord `ctrl-[` in `handle_key`. |
| `inspector_toggle` | TitleBar | implemented | Title-bar settings icon dispatches `ToggleRight`. Chord `ctrl-]`. |
| `stop` | TitleBar | implemented | When `workspace.busy`, Stop icon calls `interrupt()`. Chord `ctrl-.` always interrupts. Palette `stop` dispatches `Interrupt`. Idle chrome replaces this with uncataloged Play (see extra dead UI). |
| `command_palette` | TitleBar | implemented | Palette icon + `ctrl-k` / `ctrl-p` dispatch `TogglePalette`. |
| `help` | TitleBar | implemented | `?` button and `F1` dispatch `ToggleHelp`. `apply_layout_action` toggles `help_open`. |
| `new_thread` | LeftRail | implemented | Plus button (Threads section) + `ctrl-n` + `/new` dispatch `NewThread`. |
| `select_thread` | LeftRail | implemented | Thread rows dispatch `SelectThread(i)`. |
| `delete_thread` | LeftRail | implemented | Delete icon (Threads section) + palette item dispatch `DeleteThread` on the selected thread. |
| `chip_what` | Center | implemented | Chip `What can you do?` sets draft and `send()`. Label matches catalog. |
| `chip_summarize` | Center | implemented | Chip `Summarize this repo` sets draft and `send()`. |
| `chip_git_status` | Center | partial | Chip label is `git status` (catalog: `Git status`). Click runs `run_shell("git status")` instead of sending a prompt like the other Center chips. |
| `chip_test` | Center | missing | Catalog label `Run the tests`. No chip, no key, no palette row. The fourth chip is `List project files` (uncataloged). |
| `copy_last_message` | Center | implemented | Chip `Copy last` calls `copy_last_message`. `host_action` and palette also bind `CopyLastMessage`. Label is shortened vs catalog `Copy last message`. |
| `send` | Composer | implemented | Send button + Enter (unshifted) call `send()`. Palette `send` dispatches `Send`. |
| `newline` | Composer | implemented | No button. `shift-enter` inserts `\n`. Matches catalog shortcut. |
| `paste` | Composer | implemented | No button. `ctrl-v` reads the clipboard and `insert_text`. |
| `tab_session` | RightRail | implemented | `InspectorTab::Session` (`label` = `Session`) via `SelectTab`. |
| `tab_cores` | RightRail | implemented | `InspectorTab::Resources` (`label` = `Cores`) via `SelectTab`. |
| `tab_mcp` | RightRail | implemented | `InspectorTab::Mcp` via `SelectTab`. |
| `tab_points` | RightRail | implemented | `InspectorTab::Checkpoints` (`label` = `Points`) via `SelectTab`. |
| `tab_git` | RightRail | implemented | `InspectorTab::Git` via `SelectTab`. |
| `tab_term` | RightRail | implemented | `InspectorTab::Terminal` via `SelectTab`. |
| `tab_skills` | RightRail | implemented | `InspectorTab::Skills` via `SelectTab`. |
| `cycle_model` | RightRail | implemented | Session tab `Model` button (`InspectorAction::CycleModel`). Also title-bar model pill and `/model`. |
| `copy_session` | RightRail | implemented | Session tab `Copy` button (`CopySession` -> `copy_session`). Catalog label is `Copy session`. |
| `refresh_cores` | RightRail | implemented | Cores tab `Reload` -> `refresh_cores()`. Catalog label is `Refresh cores`. |
| `refresh_mcp` | RightRail | implemented | MCP tab `Reload` -> `refresh_mcp()`. Catalog label is `Refresh MCP`. |
| `create_checkpoint` | RightRail | implemented | Points tab `New` -> `create_checkpoint()`. Also `ctrl-s` and `/cp`. Catalog label is `Create checkpoint`. |
| `revert_checkpoint` | RightRail | implemented | Points tab `Revert` -> `revert_checkpoint()`. Palette `restore-checkpoint`. Catalog label is `Revert checkpoint`. |
| `refresh_git` | RightRail | implemented | Git tab `Reload` -> `refresh_worktrees()`. Catalog label is `Refresh git`. |
| `run_git_status` | RightRail | implemented | Git tab `Status` -> `run_shell("git status")`. Catalog label is `Run git status`. |
| `term_run` | TermStrip | implemented | `Run` button + Enter in terminal focus call `run_terminal_draft()`. |
| `term_clear` | TermStrip | implemented | `Clear` button clears `terminal_log`. Builtin `clear` does the same. |
| `palette_filter` | Palette | implemented | Overlay query box. Typing updates `palette.set_query` and `filter_items`. No labeled `Filter` control. |
| `palette_run` | Palette | implemented | Enter or click a row dispatches `item.action`. |
| `help_close` | HelpOverlay | partial | No `Close` button on the help overlay. Escape and backdrop click toggle/close help. Catalog wants a labeled Close with `escape`. |
| `allow` | ApprovalCard | implemented | Approval `Allow` dispatches `Approve` -> `respond_approval("allow")`. Hint `A` is not a bound key. |
| `deny` | ApprovalCard | implemented | Approval `Deny` dispatches `Deny` -> `respond_approval("deny")`. Hint `D` is not a bound key. |
| `dismiss` | ReminderBar | implemented | `Dismiss` dispatches `DismissReminder`. Escape also dismisses when a reminder is showing. |

### Handler map (catalog id to code)

| id | How it fires | Function / action |
|---|---|---|
| `chats_toggle` | title bar, `ctrl-[`, palette | `dispatch(ToggleLeft)` |
| `inspector_toggle` | title bar, `ctrl-]`, palette | `dispatch(ToggleRight)` |
| `stop` | Stop icon if busy, `ctrl-.`, palette, `/stop` | `interrupt()` / `dispatch(Interrupt)` |
| `command_palette` | title bar, `ctrl-k`/`ctrl-p`, `/palette` | `dispatch(TogglePalette)` |
| `help` | title bar, `F1`, `/help` | `dispatch(ToggleHelp)` |
| `new_thread` | left rail, `ctrl-n`, `/new` | `dispatch(NewThread)` |
| `select_thread` | thread row | `dispatch(SelectThread(i))` |
| `delete_thread` | left rail, palette | `dispatch(DeleteThread)` |
| `chip_what` | center chip | `set_draft` + `send()` |
| `chip_summarize` | center chip | `set_draft` + `send()` |
| `chip_git_status` | center chip | `run_shell("git status")` (not send) |
| `chip_test` | (none) | (none) |
| `copy_last_message` | center chip, palette | `copy_last_message` / `CopyLastMessage` |
| `send` | Send button, Enter, palette | `send()` / `dispatch(Send)` |
| `newline` | `shift-enter` | `insert_text("\n")` |
| `paste` | `ctrl-v` | clipboard + `insert_text` |
| `tab_*` (7) | right-rail tabs | `dispatch(SelectTab(...))` |
| `cycle_model` | Session `Model`, model pill, `/model` | `inspector_click(CycleModel)` / `dispatch(CycleModel)` |
| `copy_session` | Session `Copy` | `inspector_click(CopySession)` |
| `refresh_cores` | Cores `Reload` | `inspector_click(RefreshCores)` / `host_action` |
| `refresh_mcp` | MCP `Reload` | `inspector_click(RefreshMcp)` |
| `create_checkpoint` | Points `New`, `ctrl-s`, `/cp` | `create_checkpoint()` |
| `revert_checkpoint` | Points `Revert` | `revert_checkpoint()` |
| `refresh_git` | Git `Reload` | `refresh_worktrees()` |
| `run_git_status` | Git `Status` | `run_shell("git status")` |
| `term_run` | Run, Enter (term focus) | `run_terminal_draft()` |
| `term_clear` | Clear | `terminal_log.clear()` |
| `palette_filter` | palette query | `palette.set_query` |
| `palette_run` | Enter / row click | `dispatch(item.action)` |
| `help_close` | Esc, backdrop | `toggle_help()` (no Close button) |
| `allow` | Allow | `dispatch(Approve)` |
| `deny` | Deny | `dispatch(Deny)` |
| `dismiss` | Dismiss, Esc | `dispatch(DismissReminder)` |

`host_action` arms that back catalog ids: `Send`, `Interrupt`, `RefreshCores`, `RefreshMcp`, `CreateCheckpoint`, `RestoreCheckpoint` (revert), `RefreshGit`, `RunTerminal`, `CopyLastMessage`, `Approve`, `Deny`. `CycleFile` is in `host_action` and is not a required id.

---

## Extra UI not in the catalog

### Dead (clickable, stub handler, not a required id)

| UI | Element / site | What it does |
|---|---|---|
| Play / Idle | title bar when not busy (`icon-Idle`) | `term_meta("start a turn from the composer")` only. Replaces catalog `stop`. |
| Agents empty row | `agent-none` | `term_meta("start a session from the composer")`. |
| Agent session row | `agent-{id}` | `term_meta("session selected")`. Does not select or focus a session. |
| File row | `file-{path}` | `term_meta("file {p}")`. Does not open, mention, or select a file. |

Catalog header: "Every visible (or soon-visible) control has a handler name. Nothing is dead." These four violate that.

### Live extras (real work, still not in `REQUIRED_IDS`)

| UI | What it does | Why it is a catalog hole |
|---|---|---|
| Left-rail section icons | `SelectLeftSection` for Threads, Agents, Files, Activity | plan/30 named `left_section_*`. Never added. |
| Inspector `Files` tab | `SelectTab(Files)` | plan/30 `tab_files`. `tab_buttons` is empty. |
| Inspector `Activity` tab | `SelectTab(Activity)` | plan/30 `tab_activity`. `tab_buttons` is empty. |
| Git `New WT` | `InspectorAction::NewWorktreeHint` | Sets a worktree draft. Extra inspector button. |
| Chip `List project files` | Switches Files tab + left Files section | Occupies the `chip_test` slot. |
| Title-bar model pill | `dispatch(CycleModel)` | Duplicate of `cycle_model` on TitleBar. plan/30 `model_pill`. |
| Empty files row `file-none` | Jumps to Files inspector tab | Navigation, not cataloged. |
| Activity rows | Sets `Focus::Terminal` | Navigation, not cataloged. |
| Inspector row click | `toggle_right_row` | Expand/collapse, not cataloged. |
| Rail resize handles | `DragRail` left/right | Layout, not cataloged. |
| Palette `cycle-file` | `ClientAction::CycleFile` -> `cycle_file()` | Host action with no required id. |
| Palette `close-palette` | `ClosePalette` | Overlay chrome, not a required id. |
| Palette / help backdrop click | Closes overlay | Extra close path. |
| `ctrl-s` | `create_checkpoint()` | Extra chord. Catalog has no shortcut on `create_checkpoint`. |
| `ctrl-\`` | `ToggleBottom` | plan/30 `toggle_bottom`. Not in `REQUIRED_IDS` or `shortcut_map`. |
| Tab key | Composer <-> terminal focus | Not cataloged. |
| Project / branch pills | Display only | plan/30 `project_pill` / `branch_pill`. Not clickable, not in catalog. |

`shortcut_map` also names `close_overlay` for Escape. That action is not a `REQUIRED_IDS` entry (the controls test documents this). `handle_key` implements the close stack inline and never reads `shortcut_map`.

---

## Findings

### F1: Catalog is not the ShellView handler table
- Severity: major
- Kind: wiring
- Evidence: `controls.rs` 1-4 (`Parent matches ControlSpec::action in ShellView`); `main.rs` 173-227 (`dispatch` / `host_action` match `ClientAction`, not action strings); `main.rs` 149-156 (startup only asserts catalog length and that `send` exists)
- Detail: `REQUIRED_IDS` is a 39-row checklist. ShellView never calls `control_by_id` or matches `spec.action`. Adding or renaming a catalog id cannot change chrome. The "nothing is dead" claim is unenforced at the window.

### F2: `shortcut_map` is unused at runtime
- Severity: major
- Kind: wiring
- Evidence: `controls.rs` 186-199; `main.rs` 153-155 (assert `ctrl-k` is in the map); `main.rs` 644-707 (hardcoded chords)
- Detail: `handle_key` reimplements enter, escape, ctrl-k/p/n/[ /]/./v, shift-enter, F1, plus extra `ctrl-s` and `ctrl-\``. Escape is cataloged as `close_overlay`, which is not a required id. Changing `shortcut_map` cannot change bindings.

### F3: `chip_test` is missing
- Severity: major
- Kind: missing
- Evidence: `controls.rs` 69, 141 (`chip_test` / `Run the tests`); `main.rs` 1338-1364 (five chips, none is `Run the tests`)
- Detail: Center chips are `What can you do?`, `Summarize this repo`, `git status`, `List project files`, `Copy last`. The catalog fourth Center control is absent. `List project files` is uncataloged live chrome in that slot.

### F4: `chip_git_status` does not behave like a Center chip
- Severity: major
- Kind: partial
- Evidence: `controls.rs` 140 (`Git status` on Center); `main.rs` 1348-1351 (summarize chip sends a prompt); `main.rs` 1353-1356 (`git status` chip calls `run_shell`)
- Detail: Sibling chips set draft and `send()`. This chip runs a local shell command. Label is `git status`, not catalog `Git status`. Inspector `run_git_status` already covers `git status` in the terminal.

### F5: `help_close` has no Close control
- Severity: minor
- Kind: partial
- Evidence: `controls.rs` 180 (`help_close`, HelpOverlay, `Close`, `escape`); `main.rs` 1626-1657 (help overlay: title `Keyboard`, backdrop click, no Close button); `main.rs` 647-656 (Escape toggles help)
- Detail: Close works via Escape and backdrop. Catalog requires a labeled Close on `HelpOverlay`. Backdrop click is `toggle_help`, not a dedicated close action.

### F6: Dead clickables sit outside the catalog
- Severity: major
- Kind: extra
- Evidence: `main.rs` 955-963 (Play / Idle); `main.rs` 1076-1108 (agent rows); `main.rs` 1129-1145 (file rows)
- Detail: Four visible controls only log `term_meta`. They have no `ControlSpec`, no `ClientAction` that matches the click, and no product effect. This is the "dead UI" the catalog claims not to have.

### F7: Live chrome outgrew the pinned 39 ids
- Severity: major
- Kind: extra
- Evidence: `workspace.rs` 73-85 (`InspectorTab::all` is 9, including Files and Activity); `workspace.rs` 98-100 (`LeftSection::all` is 4); `inspector.rs` 71-75 (`New WT`); `main.rs` 928-941 (model pill); `main.rs` 696-698 (`ToggleBottom`); `palette.rs` 177-182 (`cycle-file`)
- Detail: plan/30 already listed `left_section_*`, `tab_files`, `tab_activity`, `toggle_bottom`, `model_pill`, `project_pill`, `branch_pill`, `layout_reset`, `run`. `REQUIRED_IDS` tests still pin 39. Files and Activity tabs have no `tab_buttons`. CycleFile is a host action with no catalog id.

### F8: Inspector button labels do not match `ControlSpec.label`
- Severity: minor
- Kind: partial (ids still implemented)
- Evidence: `controls.rs` 158-175 vs `inspector.rs` 38-75
- Detail: Catalog vs painted: `Cycle model`/`Model`, `Copy session`/`Copy`, `Refresh cores`/`Reload`, `Refresh MCP`/`Reload`, `Create checkpoint`/`New`, `Revert checkpoint`/`Revert`, `Refresh git`/`Reload`, `Run git status`/`Status`. Same for `copy_last_message` (`Copy last message` vs `Copy last`).

### F9: Required right-rail actions bypass `dispatch`
- Severity: minor
- Kind: wiring
- Evidence: `main.rs` 1249-1253 (`inspector_click`); `main.rs` 229-249; `main.rs` 209-227 (`host_action` is the other copy)
- Detail: `cycle_model`, `copy_session`, `refresh_cores`, `refresh_mcp`, `create_checkpoint`, `revert_checkpoint`, `refresh_git`, `run_git_status` fire through `InspectorAction`, not `ControlSpec::action` and often not `ClientAction`. Two parallel maps (`tab_buttons` and `REQUIRED_IDS`) can drift. `host_call` `Rpc` results are also ignored: `dispatch` treats `Rpc` like `NeedsHost` and `host_action` re-encodes the same work.

### F10: `stop` chrome is swapped for uncataloged Play when idle
- Severity: minor
- Kind: extra
- Evidence: `main.rs` 944-964
- Detail: Catalog `stop` is always a TitleBar control. When not busy the shell paints Play / Idle, which is dead (F6). `ctrl-.` still interrupts. The visible control no longer matches the required id.

---

FINDINGS: 10
