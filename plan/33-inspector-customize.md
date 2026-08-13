# 33: Inspector Customize Surface (Rows, Not a String Dump)

**Status:** Planning (authored by subagent, pending implementation)
**Owner:** UI / shell model
**Depends on:** `10-ui-pane-system.md`, `21-mcp-lifecycle-supervisor.md`, `26-mcp-skills-ui.md`, `24-resource-manager.md`, `25-worktree-hooks.md`
**Feeds:** `15-testing-strategy.md`, `19-roadmap-and-milestones.md`

This document is consistent with `docs/PLAN-CONTEXT.md`. New decisions proposed here are
numbered **D77+** in the style of `docs/DECISIONS.md`. They are proposals, not locked.

**Locked decisions applied (D1, D13, D18, D21, D23, D33):**
- **D1** : Rust + GPUI. The right rail stays a GPUI view. The row model is pure Rust in
  `multiplexer-shell` with no GPUI types.
- **D13** : row specs live in `multiplexer-shell`. Desktop (`apps/multiplexer-desktop`) is a
  thin renderer. Config and supervisor stay in `multiplexer-mcp`.
- **D18** : refresh I/O is host-owned and bounded. The projector never walks the disk.
- **D21 / D33** : `inspector_rows` is core logic. Mutation score ≥70% is the merge floor.
- **D23** : MCP env, headers, and hook commands never appear as resolved secrets in a row.

**Relationship to plan/21 and plan/26:** plan/21 owns process lifecycle. plan/26 owns the
full Customize editors (add/edit/remove, marketplace, trust). This doc is the **Phase 0.4
control surface**: the right rail becomes labeled rows with icon actions. It does not ship
the plan/26 editors. It consumes `load_user_mcp_inventory`, `merge_skill_rows`,
`parse_hooks_tomlish`, and (when the host can project it) `Supervisor::state`.

**Current code this replaces:**
- `apps/multiplexer-desktop/src/inspector.rs` `inspector_body` returns a `String`.
- `Workspace::{session,resource,mcp,checkpoint,git,terminal,skills}_detail` format multiline
  dumps. Those helpers stay for one slice as a fallback, then the rail stops calling them.
- `InspectorAction` is a parallel enum that bypasses `ClientAction` / `host_call`. Row
  actions must go through `ClientAction`.

---

## 1. Problem statement

The right rail is a labeled tab strip over a muted text dump. That is a debug panel, not a
control surface.

1. **MCP is a name + command string.** `refresh_mcp` loads `~/.grok/config.toml` into
   `McpRow { name, command, transport }` and prints it. There is no Ready / Stopped /
   Unknown / configured badge. The plan/21 supervisor already has `LifecycleState`, but
   the rail cannot show it.
2. **Skills are joined lines.** The desktop already calls `merge_skill_rows` and then
   flattens to `"name [source]"`. Hooks from `parse_hooks_tomlish` are never stored.
3. **Cores are ASCII bars inside a paragraph** that also dumps worktrees and files.
4. **Session is a newline sandwich** (`Project\n{}\n\nModel\n{}` ...).
5. **Git is a path list**, not cards. `selected_worktree` exists but has no row chrome.
6. **Files and activity are not first-class.** Files are a tail of the Cores dump.
   Activity is whatever landed in `terminal_log`.
7. **Actions are tab-global**, not per row. Skills and Terminal have **zero** buttons.
   Desktop `InspectorAction` (CopySession, RunGitStatus, NewWorktreeHint) is not a
   `ClientAction`, so `host_call` and the palette cannot see them.

The user cannot inspect or act. They can only read.

---

## 2. Design goals

1. **One projector.** `inspector_rows(tab, &Workspace) -> Vec<ListRowSpec>` is the only
   inspector body API. GPUI maps specs to elements. Tests assert on specs, not painted
   text.
2. **Every actionable row has 1 to 3 icon actions.** Section headers are the only rows
   allowed to have zero actions. Empty-state rows have exactly one (refresh or create).
