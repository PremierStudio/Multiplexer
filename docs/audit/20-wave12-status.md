# Audit 20: plan/36 this-wave 12 (C through N)

**Baseline:** current tree after `827279c` (source read only, no cargo).
**Spec:** `plan/36-feature-gap-ui.md` sections 2 and 4 (rows C–N).
**Code:** `crates/multiplexer-shell`, `apps/multiplexer-desktop`, `crates/multiplexer-layout`, `crates/multiplexer-mcp`, `crates/multiplexer-server`, `crates/multiplexer-wire`, `crates/multiplexer-client`, `crates/multiplexer-theme`.
**Method:** read the plan contract, then the live types, actions, inspector, desktop, and named tests. Status is `shipped` only when user-visible behavior, headless model, and the named test all match the brief.

**Headline:** none of the twelve are shipped. Four are partial headless/UI stubs (C, D, F, L/N overlapping). Eight are missing the wave contract. Desktop still has one window, cycle-model, `New WT` paste, and a command-only palette. `multiplexer-mcp::Supervisor` is still unused by the UI.

---

## Scoreboard

| # | Wave item | Status | Named test |
|---|---|---|---|
| C | Files tree | **partial** | `file_tree_select_expand_and_mention` exists, does not assert expand/collapse |
| D | MCP start/stop | **partial** | `mcp_start_sets_ready_and_stop_releases` exists and matches the projection |
| E | Worktree create UI | **partial** | `worktree_create_draft_dispatches_rpc` **missing** |
| F | Orchestration dashboard | **partial** | `agents_tab_projects_thread_tree` **missing** |
| G | Remote / Tailscale | **missing** | `remote_status_lists_local_and_tailscale_detect` **missing** |
| H | Model picker | **missing** | `select_model_sets_workspace_and_rpc` **missing** |
| I | Usage snapshot | **missing** | `usage_snapshot_formats_session_detail` **missing** |
| J | Search (names) | **partial** | `search_hits_rank_files_threads_commands` **missing** (orphan `search_finds_thread_file_and_command`) |
| K | Pop-out panes | **missing** | `popout_inspector_detaches_and_redocks` **missing** |
| L | Settings overlay | **partial** | `settings_overlay_applies_default_model` **missing** |
| M | Palette richness | **missing** | `palette_filter_includes_files_and_threads` **missing** |
| N | Notifications | **partial** | `toast_queue_caps_and_dismisses` **missing** (cap-5 `push_caps_at_five_and_dismisses`) |

`plan/36` also asked `InspectorTab::all()` to grow from 7 to 9 (`Files`, `Agents`). Live code has **10** tabs: Session, Cores, MCP, Points, Git, Term, Skills, Files, Activity, Agents.

---

## C. Files tree

**Status:** partial

**Contract:** `InspectorTab::Files`, `FileNode` tree (dirs first, expand/collapse, `*`), buttons Reveal / `@ mention` / Reload, double-click copies relative path (no editor), palette `file:`.

**Now:**

- Eighth inspector tab `Files` exists (`crates/multiplexer-shell/src/workspace.rs` `InspectorTab::Files`).
- `Workspace.files` is still `Vec<String>`. Trailing `/` marks dirs in `list_project_tree` load (`apps/multiplexer-desktop/src/main.rs`).
- `select_file`, `selected_file`, `insert_file_mention` exist. No `FileNode`, no `toggle_file_expand`, no `files_visible()`.
- Files tab button is only **Mention** (`apps/multiplexer-desktop/src/inspector.rs`). No Reveal, no Reload (an earlier Reload was wired to `RefreshMcp` and is gone).
- Left rail `LeftSection::Files` dumps the same flat list. Click writes `term_meta("file …")`.
- `ClientAction::CycleFile` still rotates the vec. Palette id `cycle-file` remains.
- `file_rows` does not mark `selected_file` with `*`.

**Evidence:**

