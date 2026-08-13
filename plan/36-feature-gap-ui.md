# 36: Feature-gap UI (honest inventory + this-wave 12)

**Status:** Implementation brief for the parent wave (not a new differentiator).
**Owner:** Desktop / shell UI
**Depends on:** `docs/PLAN-CONTEXT.md`, `docs/DECISIONS.md`, `plan/04`, `plan/10`, `plan/19`, `plan/21`, `plan/25`, `plan/26`
**Feeds:** parent implementation in `multiplexer-shell`, `apps/multiplexer-desktop`, thin `multiplexer-server` dispatch
**Wave constraint:** no new native engines. No PTY. No CDP. No in-process grok.

This document is consistent with `docs/PLAN-CONTEXT.md`. `docs/DECISIONS.md` wins on conflicts. It does not reopen D1, D4, D8, D10, or D14. It ranks **missing UI** against what the product already promised, then names the **twelve** surfaces the parent may implement **now** because the headless crates already exist.

---

## 1. Honesty rule

The desktop is a working Outlook chrome over `grok -p`, a command palette, slash commands, a one-shot terminal strip, and **seven inspector tabs** (Session, Cores, MCP, Points, Git, Term, Skills). That is a real control surface. It is **not** the product in `plan/00` / `plan/10` / `plan/19`.

Two kinds of gap exist:

| Kind | Meaning | This wave? |
|---|---|---|
| **UI on existing engines** | Headless model + GPUI projection + (maybe) a thin JSON-RPC dispatch that is already a wire constant | **Yes.** These twelve. |
| **New native engine** | Rope editor, Ghostty PTY, CDP browser/HAR, in-process `xai-grok-shell`, Tailscale serve, account billing | **No.** Ranked later. |

A toast that says "HAR ready" while `browser.*` / `har.*` stay `method not found` is a lie. A Files tab that selects paths from `list_project_tree` is not. This brief prefers the second kind.

**Verified today (desktop `main.rs` + `multiplexer-shell`, 2026-08-12):**

- Left rail: thread list, New, Delete. No Projects / Agents / Activity sections.
- Center: transcript + composer + slash hint. No editor. No diff. No split.
- Right rail: seven **text** inspector bodies + a few buttons. Files are a dump inside Cores. MCP is inventory + Reload. Git "New WT" pastes `git worktree add ...` into the composer.
- Bottom: four-line `cmd.exe` strip (`spawn_command`), not a PTY. Builtins: clear, help, cores, mcp, git, points.
- Palette (`Ctrl+K` / `Ctrl+P`): static `default_items()` catalog, substring match on id/label/hint. No files, no threads, no fuzzy rank, no namespaces.
- Slash: `/new /stop /help /cp /cores /mcp /git /term /skills /palette /model`. `/model` **cycles**.
- Model catalog is hardcoded `grok`, `grok-4.6`, `fake`. `CycleModel` only.
- Agent path is `spawn_grok_turn` (`grok -p` on a worker thread). Not the in-process adapter.
- `multiplexer-layout` can detach/redock. The desktop never constructs a `LayoutForest`.
- `flash: Option<String>` is a one-line copy ack. Not a notification stack.
- No settings surface. No search. No usage/account. No remote/Tailscale chrome. No orchestration dashboard. No pop-out window.

**Server dispatch that already works:** `session.*`, `turn.send`, `approval.respond`, `git.worktrees`, `git.worktree.create`, `checkpoint.list/create/revert`, `terminal.create/list/input/kill`, `system.ping/hello`.

**Wire constants with no router arm:** `model.list/select/get`, `telemetry.usage/resources`, `orchestration.*`, `remote.*`, `fs.list/read`, `browser.*`, `har.*`, `auth.*`, `git.status/diff`. Calling them today returns `method not found`. This wave may add **thin stubs** for the twelve. It may not implement CDP, PTY, or in-process grok to make those stubs honest.

---

## 2. Promised surfaces vs now