3. **Reuse `ClientAction`.** New variants only when an existing one would lie (max 6).
   `Local` / `Rpc` / `NeedsHost` stay in `host_call` (`bindings.rs`).
4. **Live MCP labels when projectable.** Supervisor `Ready` / `Stopped` map to those
   words. Any other live state is `Unknown`. No supervisor snapshot means `configured`.
5. **No new `InspectorTab` in this slice.** Seven tabs stay. Files hang under Cores.
   Hooks hang under Skills. Activity is the Term tab. Adding Files / Activity tabs is
   plan/26 / Phase 2.
6. **`multiplexer-shell` stays dependency-light.** It does not take `multiplexer-mcp`.
   The host writes already-merged rows onto `Workspace`. The projector only formats.

---

## 3. Tab to surface map (existing `InspectorTab`)

| Tab | Label | Body |
|---|---|---|
| `Session` | Session | Definition-list rows (key, value) |
| `Resources` | Cores | Visual core grid, then a Files tree section |
| `Mcp` | MCP | Inventory rows with a live-state badge |
| `Checkpoints` | Points | Checkpoint rows (keep the tab, stop dumping) |
| `Git` | Git | Worktree cards, then a status footer row |
| `Terminal` | Term | Activity feed, then the term-draft row |
| `Skills` | Skills | `merge_skill_rows` list, then hooks if present |

No eighth tab. `/files` and `/activity` slash commands are out of scope.

---

## 4. `ListRowSpec` (shell module)

**File:** `crates/multiplexer-shell/src/inspector_model.rs` (first code).
Export from `crates/multiplexer-shell/src/lib.rs`.

```rust
pub fn inspector_rows(tab: InspectorTab, ws: &Workspace) -> Vec<ListRowSpec> { /* */ }

pub fn tab_toolbar(tab: InspectorTab) -> Vec<RowAction> { /* */ }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRowSpec {
    pub id: String,
    pub kind: RowKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub badge: Option<String>,
    pub selected: bool,
    pub indent: u8,
    pub copy_text: Option<String>,
    pub actions: Vec<RowAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Section,      // header; 0 actions allowed
    Definition,   // session dl
    CoreCell,     // one logical CPU; GPUI wraps these into a grid
    McpServer,
    Skill,
    Hook,
    WorktreeCard,
    File,
    Activity,
    Checkpoint,
    Empty,        // 1 action
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowAction {
    pub icon: RowIcon,
    pub hint: &'static str,
    pub action: ClientAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowIcon {
    Refresh,
    Copy,
    Cycle,
    Play,
    Stop,
    Plus,
    Revert,
    Open,
    Status,
    Dismiss,
    Approve,
    Deny,
}
```

**Invariants (tested):**

1. `inspector_rows` is a pure function of `(tab, ws)`.
2. For every row where `kind != Section`, `actions.len()` is in `1..=3`.
3. `Section` rows have `actions.is_empty()`.
4. `Empty` rows have exactly one action.
5. `id` is unique within one call.
6. `CoreCell` rows are contiguous so the renderer can wrap them as a grid.
7. `copy_text` is the clipboard payload for `CopySession` / `CopyInspector`. It is never a
   resolved secret.