- `crates/multiplexer-shell/src/workspace.rs` (`files: Vec<String>`, `select_file`, `insert_file_mention`, `files_detail` join)
- `crates/multiplexer-shell/src/inspector_model.rs` `file_rows`
- `crates/multiplexer-client/src/files.rs` `list_project_tree` (depth 2, skips `.git` / `node_modules` / `target`)
- `apps/multiplexer-desktop/src/inspector.rs` `InspectorTab::Files` → Mention only
- `apps/multiplexer-desktop/src/main.rs` `cycle_file`, left-rail Files click, `MentionFile`

**Next TDD slice:** `file_tree_select_expand_and_mention` in `crates/multiplexer-shell/src/workspace.rs`

Rewrite the existing test to match the brief: expand a dir, select a file, `InsertFileMention` puts `` `@src/lib.rs` `` at `cursor`, collapse hides children from `files_visible()`. Introduce `FileNode` first so the test can fail on expand, not only on mention.

---

## D. MCP start / stop

**Status:** partial

**Contract:** per-row state badge, Start/Stop/Reload, Start disabled when ready, Stop disabled when stopped, toast on start/stop, crash badge until Start/Reload, body copy "supervised (in-process table)", projection `start_mcp` / `stop_mcp`, `apply_layout_action` flips projection, `host_call(StartMcp)` is `NeedsHost`.

**Now:**

- `McpLife` + `McpRow.state` + `start_mcp` / `stop_mcp` + `mcp_detail` badge text exist.
- Named test `mcp_start_sets_ready_and_stop_releases` matches the plan asserts.
- Inspector MCP buttons: Reload, Start, Stop. Start/Stop act on `right_expanded_id` prefix `mcp:` and push a `Notice`.
- `ClientAction::StartMcp | StopMcp` is a **host no-op** in `apply_layout_action` (does not flip the projection). Plan wanted the projection flip in `apply_layout_action` for tests.
- `mcp_rows` badge uses `state.label()` (`ready` / `stopped` / `crashed` / `failed`).
- Tab body does **not** say "supervised (in-process table)".
- Start/Stop are never disabled by state. No `SelectMcp`. No selected row for Start without expand.
- `multiplexer-mcp::Supervisor` is not constructed in the desktop. Inventory still comes from `load_user_mcp_inventory()`.

**Evidence:**

- `crates/multiplexer-shell/src/workspace.rs` `McpLife`, `start_mcp`, `stop_mcp`, `mcp_detail`
- `crates/multiplexer-shell/src/actions.rs` `StartMcp`/`StopMcp` → `false`
- `crates/multiplexer-shell/src/bindings.rs` those two → `NeedsHost`
- `crates/multiplexer-shell/src/inspector_model.rs` `mcp_rows` tone by `McpLife`
- `apps/multiplexer-desktop/src/inspector.rs` Start/Stop buttons
- `apps/multiplexer-desktop/src/main.rs` `inspector_click` StartMcp/StopMcp
- `crates/multiplexer-mcp/src/supervisor.rs` (unused by UI)

**Next TDD slice:** keep `mcp_start_sets_ready_and_stop_releases` in `crates/multiplexer-shell/src/workspace.rs`

Add the missing action test in `crates/multiplexer-shell/src/actions.rs`: `apply_layout_action(StartMcp)` after selecting `linear` sets `Ready` (unknown name is a no-op). Then wire Start/Stop disable + honesty line in the desktop.

---

## E. Worktree create UI

**Status:** partial

**Contract:** Git tab draft path / branch / create-branch (prefill `../mux-<branch>`, `feat`), **Create** primary, `ClientAction::CreateWorktree` → `git.worktree.create` with `cwd`, `NewWorktreeHint` stays secondary.

**Now:**

- Server already implements `git.worktree.create` (`crates/multiplexer-server/src/worktree_create.rs`, dispatch in `server.rs`).
- Workspace holds unused draft fields: `wt_path = "../mux-feat"`, `wt_branch = "feat"`, `wt_create_branch = true`.
- `git_detail` does not print the draft. No `WorktreeDraft` type. No `CreateWorktree` action. `host_call` has no create arm (RefreshGit is still `git.worktrees` only).
- Git tab primary is still **New WT** → paste `git worktree add ../mux-feat -b feat` into the composer.