Every row is a product promise (PLAN-CONTEXT, `plan/10`, `plan/19`, or the Orca baseline). "Now" is the live desktop.

| # | Promised surface | Now | Wave |
|---|---|---|---|
| A | **Editor** (rope, multi-cursor, Vim, LSP, inline diff-apply, diff comments) | Absent. Center is chat only. | **Later** (engine) |
| B | **Browser / HAR** (detect/launch system browsers, CDP, Design Mode, waterfall, replay) | Absent. Wire methods exist, unused. | **Later** (CDP) |
| C | **Files tree** (right-bar Files tab, open / reveal / `@` mention) | `Vec<String>` dump in Cores. `CycleFile` rotates the list. | **This wave (1)** |
| D | **MCP start/stop** (live supervisor state, start, stop, crash badge) | Inventory text + Reload. `Supervisor` exists, unused by UI. | **This wave (2)** |
| E | **Worktree create UI** (path, branch, `create_branch`) | List + reminder + composer hint. Server already implements `git.worktree.create`. | **This wave (3)** |
| F | **Orchestration dashboard** (parent → children, status, budget) | No Agents tab. Threads have a status string only. | **This wave (4)** (read model of **local** threads; no spawn engine) |
| G | **Remote / Tailscale** (list, connect, MagicDNS, Serve) | No chrome. `remote.*` not dispatched. | **This wave (5)** (status + detect; no Serve) |
| H | **Model picker** (list `[model.*]`, select per thread) | Cycle button / `/model`. Three hardcoded ids. | **This wave (6)** |
| I | **Usage / billing** (`telemetry.usage`, account, freemium gate) | CPU bars only. No token/turn/cost. | **This wave (7)** (local snapshot; no account) |
| J | **Search** (native workspace search, Orca baseline) | None. Palette is command-only. | **This wave (8)** (files + threads + commands; no content index) |
| K | **Pop-out panes** (every pane → OS window, redock) | Layout crate tested. Desktop is one window. | **This wave (9)** |
| L | **Settings** (theme, default model, project, keybindings) | Help overlay only. | **This wave (10)** |
| M | **Command palette richness** (commands / panes / files / agents, fuzzy, context) | Static 27 rows, substring. | **This wave (11)** |
| N | **Notifications** (toasts for turn done, approval, MCP crash, checkpoint) | Single `flash` string. Approval/reminder are dedicated bars. | **This wave (12)** |

Rows A and B are **not** in the twelve. They stay later even though they are the loudest differentiators. Shipping a fake editor or a fake HAR pane would make the product look finished and be wrong.

---

## 3. Ranking rule for this wave

Pick the gap if **all** of these hold:

1. A user can **see and operate** it without a new engine.
2. The headless change lives in `multiplexer-shell` (and maybe a thin server arm).
3. A named unit test can fail first.
4. It unblocks daily use of the **current** `grok -p` + inspector chrome.

Reject if it needs Ghostty, CDP, rope/LSP, in-process embedding, a billing backend, or a Tailscale daemon we do not own.

**Explicit later (not these twelve):** full editor (`plan/09`), browser + HAR (`plan/11`, `plan/12`), Ghostty PTY (`plan/08`), in-process grok (`plan/03`, D10), MCP marketplace / Customize writers (`plan/26` remainder), Tailscale Serve / relay tickets (`plan/14`, `plan/23`), account entitlements (D30), content search / ripgrep index (`plan/10` §6 files namespace at workspace scale), 4-way approval chrome (allow_once / allow_always: wire is ready, not a this-wave blocker), mobile (`plan/13`).

---

## 4. This wave: twelve gaps

Implement in this order. Each subsection is the parent contract: user-visible behavior, headless model, test name.

### 4.1 Files tree interaction

**Why now:** `list_project_tree` already walks the project (depth 2, skips `.git` / `node_modules` / `target`). The Cores tab prints the paths. The promised right-bar **Files** tab (`plan/10` §2.3) is missing. Clicking a file is the cheapest path into "this is a workspace," without an editor.

