# 30: Chrome Drawers (this sprint)

**Status:** Ready to implement
**Owner:** Parent implementer (`PARENT_IMPLEMENT`)
**Depends on:** `plan/10-ui-pane-system.md` (layout spec), `crates/multiplexer-shell/src/workspace.rs`, `apps/multiplexer-desktop/src/main.rs`
**Does not depend on:** `multiplexer-layout` pop-out/split engine, Ghostty embed, SVG icon pack, marketplace Customize panel (`plan/26`)
**Locked decisions applied:** D1 (Rust + GPUI), D13 (`multiplexer-*` crates), D21 (mutation on core chrome mutations), D33 (70% mutation floor on those mutations)

This doc is consistent with `docs/PLAN-CONTEXT.md` and `docs/DECISIONS.md`. It specifies the **shipping chrome for this sprint**, not the full Phase-2 pane engine. Everything here must compile and paint in **GPUI 0.2.2 today**.

---

## 0. Why this sprint exists

The Outlook layout in `plan/10` §2 is the product. The window we ship today is not that product.

**What the user sees now** (`apps/multiplexer-desktop/src/main.rs`):

- Native OS caption, then an in-window bar that is two wordy ghost pills (`Chats Show` / `Inspector Hide`) plus a muted path string plus `Palette` / `Help`.
- Left rail paints **only threads**. Collapsed state is a single `Chats` strip, not an icon rail.
- Right rail paints **seven tiny text chips** (`Session Cores MCP Points Git Term Skills`) and a **text dump** from `session_detail` / `resource_detail` / `mcp_detail` / …
- Bottom is a **fixed 108px strip**, always open, not a drawer, no `` Ctrl+` ``.

That is a prototype scaffold. This sprint replaces it with the plan/10 drawers: left Outlook rail, right inspector as lists, bottom slide-up terminal, and a real title toolbar. Not a 2-button title bar.

**In scope:** GPUI projection of drawers + headless `Workspace` mutations that drive them.

**Out of scope this sprint:** pop-out windows, recursive splits, saved layouts, real Ghostty PTY, SVG icon atlas, true `uniform_list` if the 0.2.2 crate surface is awkward. Those stay in `plan/10`. The lists here are **virtualized-looking** (fixed row height, overflow scroll, one section's rows only).

---

## 1. Current code (do not reinvent)

Headless model lives in `crates/multiplexer-shell/src/workspace.rs`. Keep it GPUI-free.

Already present and reused:

| Field / type | Keep | Role this sprint |
|---|---|---|
| `ChromeLayout { left_open, right_open, left_width, right_width }` | yes | rail open/width. Occupied widths stay. |
| `LEFT_WIDTH_*`, `RIGHT_WIDTH_*`, `RAIL_COLLAPSED` | yes, bump collapsed (see §6) | clamp + occupied |
| `threads`, `selected`, `new_thread`, `select`, `delete_thread` | yes | left **Threads** rows |
| `files: Vec<String>` | yes | left **Files** + right **Files** tab |
| `terminal_log`, `term_draft`, `push_terminal` | yes | left **Activity** + right **Activity** + bottom drawer |
| `cores`, `mcp`, `checkpoints`, `skills`, `worktrees`, `git_status` | yes | right-tab row sources |
| `reminder: Option<(String, String)>` | yes | title **branch** pill (`reminder.0`) |
| `model`, `models`, `cycle_model` | yes | title **model** pill |
| `project`, `connection`, `busy` | yes | title project / run-stop |
| `inspector: InspectorTab` | extend | add `Files` and `Activity` |
| `palette_open`, `help_open` | yes | toolbar buttons |
| `*_detail()` string builders | keep for unit tests | **must not** be the painted right body |

Desktop projection: `apps/multiplexer-desktop/src/{main.rs,inspector.rs,theme.rs}`. `inspector.rs` today is `tab_buttons` + `inspector_body -> String`. Replace the body with row structs. Keep `InspectorAction` / `InspectorButton` for per-tab toolbar actions.

`ClientAction` is `Copy`. New chrome actions that stay `Copy`: `SelectLeftSection(LeftSection)`, `ToggleBottom`. Row expand is a `Workspace` method called from the click handler (`toggle_right_row(&str)`), not a `String`-bearing `ClientAction` variant.

---

## 2. Target window (shipping)

Native OS caption stays (`TitlebarOptions { appears_transparent: false, title: "Multiplexer" }`). The product chrome is the **toolbar under that caption**, then three columns, then the bottom drawer, then the existing 26px status bar.

```
┌──────────────────────────────────────────────────────────────────────────┐
│  (native caption: Multiplexer)                                           │
├──────────────────────────────────────────────────────────────────────────┤
│  [≡]  [proj ▾]  [⎇ branch]  [model pill]     [▶/■] [⌘K] [▦] [?]  [⋮]   │
├────┬───────────────────────────────────────────────────────┬─────────────┤
│ 💬 │  CENTER (unchanged this sprint: transcript+composer)  │ ◎ Session  │
│ ⚡ │                                                       │ ▣ Cores    │
│ 🗂 │                                                       │ ⬡ MCP      │
│ ⏱ │                                                       │ … icons+   │
│    │                                                       │   labels   │
│    │                                                       │            │
│    │                                                       │  list rows │
│    │                                                       │  ▾ detail  │
├────┴───────────────────────────────────────────────────────┴─────────────┤
│  BOTTOM drawer  120 collapsed / 280 expanded     `` Ctrl+` ``            │
├──────────────────────────────────────────────────────────────────────────┤
│  status bar (existing)                                                   │
└──────────────────────────────────────────────────────────────────────────┘
```