**Evidence:**

- `crates/multiplexer-shell/src/workspace.rs` `wt_path` / `wt_branch` / `wt_create_branch`, `git_detail`
- `crates/multiplexer-shell/src/bindings.rs` `RefreshGit` vs explicit non-create assert
- `apps/multiplexer-desktop/src/inspector.rs` `NewWorktreeHint`
- `apps/multiplexer-desktop/src/main.rs` `InspectorAction::NewWorktreeHint`
- `crates/multiplexer-server/src/server.rs` `GIT_WORKTREE_CREATE`

**Next TDD slice:** `worktree_create_draft_dispatches_rpc` in `crates/multiplexer-shell/src/bindings.rs`

Assert draft `{path: "../mux-feat", branch: "feat", create_branch: true}` produces params with those three fields plus `cwd`. Empty path does not dispatch.

---

## F. Orchestration dashboard (local threads)

**Status:** partial

**Contract:** ninth inspector tab **Agents**, rows from threads (title, status, model, message count, `parent: None`), body "Local threads only…", palette `agent:`, optional `orchestration.list` stub `{ subagents: [] }`.

**Now:**

- `InspectorTab::Agents` exists (tenth tab, after Activity). `agents_detail()` has the honesty line. `agent_rows()` is a tuple `(id, title, status, messages)` with no `model` and no `parent`.
- `inspector_model::agent_rows` marks the selected thread.
- Left rail `LeftSection::Agents` lists **session ids**, not the thread tree.
- No `AgentRow` type. No palette `agent:`. Server `orchestration.list` is still `method not found`.

**Evidence:**

- `crates/multiplexer-shell/src/workspace.rs` `InspectorTab::Agents`, `agent_rows`, `agents_detail`
- `crates/multiplexer-shell/src/inspector_model.rs` `agent_rows`
- `apps/multiplexer-desktop/src/main.rs` left-rail Agents
- `crates/multiplexer-server/src/server.rs` dispatch (no `orchestration.*`)
- `crates/multiplexer-wire/src/methods.rs` `ORCHESTRATION_LIST`

**Next TDD slice:** `agents_tab_projects_thread_tree` in `crates/multiplexer-shell/src/workspace.rs`

Two threads → two rows, `parent` is `None`, selecting row 1 sets `selected == 1`, detail contains both titles.

---

## G. Remote / Tailscale status

**Status:** missing

**Contract:** Settings Remote section (also Session lines), `local` + `tailscale` detect (`PATH` / `TAILSCALE_EXE`), Copy local URL placeholder, Refresh detect, no Serve.

**Now:**

- No `RemoteRow`, `set_remotes`, `tailscale_detected`, `RefreshRemote`, or `remote_detail`.
- Settings overlay has theme / density / default model text only. Session tab has connection label, not remotes.
- `remote.list` is a wire constant with no router arm.

**Evidence:**

- no hits in `crates/multiplexer-shell` or `apps/multiplexer-desktop` for `RemoteRow` / `tailscale`
- `apps/multiplexer-desktop/src/main.rs` `settings_overlay`
- `crates/multiplexer-wire/src/methods.rs` `REMOTE_LIST`
- `crates/multiplexer-server/src/server.rs` unmatched methods → `method not found`

**Next TDD slice:** `remote_status_lists_local_and_tailscale_detect` in `crates/multiplexer-shell/src/workspace.rs`

Default has a `local` row. `set_tailscale_detected(true)` shows `detected` in `remote_detail()`. False shows `not found`.

---

## H. Model picker

**Status:** missing (cycle still ships)

**Contract:** Session lists catalog with `*`, click selects (no cycle), `/model <id>` selects, bare `/model` still cycles, `SelectModel(usize)`, `host_call` → `model.select` when a session exists.

**Now:**

- Catalog is hardcoded `grok`, `grok-4.6`, `fake`. Session button **Model** cycles. `/model` cycles. `cycle_model` only.
- No `select_model`, no `SelectModel`, no `/model <id>` parse (slash token only).
- `model.list` / `model.select` are wire constants, not dispatched.
- Title bar already shows the active model.