**User-visible:**

- Eighth inspector tab **Files** (`InspectorTab::Files`, label `Files`).
- Tree rows: directories first, expand/collapse, selected row marked `*`.
- Buttons: **Reveal** (copy absolute path), **@ mention** (insert `` `@path` `` at the composer cursor), **Reload**.
- Double-click / Enter on a file selects it and copies the relative path into the status/flash line. It does **not** open an editor.
- Palette namespace `file:` lists the same rows.

**Headless model:**

- `FileNode { path, name, is_dir, expanded }`. `Workspace.files` becomes `Vec<FileNode>` (keep `set_files` as a compatibility loader from `Vec<String>` by treating trailing `/` as dirs).
- `selected_file: Option<String>`, `toggle_file_expand(path)`, `select_file(path)`.
- `ClientAction::{SelectFile, ToggleFileExpand, CopyFilePath, InsertFileMention, RefreshFiles}`.
- `InspectorAction` gains the same four host clicks. `tab_buttons(Files)` is not empty.
- Host: `RefreshFiles` re-runs `list_project_tree`. Copy uses the existing clipboard path.

**Test name:** `file_tree_select_expand_and_mention`

Assert: expand a dir, select a file, `InsertFileMention` puts `` `@src/lib.rs` `` at `cursor`, collapse hides children from `files_visible()`.

---

### 4.2 MCP start / stop

**Why now:** `multiplexer-mcp::Supervisor` is a tested state machine (`Spawned` / `Ready` / `Crashed` / `Stopped` / `Failed`). The MCP tab only reloads `~/.grok/config.toml`. Plan/21 and plan/26 both say the differentiator is **lifecycle visibility**, not another TOML dump.

**User-visible:**

- Each MCP row shows name, transport, command, and a **state badge** (`stopped` / `ready` / `crashed` / `failed`).
- Buttons: **Start**, **Stop**, **Reload**. Start is disabled when `ready`. Stop is disabled when `stopped`.
- Start/stop write a toast (gap 12). A crash badge stays visible until Start or Reload.
- This wave does **not** spawn a real child. The supervisor's existing instant acquire/release is the source of truth. The UI must say "supervised (in-process table)" in the tab body so we do not pretend npx is running.

**Headless model:**

- `McpRow` gains `state: McpLife` (`Stopped` default, `Ready`, `Crashed`, `Failed`) and `selected: bool`.
- `Workspace` holds `mcp_supervisor` **or** a pure projection updated by `apply_mcp_life(name, state)`. Prefer a projection so `Workspace` stays `Clone` + test-friendly: `start_mcp(name)` sets `Ready`, `stop_mcp(name)` sets `Stopped`.
- `ClientAction::{StartMcp, StopMcp, SelectMcp}`. `host_call(StartMcp)` is `NeedsHost` so the desktop can drive a real `Supervisor` later; `apply_layout_action` still flips the projection for tests.
- `mcp_detail()` includes the badge per row.

**Test name:** `mcp_start_sets_ready_and_stop_releases`

Assert: unknown name is a no-op; start `linear` → `Ready`; stop → `Stopped`; `mcp_detail()` contains `ready` then `stopped`.

---

### 4.3 Worktree create UI

**Why now:** `git.worktree.create` is implemented (`cwd`, `path`, `branch`, `create_branch`). The Git tab's **New WT** button pastes a shell string. That is a hint, not a UI.

**User-visible:**

- Git tab grows a three-field draft: **path**, **branch**, **create branch** (toggle). Prefill path `../mux-<branch>` and branch `feat`.
- Button **Create** (replaces "New WT" as the primary). On success the worktree list refreshes and the new path is selected. On error the body shows the RPC message.
- Reminder bar is unchanged (still dismissible). Create must not clobber an existing worktree; the server already errors.

**Headless model:**