`tab_toolbar` is the 1 to 3 icons above the list (today's `tab_buttons`). Same `RowAction`
type. Desktop drops `InspectorAction` and dispatches toolbar + row icons through
`ClientAction` + `host_call`.

**Session id.** The specified signature has no `session_id` argument. Read it from
`Workspace.connection`: first id in `Connected { session_ids }`, else `"(none yet)"`.
The desktop must keep `connect(vec![live_id])` in sync with its view-level session.

---

## 5. Workspace fields the projector reads

Keep `multiplexer-shell` free of `multiplexer-mcp` types.

### 5.1 `McpRow` (extend)

```rust
pub struct McpRow {
    pub name: String,
    pub command: String,
    pub transport: String,
    pub state: McpLiveLabel, // NEW, default Configured
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpLiveLabel {
    Configured, // no supervisor snapshot
    Ready,
    Stopped,
    Unknown,    // supervisor row exists but is not Ready/Stopped
}
```

Host mapping (not in shell), when a `Supervisor` is in process:

| `LifecycleState` | `McpLiveLabel` |
|---|---|
| `Ready` | `Ready` |
| `Stopped` | `Stopped` |
| `Spawned`, `Crashed { .. }`, `Failed` | `Unknown` |
| name not in the process table | `Configured` |

Today the desktop has no supervisor handle and no `mcp.status` wire method. First
implementation writes `Configured` for every inventory row. The badge string is still
`"configured"` so the UI is honest. When the host can project, it sets `state` before
the next frame. Do not add a wire method in this slice.

Badge copy (exact, tested):

| Label | Badge |
|---|---|
| `Ready` | `Ready` |
| `Stopped` | `Stopped` |
| `Unknown` | `Unknown` |
| `Configured` | `configured` |

### 5.2 Skills

Keep `Workspace.skills: Vec<String>` as `"name [user]"` / `"name [project]"` (desktop
already formats this from `merge_skill_rows`). `inspector_rows` splits a trailing
` [user]` or ` [project]` into `subtitle`. Anything else is title-only.

### 5.3 Hooks (new field)

```rust
pub struct HookItem {
    pub name: String,
    pub when: String,
}

// on Workspace:
pub hooks: Vec<HookItem>,
```

Host fill (NeedsHost, same path as `RefreshSkills`):

1. Candidates, first existing file wins:
   - `{project}/.grok/hooks.toml`
   - `{home}/.grok/hooks.toml`
2. Missing file: `hooks` stays empty. **No Hooks section.**
3. Existing file: `parse_hooks_tomlish(&text)`. Empty parse: no Hooks section.

`parse_hooks_tomlish` already skips comments, blanks, and lines without `:`.

### 5.4 Files, cores, worktrees, activity

No new collections.

- Cores: `ws.cores: Vec<CoreRow>`.
- Files: `ws.files: Vec<String>` (dirs already end in `/` from the desktop tree walk).
- Worktrees: `ws.worktrees: Vec<String>` plus `selected_worktree: Option<usize>`.
- Activity: derived from `reminder`, `pending`, `busy`, then `terminal_log` (oldest first).

Optional later (not this slice): structured `Worktree` cards with branch/head from
`multiplexer-worktree::Worktree`. Cards can already show path + selected mark.

---

## 6. Per-tab UI

### 6.1 Session: definition list

One `Definition` row per field. `title` is the key, `subtitle` is the value.

| id | title | value | actions (icon → ClientAction, class) |
|---|---|---|---|
| `session.project` | Project | `ws.project` | Copy → `CopyInspector` NeedsHost |
| `session.model` | Model | `ws.model` | Cycle → `CycleModel` Local |
| `session.connection` | Connection | `status_label()` | Stop → `Interrupt` Rpc if session else NeedsHost |
| `session.id` | Session | connected id or `(none yet)` | Copy → `CopySession` NeedsHost |
| `session.threads` | Threads | `ws.threads.len()` | Plus → `NewThread` Local |
| `session.models` | Models | `ws.models.join(", ")` | Cycle → `CycleModel` Local |
| `session.palette` | Palette | `open` / `closed` | Open → `TogglePalette` Local |
| `session.help` | Help | `open` / `closed` | Open → `ToggleHelp` Local |

Toolbar: Cycle (`CycleModel` Local), Copy (`CopySession` NeedsHost).

### 6.2 Cores: visual grid, then files tree

**Section `cores.header`** title `Cores`.

If `ws.cores` is empty: one `Empty` row `cores.empty` title `(waiting)`, action Refresh
→ `RefreshCores` NeedsHost.

Else one `CoreCell` per sample:

- `id`: `core.{index}`
- `title`: `cpu{index}`
- `subtitle`: `{usage:.1}%`
- `badge`: `R` when `reserved`, else none
- `selected`: `reserved`
- actions: Refresh → `RefreshCores` NeedsHost

GPUI: when it sees a run of `CoreCell`, it wraps them into a 4-wide grid (2 reserved
cores highlighted). It does **not** render the old `tiny_usage_bar` paragraph. The spec
still carries the percent so tests and a11y have a number.

**Section `files.header`** title `Files`.

If `ws.files` is empty: `Empty` `files.empty` title `(none listed)`, action Open →
`CycleFile` NeedsHost.

Else one `File` row per path:

- `id`: `file.{i}`
- `title`: last path segment (strip trailing `/`)
- `subtitle`: full path
- `indent`: number of `/` separators (so `src/lib.rs` is indent 1)
- `badge`: `dir` when the stored name ends with `/`
- `copy_text`: path
- actions: Open → `CycleFile` NeedsHost, Copy → `CopyInspector` NeedsHost

`CycleFile` already rotates `ws.files` so the clicked-or-first path is index 0. A real
`fs.read` / editor open is plan/09. Do not add `OpenFile` in this slice.

Toolbar: Reload → `RefreshCores` NeedsHost.

### 6.3 MCP inventory

If `ws.mcp` is empty: `Empty` `mcp.empty` title `No MCP servers in ~/.grok/config.toml`,
action Refresh → `RefreshMcp` NeedsHost.

Else one `McpServer` per row:

- `id`: `mcp.{name}`
- `title`: name
- `subtitle`: `command` (stdio) or url (http). Never expand `${VAR}` / `op://`.
- `badge`: live label from §5.1
- `copy_text`: command
- actions: Refresh → `RefreshMcp` NeedsHost, Copy → `CopyInspector` NeedsHost

No enable/disable, no "Test connection" here. Those are plan/26 editors and would need
new RPCs.

Toolbar: Reload → `RefreshMcp` NeedsHost.

### 6.4 Skills + hooks

**Section `skills.header`** title `Skills`.

Empty skills: `Empty` `skills.empty` title `No skills found under .grok/skills`, action
Refresh → `RefreshSkills` NeedsHost.

Else one `Skill` per `ws.skills` entry:

- `id`: `skill.{name}`
- `title`: name
- `subtitle`: `user` or `project` when the suffix parses
- `badge`: same as subtitle
- `copy_text`: name
- actions: Refresh → `RefreshSkills` NeedsHost, Copy → `CopyInspector` NeedsHost

**Section `hooks.header`** title `Hooks`, **only if `ws.hooks` is non-empty.**

One `Hook` per item:

- `id`: `hook.{name}.{when}`
- `title`: name
- `subtitle`: `when` (`SessionStart`, `PreToolUse`, ...)
- `badge`: `block` when `when` is `PreToolUse` (plan/26 blocking event), else none
- `copy_text`: `{name}:{when}`
- actions: Refresh → `RefreshSkills` NeedsHost, Copy → `CopyInspector` NeedsHost

Trust grant UI is plan/26 D76. This slice only lists.

Toolbar: Reload → `RefreshSkills` NeedsHost.

### 6.5 Git worktrees as cards

**Section `git.header`** title `Worktrees`.

Empty: `Empty` `git.empty` title `(none listed)`, action Refresh → `RefreshGit` Rpc.

Else one `WorktreeCard` per path:

- `id`: `wt.{i}`
- `title`: last path segment
- `subtitle`: full path
- `selected`: `ws.selected_worktree == Some(i)`
- `badge`: `*` when selected
- `copy_text`: path
- actions: Refresh → `RefreshGit` Rpc, Status → `RunGitStatus` NeedsHost, Copy →
  `CopyInspector` NeedsHost

Selecting a card is `SelectWorktree(i)` Local (new). The card itself does not carry that
as an icon. The renderer fires `SelectWorktree(i)` on row click, then icons act on the
selection.

**Footer row** `git.status` kind `Activity` (or `Definition`): title `Status`, subtitle
`ws.git_status` or `(none)`, actions: Status → `RunGitStatus` NeedsHost, Refresh →
`RefreshGit` Rpc.

Toolbar: Reload → `RefreshGit` Rpc, Status → `RunGitStatus` NeedsHost, Plus →
`CreateWorktree` Rpc (`git.worktree.create`, `{ "cwd": project }`). This replaces
`InspectorAction::NewWorktreeHint` (composer draft hack). If the RPC is not yet wired in
the desktop host, `host_call` still returns Rpc and the host may fall back to the old
draft hint. The action name is `CreateWorktree`, not a third inspector enum.

### 6.6 Files tree

Specified in §6.2 (Cores tab, second section). Same `File` rows. No extra tab.

### 6.7 Activity feed (Term tab)

Order:

1. If `ws.reminder` is `Some((branch, path))`: `Activity` `activity.reminder`, title
   `Worktree reminder`, subtitle `{branch} · {path}`, actions: Dismiss →
   `DismissReminder` Local.
2. If `ws.pending` is `Some`: `Activity` `activity.approval`, title `Approval`,
   subtitle request summary already on `PendingApproval`, actions: Approve → `Approve`
   Rpc/NeedsHost, Deny → `Deny` Rpc/NeedsHost.
3. If `ws.busy`: `Activity` `activity.busy`, title `Turn running`, action Stop →
   `Interrupt`.
4. Each `terminal_log` line: `Activity` `activity.log.{i}`, title = line, action Copy
   → `CopyInspector` NeedsHost (and `CopyLastMessage` NeedsHost on the last line only,
   if that would exceed 3 icons, keep CopyInspector only).
5. Empty log and no reminder/pending/busy: `Empty` `activity.empty` title `(empty)`,
   action Play → `RunTerminal` NeedsHost.
6. Footer `activity.draft` `Definition`: title `Draft`, subtitle `ws.term_draft`,
   action Play → `RunTerminal` NeedsHost.

Toolbar: Play → `RunTerminal` NeedsHost.

This is the activity feed. The log is no longer a single string blob.

### 6.8 Checkpoints (keep the tab)

Empty: `Empty` `cp.empty`, Plus → `CreateCheckpoint` Rpc/NeedsHost.

Else one `Checkpoint` per row:

- `id`: `cp.{id}`
- `title`: `label`
- `subtitle`: `id`
- `selected`: matches `ws.selected_checkpoint`
- actions: Plus → `CreateCheckpoint`, Revert → `RestoreCheckpoint` Rpc/NeedsHost

Toolbar: New / Revert, same as today, but via `ClientAction`.

---

## 7. Action catalog

### 7.1 Existing `ClientAction` used on rows

| Action | Class (`host_call`) | Used on |
|---|---|---|
| `CycleModel` | Local | Session model / models |
| `NewThread` | Local | Session threads |
| `TogglePalette` | Local | Session palette |
| `ToggleHelp` | Local | Session help |
| `Interrupt` | Rpc `session.interrupt` if `session_id`, else NeedsHost | Connection, busy |
| `RefreshCores` | NeedsHost | Core cells, Cores empty |
| `RefreshMcp` | NeedsHost | MCP rows |
| `RefreshGit` | Rpc `git.worktrees` `{cwd}` | Worktree cards |
| `CreateCheckpoint` | Rpc `checkpoint.create` if session, else NeedsHost | Points |
| `RestoreCheckpoint` | Rpc `checkpoint.revert` if `checkpoint_id`, else NeedsHost | Points |
| `RunTerminal` | NeedsHost | Term draft / empty |
| `CycleFile` | NeedsHost | File rows |
| `CopyLastMessage` | NeedsHost | Last activity line (optional) |
| `DismissReminder` | Local | Reminder card |
| `Approve` / `Deny` | Rpc `approval.respond` if ids, else NeedsHost | Approval card |
| `SelectTab(_)` | Local | not a row icon (tab strip) |

### 7.2 New `ClientAction` variants (exactly 6, all required)

| Variant | Class | Why existing ones lie |
|---|---|---|
| `CopySession` | NeedsHost (clipboard) | `CopyLastMessage` copies chat, not the session id. Desktop already has this as `InspectorAction`. |
| `CopyInspector` | NeedsHost (clipboard of `copy_text`) | One copy verb for MCP command, file path, skill, hook, activity, project. |
| `RunGitStatus` | NeedsHost (`git status` → `ws.git_status`) | `RefreshGit` lists worktrees. Status is a different host call. Already `InspectorAction`. |
| `CreateWorktree` | Rpc `git.worktree.create` `{ "cwd": project }` | Replaces `NewWorktreeHint`. Wire method already exists. |
| `RefreshSkills` | NeedsHost | Reloads `merge_skill_rows` **and** hooks. `RefreshMcp` is the wrong inventory. |
| `SelectWorktree(usize)` | Local | `selected_worktree` has no action. Card click. |

No seventh variant. No `OpenFile`, no `ToggleMcp`, no `TrustHook`, no `SelectFile`,
no `SelectCheckpoint` (host may call `Workspace::select_checkpoint` on row click the
same way it already does for checkpoints).

### 7.3 `host_call` / `apply_layout_action` updates

`apply_layout_action`:

- `SelectWorktree(i)` sets `ws.selected_worktree = Some(i)` when `i < worktrees.len()`,
  else no-op. Returns whether the value changed.
- The other five new variants are host no-ops (return false, do not mutate).

`host_call`:

- `CopySession`, `CopyInspector`, `RunGitStatus`, `RefreshSkills` → `NeedsHost`
- `CreateWorktree` → `Rpc { method: "git.worktree.create", params_json: {"cwd": project} }`
- `SelectWorktree(_)` → `Local`

`ActionContext` does not grow. `CopyInspector` uses the last clicked row's `copy_text`,
which the desktop holds in view state (not shell).

### 7.4 Retire `InspectorAction`

After row + toolbar dispatch through `ClientAction`:

| Old `InspectorAction` | Replacement |
|---|---|
| `RefreshCores` | `ClientAction::RefreshCores` |
| `RefreshMcp` | `ClientAction::RefreshMcp` |
| `RefreshGit` | `ClientAction::RefreshGit` |
| `CreateCheckpoint` | `ClientAction::CreateCheckpoint` |
| `RevertCheckpoint` | `ClientAction::RestoreCheckpoint` |
| `CycleModel` | `ClientAction::CycleModel` |
| `CopySession` | `ClientAction::CopySession` |
| `RunGitStatus` | `ClientAction::RunGitStatus` |
| `NewWorktreeHint` | `ClientAction::CreateWorktree` |

Delete the desktop-only enum once `tab_buttons` is gone.

---

## 8. Desktop render contract (not first code)

`apps/multiplexer-desktop/src/inspector.rs` becomes a thin wrapper:

1. `tab_toolbar(tab)` → icon buttons (same `ghost_btn` chrome).
2. `inspector_rows(tab, &workspace)` → body.
3. `Section`: muted header.
4. `Definition`: two-line key / value.
5. `CoreCell`: wrap in a 4-column grid; reserved cells use the reserved fill.
6. `WorktreeCard`: padded card, selected hairline.
7. `McpServer` / `Skill` / `Hook` / `File` / `Checkpoint` / `Activity`: one row, badge
   chip, 1 to 3 icon hits.
8. `Empty`: muted title + one button.

`inspector_body` remains exported until every caller (including tests) uses rows. The
rail must not paint `inspector_body` after the GPUI switch.

Host refresh paths already exist (`refresh_mcp`, `refresh_cores`, `refresh_worktrees`,
skills load in `new`). Add:

- `refresh_skills`: `merge_skill_rows` + hooks file parse → `set_skills` / `ws.hooks`.
- `copy_inspector`: clipboard `copy_text`.
- `run_git_status`: already `run_shell("git status")`; write stdout into
  `set_git_status`.
- `create_worktree`: `git.worktree.create` then `RefreshGit`.

Do not block first_code on the GPUI switch. First_code is the projector + tests.

---

## 9. Proposed decisions (D77+)

### D77. Inspector body is `inspector_rows`, not a string (PROPOSED)
- **Decision:** The right rail body is `Vec<ListRowSpec>` from a pure projector in
  `multiplexer-shell`. GPUI does not format inspector text.
- **Rationale:** Tests can lock structure, badges, and actions without a window.

### D78. MCP badge is a supervisor projection or `configured` (PROPOSED)
- **Decision:** Badge is `Ready` / `Stopped` / `Unknown` only when the host wrote a
  supervisor snapshot onto `McpRow.state`. Otherwise `configured`. Shell never imports
  `Supervisor`.
- **Rationale:** Matches plan/21 states the user asked to see, without pretending the
  desktop owns a process table it does not have yet.

### D79. Skills tab owns hooks when a hooks file exists (PROPOSED)
- **Decision:** Hooks render as a second section of `InspectorTab::Skills` only when
  `parse_hooks_tomlish` of the first existing candidate file is non-empty.
- **Rationale:** plan/26 is one Customize surface. This slice does not add a Hooks tab.

### D80. At most six new `ClientAction` variants (PROPOSED)
- **Decision:** This slice adds only `CopySession`, `CopyInspector`, `RunGitStatus`,
  `CreateWorktree`, `RefreshSkills`, `SelectWorktree(usize)`. Everything else reuses.
- **Rationale:** Keeps `host_call` and the palette enumerable. Editors in plan/26 can
  add toggle / trust / test later.

---

## 10. Testing strategy (TDD)

Write tests in `inspector_model.rs` **before** the projector body is complete.
`cargo test -p multiplexer-shell` is the implementer's loop (this plan file is not
that loop).