**Evidence:**

- `crates/multiplexer-shell/src/workspace.rs` `cycle_model`, `set_models`, `session_detail`
- `crates/multiplexer-shell/src/slash.rs` `SlashCommand::Model` (no id)
- `apps/multiplexer-desktop/src/main.rs` `set_models(vec![…])`, `SlashCommand::Model` → `cycle_model`
- `apps/multiplexer-desktop/src/inspector.rs` `InspectorAction::CycleModel`
- `crates/multiplexer-server/src/server.rs` no `model.*`

**Next TDD slice:** `select_model_sets_workspace_and_rpc` in `crates/multiplexer-shell/src/workspace.rs` (and `bindings.rs` for the RPC arm)

Catalog `[grok, ds-flash]`. `SelectModel(1)` sets `model == "ds-flash"`. `host_call` with a session id is `model.select` containing `ds-flash`. Out-of-range is a no-op.

---

## I. Usage snapshot (not billing)

**Status:** missing

**Contract:** Session **Usage** block (turns, last_ms, last_prompt_chars, tokens n/a, `account: local`), `record_turn` on finished `grok -p`, optional `telemetry.usage` echo.

**Now:**

- CPU bars only (`CoreRow.usage`, `tiny_usage_bar`, `usage_bar`). Plan called that out as not usage.
- No `UsageSnapshot`, no `record_turn`. `session_detail` has project/model/connection/session/threads/palette/help.
- `telemetry.usage` is a wire constant, not dispatched.
- Desktop `pump` on turn finish does not increment a usage struct.

**Evidence:**

- `crates/multiplexer-shell/src/workspace.rs` `session_detail`, `tiny_usage_bar`
- `crates/multiplexer-shell/src/bars.rs` (CPU bar helper)
- `apps/multiplexer-desktop/src/main.rs` `pump` turn completion
- `crates/multiplexer-wire/src/methods.rs` `TELEMETRY_USAGE`

**Next TDD slice:** `usage_snapshot_formats_session_detail` in `crates/multiplexer-shell/src/workspace.rs`

Two `record_turn` calls → `turns: 2` in `session_detail()`. `account` contains `local`. Missing tokens print `n/a`.

---

## J. Search (names, not content)

**Status:** partial

**Contract:** `Ctrl+Shift+F` overlay, groups Files / Threads / Commands, empty query is hint-only, `SearchState`, `search_hits`, Esc closes search before palette before help.

**Now:**

- `crates/multiplexer-shell/src/search.rs` implements `search_workspace` (substring over threads, `files` strings, `default_items()`). Empty query returns `[]`.
- Test name is `search_finds_thread_file_and_command`, not the plan name. No ranking contract for `lib` / `new`.
- No `SearchState`, `ToggleSearch`, overlay, or `Ctrl+Shift+F`. Desktop never calls `search_workspace`.
- Esc order is palette → help → reminder (`apps/multiplexer-desktop/src/main.rs` `handle_key`).

**Evidence:**

- `crates/multiplexer-shell/src/search.rs`
- `crates/multiplexer-shell/src/lib.rs` re-exports `search_workspace`
- `apps/multiplexer-desktop/src/controls.rs` shortcut map (no `ctrl-shift-f`)
- `apps/multiplexer-desktop/src/main.rs` `handle_key`

**Next TDD slice:** `search_hits_rank_files_threads_commands` in `crates/multiplexer-shell/src/search.rs`

Query `lib` hits `src/lib.rs`. Query `new` hits the New chat command **and** a thread titled "New chat" in different groups. Empty query: `hits.is_empty()`.

---

## K. Pop-out panes

**Status:** missing (engine exists, desktop unused)

**Contract:** inspector **Pop out** / `Ctrl+Shift+D`, second GPUI window + Dock `Ctrl+Shift+E`, ghost strip, `Workspace.layout: LayoutForest`, `PopOutInspector` / `DockInspector`.

**Now:**