- `WorktreeDraft { path: String, branch: String, create_branch: bool }`.
- `ClientAction::CreateWorktree`. `host_call` becomes `Rpc { method: "git.worktree.create", params_json }` with `cwd` from `ActionContext.project`.
- `NewWorktreeHint` stays as a secondary "copy git command" action so existing tests do not rot; the primary control id becomes `create_worktree`.

**Test name:** `worktree_create_draft_dispatches_rpc`

Assert: draft `{path: "../mux-feat", branch: "feat", create_branch: true}` produces params containing those three fields plus `cwd`. Empty path does not dispatch.

---

### 4.4 Orchestration dashboard (local threads)

**Why now:** The left rail is a chat list. Plan/06 §4.3 and plan/19 Phase 5.5 promise a live parent → child dashboard. We do **not** have `spawn_subagent`. We **do** have threads with `id`, `title`, `status`. A read-only **Agents** tab that projects that tree is honest. A fake fan-out animation is not.

**User-visible:**

- Ninth inspector tab **Agents** (`InspectorTab::Agents`).
- Rows: one per thread. Columns: title, status, model, message count. Selected thread marked.
- Body copy: "Local threads only. Subagent spawn is not wired."
- Palette namespace `agent:` jumps to a thread and opens this tab.
- When `orchestration.list` is later dispatched, the same rows grow a `parent_id`. This wave may stub `orchestration.list` as `{ subagents: [] }` plus the local thread projection so the client path exists.

**Headless model:**

- `AgentRow { id, title, status, model, messages, parent: Option<String> }`.
- `Workspace::agent_rows()` derives from `threads` (parent always `None` this wave).
- `ClientAction::SelectTab(Agents)` is enough; selecting a row reuses `SelectThread`.
- `agents_detail()` formats the table.

**Test name:** `agents_tab_projects_thread_tree`

Assert: two threads → two rows; `parent` is `None`; selecting row 1 sets `selected == 1`; detail contains both titles.

---

### 4.5 Remote / Tailscale status

**Why now:** Plan/14 and plan/23 are Phase 4 engines. The desktop today has no hint that Multiplexer is server-centric or that a tailnet exists. A **status** panel is UI. Serve, tickets, and DPoP are not.

**User-visible:**

- Settings (gap 10) section **Remote**, also reachable as Session-tab lines.
- Rows: `local` (`ws://127.0.0.1`, status from `ConnectionState`), `tailscale` (`detected` / `not found`).
- Detect = `tailscale` on `PATH` (or `TAILSCALE_EXE`). Do not run `tailscale serve`. Do not mint tickets.
- Buttons: **Copy local URL** (placeholder `ws://127.0.0.1:8787` until listen lands), **Refresh detect**.
- Body copy: "Connect UI only. Relay and Serve are later."

**Headless model:**

- `RemoteRow { id, kind: Local | Tailscale, endpoint, status }`.
- `Workspace.remotes: Vec<RemoteRow>`, `set_remotes`, `tailscale_detected: bool`.
- `ClientAction::RefreshRemote`. Host fills detect + local endpoint. Tests inject rows.
- Optional thin `remote.list` stub: `{ remotes: [...] }` mirroring the projection.

**Test name:** `remote_status_lists_local_and_tailscale_detect`

Assert: default has a `local` row; `set_tailscale_detected(true)` shows `detected` in `remote_detail()`; false shows `not found`.

---

### 4.6 Model picker

**Why now:** Session tab has **Model** = cycle. `/model` cycles. The catalog is three literals. Wire already names `model.list` / `model.select`. Users cannot pick `ds-flash` without us hardcoding it.

**User-visible:**

- Session tab lists every catalog id. The active model is marked `*`. Click selects (does not cycle).
- Title bar shows the active model (already does). Palette command **Select model…** opens Session and focuses the list.
- `/model <id>` selects that id when present; bare `/model` still cycles (compat).
- This wave may seed the catalog from a static plus optional `~/.grok/config.toml` `[model.*]` keys **without** loading the grok-build runtime. Unknown id stays on the list as a label, not a silent drop.

