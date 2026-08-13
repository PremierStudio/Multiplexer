# 32: List Rows (Dashboard Items That Open)

**Status:** Planning (parent implement)
**Owner:** Desktop chrome / `multiplexer-shell` workspace
**Depends on:** `10-ui-pane-system.md`, `21-mcp-lifecycle-supervisor.md`, `24-resource-manager.md`, `26-mcp-skills-ui.md`
**Feeds:** desktop GPUI rails, inspector rewrite, later Customize panel (plan/26)
**First code:** `Workspace::expand_row` + desktop row renderer

This document is consistent with `docs/PLAN-CONTEXT.md` and `docs/DECISIONS.md` (D1, D13, D21, D33). It does not add inspector tabs. It replaces the current **string dumps** with dashboard-quality rows that select, toggle, or **open**.

The inspector today is one muted `child(body: String)` block (`inspector_body` in `apps/multiplexer-desktop/src/inspector.rs`). The left rail is a title + preview + `"status · id"` stack. Nothing expands. Hover does not reveal delete. Cores do not toggle. Files rotate via `CycleFile`. That is inventory text, not a control surface.

---

## 1. Problem

Users scan lists to **open** something: a thread, an MCP server, a skill summary, a reserved core, a checkpoint, a worktree, a file, a log line. The headless model already has the data (`Thread`, `McpRow`, `CoreRow`, `CheckpointRow`, `files`, `skills`, `worktrees`, `terminal_log`). The GPUI shell does not project it as rows.

Constraints we keep:

- Headless model in `multiplexer-shell`. No GPUI types there (existing crate rule).
- Desktop is a thin painter. Clicks dispatch into `Workspace` or a documented host stub.
- One native binary owns processes and config (D1, D13). Rows never write `config.toml` or spawn MCP themselves.
- Mutation floor 70% on the expand machine and row identity (D21, D33).
- Windows-first, 60fps. Rows are ordinary GPUI `div`s, not a new virtualizer in this slice.

---

## 2. Design goals

1. **Every listed catalog is a row.** Glyph, title, subtitle, badge, optional pulse, optional hover action, optional expanded body.
2. **Rows open.** Click either selects, toggles, or expands. Expanded rows show the next action, not a second dump of the same text.
3. **Accordion:** at most one expanded id. `none -> one`. Same id again collapses.
4. **Dashboard chrome.** Glass row, 12px radius (`Theme::panel_radius`), hairline, selected wash, hover reveal. Matches the existing thread card, not a spreadsheet.
5. **Honest host actions.** MCP Start/Stop are visible even while the supervisor is inventory-only. They must not pretend a process started.
6. **Reuse existing helpers.** Cores use `usage_bar` from `multiplexer-shell::bars`. Activity uses `TermLineKind`. Skills reuse `multiplexer_mcp::SkillRow` source strings (`user` / `project`).

Non-goals for this slice:

- New `InspectorTab` variants (no Files tab, no Activity tab).
- Live MCP spawn/teardown (plan/21). Buttons exist; host logs a stub.
- OS core pin (plan/24). Reserved is a **model flag** only.
- Virtualized lists of thousands of rows (plan/10 §7.3). Current inventories are small.
- SKILL.md editor (plan/26). Expand shows the first summary line only.

---

## 3. Expand state machine

Single field on `Workspace`:

```text
expanded_row: Option<RowId>
```

`RowId` is the accordion key. It is **not** the same as selection (`selected`, `selected_checkpoint`, `selected_worktree`, `selected_file`).

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowId {
    Thread(String),      // thread.id
    Mcp(String),         // server name
    Skill(String),       // skill name
    Core(usize),         // core index
    Checkpoint(String),  // checkpoint id
    Git(usize),          // worktree index
    File(String),        // path as stored
    Activity(usize),     // stable index in the current activity vec
}
```

### 3.1 `expand_row`

```text
expand_row(id):
  None              -> Some(id)
  Some(id)          -> None          // collapse same
  Some(other)       -> Some(id)      // replace previous
