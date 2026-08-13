# 03. Right inspector audit

Scope: right rail tabs, toolbar buttons, list rows, expand. Compared to
`plan/33-inspector-customize.md`, `plan/30-chrome-drawers.md`,
`plan/32-list-rows.md`, `plan/29-icon-system.md`, and `plan/36-feature-gap-ui.md`.

Sources: `apps/multiplexer-desktop/src/main.rs` (`right_rail`, `inspector_row_el`),
`apps/multiplexer-desktop/src/inspector.rs`,
`crates/multiplexer-shell/src/inspector_model.rs`,
`crates/multiplexer-shell/src/workspace.rs`,
`crates/multiplexer-shell/src/icons.rs`.

The painted body is `inspector_rows`, not `inspector_body`. The string dumps
are unused by GPUI but still define leftover copy and tests. Plan/30 promised
nine tabs (seven plus Files and Activity). The enum is now ten (`Agents` last).

---

## Findings

### F01. Files Reload button calls RefreshMcp

- **Severity:** High
- **Evidence:** `apps/multiplexer-desktop/src/inspector.rs` `tab_buttons`
  (`InspectorTab::Files` => `InspectorAction::RefreshMcp`, hint `"Refresh files"`).
  `inspector_click` maps that enum only to `refresh_mcp()`. `InspectorAction`
  has no files variant. `ClientAction` also has no `RefreshFiles`.
- **Problem:** The one Files toolbar control is a lie. Click reloads
  `~/.grok/config.toml` into `ws.mcp` and leaves `ws.files` unchanged. The
  button looks live and does the wrong inventory.
- **Fix + test:** Add `InspectorAction::RefreshFiles` (or drop the parallel
  enum and dispatch `ClientAction` through `host_call`) that re-runs
  `list_project_tree` into `ws.files`. Test:
  `tab_buttons_files_reload_is_not_refresh_mcp` asserts the Files Reload
  action is not `RefreshMcp`, then a host test that a planted `ws.files`
  list is replaced by the tree walk.

### F02. Expand highlights a row and then shows nothing

- **Severity:** High
- **Evidence:** `inspector_row_el` (`apps/multiplexer-desktop/src/main.rs`)
  click only calls `toggle_right_row`. The extra child is
  `if expanded && !meta.is_empty()`. `inspector_model.rs` sets `meta` on
  Cores (`usage_bar`), Session Threads (count), and Agents (`N msgs`).
  Session Project/Model/Connection/Id, MCP, Points, Git, Term, Skills,
  Files, and Activity log rows have empty `meta`. `ListRowSpec` has no
  `actions` / `detail` field. Plan/30 paints a detail block when
  `right_expanded_id == row.id`. Plan/32: expanded rows show the next
  action, not a second dump.
- **Problem:** Accordion state works (`expand_flag_follows_workspace`).
  The user click still looks like a no-op on eight of ten tabs: wash
  changes, no command, no extra copy. Cores expand only to an ASCII bar
  (see F07). Height helper `ListRowSpec::height()` (88 when expanded) is
  unused by GPUI.
- **Fix + test:** Put expand payload on the spec (`meta` or a real
  `detail` plus 1 to 3 `RowAction`s per plan/33). Renderer must paint
  that block and icon hits. Click on Points must also
  `select_checkpoint`. Click on Files must `select_file`. Click on Git
  must `SelectWorktree(i)`. Tests: `expanded_non_section_paints_detail`
  (every non-empty catalog row has non-empty `meta` or `actions` when
  expanded); `toggle_right_row_on_point_selects_checkpoint`;
  `file_row_click_sets_selected_file`.

### F03. Brand marks are slug text, not icons

- **Severity:** High
- **Evidence:** `mcp_rows` stores `BrandIcon::slug()` (`"github-light"`)
  on `ListRowSpec.icon`. `inspector_row_el` does
  `.child(div().text_color(Theme::accent()).child(icon))`. No `img()`
  in `apps/multiplexer-desktop`. No `apps/multiplexer-desktop/src/icons.rs`,
  no `assets/brands/`. `skill_rows` always uses `ChromeGlyph::Sparkle`
  and never `BrandIcon::from_name`. Tab chips use `t.glyph()` unicode.
  Plan/29 D80: a matching MCP/Skills name **must** show the brand PNG.
  The unit test `mcp_row_uses_brand_slug_when_known` locks the slug
  string, not a paint path.
- **Problem:** GitHub/Linear/Docker rows show the characters
  `github-light` (or a plug glyph). Skills never get a brand. The
  catalog and `docs/THIRD_PARTY_ICONS.md` exist; the rail does not use
  them.