When the left rail is **open**, the 4-icon rail stays as a 44px column and the section list sits to its right (Outlook). When **collapsed**, only the 44px icon rail remains (`occupied_left() == RAIL_COLLAPSED`).

When the right rail is **open**, a vertical tab strip (icon + label) sits above a scrollable row list. When **collapsed**, only a 44px icon column of the 9 tabs remains. Clicking an icon selects that tab **and** opens the rail.

Bottom is never zero height this sprint. Collapsed is a 120px strip. Expanded is 280px. `` Ctrl+` `` toggles.

---

## 3. State to add on `Workspace` (exact names)

Add these four fields. Do not nest them in `ChromeLayout`. Tests and the desktop read these names.

```rust
pub left_section: LeftSection,
pub right_expanded_id: Option<String>,
pub bottom_open: bool,
pub bottom_height: f32,
```

### 3.1 `LeftSection`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftSection {
    Threads,
    Agents,
    Files,
    Activity,
}

impl LeftSection {
    pub fn all() -> [LeftSection; 4];
    pub fn label(self) -> &'static str;   // "Threads" | "Agents" | "Files" | "Activity"
    pub fn rail_label(self) -> &'static str; // icon-rail tooltip: "Chats" | "Agents" | "Projects" | "Activity"
    pub fn glyph(self) -> &'static str;   // see §7
}
```

Icon-rail names follow `plan/10` §2.1 (Chats, Agents, Projects, Activity). The **enum** uses `Threads` / `Files` because those are the data they bind:

| `LeftSection` | Icon-rail label | Data source |
|---|---|---|
| `Threads` | Chats | `workspace.threads` |
| `Agents` | Agents | session list: `connection` session ids, else threads as sessions |
| `Files` | Projects | `workspace.files` (already the project tree) |
| `Activity` | Activity | `workspace.terminal_log` plus `busy` / connection status |

Default: `LeftSection::Threads`.

### 3.2 `right_expanded_id`

Accordion: **at most one** inspector row shows its detail. `None` means all rows collapsed to one line.

Stable ids (desktop and tests must use these prefixes):

| Tab | Row id |
|---|---|
| Session | `session:project`, `session:model`, `session:connection`, `session:id`, `session:threads`, `session:models` |
| Cores | `core:{index}` |
| MCP | `mcp:{name}` |
| Points | `point:{id}` |
| Git | `git:wt:{index}`, `git:status` |
| Term | `term:{index}`, `term:draft` |
| Skills | `skill:{name}` |
| Files | `file:{path}` |
| Activity | `act:{index}`, `act:status` |

`toggle_right_row("core:0")` sets `right_expanded_id = Some("core:0")`. Calling it again with the same id sets `None`. Calling it with a different id replaces. Selecting a different `InspectorTab` **clears** `right_expanded_id` (a stale id must not highlight a row on another tab).

### 3.3 Bottom drawer fields

```rust
pub const BOTTOM_HEIGHT_COLLAPSED: f32 = 120.0;
pub const BOTTOM_HEIGHT_EXPANDED: f32 = 280.0;
pub const BOTTOM_HEIGHT_MIN: f32 = 120.0;
pub const BOTTOM_HEIGHT_MAX: f32 = 420.0;
```

Defaults on `Workspace::new`:

- `bottom_open = false`
- `bottom_height = BOTTOM_HEIGHT_COLLAPSED` (120.0)

`occupied_bottom()` returns `bottom_height` (the strip is always painted).

### 3.4 `InspectorTab` extension

Today: 7 variants, `all() -> [InspectorTab; 7]`.

This sprint **append** two variants (do not reorder the first seven; existing tests pin order):

```rust
pub enum InspectorTab {
    Session,
    Resources,    // label "Cores"
    Mcp,
    Checkpoints,  // label "Points"
    Git,
    Terminal,     // label "Term"
    Skills,
    Files,        // NEW, label "Files"
    Activity,     // NEW, label "Activity"
}

impl InspectorTab {
    pub fn all() -> [InspectorTab; 9];
    pub fn label(self) -> &'static str;  // existing + "Files" | "Activity"
    pub fn glyph(self) -> &'static str;  // NEW, see §7
}
```

`inspector_all_is_seven_tabs` and `InspectorTab::all().len() == 7` **must be rewritten** to nine. That is an intentional, reviewed test change, not drift.

---

## 4. Workspace mutations (headless API)