**Headless model:**

- `Workspace::select_model(id) -> bool` (false if unknown). Keep `cycle_model`.
- `ClientAction::SelectModel` needs an id. `ClientAction` is currently `Copy`. Add `SelectModel` as a host action with id in `ActionContext.model` **or** add `Workspace.pending_model: Option<String>` set by UI then `ClientAction::ApplyModel`. Prefer `select_model` on the workspace plus `ClientAction::ApplySelectedModel` if we must stay `Copy`. Cleaner: change `ClientAction` to carry data for this one arm (`SelectModel` with a small interned index into `models`). Use **index**: `ClientAction::SelectModel(usize)`.
- `host_call(SelectModel)` → `Rpc { "model.select", { thread/session, model } }` when a session exists, else `Local` (catalog only).
- Thin server stub for `model.list` / `model.select` is allowed.

**Test name:** `select_model_sets_workspace_and_rpc`

Assert: catalog `[grok, ds-flash]`; `SelectModel(1)` sets `model == "ds-flash"`; `host_call` with a session id is `model.select` containing `ds-flash`; out-of-range index is a no-op.

---

### 4.7 Usage snapshot (not billing)

**Why now:** Orca baseline and D30 promise account/usage. There is no account. CPU bars are not usage. A **local session snapshot** (turns sent, last turn ok, optional token line if `grok -p` printed one) is honest. A subscribe wall is not.

**User-visible:**

- Session tab gains a **Usage** block: turns this session, last turn duration (if known), last prompt chars, "account: local / not signed in".
- No dollar amount unless a future stub provides `cost_hint`. Default prints `n/a`.
- Palette: **Show usage** selects Session and scrolls the block into the body (body is text; putting usage first is enough).

**Headless model:**

- `UsageSnapshot { turns: u32, last_ms: Option<u64>, last_prompt_chars: usize, tokens_in: Option<u64>, tokens_out: Option<u64>, account: UsageAccount }` where `UsageAccount` is `Local` this wave.
- `Workspace.usage`. `record_turn(ok, ms, prompt_chars)` increments on every finished `grok -p` / send.
- `session_detail()` includes the block.
- Thin `telemetry.usage` stub may echo `Workspace.usage` as JSON. No persistence.

**Test name:** `usage_snapshot_formats_session_detail`

Assert: two `record_turn` calls → `turns: 2` in `session_detail()`; `account` contains `local`; missing tokens print `n/a`.

---

### 4.8 Search (names, not content)

**Why now:** Plan/10 §6 and the Orca bar require native search. A ripgrep index is an engine. Searching **file paths + thread titles + command ids** is a palette/search overlay on data we already hold.

**User-visible:**

- `Ctrl+Shift+F` (and palette item **Search**) opens a search overlay (same chrome family as the palette, different id).
- Query filters three groups: Files, Threads, Commands. Arrow keys move; Enter runs: file → select in Files tab; thread → `SelectThread`; command → existing `ClientAction`.
- Empty query shows a short hint, not the entire repo.
- No file **contents**. The hint says "Names only."

**Headless model:**

- `SearchHit { group: File | Thread | Command, id, label, action }`.
- `search_hits(ws, query) -> Vec<SearchHit>`.
- `SearchState { open, query, selected }` parallel to `PaletteState`.
- `ClientAction::ToggleSearch`, `CloseSearch`. Esc closes search before palette before help.

**Test name:** `search_hits_rank_files_threads_commands`

Assert: query `lib` hits a file `src/lib.rs`; query `new` hits the New chat command **and** a thread titled "New chat" in different groups; empty query is empty or hint-only (`hits.is_empty()`).

---

### 4.9 Pop-out panes

**Why now:** Differentiator #5 and `plan/10` §4. `LayoutForest::detach` / `redock` are mutation-tested. The desktop is a single window with two rails. Wiring **one** pop-out (inspector **or** terminal strip) proves the forest without a pane registry rewrite.