### 10.1 Unit (co-located)

| Test | Assert |
|---|---|
| `session_is_definition_list` | titles are Project, Model, Connection, Session, Threads, Models, Palette, Help in that order; all `Definition`; each has 1..=3 actions |
| `session_id_from_connection` | disconnected → subtitle `(none yet)`; `connect(vec!["sess-1"])` → `sess-1` |
| `cores_empty_waiting` | one `Empty` + Files section |
| `cores_are_cells_not_text_bar` | two `CoreRow`s → two `CoreCell`s, no `█` / `░` in any title/subtitle |
| `files_indent_from_slashes` | `src/lib.rs` indent 1, `README.md` indent 0, `src/` badge `dir` |
| `mcp_empty_configured_copy` | empty → `Empty` with `RefreshMcp` |
| `mcp_badge_configured_by_default` | pushed row with `McpLiveLabel::Configured` → badge `configured` |
| `mcp_badge_ready_stopped_unknown` | table over the four labels |
| `skills_parse_source_suffix` | `"fmt [project]"` → title `fmt`, badge `project` |
| `hooks_section_absent_when_empty` | no row id starts with `hooks` |
| `hooks_section_when_present` | `PreToolUse` gets badge `block` |
| `git_cards_mark_selected` | `selected_worktree = Some(0)` → first card `selected` |
| `activity_orders_reminder_then_log` | reminder row before `terminal_log` rows |
| `checkpoints_mark_selected` | `selected_checkpoint` matches |
| `every_non_section_has_one_to_three_actions` | walk `InspectorTab::all()` on a rich fixture |
| `tab_toolbar_skills_has_refresh` | Skills toolbar is `[RefreshSkills]` |
| `ids_unique_per_tab` | no duplicate `id` in one call |