Add these methods on `impl Workspace`. They are the TDD surface. Desktop is a thin caller.

```rust
impl Workspace {
    pub fn select_left_section(&mut self, section: LeftSection) -> bool {
        // false when already that section
        // true when changed
    }

    pub fn toggle_bottom(&mut self) {
        // if bottom_open: close -> bottom_open=false, bottom_height=120
        // else:           open  -> bottom_open=true,  bottom_height=280
    }

    pub fn set_bottom_height(&mut self, height: f32) {
        // clamp to BOTTOM_HEIGHT_MIN..=BOTTOM_HEIGHT_MAX
        // bottom_open = height > BOTTOM_HEIGHT_COLLAPSED + 0.5
        // (drag-resize of the drawer handle)
    }

    pub fn occupied_bottom(&self) -> f32 {
        self.bottom_height
    }

    pub fn toggle_right_row(&mut self, id: impl Into<String>) {
        let id = id.into();
        if self.right_expanded_id.as_deref() == Some(id.as_str()) {
            self.right_expanded_id = None;
        } else {
            self.right_expanded_id = Some(id);
        }
    }

    pub fn collapse_right_row(&mut self) {
        self.right_expanded_id = None;
    }

    pub fn select_inspector(&mut self, tab: InspectorTab) -> bool {
        // if unchanged: false
        // else: set inspector, clear right_expanded_id, true
    }
}
```

`ClientAction::SelectTab` should call `select_inspector` (so tab changes clear the accordion). Keep `apply_layout_action` returning `false` when the tab is unchanged.

New `ClientAction` variants (still `Copy`):

```rust
SelectLeftSection(LeftSection),
ToggleBottom,
```

`apply_layout_action`:

- `SelectLeftSection(s)` -> `ws.select_left_section(s)`
- `ToggleBottom` -> `ws.toggle_bottom(); true`

`host_call` maps both to `HostCall::Local`.

Selecting a left section while the left rail is collapsed **opens** the rail (`chrome.left_open = true`) so the icon rail is a real navigator, not a no-op. That is part of `select_left_section` (or the desktop dispatch: if `!left_open { toggle_left(); } select_left_section`). Prefer doing the open in the **desktop** click/key handler so the headless method stays a pure section write. Headless tests then cover section-only; an actions test covers "select while closed opens the rail" if you put the open in `apply_layout_action`. **Decision for this sprint:** `select_left_section` only writes `left_section`. Desktop (and `apply_layout_action` for `SelectLeftSection`) opens the left rail when it was closed. Same for a collapsed right rail + `SelectTab`.

---

## 5. TDD test names (write these first)

Co-locate on `workspace.rs` under `#[cfg(test)]`. Names are the contract.

### 5.1 Defaults

- `new_workspace_defaults_left_section_threads`
- `new_workspace_defaults_bottom_collapsed_120`
- `new_workspace_right_expanded_id_is_none`

Assert on a fresh `Workspace::new("p", "m")`:

- `left_section == LeftSection::Threads`
- `right_expanded_id.is_none()`
- `bottom_open == false`
- `bottom_height == 120.0`
- `occupied_bottom() == 120.0`
- `inspector == InspectorTab::Session`

### 5.2 Left section

- `select_left_section_changes_only_when_different`
- `select_left_section_does_not_toggle_chrome`
- `left_section_all_is_four_in_outlook_order`
- `left_section_rail_labels_match_plan10`

Pin:

- `all() == [Threads, Agents, Files, Activity]`
- `rail_label`: Chats, Agents, Projects, Activity
- `label`: Threads, Agents, Files, Activity
- `select_left_section(Threads)` on a new workspace returns `false`
- `select_left_section(Files)` returns `true` and sets `Files`
- `chrome.left_open` / widths are **unchanged** by the method itself

### 5.3 Bottom drawer

- `toggle_bottom_opens_to_280`
- `toggle_bottom_closes_to_120`
- `set_bottom_height_clamps_and_sets_open`
- `set_bottom_height_below_min_stays_collapsed`
- `occupied_bottom_returns_height`

Pin:

- `toggle_bottom` from default: `bottom_open == true`, `bottom_height == 280.0`
- second `toggle_bottom`: `false` / `120.0`
- `set_bottom_height(80.0)` -> `120.0`, `bottom_open == false`
- `set_bottom_height(900.0)` -> `420.0`, `bottom_open == true`
- `set_bottom_height(200.0)` -> `200.0`, `bottom_open == true`
- `set_bottom_height(120.0)` -> `120.0`, `bottom_open == false`

### 5.4 Right accordion

- `toggle_right_row_expands_id`
- `toggle_right_row_same_id_collapses`
- `toggle_right_row_replaces_previous_id`
- `collapse_right_row_clears`
- `select_inspector_clears_right_expanded_id`
- `select_inspector_same_tab_keeps_expanded_id`

Pin:

- `toggle_right_row("core:0")` => `Some("core:0")`
- again => `None`
- then `"mcp:linear"` => `Some("mcp:linear")` (not both)
- `collapse_right_row` => `None`
- expand `"core:0"`, `select_inspector(Mcp)` => tab is Mcp **and** `right_expanded_id` is `None`
- expand `"core:0"`, `select_inspector(Resources)` (already there) => still `Some("core:0")`, returns `false`

### 5.5 Inspector tab catalog

- `inspector_all_is_nine_tabs`
- `inspector_files_and_activity_labels`
- `inspector_glyph_is_nonempty_for_every_tab`

Pin `all()` order:

`Session, Resources, Mcp, Checkpoints, Git, Terminal, Skills, Files, Activity`

Labels: `Session, Cores, MCP, Points, Git, Term, Skills, Files, Activity`

### 5.6 Actions (in `actions.rs`)

- `select_left_section_action_opens_closed_rail`
- `select_tab_action_opens_closed_right_rail`
- `toggle_bottom_action_flips_height`
- `host_actions_still_do_not_include_new_chrome`

`SelectLeftSection` / `ToggleBottom` are local. They must not appear in the host-noop list.

### 5.7 Bindings

- `toggle_bottom_is_local`
- `select_left_section_is_local`

### 5.8 Mutation target (D21)

`select_left_section`, `toggle_bottom`, `set_bottom_height`, `toggle_right_row`, `select_inspector` are **core chrome logic**. cargo-mutants on `workspace.rs` must kill:

- dropping the same-section early return
- failing to snap 120/280 on toggle
- clamp that ignores min or max
- accordion that appends instead of replacing
- tab change that leaves a stale `right_expanded_id`

Existing `*_detail()` tests stay. They are not the UI.

---

## 6. Layout numbers (GPUI now)

Keep the existing width clamps. Change only the collapsed strip.

| Token | Value | Notes |
|---|---|---|
| `RAIL_COLLAPSED` | **44.0** (was 36.0) | 32px glyph + 6px pad each side. Update `chrome_toggle_hides_to_collapsed_strip`. |
| `LEFT_WIDTH` default | 248.0 (keep) | open list, **not** including the 44px icon column? |
| `RIGHT_WIDTH` default | 300.0 (keep) | |
| Title toolbar height | 44.0 (was 48.0) | icon buttons, not wordy pills |
| Icon button hit | 32 x 32 | |
| Left row height | 56.0 (threads with preview) / 36.0 (files, activity) | |
| Right tab row height | 32.0 | icon + label, **no wrap** |
| Right list row height | 40.0 collapsed / auto when expanded | |
| Bottom collapsed | 120.0 | |
| Bottom expanded | 280.0 | |
| Status bar | 26.0 (keep) | |

**Left open geometry:** `occupied_left()` stays `left_width` when open (do not add 44 on top). The icon column is **inside** the left pane: 44px rail + remaining width for the list. When closed, the whole pane is 44px and only the rail paints.

**Right open geometry:** tab strip is a **vertical** column of icon+label rows at the top of the pane (not a wrapping chip wrap). Below it, a flex-1 overflow list. When closed, only the 44px glyph column (labels hidden).

**Bottom:** `terminal_strip` reads `workspace.occupied_bottom()` instead of `h(px(108.0))`. A 5px drag handle on the top edge of the drawer calls `set_bottom_height`. `` Ctrl+` `` calls `ToggleBottom`.

Resize handle for the bottom: desktop-only, same pattern as `DragRail::Left/Right`. Add `DragRail::Bottom`. Mouse move: `set_bottom_height(win_h - mouse_y - status_h)`.

---

## 7. Icons (GPUI 0.2.2: Unicode glyphs, not an SVG pack)

There is no icon font or SVG atlas in this app. **Do not add one this sprint.** Each control has a `glyph: &'static str` painted in `Theme::text()` (active) or `Theme::muted()` (idle), 32x32 hit target, `rounded_lg`, accent wash when selected.

```rust
impl LeftSection {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Threads => "💬",
            Self::Agents => "⚡",
            Self::Files => "🗂",
            Self::Activity => "⏱",
        }
    }
}

impl InspectorTab {
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Session => "◎",
            Self::Resources => "▣",
            Self::Mcp => "⬡",
            Self::Checkpoints => "⚑",
            Self::Git => "⎇",
            Self::Terminal => ">_",
            Self::Skills => "✦",
            Self::Files => "🗂",
            Self::Activity => "⏱",
        }
    }
}
```

Toolbar glyphs (constants on a small `ChromeGlyph` helper in `workspace.rs` or `apps/.../inspector.rs`):

| Control | Glyph | Label (tooltip / `aria` only, not a wordy pill) |
|---|---|---|
| Toggle left | `≡` | Chats |
| Project | folder mark `⌂` + truncated name | Project |
| Branch | `⎇` + branch text | Branch |
| Model | no extra glyph, **pill** of `model` | Model (click cycles) |
| Run | `▶` | Run (sends draft when idle) |
| Stop | `■` | Stop (when `busy`) |
| Palette | `⌘K` as two-char mark, or `⌕` | Palette |
| Layout | `▦` | Layout (reset Outlook: both rails open, default widths, bottom collapsed) |
| Help | `?` | Help |
| Toggle right | `⋮` | Inspector |