**User-visible:**

- Inspector tab bar gains **Pop out**. Shortcut `Ctrl+Shift+D` pops the **focused** rail (right if focus is inspector-ish, else the terminal strip).
- A second GPUI window shows that pane's body + a **Dock** button (`Ctrl+Shift+E`).
- Closing the pop-out window docks. The primary window keeps a ghost strip ("Inspector popped out") so the layout does not collapse to a surprise two-column.
- This wave does **not** drag-dock, does not split-anything, does not persist `.multiplexer/layout.json`.

**Headless model:**

- `Workspace.layout: LayoutForest` initialized to left | center | right (three leaves). Terminal is a fourth leaf under a vertical split if we can do it without breaking `ChromeLayout` widths. Minimum: three leaves `PaneId(0..=2)` = chats, build, inspector; pop-out inspector calls `layout.detach(PaneId(2))`.
- `ClientAction::{PopOutInspector, DockInspector}`. `apply_layout_action` calls detach/redock.
- Desktop maps `layout.windows().len() > 1` to "open or close the extra window."

**Test name:** `popout_inspector_detaches_and_redocks`

Assert: detach inspector → `windows().len() == 2` and a ghost in primary; redock → one window, inspector live; second detach after redock allocates a new `WindowId`.

---

### 4.10 Settings overlay

**Why now:** Theme lives in `theme.rs` as a constant. Model default is a constructor argument. Keybindings exist in `controls.rs` and are undiscoverable except F1. There is no settings surface.

**User-visible:**

- `Ctrl+,` and palette **Settings** open a modal (same overlay family as help).
- Fields: **theme** (dark / light; this wave can swap `Theme` tokens), **default model** (writes `Workspace.model` via `select_model`), **project path** (read-only display + **Copy**), **Remote** block (gap 5), **keybindings** list from `controls::shortcut_map()`.
- Save is immediate (no dirty buffer). Esc closes.
- Does **not** edit `~/.grok/config.toml` and does not write secrets.

**Headless model:**

- `SettingsState { open, theme: ThemeMode, default_model: String }`. `ThemeMode = Dark | Light`.
- `Workspace.settings`. `ClientAction::ToggleSettings`.
- `Surface::Settings` in `controls.rs` with ids `settings_theme`, `settings_model`, `settings_close`.
- Help text gains the `Ctrl+,` line.

**Test name:** `settings_overlay_applies_default_model`

Assert: open settings, `default_model = "fake"` applies `ws.model == "fake"`; close clears `settings.open`; theme flip is stored.

---

### 4.11 Command palette richness

**Why now:** `plan/10` §6 promises four namespaces, fuzzy rank, and context. `filter_items` is a case-insensitive substring over a static vec. That is a command runner, not a palette.

**User-visible:**

- Results grouped: **Commands**, **Panes** (inspector tabs + pop-out + settings + search), **Files** (from the tree), **Threads** (from the rail).
- Match: substring still works; add a simple fuzzy (contiguous subsequence) so `mcp` still hits MCP and `wt` can hit "Create worktree" if we add that row.
- Each row shows namespace prefix (`cmd`, `pane`, `file`, `thread`) and the existing hint.
- Enter runs the row. Files go to `SelectFile` + Files tab. Threads go to `SelectThread`.
- Dynamic rows are rebuilt from `Workspace` on each query. Static catalog stays the command source.

**Headless model:**

- `PaletteItem` gains `namespace: PaletteNs` (or a parallel `DynamicItem` so `Copy` static items stay `Copy`). Dynamic file/thread rows cannot be `'static`. Change `filter_items(query) -> Vec<PaletteItem>` to `filter_items(ws, query) -> Vec<PaletteHit>` where `PaletteHit` owns `String` labels and a `ClientAction`.
- Keep `default_items()` for commands. Tests that call `filter_items("mcp")` need a `Workspace` argument (breaking, local to shell).
- `PaletteState::active_item` uses the new hit list.