- **Fix + test:** Vendor the pin-table PNGs. Desktop `BrandBadge` calls
  `img()` on `asset_path()`. Keep the slug only as the resolver key.
  Skills use `icon_for_skill`. Test: `mcp_row_icon_is_brand_slug` stays
  for the resolver; add a desktop/component assert that a github row
  is not painted as the literal `"github-light"` text, and
  `icon_for_skill("github-triage")` is `Brand(Github)`.

### F04. Ten glyph+label chips wrap in a 300px rail

- **Severity:** High
- **Evidence:** `right_rail` open strip is
  `div().flex().px_2().pt_2().gap_1().flex_wrap()` over
  `InspectorTab::all()` (`[InspectorTab; 10]`), each child
  `format!("{} {}", t.glyph(), t.label())`. Default
  `ChromeLayout.right_width` is 300, min 220
  (`crates/multiplexer-shell/src/workspace.rs`). Labels include
  Session, Points, Skills, Activity, Agents. Plan/30 §6: tab strip is
  a **vertical** icon+label column, "not a wrapping chip wrap."
  `apps/multiplexer-desktop/src/controls.rs` `REQUIRED_IDS` still lists
  seven tabs (`tab_session` … `tab_skills`) and omits `tab_files`,
  `tab_activity`, and `tab_agents`.
- **Problem:** The chip wrap eats two to three rows before the toolbar
  and list. Hits shrink. Files/Activity/Agents are easy to miss. The
  catalog still thinks there are seven tabs, so tests cannot catch a
  dead ninth/tenth chip.
- **Fix + test:** Paint a single-row icon strip (or the plan/30
  vertical stack). Tooltips carry the word. Add `tab_files`,
  `tab_activity`, `tab_agents` to `REQUIRED_IDS`. Test:
  `inspector_all_len_matches_catalog_tab_ids` (every `InspectorTab::all()`
  label has a RightRail control id); a layout test that the open tab
  strip height is one icon row (or N vertical 32px rows), not a wrap.

### F05. Files and Activity have no real actions

- **Severity:** High
- **Evidence:** Plan/36 §4.1 Files buttons: Reveal (copy path),
  `@` mention, Reload. Headless already has `Workspace::select_file`,
  `insert_file_mention`, `selected_file`. `file_rows` never sets
  `selected`, never splits last-path-segment vs full path, never
  attaches Open/Copy. Desktop `cycle_file` still rotates `ws.files`.
  Activity `tab_buttons` is `Vec::new()`. `activity_rows` is
  `terminal_log` plus an idle/busy chip. It ignores `ws.reminder`,
  `ws.pending`, and `ws.busy` as their own cards. Plan/33 §6.7 wants
  Dismiss / Approve / Deny / Copy / Play (`RunTerminal`).
  `ClientAction::InsertFileMention` exists and is not on any inspector
  button.
- **Problem:** Files is a path list you can highlight. You cannot
  reload, reveal, or `@` mention from the rail. Activity is a second
  Term log with no copy, run, or approval. The left Files empty-state
  even says "Reload from the Files tab."
- **Fix + test:** `tab_buttons(Files)` = Reload, Reveal, Mention
  (or row icons for the last two). Wire Reload to a real tree walk,
  Reveal to clipboard of `copy_text`, Mention to `insert_file_mention`
  after `select_file`. Activity toolbar: Play. Rows: reminder,
  approval, busy, then log, with the plan/33 actions. Test:
  `file_tree_select_expand_and_mention` (plan/36); 
  `tab_buttons_files_has_reveal_mention_reload`;
  `activity_orders_reminder_then_approval_then_log`.

### F06. Term, Skills, Activity, and Agents toolbars are empty

- **Severity:** Medium
- **Evidence:** `tab_buttons` returns `Vec::new()` for
  `Terminal | Skills`, `Activity`, and `Agents`. The rail still
  mounts the padded toolbar `div` (`px_2().pt_2().flex().gap_1()`),
  so those tabs keep a blank strip. Plan/33 `tab_toolbar`: Skills =
  Reload (`RefreshSkills`), Term = Play (`RunTerminal`). Plan/36
  Agents is a local-thread dashboard (no spawn). `RefreshSkills` is
  not a `ClientAction`. MCP has Reload only; `StartMcp` / `StopMcp`
  exist on `ClientAction` and `Workspace::{start,stop}_mcp` and never
  appear on a row or toolbar.
- **Problem:** Four tabs look unfinished. Skills cannot refresh. Term
  cannot run from the inspector. MCP start/stop are headless-only.
  The empty flex row still consumes vertical space (with F04, the
  list starts far down).
- **Fix + test:** `tab_toolbar(tab)` as specified. Hide the toolbar
  container when empty until icons exist. MCP rows get Start/Stop
  (honest in-process projection). Tests: `tab_toolbar_skills_has_refresh`,
  `tab_toolbar_term_has_play`, `tab_toolbar_agents_empty_or_new_thread`,
  `mcp_row_actions_include_start_and_stop`.