- `LayoutForest::detach` / `redock` are mutation-tested (`crates/multiplexer-layout`).
- `DesktopChrome` owns a forest. The **desktop app never constructs** `DesktopChrome` or `LayoutForest`. `Workspace` has `ChromeLayout` widths only.
- One `cx.open_window` in `main()`. No Pop out button. No `Ctrl+Shift+D` / `E`.
- `controls.rs` has no pop-out surface or ids.

**Evidence:**

- `crates/multiplexer-layout/src/tree.rs` `detach`, `redock`, `default_outlook`
- `crates/multiplexer-layout/tests/layout.rs` `detach_creates_window_and_redock_restores`
- `crates/multiplexer-shell/src/lib.rs` `DesktopChrome` (tests only)
- `crates/multiplexer-shell/tests/chrome.rs`
- `apps/multiplexer-desktop/src/main.rs` `fn main` single window
- `crates/multiplexer-shell/src/workspace.rs` `ChromeLayout` (no `layout` field)

**Next TDD slice:** `popout_inspector_detaches_and_redocks` in `crates/multiplexer-shell/src/workspace.rs` (uses `multiplexer-layout`)

Detach inspector → `windows().len() == 2` and a ghost in primary. Redock → one window, inspector live. Second detach after redock allocates a new `WindowId`.

---

## L. Settings overlay

**Status:** partial

**Contract:** `Ctrl+,` + palette Settings, fields theme / default model / project copy / Remote / keybindings from `shortcut_map()`, `SettingsState`, `Surface::Settings`, save immediate.

**Now:**

- `UiSettings { mode, density, default_model }` + `settings_open` + `ToggleSettings`.
- Desktop paints `settings_overlay` on F2 (not `Ctrl+,`). Theme/Density cycle buttons. Close. No project copy, no Remote, no keybindings list.
- `Theme::tokens()` always returns `ThemeTokens::dark()` (`apps/multiplexer-desktop/src/theme.rs`). Stored `ThemeMode::Light` does not change paint (plan allows stored-mode-only until tokens wire).
- `set_default_model` does not write `ws.model`.
- `controls.rs` has `HelpOverlay` only. No `Surface::Settings`, no `settings_theme` / `settings_model` / `settings_close`.
- Help text has no `Ctrl+,` line.

**Evidence:**

- `crates/multiplexer-shell/src/settings.rs`
- `crates/multiplexer-shell/src/workspace.rs` `settings`, `settings_open`
- `crates/multiplexer-shell/src/actions.rs` `ToggleSettings`
- `apps/multiplexer-desktop/src/main.rs` `settings_overlay`, F2
- `apps/multiplexer-desktop/src/theme.rs` `ThemeTokens::dark()`
- `apps/multiplexer-desktop/src/controls.rs` `Surface::all()` length 10

**Next TDD slice:** `settings_overlay_applies_default_model` in `crates/multiplexer-shell/src/settings.rs` (or `workspace.rs`)

Open settings, `default_model = "fake"` applies `ws.model == "fake"`. Close clears `settings.open`. Theme flip is stored.

---

## M. Command palette richness

**Status:** missing (static runner still ships)

**Contract:** groups Commands / Panes / Files / Threads, substring + simple fuzzy, namespace prefix, `filter_items(ws, query) -> Vec<PaletteHit>`, empty query = commands + panes not every file.

**Now:**

- `PaletteItem` is `Copy` + `'static`. `filter_items(query)` is case-insensitive substring over `default_items()` (28 static rows). Empty query returns the full catalog.
- No `PaletteNs`, `PaletteHit`, files, threads, or fuzzy subsequence.
- Desktop overlay shows `label` + `hint` only, first 12 hits.
- Stale module comment still says inspector has no Git/Term/Skills (it does).

**Evidence:**

- `crates/multiplexer-shell/src/palette.rs` `default_items`, `filter_items`, `PaletteState::active_item`
- `apps/multiplexer-desktop/src/main.rs` `palette_overlay`

**Next TDD slice:** `palette_filter_includes_files_and_threads` in `crates/multiplexer-shell/src/palette.rs`

