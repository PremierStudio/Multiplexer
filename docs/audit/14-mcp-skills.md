# 14. MCP / Skills / Hooks UI depth

**Scope:** Inspector MCP and Skills rows, `multiplexer-mcp` supervisor + skills/hooks parse, desktop paint path.
**Plans:** `plan/21-mcp-lifecycle-supervisor.md`, `plan/26-mcp-skills-ui.md`, `plan/33-inspector-customize.md` (icon paint also `plan/29`).
**Method:** Read-only. No cargo.

## Verdict

The rail lists configured MCP names and skill names. It is not a control surface. There is no start or stop, no live lifecycle badge, no hooks section, no PNG brand marks, and no supervisor in the running app. Plan/21 owns the state machine, plan/26 owns the Customize editors, plan/33 is the Phase 0.4 row projector. The projector exists as a thin list. The host never feeds it live state.

## Findings

### F1. No start / stop (or any per-row MCP action)

- **Severity:** major
- **Category:** gap
- **Path:** `crates/multiplexer-shell/src/inspector_model.rs`, `crates/multiplexer-shell/src/actions.rs`, `apps/multiplexer-desktop/src/inspector.rs`
- **Quote:** `InspectorTab::Mcp => vec![button("Reload", "Refresh MCP inventory", InspectorAction::RefreshMcp)]`
- **Plan:** plan/21 spawn and teardown; plan/26 enable/disable and "Trigger teardown"; plan/33 `RowIcon::{Play, Stop}` and 1 to 3 actions per non-section row.
- **Evidence:** `ClientAction` has `RefreshMcp` only. There is no `StartMcp`, `StopMcp`, or `ToggleMcp`. `ListRowSpec` has no `actions` field, so `mcp_rows` cannot attach Play/Stop. `ChromeGlyph::{Play, Stop}` exist in `icons.rs` and are unused on MCP rows. The MCP toolbar is inventory Reload. The Skills toolbar is empty (`InspectorTab::Terminal | InspectorTab::Skills => Vec::new()`). Palette "Stop" is `ClientAction::Interrupt` (running turn), not a server teardown.
- **Impact:** A listed server cannot be started, stopped, or tested from the rail. The pile-up plan/21 diagnoses stays invisible and unactable.
- **Fix:** Add row actions on `ListRowSpec` (plan/33 `RowAction`). MCP rows: Refresh + Copy now; Play/Stop when the host can acquire/release a `Supervisor` handle. Do not fake Ready.

### F2. No lifecycle state on MCP rows

- **Severity:** major
- **Category:** gap
- **Path:** `crates/multiplexer-shell/src/workspace.rs`, `crates/multiplexer-shell/src/inspector_model.rs`, `crates/multiplexer-shell/src/integrations.rs`
- **Quote:** `pub struct McpRow { pub name: String, pub command: String, pub transport: String, }`
- **Plan:** plan/33 §5.1 `McpRow.state: McpLiveLabel` (`Configured` / `Ready` / `Stopped` / `Unknown`). Badge copy is `configured`, `Ready`, `Stopped`, or `Unknown`. plan/21 states are `spawned` / `ready` / `crashed` / `stopped` (plus `Failed` in code).
- **Evidence:** `McpRow` has no `state` field and no `McpLiveLabel`. `mcp_rows` badges `m.transport` (`stdio` / `http`) with `Tone::Neutral`. `integration_tiles` badges every MCP tile `"configured"` as a constant, not a projection. Desktop `refresh_mcp` copies inventory `name` / `command` / `transport` only. There is no `mcp.status` wire method (`crates/multiplexer-wire` has no mcp symbols). The string fallback `mcp_detail` still prints `name [transport]` plus command.
- **Impact:** Ready, crashed, stopped, and failed look the same as "a line in config.toml". The differentiator in plan/26 §4.7 (configured plus live fleet) is not on screen.
- **Fix:** Add `McpLiveLabel` defaulting to `Configured`. Host maps `Supervisor::state` when a snapshot exists. Badge the label, not the transport. Keep transport in the subtitle or meta.

### F3. Hooks parse is unused by the UI