### 10.2 Property

`proptest` over a generated `Workspace` (bounded vec lengths): for every tab, every
non-`Section` row has `1..=3` actions, every action is a `ClientAction` that
`host_call` accepts (no panic), and `id`s are unique.

### 10.3 Mutation

`cargo-mutants` on `inspector_model.rs`: badge mapping, indent count, hooks-section
gate, empty vs list, action counts. A mutant that paints `Ready` for `Configured`
must die.

### 10.4 Integration / component (after first_code)

- Desktop: MCP tab shows a badge chip, not `name [stdio]\n  command`.
- Clicking a worktree card selects it (`SelectWorktree`).
- Skills refresh fills hooks when a temp `hooks.toml` exists.

### 10.5 First failing tests (write these first)

```text
inspector_rows(Session, &ws)[0].kind == Definition
inspector_rows(Resources, &ws_with_cores)[1].kind == CoreCell
inspector_rows(Mcp, &ws_with_linear).iter().any(|r| r.badge.as_deref() == Some("configured"))
inspector_rows(Skills, &ws_with_hook).iter().any(|r| r.kind == RowKind::Hook)
inspector_rows(Git, &ws_with_wt)[1].kind == WorktreeCard
inspector_rows(Resources, &ws_with_files).iter().any(|r| r.kind == RowKind::File)
inspector_rows(Terminal, &ws_with_log).iter().any(|r| r.kind == RowKind::Activity)
```