Workspace with file `src/lib.rs` and thread "Fix MCP". Query `lib` returns a `file` hit. Query `fix` returns a `thread` hit. Query `mcp` returns the MCP command **and** the thread. Empty query returns commands + panes, not every file.

---

## N. Notifications

**Status:** partial

**Contract:** toast stack top-right of center (max 3 visible), kinds info/ok/warn/error, auto-dismiss info/ok ~4s, cap 8 drop oldest, `DismissToast`, emit on turn / MCP / checkpoint / worktree / copy. Approval and reminder bars stay.

**Now:**

- `Notice` / `NoticeKind` / `push_notice` cap **5** (`crates/multiplexer-shell/src/notices.rs`). Workspace `notices` + `push_notice`.
- Desktop `notice_bar` is a **full-width strip under the title bar**, not a top-right stack of 3. Click dismisses by id. No timer. No Esc dismiss-newest.
- Emits on MCP start/stop and file mention only. Turn finish, checkpoint, copy session, worktree still use `flash` / `term_meta`.
- `flash: Option<String>` still suffixes the status bar.
- Approval and reminder bars still exist (correct).
- Test is `push_caps_at_five_and_dismisses`, not `toast_queue_caps_and_dismisses`. No `Surface::ToastStack`.

**Evidence:**

- `crates/multiplexer-shell/src/notices.rs` `NOTICE_CAP = 5`
- `crates/multiplexer-shell/src/workspace.rs` `notices`, `push_notice`
- `apps/multiplexer-desktop/src/main.rs` `notice_bar`, `flash`, `copy_session`, `pump`

**Next TDD slice:** `toast_queue_caps_and_dismisses` in `crates/multiplexer-shell/src/notices.rs`

Nine pushes → len 8 (drop oldest). Dismiss newest removes the last. Status/detail can see the top toast text.

---

## plan/10 surfaces still absent

These are promised in `plan/10-ui-pane-system.md` and are not present as product surfaces in the current desktop (headless crates noted where they exist without projection).

**Layout / panes**

- Center build pane: editor + diff, recursive splits (center is chat + chips only).
- Right-bar tabs as specified: Browser, HAR, Diff, Terminal-as-tab, Model info (live tabs are Session/Cores/MCP/Points/Git/Term/Skills/Files/Activity/Agents).
- Tab drag-reorder, drag-out, right-bar split showing two tabs.
- `PaneRegistry` / `PaneDescriptor` / content-state keyed by `PaneId`.
- Desktop projection of `LayoutForest` (crate exists). Split-anything UI, resize of split ratios in the tree (rail drag only).
- Pop-out every pane to an OS window, ghost slot, dock, `Ctrl+Shift+D` / `Ctrl+Shift+E` (K above).
- Saved layouts: `.multiplexer/layout.json`, presets (Debug / Focus / Review), restore.
- Sidebar as a pane (pop out, dock right). Section reorder.
- Bottom Ghostty-class PTY with slide animation (live strip is `cmd.exe` via `spawn_command`, `Ctrl+\`` toggles height only).

**Left rail / lists**

- Virtualized thread / agent / activity lists.
- Thread model badge, unread/attention indicator (preview + status string exist).
- Agents section as parent→child dashboard (left rail lists session ids; inspector Agents is a local thread dump).

**Design system / chrome**

- Applied light theme and system-follow (tokens exist in `multiplexer-theme`; desktop hardcodes dark).
- GPU motion: sidebar collapse, tab swap, pop-out, theme cross-fade, `prefers-reduced-motion`.
- Elevation / token use is incomplete vs plan/10 §5.1 (many raw `hsla` literals in `main.rs`).

**Palette / keyboard**