**Test name:** `palette_filter_includes_files_and_threads`

Assert: a workspace with file `src/lib.rs` and thread title "Fix MCP" ; query `lib` returns a `file` hit; query `fix` returns a `thread` hit; query `mcp` still returns the MCP command **and** the thread; empty query returns commands + panes, not every file.

---

### 4.12 Notifications

**Why now:** Approvals and worktree reminders have bars. Everything else screams into the terminal strip or a single `flash`. Turn-complete, MCP crash, checkpoint create, remote detect, and copy-ack should be a **stack**, not a log the user misses.

**User-visible:**

- Toast stack, top-right of the center pane (max 3 visible). Each toast: kind (info / ok / warn / error), text, dismiss.
- Auto-dismiss info/ok after ~4s (desktop timer). Warn/error stay until click or Esc (Esc dismisses newest).
- Emit on: turn finished, turn failed, checkpoint created, MCP start/stop/crash, worktree create ok/err, copy session/path.
- Approval and reminder **bars stay**. Toasts do not replace gated decisions.

**Headless model:**

- `Toast { id, kind: ToastKind, text }`. `Workspace.toasts: Vec<Toast>` capped at 8.
- `push_toast`, `dismiss_toast(id)`, `dismiss_newest`.
- `ClientAction::DismissToast`. Desktop maps timer → `dismiss_newest` for info/ok only.
- `Surface::ToastStack` with id `toast_dismiss`.

**Test name:** `toast_queue_caps_and_dismisses`

Assert: nine pushes → len 8 (drop oldest); dismiss newest removes the last; detail/status can see the top toast text.

---

## 5. Later (not this wave)

These stay promised. They are not parent work this wave.

| Gap | Why later | Lands (plan/19) |
|---|---|---|
| Native editor, inline diff-apply, Vim, LSP | New engine (`plan/09`). Center stays chat. | Phase 2 |
| System browser + HAR waterfall/replay + Design Mode | CDP (`plan/11`, `plan/12`). No fake pane. | Phase 3 |
| Ghostty-class PTY, splits, agent terminal tool | PTY (`plan/08`). Keep the cmd strip. | Phase 1.7 (blocked on embed) |
| In-process grok-build / ACP fallback | D10 spike. Keep `grok -p`. | Phase 0/1 |
| MCP Customize writers, marketplace, hooks trust UI | plan/26 beyond start/stop badges | Phase 2.8 |
| Live subagent fan-out + budget dashboard | Needs scheduler (D11), not a thread list | Phase 5.4 / 5.5 |
| Tailscale Serve, relay tickets, DPoP, SSH remotes | plan/14, plan/23 | Phase 4 |
| Account, entitlements, paid-tier billing | D30. Usage snapshot is enough. | Phase 6 / GTM |
| Workspace **content** search, virtualized 10k-file index | Search engine | Phase 2.7 remainder |
| Split-anything, saved layouts, drag-dock | Pane engine beyond one pop-out | Phase 2.3 to 2.5 |
| 4-way approval (`allow_once` / `allow_always`) | Wire is ready; not a missing *surface* vs cycle/files | with provider embed |
| Mobile companion | plan/13 | Phase 4.1 (MVP gate, not this desktop wave) |

If a later item is pulled forward, it must ship behind a real engine or stay labeled stub in the UI body (same honesty rule as MCP start/stop).

---

## 6. Headless / wire additions (summary)

All of this is `multiplexer-shell` plus optional thin router arms. No new crate.