```

Helpers:

- `is_expanded(&self, id: &RowId) -> bool`
- `collapse_row(&mut self)` (sets `None`)
- `fn expanded(&self) -> Option<&RowId>`

`ClientAction` stays `Copy`. Desktop calls `ws.expand_row(id)` directly in this slice. Do **not** put `RowId` on `ClientAction` (it owns `String`s). Index-based actions that stay `Copy` are listed in §6.

### 3.2 Who uses expand

| Catalog | Click | Uses accordion? |
|---|---|---|
| Threads | select thread | no (hover delete) |
| MCP | expand | yes |
| Skills | expand | yes |
| Cores | toggle `reserved` | no |
| Checkpoints | select; Revert is the second action | optional: expand shows Revert |
| Git | set `selected_worktree` | yes: Refresh / Status |
| Files | set `selected_file`, show Session | no |
| Activity | expand full line | yes (long lines) |

Checkpoints: click selects (`select_checkpoint(Some(id))`). The **double-action** is Revert on the selected row (always visible when selected, or in the expanded body). Either is fine if tests pin one. Prefer: selected row shows a Revert chip; expand is not required.

### 3.3 Collapse side effects

- `SelectTab` to a **different** tab calls `collapse_row`. Same tab is a no-op (already true in `apply_layout_action`).
- `delete_thread` that removes the expanded thread id clears expand.
- Replacing `mcp` / `skills` / `files` / `checkpoints` / activity drops expand if that `RowId` is no longer present (`retain_expanded` helper).
- `new_thread` does not clear expand unless a thread row was expanded (they are not).

### 3.4 Required tests (workspace)

```text
expand_replaces_previous
  expand_row(Mcp("a"))
  expand_row(Mcp("b"))
  assert_eq!(expanded, Some(Mcp("b")))
  assert!(!is_expanded(&Mcp("a")))

collapse_same_id
  expand_row(Skill("review"))
  expand_row(Skill("review"))
  assert!(expanded.is_none())
```

Also:

- `expand_from_none_sets_id`
- `collapse_row_clears`
- `select_tab_collapses_when_tab_changes` (and does not collapse when the tab is unchanged)
- `retain_expanded_drops_missing`
- property: after any sequence of `expand_row` calls, `expanded_row` is `None` or exactly one `RowId`

GPUI element ids use `RowId` display (`"mcp:linear"`, `"core:3"`) so hit targets are stable.

---

## 4. Shared row chrome (desktop renderer)

**File:** `apps/multiplexer-desktop/src/rows.rs`

Headless projection stays in shell. The renderer is a GPUI builder used by `left_rail` and `right_rail`. `main.rs` must not grow another 80-line inline row.

### 4.1 Visual spec

One collapsed row:

```text
┌──────────────────────────────────────────────────────────┐
│  [glyph]  Title                         [badge]  [pulse] │
│           subtitle (muted, 1 line, ellipsize)            │
└──────────────────────────────────────────────────────────┘
```

Expanded (MCP / Skills / Git / Activity):

```text
┌──────────────────────────────────────────────────────────┐
│  [glyph]  Title                         [badge]          │
│           subtitle                                       │
│  ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─  │
│  [ action ] [ action ] [ action ]                        │
│  extra body (skill first line, activity wrap, …)         │
└──────────────────────────────────────────────────────────┘
```

Tokens (all existing `Theme` unless noted):

| Slot | Treatment |
|---|---|
| Surface | `hsla(0,0,1,0.03)` idle, selected `hsla(0.58,0.35,0.22,0.45)` (same as current thread card) |
| Border | `Theme::hairline()` idle, `Theme::hairline_bright()` selected or expanded |
| Radius | `Theme::panel_radius()` (12px) |
| Title | `Theme::text()`, one line |
| Subtitle | `Theme::muted()`, 48 chars max (reuse `thread_preview` style) |
| Badge | small chip, accent wash, muted text |
| Pulse idle | 6px `Theme::muted()` disc |
| Pulse running | 6px `Theme::good()` disc |
| Pulse error | 6px `Theme::danger()` disc |
| Hover | brighter fill; trailing action fades in (delete, or nothing) |
| Expanded actions | same `ghost_btn` family as inspector buttons |
| Empty catalog | one muted line, not a fake row, not clickable |

Density: 8px vertical padding, 12px horizontal, 4px gap between rows (`mx_2` / `mb_1` like today's threads). Collapsed ~44px. Do not animate height in this slice (input budget is 16ms; skip layout animation until a later chrome pass).

### 4.2 Renderer API (desktop)

```rust
pub struct RowChrome {
    pub id: SharedString,          // "mcp:linear"
    pub glyph: SharedString,       // "L" or "✦"
    pub title: SharedString,
    pub subtitle: SharedString,
    pub badge: Option<SharedString>,
    pub pulse: Option<Pulse>,      // Idle / Running / Error
    pub selected: bool,
    pub expanded: bool,
    pub hover_delete: bool,        // threads only
}