- Four-namespace fuzzy palette with context rank and async file results (M above).
- User-editable JSON keybinding map, context-scoped bindings, "show all keybindings".
- Core nav still missing: `Ctrl+Tab` / `Ctrl+Shift+Tab` focus, `Ctrl+\` split, `Ctrl+W` close pane, `Ctrl+Shift+B` as specified (live uses `Ctrl+[` / `]`).

**Testing (plan/10 §9)**

- GPUI component / element tests for panes.
- Visual layout snapshots.
- Desktop e2e for split / pop-out / palette / theme.

**Search**

- Native workspace **content** search / virtualized 10k index (explicitly later in plan/36). Name search overlay is also still absent (J).

Later engines called out in plan/36 §5 (not this wave, still absent): native editor (plan/09), CDP browser + HAR (plan/11, 12), in-process grok, MCP marketplace / Customize writers, live subagent spawn, Tailscale Serve / relay, account billing.

---

## Prioritized fix order (15)

Parent implements these next. P0 first. Each item is one TDD slice on existing engines (no PTY, no CDP, no in-process grok).

| Pri | # | Item | First failing test | File |
|---|---|---|---|---|
| P0 | 1 | Finish Files as a real tree (`FileNode`, expand/collapse, Reveal + Reload, selected `*`) | `file_tree_select_expand_and_mention` | `crates/multiplexer-shell/src/workspace.rs` |
| P0 | 2 | MCP projection on actions + honesty line + disable Start/Stop by state | `mcp_start_sets_ready_and_stop_releases` (extend via `apply_layout_action`) | `crates/multiplexer-shell/src/actions.rs` |
| P0 | 3 | Worktree Create draft → `git.worktree.create` (keep New WT as copy hint) | `worktree_create_draft_dispatches_rpc` | `crates/multiplexer-shell/src/bindings.rs` |
| P0 | 4 | Model pick by id (`SelectModel(usize)`, `/model <id>`, Session `*`) | `select_model_sets_workspace_and_rpc` | `crates/multiplexer-shell/src/workspace.rs` |
| P0 | 5 | Toast stack cap 8, dismiss newest, emit on turn/copy/checkpoint | `toast_queue_caps_and_dismisses` | `crates/multiplexer-shell/src/notices.rs` |
| P1 | 6 | Settings: `Ctrl+,`, apply `default_model` to `ws.model`, keybinding list | `settings_overlay_applies_default_model` | `crates/multiplexer-shell/src/settings.rs` |
| P1 | 7 | Search overlay `Ctrl+Shift+F` over files + threads + commands | `search_hits_rank_files_threads_commands` | `crates/multiplexer-shell/src/search.rs` |
| P1 | 8 | Palette namespaces + `filter_items(ws, q)` + fuzzy subsequence | `palette_filter_includes_files_and_threads` | `crates/multiplexer-shell/src/palette.rs` |
| P1 | 9 | Agents tab as typed `AgentRow` (parent `None`) + palette `agent:` | `agents_tab_projects_thread_tree` | `crates/multiplexer-shell/src/workspace.rs` |
| P1 | 10 | Session usage snapshot + `record_turn` on `grok -p` finish | `usage_snapshot_formats_session_detail` | `crates/multiplexer-shell/src/workspace.rs` |
| P2 | 11 | Remote status: local URL + tailscale detect, no Serve | `remote_status_lists_local_and_tailscale_detect` | `crates/multiplexer-shell/src/workspace.rs` |
| P2 | 12 | Pop out inspector: `Workspace.layout.detach(PaneId)` + second window | `popout_inspector_detaches_and_redocks` | `crates/multiplexer-shell/src/workspace.rs` |
| P2 | 13 | Thin `model.list` / `model.select` router stubs | `select_model_sets_workspace_and_rpc` (RPC arm) | `crates/multiplexer-server/src/server.rs` |
| P2 | 14 | Thin `telemetry.usage` echo of `UsageSnapshot` | `usage_snapshot_formats_session_detail` (JSON echo) | `crates/multiplexer-server/src/server.rs` |
| P2 | 15 | Honest catalog: `controls::REQUIRED_IDS` + `Surface` for Files, Agents, Settings, Search, Toast, Pop out; slash `/files /agents /usage /settings /search` | desktop `controls.rs` `all_required_ids_present` | `apps/multiplexer-desktop/src/controls.rs` |

Do not pull editor, Browser/HAR, PTY, or Tailscale Serve forward to fill the list. A fake pane is worse than a missing tab.