If a glyph fails to rasterize on a given Windows font, the **label string** is the fallback (`div().child(glyph).when(glyph_missing, label)`). Ship the Unicode first. We already paint `█░` bars, so the console font path works.

**Not allowed:** a title bar that is only `Chats` + `Inspector` ghost pills. Those two become icon buttons. The center of the toolbar is project / branch / model, not a single muted `path · model · disconnected` string.

---

## 8. Title toolbar (under native caption)

Replace `ShellView::title_bar` in `main.rs`. Height 44. `glass_bar()`, `border_b_1`, `px_2`, `gap_2`, `items_center`.

**Left cluster**

1. Icon button `≡` -> `ToggleLeft` (accent wash when `left_open`).
2. **Project pill:** `⌂` + `short_path(&workspace.project)`. Click is a no-op host hint this sprint (or copies the path). Not a file dialog.
3. **Branch pill:** `⎇` + branch text.
   - If `reminder` is `Some((branch, _))`, use `branch`.
   - Else if `git_status` is non-empty, first token before `·` or space.
   - Else literal `no branch`, muted.
4. **Model pill:** `workspace.model` in a rounded accent-wash chip. Click -> `CycleModel`. Tooltip lists `models`.

**Flex spacer**

**Right cluster**

5. **Run / Stop (one slot):**
   - `busy == false`: `▶` Run -> host `Send` (same as composer Enter). Disabled look when draft is empty.
   - `busy == true`: `■` Stop in `Theme::danger()` -> `Interrupt`.
6. `⌕` Palette -> `TogglePalette`.
7. `▦` Layout -> `reset_outlook_chrome()` (new workspace method, see below).
8. `?` Help -> `ToggleHelp`.
9. `⋮` Toggle right -> `ToggleRight` (accent wash when `right_open`).

```rust
impl Workspace {
    pub fn reset_outlook_chrome(&mut self) {
        self.chrome = ChromeLayout::default();
        self.left_section = LeftSection::Threads;
        self.bottom_open = false;
        self.bottom_height = BOTTOM_HEIGHT_COLLAPSED;
        // do not clear inspector, threads, or right_expanded_id
    }

    pub fn branch_label(&self) -> String { /* reminder / git_status / "no branch" */ }
}
```

Test: `reset_outlook_chrome_reopens_rails_and_collapses_bottom`.

**Connection** stays off the toolbar. It already lives in the status bar. Do not put `disconnected` in the title cluster.

Existing `title_bar()` **string** method on `Workspace` can stay for tests (`Multiplexer  ·  {project}  ·  {model}  ·  {status}`). The GPUI toolbar does not paint that string as the whole bar.

---

## 9. Left drawer (Outlook)

### 9.1 Collapsed: icon rail

Four stacked icon buttons, 44px wide, full height, `justify_start`, `pt_2`, `gap_1`.

- Click `LeftSection` icon: `SelectLeftSection(s)` (opens rail via actions, see §4).
- Selected section: accent wash + `Theme::accent()` glyph.
- Tooltip / `SharedString` id: `left-rail-threads` etc.

No `collapsed_strip("Chats")` one-button strip.

### 9.2 Expanded: rail + section list

Horizontal split inside the left pane:

```
[ 44px icons ][ section header + rows ]
```

**Section header** (36px): muted uppercase `left_section.label()`, plus section actions on the right.

| Section | Header actions |
|---|---|
| Threads | New (`+`), Del (`⌫`) : existing `NewThread` / `DeleteThread` |
| Agents | none this sprint (list is read-only) |
| Files | none (tree is `set_files` from the host) |
| Activity | none |

**Rows** (virtualized-looking): `div().id("left-list").flex_1().min_h_0().overflow_y_scroll()` then one child per row. Do **not** paint every section at once. Only `left_section`'s rows.

#### Threads rows (56px)

Reuse today's thread card, cleaned up:

- title (`thread.title`)
- preview (`Workspace::thread_preview`)
- status chip: `idle` / `running` / `error` using `Theme::muted` / `accent` / `danger`
- click -> `SelectThread(i)`
- selected index: accent wash + hairline

#### Agents rows (session list)

Build a headless helper so the desktop does not invent the list:

```rust
impl Workspace {
    pub fn agent_rows(&self) -> Vec<AgentRow> { /* see below */ }
}

pub struct AgentRow {
    pub id: String,
    pub title: String,
    pub status: String,
}
```

Rules:

1. If `connection` is `Connected { session_ids }` and `session_ids` is non-empty: one row per id, `title = id`, `status = connection.status_label()`.
2. Else: one row per `threads` entry, `id = thread.id`, `title = thread.title`, `status = thread.status`.

Click an agent row: if it came from a thread, `select` that thread. Session-id-only rows are display-only this sprint (no extra session switch API).

#### Files rows (project tree)

`workspace.files` is already a `Vec<String>` of paths. Paint each as a 36px row:

- glyph `·` or `🗎`
- path text, truncated from the left if needed (`short_path`)
- click this sprint: host `CycleFile` **or** a new no-op that sets a `selected_file: Option<String>` if you add it. **Do not add `selected_file` unless a test needs it.** Click may copy the path into the composer draft only if that is already a host action. Prefer: click focuses the right **Files** tab and expands `file:{path}`.

```
on click: select_inspector(Files); toggle_right_row(format!("file:{path}")); chrome open right
```

That is local and testable.

#### Activity rows

One **status** row (`act:status`): `{connection.status_label()} · {busy? running : idle} · {threads.len()} chats`.

Then one row per `terminal_log` line (`act:{i}`), 36px, muted mono-ish text, last 40 lines max (the log is already capped at 80). Click expands on the right Activity tab (same jump pattern as Files).

Empty states (one muted row, not a blank pane):

- Threads: never empty (`new` keeps one)
- Agents: `No sessions`
- Files: `No files yet`
- Activity: `No activity`

---

## 10. Right drawer (inspector is a list, not a dump)

### 10.1 Tab strip: icons + labels, not 7 tiny chips

Delete the `flex_wrap` chip row.

**Open:** a vertical stack under a 28px `INSPECTOR` caption:

```
[◎  Session]
[▣  Cores  ]
[⬡  MCP    ]
...
```

Each tab: `h(px(32.0))`, `px_2`, `gap_2`, `items_center`, glyph then `t.label()`. Selected: accent wash. Click -> `SelectTab(t)` (opens rail if closed).

Nine tabs. They will scroll if the window is short: wrap the tab stack in `overflow_y_scroll` with a max height of ~40% of the pane **or** keep tabs in a single non-wrapping column and let the list below shrink. Prefer: tabs take their natural height (9 × 32 = 288) only if the pane is tall; otherwise a compact **horizontal** icon+label strip that **does not wrap** (`overflow_x_scroll`, `flex_row`, `flex_nowrap`). 

**Sprint pick (implement this):** horizontal strip, `flex_row`, `flex_nowrap`, `overflow_x_hidden` is wrong. Use `overflow_x_scroll` if the pane is narrower than ~9×72. Each tab is **icon + label** at ~72px (`w(px(72.0))`, glyph above or before the label). **Not** a 28px text chip. This matches "tab strip with ICONS + labels (not 7 tiny text chips)" and fits a 300px rail better than a 288px vertical stack eating the list.

Collapsed right rail: **vertical** 44px column of the 9 glyphs only (no labels). Click selects + opens.

### 10.2 Body: list of interactive rows

`inspector_body() -> String` remains for existing unit tests. The GPUI rail **must not** call it as the painted child.

Add a headless row model in `workspace.rs` (or `inspector.rs` if you want the desktop crate to own presentation). Prefer `workspace.rs` so tests stay headless:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub detail: String,
}

impl Workspace {
    pub fn inspector_rows(&self, session_id: Option<&str>) -> Vec<InspectorRow>;
}
```

`inspector_rows` switches on `self.inspector`:

| Tab | Rows |
|---|---|
| Session | project, model, connection, session id, thread count, model catalog |
| Cores | one per `cores` (title `cpu{index}`, subtitle `{usage:.1}%` + reserved mark, detail = bar + reserved). Empty: one row `cores:empty` / `waiting` |
| MCP | one per `mcp` (title name, subtitle transport, detail command). Empty: `No MCP servers` |
| Points | one per `checkpoints` (title id, subtitle label, `*` in subtitle if selected). Empty: `No checkpoints yet` |
| Git | one row per worktree + one `git:status` row |
| Term | one row per log line (cap visible 40) + `term:draft` |
| Skills | one per skill name. Empty: `No skills found under .grok/skills` |
| Files | one per `files` path |
| Activity | `act:status` + log lines |

**Paint:**

```
for row in inspector_rows:
  header (40px): title + muted subtitle
  on click: toggle_right_row(&row.id)
  if right_expanded_id == row.id:
      detail block (muted, wrap, px_3 py_2)