### F07. String dumps and ASCII bars are still the fallback contract

- **Severity:** Medium
- **Evidence:** `inspector_body` is `#[allow(dead_code)]` and still
  maps every tab onto `Workspace::{session,resource,mcp,checkpoint,git,
  terminal,skills,files,activity,agents}_detail`. `resource_detail`
  still prints Worktrees and Files and `tiny_usage_bar`. `session_detail`
  is the eight-field newline sandwich (Project … Help). Cores rows put
  `usage_bar(c.usage, 10)` (`█` / `░`) in `meta`, so the only working
  expand is the old paragraph. Plan/33: no `█` / `░` in title/subtitle;
  CoreCell grid, not a text bar. Plan/32: rail must not paint one blob.
  The rail does not call `inspector_body`, but tests still treat the
  blob as the tab oracle (`inspector_body_matches_tab`).
- **Problem:** Dead copy keeps the debug-panel model alive. Cores
  expand reintroduces the bar the row rewrite was meant to kill.
  Session rows dropped Models, Palette, and Help even though the dump
  still has them. Agents has a second dump (`agents_detail`) that the
  rail never shows.
- **Fix + test:** Stop calling `*_detail` from anything but a
  deprecated test module. Cores `meta` becomes the numeric percent
  (already on subtitle) or a reserved sentence, never `usage_bar`.
  Session projector gains Models / Palette / Help definition rows
  (plan/33 §6.1). Tests: `cores_are_cells_not_text_bar` (no `█`/`░`);
  `session_is_definition_list` titles in order including Models,
  Palette, Help; delete or `#[ignore]` `inspector_body_matches_tab`
  once rows are the contract.

### F08. Toolbar ghost buttons paint the hint as a second label

- **Severity:** Medium
- **Evidence:** `right_rail` maps each `InspectorButton` through
  `ghost_btn(b.label, b.hint, …)`. `ghost_btn` children are `label`
  then a muted `hint`. Session paints "Model" plus "Cycle the session
  model" and "Copy" plus "Copy the session id". Git paints three
  long hints ("Refresh worktrees", "Run git status", "Hint a worktree
  path") in the same 300px wrap as F04.
- **Problem:** Hints were written as tooltips. On the rail they are
  visible words, so the action row wraps and collides with the tab
  chips. Plan/33 wants 1 to 3 **icons**, not labeled sentences.
- **Fix + test:** Icon-only toolbar (`RowIcon` / `ChromeGlyph`) with
  hint as hover/aria only. Test: `tab_toolbar_git_is_three_icons`
  (labels empty or single glyphs, hints stay on the spec); a render
  assert that the toolbar is one 32px row.

### F09. Row click never selects the thing Revert / Mention / Status need

- **Severity:** Medium
- **Evidence:** `inspector_row_el` on_mouse_down only
  `toggle_right_row(id)`. It does not call `select_checkpoint`,
  `select_file`, or set `selected_worktree`. `file_rows` never sets
  `row.selected` from `ws.selected_file`. `revert_checkpoint` falls
  back to `checkpoints.last()`. `insert_file_mention` no-ops without
  `selected_file`. Plan/30: Points expand should select so Revert has
  a target. Plan/33: Git card click fires `SelectWorktree(i)`.
- **Problem:** Revert can hit the wrong checkpoint. Mention and
  Reveal have no selection. Git Status/New WT ignore which card was
  opened. Expand and selection are different machines and only expand
  is wired.
- **Fix + test:** Desktop click: parse id prefix, call the matching
  select, then toggle. `file_rows` marks `selected_file`. Tests:
  `toggle_right_row_on_point_selects_checkpoint`;
  `file_row_click_sets_selected_file`;
  `git_card_click_sets_selected_worktree`.

### F10. New WT is still a composer paste

- **Severity:** Medium
- **Evidence:** `InspectorAction::NewWorktreeHint` sets
  `draft = "git worktree add ../mux-feat -b feat"`. Plan/33 replaces
  that with `ClientAction::CreateWorktree` (`git.worktree.create`).
  Plan/36 §4.3 wants path/branch/create-branch fields (`ws.wt_path`,
  `ws.wt_branch`, `ws.wt_create_branch` already exist on `Workspace`)
  and a Create button. Those fields are unused by the Git tab.
- **Problem:** The third Git button is a hint, not a control. The
  draft fields for a real create UI sit on the workspace unused.
- **Fix + test:** Toolbar Plus/Create dispatches `CreateWorktree`
  with `{cwd, path, branch, create_branch}`. Keep the hint as a
  secondary copy action if tests need it. Test:
  `worktree_create_draft_dispatches_rpc` (plan/36).

---

## FINDINGS: 10