| Area | Types / actions | Server |
|---|---|---|
| Files | `FileNode`, `InspectorTab::Files`, `SelectFile`, `ToggleFileExpand`, `InsertFileMention` | none (local `list_project_tree`) |
| MCP | `McpLife` on `McpRow`, `StartMcp`, `StopMcp` | none this wave (in-process `Supervisor` optional) |
| Worktree | `WorktreeDraft`, `CreateWorktree` | existing `git.worktree.create` |
| Agents | `InspectorTab::Agents`, `agent_rows()` | optional `orchestration.list` → `{subagents:[]}` |
| Remote | `RemoteRow`, `RefreshRemote` | optional `remote.list` stub |
| Model | `SelectModel(usize)`, `select_model` | optional `model.list` / `model.select` stub |
| Usage | `UsageSnapshot`, `record_turn` | optional `telemetry.usage` echo |
| Search | `SearchState`, `search_hits` | none |
| Pop-out | `Workspace.layout`, `PopOutInspector`, `DockInspector` | none |
| Settings | `SettingsState`, `ToggleSettings` | none |
| Palette | `PaletteHit`, `filter_items(ws, q)` | none |
| Toasts | `Toast`, `push_toast` | none |

`InspectorTab::all()` grows from 7 to 9 (`Files`, `Agents`). `controls::REQUIRED_IDS` and `Surface::all()` gain Files / Agents / Settings / Search / Toast / Pop out. Palette default catalog adds the new commands (create worktree, settings, search, pop out, show usage, start/stop MCP).

Slash additions (small, keep parser exhaustive): `/files`, `/agents`, `/usage`, `/settings`, `/search`. Unknown still prints the hint list.

---

## 7. Test name index (this wave)

Parent implements these names. One behavior test each. Property tests are welcome on `search_hits` and `filter_items` but are not the gate.

| # | Test name | Crate |
|---|---|---|
| 1 | `file_tree_select_expand_and_mention` | `multiplexer-shell` |
| 2 | `mcp_start_sets_ready_and_stop_releases` | `multiplexer-shell` |
| 3 | `worktree_create_draft_dispatches_rpc` | `multiplexer-shell` (bindings) |
| 4 | `agents_tab_projects_thread_tree` | `multiplexer-shell` |
| 5 | `remote_status_lists_local_and_tailscale_detect` | `multiplexer-shell` |
| 6 | `select_model_sets_workspace_and_rpc` | `multiplexer-shell` |
| 7 | `usage_snapshot_formats_session_detail` | `multiplexer-shell` |
| 8 | `search_hits_rank_files_threads_commands` | `multiplexer-shell` |
| 9 | `popout_inspector_detaches_and_redocks` | `multiplexer-shell` (uses `multiplexer-layout`) |
| 10 | `settings_overlay_applies_default_model` | `multiplexer-shell` |
| 11 | `palette_filter_includes_files_and_threads` | `multiplexer-shell` |
| 12 | `toast_queue_caps_and_dismisses` | `multiplexer-shell` |

Desktop `inspector.rs` / `controls.rs` tests update in the same wave so tab buttons and `REQUIRED_IDS` stay honest. No e2e gate for these twelve. No cargo-mutants requirement beyond existing shell floors.

---

## 8. Non-goals

- Do not add a text-area "editor" and call it `plan/09`.
- Do not add a Browser/HAR tab that renders lorem or a screenshot file.
- Do not spawn MCP children, `tailscale serve`, or a billing HTTP client.
- Do not replace `grok -p` with in-process embedding in this wave.
- Do not persist layout, settings, or usage beyond process lifetime unless a test needs a temp file.
- Do not invent wire methods. Reuse `plan/04` names or stay local.

---

## 9. Open questions (do not block the twelve)

1. **Listen port for "Copy local URL".** Until `multiplexer-server` listens, the copied URL is a documented placeholder. Fine.
2. **`ClientAction` stays `Copy` vs grows payloads.** Index-based `SelectModel(usize)` / `SelectFile` via `Workspace.selected_file` avoids an enum change storm. Parent picks one style and uses it for Files + Model.
3. **Theme light mode.** Tokens in `theme.rs` are dark-only. Settings may store `ThemeMode::Light` and still render dark until tokens exist. The test asserts the **stored** mode.
4. **Orchestration stub vs silence.** Prefer a labeled empty `orchestration.list` over hiding the Agents tab.

No new D-numbers. This brief does not propose product decisions; it ranks missing UI.