---

## 11. Security

1. **No resolved secrets** in `subtitle` or `copy_text` (D23, plan/26 D74). Inventory
   already stores command/url as configured, not env maps.
2. **CopyInspector** copies the spec's `copy_text` only, never `ServerConfig.env`.
3. **CreateWorktree** is Rpc on the server, which already scopes paths to the project
   cwd (wire `path outside worktree`).
4. **Hooks** are listed, not executed. Trust remains plan/26.
5. **Project files** use the existing `list_project_tree` skip list (`.git`,
   `node_modules`, `target`). Hidden names stay skipped.

---

## 12. Open questions

1. **When does the host gain a supervisor snapshot?** In-process in the desktop vs a
   future `mcp.status` RPC. Until then every badge is `configured`. Do not fake Ready.
2. **Files / Activity as real tabs.** Deferred so `InspectorTab::all()` stays length 7
   and existing slash / palette tests stay green.
3. **`CreateWorktree` params.** Wire may want branch + path. This slice sends `{cwd}`
   only, matching `RefreshGit`. Extra fields are a follow-up.
4. **Structured `Worktree` on `Workspace`.** Porcelain already has branch/head/locked.
   Cards can grow a subtitle without a new `ClientAction`.
5. **plan/26 editors** (add/edit/remove MCP, skill frontmatter, hook trust) stay out.
   This slice is the inventory + actions rail.

**Flagged consistency:** this doc does not change crate layout, wire methods (except
using existing `git.worktree.create`), or supervisor transitions. If D13 or the
seven-tab `InspectorTab` enum flips, §3 and §5 must be revisited.

---

## PARENT_IMPLEMENT

```
files: plan/33-inspector-customize.md
first_code: crates/multiplexer-shell/src/inspector_model.rs
```

Implementer order:

1. Add `inspector_model.rs` with types + `inspector_rows` / `tab_toolbar` + the failing
   tests in §10.5.
2. Extend `McpRow` with `state: McpLiveLabel` (default `Configured`). Add
   `HookItem` + `Workspace.hooks`.
3. Add the six `ClientAction` variants. Update `apply_layout_action`, `host_call`,
   palette items, and host-noop tests.
4. Export the new types from `lib.rs`.
5. Leave GPUI `inspector.rs` string body in place until the projector is green. Then
   switch the rail to rows and delete `InspectorAction`.

Do not add `multiplexer-mcp` to `multiplexer-shell` Cargo.toml. Do not add Files or
Activity tab variants. Do not add a seventh `ClientAction`.