pub enum Pulse { Idle, Running, Error }

// paint collapsed + optional expanded children
fn list_row(...) -> impl IntoElement
```

Kind color for activity is **not** a pulse. It tints the glyph / title (`Input` = accent, `Output` = text, `Meta` = muted, `Error` = danger).

### 4.3 Glyphs

| Catalog | Glyph |
|---|---|
| Threads | first alphanumeric of `title`, uppercased; `"•"` if none |
| MCP | `mcp_brand_glyph(name)`: tiny static map (`linear`, `github`, `filesystem`, `browser`, `slack`, …) else first letter |
| Skills | `"✦"` (sparkle) |
| Cores | `"#"` or the index as text in the glyph slot (`0`, `1`, …) |
| Checkpoints | `"◉"` or `"*"` when selected |
| Git | `"⎇"` |
| Files | `"▣"` folder (path ends with `/` or `\`) else `"▤"` |
| Activity | `"$"` Input, `"›"` Output, `"·"` Meta, `"!"` Error |

No network, no image assets. Glyphs are text in the glass typeface.

---

## 5. Row catalogs

### 5.1 THREADS (left rail)

**Data:** `Workspace.threads`, `selected`, `Workspace::thread_preview`.

**Fields on the row:**

| Slot | Source |
|---|---|
| Glyph | avatar from title (§4.3) |
| Title | `Thread.title` |
| Preview | `Workspace::thread_preview` (already `"You: …"` / `"Agent: …"` / `"Empty thread"`) |
| Model badge | `Thread.model` if set, else `Workspace.model` |
| Pulse | `Thread.status`: `idle` / `running` / `error` |

**Add** `model: String` on `Thread`. `new_thread` copies `self.model`. `cycle_model` updates `workspace.model` and the **selected** thread's `model` so the badge tracks the session.

**Click:** `SelectThread(i)` (existing). Does not expand.

**Hover:** reveal a Delete chip. Calls `Workspace::delete_thread(i)` (already refuses the last thread). This replaces the header-only Del button as the primary delete, but keep the header Del as "delete selected" (`ClientAction::DeleteThread`).

**Not shown:** raw `thr-N` id on the card. Id stays in Session detail.

**Tests:** `new_thread_copies_workspace_model`, `cycle_model_updates_selected_thread_model`, existing delete tests stay green.

### 5.2 MCP (inspector tab `Mcp`)

**Data:** `Workspace.mcp` (`McpRow { name, command, transport }`).

| Slot | Source |
|---|---|
| Glyph | brand map |
| Title | `name` |
| Badge | `transport` (`stdio` / `http`) |
| Subtitle | `command` (or url for http) |

**Click:** `expand_row(RowId::Mcp(name))`.

**Expanded actions (host, even if inventory-only):**

| Action | Host behavior this slice |
|---|---|
| Start | `term_meta("mcp start {name} (inventory)")`. Do not spawn. |
| Stop | `term_meta("mcp stop {name} (inventory)")`. Do not kill. |
| Copy | clipboard `command` (or `name` if command empty); flash "copied mcp" |
| Reveal | flash + `term_meta` the user config path (`~/.grok/config.toml`). Do not open an editor. Do not dump secrets. |

Start/Stop remain visible so the row **opens to controls**. Plan/21 later binds them to the supervisor. The stub text must include `inventory` so tests can assert honesty.

**Empty:** keep the current copy: no servers in `~/.grok/config.toml`.

**Tests:** expand/collapse on name; host stubs do not change `mcp` vec.

### 5.3 SKILLS (inspector tab `Skills`)

**Data:** stop stuffing `"name [source]"` strings into `Workspace.skills: Vec<String>`.

Promote to a shell row (name clash with `multiplexer_mcp::SkillRow` is fine if the shell type is `SkillEntry` or we store the mcp struct fields):

```rust
pub struct SkillEntry {
    pub name: String,
    pub source: String,    // "user" | "project"
    pub summary: String,   // first line of SKILL.md, may be empty
}
```

Desktop `set_skills` maps `merge_skill_rows` plus a pure summary parse.

| Slot | Source |
|---|---|
| Glyph | sparkle |
| Title | `name` |
| Badge | `source` |
| Subtitle | `summary` if non-empty, else `"SKILL.md"` |

**Click:** `expand_row(RowId::Skill(name))`.

**Expanded body:** the summary line, wrapped. If empty: `"No SKILL.md summary"`.

**First-line helper** (pure, `multiplexer-mcp` or shell):

- Read is host I/O. Parse is pure: `skill_summary_first_line(&str) -> String`.
- Skip `---` YAML frontmatter if the file starts with `---`.
- Then first non-empty line, trimmed, max 160 chars.
- Host looks up `{dir}/{name}/SKILL.md` then `{dir}/{name}.md` using `skill_dir_candidates`. Missing file => empty summary.

**Empty:** current copy, no skills under `.grok/skills`.

### 5.4 CORES (inspector tab `Resources`)

**Data:** `Workspace.cores` (`CoreRow { index, usage, reserved }`).

| Slot | Source |
|---|---|
| Glyph | index |
| Title | `cpu{index}` |
| Badge | `"R"` / `"reserved"` when `reserved` |
| Subtitle | `usage_bar(usage, 8)` plus `" {usage:.1}%"` |

Use the public `usage_bar` (already tested). Delete or stop calling the private `tiny_usage_bar` once cores render as rows. Resource header copy ("Reserved cores: 0, 1 (app)") can stay as a one-line caption **above** the list.

**Click:** `toggle_core_reserved(index)` flips `cores[i].reserved`. No accordion. No Job Object / affinity call (plan/24). The inspector Reload button still resamples usage (host) and must **preserve** the reserved flags the user set (merge by index).

**Tests:** `toggle_core_reserved_flips_flag`, `toggle_unknown_index_is_false`, `refresh_preserves_reserved` (desktop or workspace merge helper).

### 5.5 CHECKPOINTS (inspector tab `Checkpoints`)

**Data:** `CheckpointRow` plus `selected_checkpoint`.

**Add** `seq: u64` (1-based per session, same meaning as `multiplexer_checkpoint::Checkpoint.seq`). `create_local_checkpoint` assigns `checkpoints.len() + 1` when the host does not pass seq. When the server returns a checkpoint, copy `seq` if present; else increment.

| Slot | Source |
|---|---|
| Glyph | selected mark |
| Title | `label` |
| Badge | `#seq` |
| Subtitle | `id` |