- **Severity:** major
- **Category:** debt
- **Path:** `crates/multiplexer-mcp/src/skills.rs`, `apps/multiplexer-desktop/src/main.rs`, `crates/multiplexer-shell/src/workspace.rs`, `crates/multiplexer-shell/src/inspector_model.rs`
- **Quote:** desktop import is `list_dir_entry_names, load_user_mcp_inventory, merge_skill_rows, parse_skill_names, skill_dir_candidates` (no `parse_hooks_tomlish`)
- **Plan:** plan/33 §5.3 `Workspace.hooks: Vec<HookItem>`; Skills tab renders a Hooks section only when parse is non-empty; `PreToolUse` badge `block`. plan/26 lists hooks by event and flags the blocking `PreToolUse` event.
- **Evidence:** `parse_hooks_tomlish` and `HookRow` are exported from `multiplexer-mcp`. Call sites outside `skills.rs` tests: none. `Workspace` has `skills: Vec<String>` and no `hooks` field. `skill_rows` maps each string to a sparkle row and never emits a `Hook` row. Host fill in `ShellView::new` formats `"{name} [{source}]"` and never reads `{project}/.grok/hooks.toml` or `{home}/.grok/hooks.toml`. `inspector_model` tests do not cover a hooks section.
- **Impact:** Project and user hooks are invisible. A blocking `PreToolUse` hook cannot be seen or trusted from the rail (plan/26 D76 stays unstarted).
- **Fix:** Host `refresh_skills` reads the first existing hooks candidate, calls `parse_hooks_tomlish`, writes `ws.hooks`. Projector adds `hooks.header` plus one row per item when the vec is non-empty.

### F4. BrandIcon slugs are painted as text, not images

- **Severity:** major
- **Category:** gap
- **Path:** `crates/multiplexer-shell/src/icons.rs`, `crates/multiplexer-shell/src/inspector_model.rs`, `apps/multiplexer-desktop/src/main.rs`
- **Quote:** `.child(div().text_color(Theme::accent()).child(icon))` in `inspector_row_el`
- **Plan:** plan/29 `BrandBadge` is `img()` of a vendored PNG (`assets/brands/{slug}.png`). plan/33 MCP rows resolve `BrandIcon::from_name`. Desktop `icons.rs` + `apps/multiplexer-desktop/assets/brands/` do not exist.
- **Evidence:** `mcp_rows` and `integration_tiles` store `BrandIcon::slug()` (`"github-light"`) or a `ChromeGlyph` mark in a `String`. The only `asset_path` use is the unit test in `icons.rs`. Desktop has no `src/icons.rs`, no `assets/brands/`, and no `img(` call. `inspector_row_el` and the left-rail row helper both put `row.icon` in a text child. A github MCP row therefore shows the literal characters `github-light`, not `github-light.png`. `integration_tiles` is exported and never rendered by the desktop.
- **Impact:** Brand resolution is tested and then thrown away at paint time. Known servers look like a slug leak, not a product mark.
- **Fix:** Vendor the pin-table PNGs. In the desktop painter, if `BrandIcon::from_name` or a slug table hits, `img()` the asset; else keep the chrome glyph. Do not treat the slug string as a glyph.

### F5. Supervisor is unused by the running app

- **Severity:** major
- **Category:** debt
- **Path:** `crates/multiplexer-mcp/src/supervisor.rs`, `apps/multiplexer-desktop/src/main.rs`, `crates/multiplexer-server/`
- **Quote:** `//! No process spawn lives here. "Spawn" is instant and fake`
- **Plan:** plan/21: one in-process supervisor owns spawn, reuse, reap, backoff. plan/33: host may project `Supervisor::state`; shell must not import `Supervisor`. plan/26 Customize panel is a read-only view of that projection.
- **Evidence:** `Supervisor`, `LifecycleState`, `acquire` / `release` / `mark_crashed` live in `multiplexer-mcp` and are covered by crate tests only. `apps/multiplexer-desktop` depends on `multiplexer-mcp` but constructs no `Supervisor`. `refresh_mcp` is `load_user_mcp_inventory()` into `McpRow`. `crates/multiplexer-server` and `crates/multiplexer-wire` have no Supervisor, no mcp methods, and no process table. Acquire is instant Ready (Spawned is not observable). There is no Job Object, no real child, no idle teardown.
- **Impact:** The state machine cannot go stale in production because nothing calls it. Inventory refresh cannot become Ready/Stopped. F1 and F2 cannot close until a host owns one supervisor instance (desktop in-process, or a later `mcp.status` RPC).
- **Fix:** Hold a `Supervisor` in the desktop (or server) host. On refresh, map each inventory name to `state()` → `McpLiveLabel`. Wire Play/Stop to `acquire` / `release` once real spawn exists. Keep shell free of the `multiplexer-mcp` crate.

## Related (not counted)

- Skills rows do not split a trailing ` [user]` / ` [project]` into subtitle/badge (plan/33 §5.2). The title is the raw formatted string.
- Skills tab has zero toolbar buttons. There is no `ClientAction::RefreshSkills`.
- `ListRowSpec` has no `kind`, `indent`, `copy_text`, or `actions`. plan/33 invariants (1 to 3 actions, Hook kind, `configured` badge table) are untested because the types are absent.
- plan/26 editors (add/edit/remove, marketplace, Test connection, hooks-trust) are out of this slice by plan/33. They are still absent, as specified.

## FINDINGS: 5