```

Per-tab **action row** stays above the list, using existing `tab_buttons` (Model/Copy, Reload, New/Revert, …). Skills / Term / Files / Activity may add:

- Files: none
- Activity: none
- Term: keep `term_run` / `term_clear` in the **bottom** drawer, not duplicated here. Term tab is a log list.

Click behaviors that already exist stay wired (`inspector_click`):

- Session / Model -> `CycleModel`
- Copy -> clipboard session id
- Cores Reload -> `RefreshCores`
- MCP Reload -> `RefreshMcp`
- Points New / Revert -> create / restore (`selected_checkpoint` from the expanded `point:{id}` or `selected_checkpoint`)
- Git Reload / Status / New WT -> existing host actions

Expanding a Points row should also `select_checkpoint(Some(id))` so Revert has a target. Test: `toggle_right_row_on_point_selects_checkpoint` if you put that side effect in `toggle_right_row`. Cleaner: desktop click on a Points row calls `select_checkpoint` then `toggle_right_row`. Prefer desktop for that side effect. Headless `toggle_right_row` stays accordion-only.

### 10.3 Empty and waiting

Never paint a raw multi-line dump. Never paint an empty `div`. One `InspectorRow` with a muted title is the empty state.

---

## 11. Bottom drawer (slide-up terminal)

Replace the always-on `h(px(108.0))` strip.

### 11.1 Chrome

```
┌─ grab handle (5px, cursor resize NS) ─────────────────────────┐
│  >_  Terminal                          [Clear]  [`` ` ``]     │
│  log (flex-1, overflow_y_scroll, last N lines)                │
│  [ draft input ] [ Run ↵ ]                                    │
└───────────────────────────────────────────────────────────────┘
```

- Height: `px(workspace.occupied_bottom())`.
- Header 28px: glyph `>_`, label `Terminal`, Clear (existing), chevron that calls `ToggleBottom`.
- Collapsed (120): show last **4** lines (`visible_tail`) + draft. Same as today, 12px taller.
- Expanded (280): show last **16** lines + draft. Still not a PTY. Still `term_draft` + `Run` + builtins.

### 11.2 Toggle

- `` Ctrl+` `` in `handle_key`: if `key == "\`` || key == "`"` with `mods.control` -> `ToggleBottom`. GPUI on Windows reports the key as `` ` ``. Bind that. Also accept `key == "oem_3"` if that is what 0.2.2 emits; pin the real key in a comment after the first run.
- Palette item: `{ id: "toggle-terminal", label: "Toggle terminal", hint: "Ctrl+`", action: ToggleBottom }`.
- Help overlay line: add `` Ctrl+` terminal drawer ``.

When the user focuses the draft (`Focus::Terminal`) and the drawer is collapsed, **do not** auto-expand this sprint. `` Ctrl+` `` is the explicit gesture.

### 11.3 Not this sprint

Ghostty, splits, scrollback search, true PTY. `plan/08` still owns those. This drawer is the existing log+draft, resized.

---

## 12. Keyboard (additions only)

Existing chords stay (`Ctrl+K`, `Ctrl+[`, `Ctrl+]`, `Ctrl+N`, `Ctrl+.`, `Ctrl+S`, `F1`, `Esc`).

| Action | Chord |
|---|---|
| Toggle bottom drawer | `` Ctrl+` `` |
| Left section Threads | `Ctrl+1` |
| Left section Agents | `Ctrl+2` |
| Left section Files | `Ctrl+3` |
| Left section Activity | `Ctrl+4` |
| Reset Outlook chrome | `Ctrl+Shift+L` (layout button) |

`Ctrl+1..4` are ignored when `Focus::Terminal` or the palette is open (digits must type). Bind them in the global branch of `handle_key` **before** the composer character path, same as `Ctrl+N`.

`controls.rs` catalog: add ids (do not break the 39-count test silently; **update** `REQUIRED_IDS` and the length asserts in the same change).

New required ids:

```
left_section_threads
left_section_agents
left_section_files
left_section_activity
tab_files
tab_activity
toggle_bottom
layout_reset
run
branch_pill
project_pill
model_pill
```

Title-bar surface picks up `run`, `layout_reset`, `project_pill`, `branch_pill`, `model_pill`. `stop` stays. `chats_toggle` / `inspector_toggle` stay but their **labels** become `Chats` / `Inspector` with glyph projection (the catalog label can stay words; the painted control is the glyph).

---

## 13. Desktop file map (first_code)

Implement in this order. Tests on the model go red first.

| Step | File | What |
|---|---|---|
| 1 | `crates/multiplexer-shell/src/workspace.rs` | `LeftSection`, four fields, constants, methods, inspector 9-tab `all`/`glyph`, `inspector_rows`, `agent_rows`, `branch_label`, `reset_outlook_chrome`, tests in §5 |
| 2 | `crates/multiplexer-shell/src/lib.rs` | re-export `LeftSection`, `InspectorRow`, `AgentRow`, bottom constants |
| 3 | `crates/multiplexer-shell/src/actions.rs` | `SelectLeftSection`, `ToggleBottom`; open-rail-on-select; tests |
| 4 | `crates/multiplexer-shell/src/bindings.rs` | Local mapping + tests |
| 5 | `crates/multiplexer-shell/src/palette.rs` | toggle-terminal, left-section, layout-reset items |
| 6 | `apps/multiplexer-desktop/src/controls.rs` | new ids / surfaces / shortcut `` ctrl-` `` |
| 7 | `apps/multiplexer-desktop/src/inspector.rs` | `inspector_rows` projection helpers if not all in workspace; keep `tab_buttons`; stop using the dump as the only body |
| 8 | `apps/multiplexer-desktop/src/main.rs` | `title_bar`, `left_rail` (icon rail + section lists), `right_rail` (icon+label tabs + row list), `terminal_strip` (height from `occupied_bottom`), `DragRail::Bottom`, `` Ctrl+` ``, `Ctrl+1..4` |

Do not touch `multiplexer-layout` for this sprint. The Outlook forest (3 live panes) stays as-is. Drawers are chrome **inside** the primary window, not new `LayoutNode`s.

---

## 14. GPUI implementation notes (so this actually ships)

These are constraints of **gpui 0.2.2** as used in `main.rs` today. Follow the existing style (`div()`, `glass_pane()`, `ghost_btn`, `cx.listener`, `SharedString` ids).

1. **No new widgets.** Every control is a `div` with `on_mouse_down(MouseButton::Left, …)` like the current thread rows. Icon buttons are the same as `ghost_btn` but square (`w(px(32.0)).h(px(32.0))`) and a glyph child instead of `"Chats" / "Hide"`.
2. **Scroll:** `.overflow_y_scroll()` on the left list, right list, and bottom log. GPUI 0.2.2 already uses this pattern? If a method name differs, use whatever `div()` already accepts in this crate (check sibling code). If overflow scroll is missing, fall back to painting a window of rows (`visible_tail` style) with a clamped slice. That is still "virtualized-looking."
3. **Do not introduce `uniform_list`** unless a one-line compile proves it exists on 0.2.2. The sprint accepts a mapped `Vec` of rows. Threads and logs are tens of items, not thousands, today.
4. **Ids:** every clickable `div` needs `.id(SharedString::from(...))`. Use the row ids from §3.2 so hover/click state is stable.
5. **Clone for listeners:** same as today (`threads.clone()`, `move |this, …|`). Do not refactor to entities.
6. **Theme:** reuse `Theme::{glass, ink, text, muted, accent, danger, hairline, hairline_bright}`. Selected row wash: `hsla(0.58, 0.35, 0.22, 0.45)` (already used).
7. **Native caption:** leave `appears_transparent: false`. A custom Windows titlebar is a later sprint. The product toolbar is the **next** row.
8. **Help overlay** copy: replace the current blob with lines that mention the new chords. Do not leave "Ctrl+[ / ] rails" as the only chrome story.

`ghost_btn` can stay for secondary actions (New, Del, Clear, tab action row). The **title toolbar** must be icon buttons.

---

## 15. Acceptance (parent checks this before calling the sprint done)

A person launching `multiplexer-desktop` on Windows must see:

1. Native caption **and** a toolbar with project pill, branch pill, model pill, run/stop, palette, layout, help. Not two text pills.
2. Left: four icon buttons when collapsed. Expanded: those icons plus a section header and real rows for Threads / Agents / Files / Activity.
3. Right: nine tabs as icon+label controls. Each tab is a list of rows. Clicking a row expands detail. Clicking again collapses. Switching tabs collapses.
4. Bottom: 120px strip that grows to 280px on `` Ctrl+` `` and shrinks back. Log + draft still work. Clear still works.
5. `Ctrl+[`, `Ctrl+]`, `Ctrl+1..4`, `Ctrl+K`, `F1` still work.
6. Existing send / stop / approvals / palette / grok -p path still work. This sprint does not regress the agent loop.
7. `cargo test -p multiplexer-shell` is green, including every test name in §5.
8. `cargo test -p multiplexer-desktop` is green (`inspector.rs` tests updated for 9 tabs; body tests may still assert `*_detail()` equality for the dump helper).
9. No em dash in new user-visible copy. No secrets in chrome. No new files except this plan (implementation edits existing crates).

---

## 16. Explicit non-goals (do not sneak in)

- Pop-out / detach / `LayoutForest` edits (`plan/10` §4)
- Saved `.multiplexer/layout.json`
- Ghostty / real PTY (`plan/08`)
- MCP Customize / registry browse (`plan/26`)
- Resource visual graphs (`plan/24`) beyond the existing core rows
- SVG / Lucide / Zed icon crate
- Light theme
- Transparent custom Windows caption
- Making `ClientAction` non-`Copy`

If a follow-up needs any of those, it is a new plan doc, not a drive-by in this sprint.

---

## 17. Open questions (none blocking)

Plan/10's deferred items (default preset, mobile layout sync, multi-window) stay deferred. This sprint **fixes** plan/10 open question 6 for the desktop default: the shipping default **is** the Outlook three-column + collapsed-120 bottom drawer, both side rails open, left section Threads, inspector Session.

No new product decision is required to implement this file.

---

## PARENT_IMPLEMENT

```
files: plan/30-chrome-drawers.md
first_code: workspace.rs fields + desktop rails
```

Parent sequence:

1. Red tests in `workspace.rs` for the four fields and the names in §5.
2. Implement `LeftSection`, fields, mutations, 9-tab `InspectorTab`, `inspector_rows`.
3. Wire `ClientAction` + palette + bindings + `controls.rs`.
4. Replace `title_bar`, `left_rail`, `right_rail`, `terminal_strip` in `apps/multiplexer-desktop/src/main.rs`.
5. Run `cargo test -p multiplexer-shell` and `cargo test -p multiplexer-desktop`. No other packages required to prove this sprint.

The model is the source of truth. The desktop only projects it.