**Click:** `select_checkpoint(Some(id))`. Selected row uses the selected wash.

**Double-action Revert:** the selected row shows a Revert chip (or the existing tab-level Revert stays and operates on `selected_checkpoint`). Row-level Revert is the dashboard version; keep the tab button as the same `InspectorAction::RevertCheckpoint`.

**Empty:** current "No checkpoints yet" copy.

### 5.6 GIT (inspector tab `Git`)

**Data:** replace `worktrees: Vec<String>` with:

```rust
pub struct GitRow {
    pub path: String,
    pub branch: String, // empty => "(detached)"
}
```

`git.worktrees` already returns `path` + `branch` (`WorktreeInfo` in `multiplexer-server`). Desktop currently drops branch in `worktree_paths`. Keep branch.

| Slot | Source |
|---|---|
| Glyph | branch glyph |
| Title | short path (existing `short_path` helper) |
| Badge | branch or `detached` |
| Subtitle | full path, muted |

**Click:** `select_worktree(Some(i))` (wrap the existing `selected_worktree: Option<usize>`).

**Expanded actions:**

| Action | Behavior |
|---|---|
| Refresh | existing `refresh_worktrees` |
| Status | existing `run_shell("git status")` (sets `git_status`) |

Status text stays a caption under the list (current "Status" block), not a row.

**Empty:** `(none listed)`.

**Tests:** `select_worktree_sets_index`, `select_worktree_out_of_range_false`. `git_detail` string can remain for Session-style tests until the rail paints rows; update it to show `path` + `branch`.

### 5.7 FILES (Session body, not a new tab)

**Data:** `files: Vec<String>` plus **new** `selected_file: Option<String>`.

Folder vs file: trailing `/` or `\` => folder (desktop already appends `/` for dirs in `list_project_tree`).

| Slot | Source |
|---|---|
| Glyph | folder / file |
| Title | file name (last component) |
| Subtitle | parent path |

**Click:**

1. `select_file(path)` sets `selected_file`.
2. `inspector = InspectorTab::Session` so the detail is visible even if the user was on Cores (files today live in `resource_detail`; they **move** to Session).

**Session tab layout after this slice:**

1. Existing metadata (project, model, connection, session id).
2. Selected file block: path or `(no file selected)`.
3. FILES list.

`CycleFile` / `cycle_file` becomes "select the next file after `selected_file`" (wrap). It must set `selected_file` and jump to Session, not rotate the vec (rotating the vec is hostile to row identity).

**Tests:** `select_file_sets_and_shows_session`, `cycle_file_selects_next_without_rotating`.

Do **not** add `InspectorTab::Files` here. Plan/10 still owns a future Files pane.

### 5.8 ACTIVITY (Term tab)

**Data:** timestamp-less log lines as rows. Kind color from `TermLineKind`.

Today `terminal_log: Vec<String>` stores **already formatted** lines (`format_line`). That throws away kind.

Promote:

```rust
pub struct ActivityLine {
    pub kind: TermLineKind,
    pub text: String, // raw, not prefixed
}
```

`push_terminal` / desktop `term_line` push `ActivityLine`. Keep a thin `fn activity_text(line) -> String` that calls `format_line` for any leftover string UI.

**No timestamps.** Do not add clocks.

| Slot | Source |
|---|---|
| Glyph | kind glyph |
| Title | `text` (one line, ellipsize) |
| Color | kind -> Theme (see §4.2) |

**Click:** `expand_row(RowId::Activity(index))` unwraps the full line.

**Cap:** keep `TERM_HISTORY_MAX` (80). After drain, re-run `retain_expanded` (activity indices shift: **collapse** on drain rather than reindex). Simplest: `push` that drops the front also `collapse_row` if expanded is `Activity(_)`.

Term draft stays the input line under the list (existing `term_draft`).

**Empty:** `(empty)`.

---

## 6. Workspace and action surface

### 6.1 Fields

| Field | Change |
|---|---|
| `expanded_row` | new `Option<RowId>` |
| `Thread.model` | new `String` |
| `skills` | `Vec<SkillEntry>` (break `Vec<String>`) |
| `worktrees` | `Vec<GitRow>` (break `Vec<String>`) |
| `checkpoints[].seq` | new `u64` |
| `selected_file` | new `Option<String>` |
| `activity` | new `Vec<ActivityLine>` (source of truth for Term) |
| `terminal_log` | either removed or derived; prefer derived in tests via `format_line` |

`set_skills(Vec<String>)` becomes `set_skills(Vec<SkillEntry>)`. Update desktop merge + workspace tests.

### 6.2 Methods (headless)

| Method | Effect |
|---|---|
| `expand_row(RowId)` | accordion §3.1 |
| `collapse_row()` | `None` |
| `is_expanded(&RowId)` | bool |
| `toggle_core_reserved(usize) -> bool` | flip flag |
| `select_worktree(Option<usize>) -> bool` | range check |
| `select_file(Option<String>)` | set + `inspector = Session` when `Some` |
| `cycle_selected_file() -> bool` | next file, then `select_file` |
| `retain_expanded()` | drop expand if id missing |

### 6.3 `ClientAction` (Copy, optional this slice)

If the parent wants actions in the enum without breaking `Copy`:

```text
ToggleCoreReserved(usize)
SelectWorktree(usize)
SelectFile(usize)          // index into files
ExpandMcp(usize)
ExpandSkill(usize)
ExpandGit(usize)
ExpandActivity(usize)
DeleteThreadAt(usize)
```

`apply_layout_action` handles the local ones. MCP Start/Stop/Copy/Reveal stay **host-only** (like `RefreshMcp`).

If that enum growth is deferred, desktop may call the `Workspace` methods directly. Expand tests still live on `Workspace`, not on `ClientAction`.

### 6.4 Controls catalog

Add live ids (keep `REQUIRED_IDS.len() == all_controls().len()`):

```text
row_delete_thread     LeftRail
row_expand            RightRail
row_toggle_core       RightRail
row_select_file       RightRail
row_select_worktree   RightRail
row_mcp_start         RightRail
row_mcp_stop          RightRail
row_mcp_copy          RightRail
row_mcp_reveal        RightRail
```

Labels are real words (`Delete`, `Expand`, `Reserve`, `Start`, …). No dead placeholders.

### 6.5 Inspector string bodies

Keep `inspector_body` for unit tests and as a fallback caption, but **right rail must not paint one blob** for MCP / Skills / Cores / Points / Git / Term / Files. Session metadata may stay a short labeled block **above** the files list.

`resource_detail` loses the Files section (moved to Session). Worktrees on the Cores tab become a one-line count or are omitted (Git tab owns them).

---

## 7. First code (parent implement)

Ship in this order. Each step is green before the next.

### Step A: expand machine (headless)

**Files:** `crates/multiplexer-shell/src/workspace.rs`, re-export `RowId` from `lib.rs`.

1. Add `RowId` + `expanded_row`.
2. Implement `expand_row` / `collapse_row` / `is_expanded`.
3. Tests: `expand_replaces_previous`, `collapse_same_id`, plus the extras in §3.4.
4. `SelectTab` collapse when the tab actually changes.

No GPUI. No catalog rewrite yet.

### Step B: desktop row renderer

**Files:** `apps/multiplexer-desktop/src/rows.rs`, `mod rows` in `main.rs`.

1. `RowChrome` + `list_row` with glyph, title, subtitle, badge, pulse, selected/expanded chrome, hover-delete slot.
2. Point **left-rail threads** at `list_row` (same click = select). Hover delete wired.
3. Model badge + status pulse on threads (`Thread.model` if Step A grew it; otherwise `workspace.model`).

After A+B the product looks like a dashboard on the left. Inspector can still be text for one commit if needed. Prefer to land MCP rows in the same desktop commit if the renderer is generic.

### Step C: inspector catalogs (same or next commit)

Order: Cores (toggle + `usage_bar`) -> MCP (expand + host stubs) -> Skills (promote type + summary) -> Checkpoints (seq + select) -> Git (`GitRow` + select) -> Files (`selected_file`) -> Activity (kind rows).

Do not rewrite the Customize panel (plan/26). These are the live inspector lists.

---

## 8. Testing

TDD at inception. Headless first.

| Layer | What |
|---|---|
| Unit | expand accordion, toggle reserved, select file/worktree/checkpoint, cycle file without rotate, skill first-line parser (frontmatter skip, empty, trim), `mcp_brand_glyph` fallback, `retain_expanded`, tab change collapses |
| Property | any `expand_row` sequence => 0 or 1 expanded id (proptest on a small `RowId` strategy) |
| Mutation | `expand_row` replace vs collapse; reserved flip; `select_file` sets Session; first-line parser must not take the `---` line |
| Component | thread row count; click selects; hover delete refuses last thread; MCP expand shows Start; core click flips badge (if GPUI test harness is ready; otherwise assert via workspace after the same methods the renderer calls) |
| Desktop assert | keep `controls::no_dead_labels()` and required-id length |

Coverage gates unchanged (D21, D33). Expand + reserved + select_file are mandatory mutation targets.

Do not add e2e in this slice beyond existing smoke. Plan/15 still owns the broader rail e2e.

---

## 9. Mapping to existing code

| Today | After |
|---|---|
| `left_rail` inline thread `div`s | `rows::list_row` |
| `right_rail` `.child(body)` string | per-tab row lists |
| `inspector_body` | metadata + empty copy only |
| `McpRow` | unchanged fields; expand + host actions |
| `CoreRow` | click toggles `reserved` |
| `CheckpointRow { id, label }` | add `seq` |
| `skills: Vec<String>` | `Vec<SkillEntry>` |
| `worktrees: Vec<String>` | `Vec<GitRow>` |
| `files` + `cycle_file` rotate | `selected_file` + cycle-next |
| `terminal_log: Vec<String>` | `activity: Vec<ActivityLine>` |
| `usage_bar` unused by UI | core subtitle |
| `ClientAction::DeleteThread` | remains; hover uses `delete_thread(i)` |

Wire: no new JSON-RPC methods. MCP start/stop are not `mcp.start` until plan/21 lands a real command. Git refresh/status reuse `git.worktrees` and the shell.

Secrets: Reveal prints the config **path**, never env values (D23). Copy copies the command string already in the inventory (it may contain a path, not a token).

---

## 10. Proposed decisions (not locked)

These are local to this slice. They do not override D1 to D40.

1. **One global expanded id** on `Workspace`, not per-tab maps.
2. **No new inspector tabs.** Files live under Session. Activity lives under Term.
3. **MCP Start/Stop are visible stubs** until the supervisor is wired.
4. **Core reserved is a model flag.** Affinity / Job Object is plan/24.
5. **`ClientAction` stays `Copy`.** Expand uses `RowId` on the workspace, or index-based actions.

---

## 11. Open questions

Flagged, not decided here:

1. **Hover-delete vs header Del.** This doc keeps both. If the header Del feels redundant after hover, remove it in a chrome cleanup, not in first_code.
2. **Checkpoint Revert on the row vs tab button.** Prefer both calling the same host path.
3. **Activity index after cap drain.** This doc collapses rather than reindexes. A later pass can use a monotonic line id if expand-across-drain matters.
4. **Brand glyph map size.** Keep it a short static table. Do not fetch MCP Registry icons (plan/26 marketplace).
5. **Virtualization.** When thread or file lists pass a few hundred rows, apply plan/10 virtualization. Not this slice.

**Consistency:** server-centric runtime, GPUI desktop, thin client, no Electron, TDD, no plaintext secrets. If D1 or D13 flips, the renderer moves with `multiplexer-ui`; the expand machine stays in `multiplexer-shell`.

---

*Parent implement: Step A `Workspace::expand_row` + tests `expand_replaces_previous` / `collapse_same_id`, then Step B `apps/multiplexer-desktop/src/rows.rs` painting left-rail threads.*
