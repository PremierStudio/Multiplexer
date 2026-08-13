# 39: Full-featured UI program (wave skeleton)

Superseded for depth by **`plan/40-full-featured-ui-complete.md`**. This file stays as the original Wave 0-5 skeleton. When the two disagree, **plan/40 wins**.

Authoritative synthesis of twelve exclusive surface reviews (title, left rail, center, inspector, terminal/layout, overlays, visual system, competitors, engine honesty, click catalog, editor/diffs, fleet/MCP). Live tree as of `4da32e7`. Parent Grok owns this doc. Children do not run cargo.

This file does not replace `docs/PLAN-CONTEXT.md` or `docs/DECISIONS.md`. Where those still say "replace the Grok pager with GPUI" or "editor is the center pane," **plan/40 plus the user's 2026-08-12 direction win** until DECISIONS is amended.

---

## 1. Product locks (do not reopen)

1. **Multiplexer is the Outlook workbench.** Left list, center reading/host, right inspector, bottom drawer. Pop-out is part of the metaphor.
2. **Do not rebuild the Grok pager in GPUI.** Chat log is `grok -p` only. Grok TUI is a host for the real interactive `grok` binary (console / Windows Terminal now, ConPTY later).
3. **Grok edits. Multiplexer reviews.** Diffs, files, mention, later hunks. Native editor is a pane/pop-out, never the center.
4. **UI on a real engine, or a labeled stub.** No toast that says a missing method is ready. Silent no-ops are bugs.
5. **Hide must become real hide.** Collapse-to-icon-rail stays as a third state. Hidden occupied size is 0.
6. **Native Windows caption stays.** `appears_transparent: false`. Custom glass caption is later and only with `WindowControlArea`.

---

## 2. Honesty table (brochure vs window)

| Surface | Engine today | UI may say | Forbidden |
|---|---|---|---|
| Chat | `grok -p` off UI thread | Headless log | Streaming tokens, tool cards, in-process harness |
| Grok TUI | `wt.exe` / new console, no `-p` | Host for the real pager | In-pane pager clone |
| Terminal | `cmd.exe /C` one-shot | Command log | Ghostty, vim, PTY ready |
| Diffs | porcelain parse + sort | Working-tree index | Apply, comments, fake hunks |
| Browser | `start "" url` | System browser, CDP later | Viewport, HAR waterfall, Design Mode |
| MCP | inventory + `McpLife` flag | Supervised table, no child | PID, crash from a child, marketplace |
| Points | RAM `CheckpointStore` | Pointer only, files unchanged | Restored workspace |
| Cores | `sample_cores` | Sampled. Reservation is a flag | Job Object armed |
| Remotes | local + `where tailscale` | Detect only, no Serve | Connect, tickets, 1-100 machines |
| Agents | local threads | Local threads only | Fan-out graph, spawn |
| Editor | none | (absent or Open external) | Fake buffer, Vim, LSP |
| Pop-out | `LayoutForest` unused by desktop | (absent until second window) | Help that says every pane pops out |

Lying strings to delete in the first honesty slice: Cores "Job Object kill-on-close is armed," checkpoint "restored," Session "connected" without hello/ping, MCP Ready as a live process.

---

## 3. Live vs dead (compressed)

**Works:** Outlook three-pane + bottom; hide X and 44px icon strip; left/right drag; bottom 36px collapse + 8px handle (math still wrong); Chat log send; Grok TUI launch/stop; porcelain diffs; system browser; MCP start/stop table; worktree create RPC; palette substring + file/thread hits; F2 settings live theme; Ctrl+K / F1 / Ctrl+` / Ctrl+Shift+G.

**Feels dead:** title pills (project/branch/turns/remotes) have no click; left Agents is `term_meta` only; expand-empty inspector rows (cores, term, skills, activity); MCP Start/Stop/Mention silent if nothing selected; empty Send silent; title bar `overflow_hidden` can clip Reset/Inspector; palette hints `g c` etc. do not bind; A/D unbound; settings ignore Esc; `shortcut_map` unused at runtime.

**Missing vs full featured (no new engine):** fuzzy palette, search overlay, Git create form on Git tab, diff click to `git diff` text, Agents rows with model/status/elapsed, inspector pop-out, settings as a page + persist, toast stack, kill pending shell, files reload/reveal/open-external, hide-to-zero, drag math, click always notices.

**Missing engines (later):** ConPTY + cell host, Ghostty, CDP/HAR, hidden-git restore, in-process streaming grok, live MCP children, Tailscale Serve, native editor crate, subagent scheduler.

---

## 4. Wave program (execute in order)

Each wave is TDD: red tests named in the child reviews, then impl, parent cargo. No cargo in children.

### Wave 0. Honesty + click contract (one week of parent slices)

- Delete lying copy (Job Object, restore, connected).
- Every visible click: mutate chrome **or** `Notice`. Empty send, last-thread delete, MCP without row, mention without file.
- Title pills: project copies path or opens Files; branch opens Git; turns opens Session; remotes opens Settings ("Serve later").
- Left Agents: select backing thread or focus composer. No `term_meta`-only stubs.
- `row_detail` for every expandable row, or drop `cursor_pointer`.
- Help Close button. Approval A/D only when pending.
- Catalog = painted set (`project_pill`, left sections, `tab_files` / `activity` / `agents`, hide_left/right).
- Amend PLAN-CONTEXT / plan/03: host the pager, do not replace it.

### Wave 1. Chrome that hides and resizes

- Bottom: Hidden occupied `0`, peek `36`, open default `240`, min open `80`, max `480`. Restore last open height.
- Drag: `win_h - mouse_y - status_h - handle_h`. Double-click handle resets to 240. Keyboard `Ctrl+Alt+Up/Down` fallback.
- Rails: `Open | IconRail | Hidden`. Title hide cycles Open to IconRail. In-pane X is Hidden (occupied 0).
- Title: 20px pills, 32px icons, `≡` hide left, `⋮` hide right, `▦` reset only. Disabled Run when draft empty. Danger Stop. No wrap. Overflow drops remotes then turns then branch.

### Wave 2. Command system

- One overlay stack: Palette / Help / Settings / Search. Esc pops top. Settings Esc works.
- Fuzzy `PaletteHit` + namespaces Commands / Panes / Files / Threads. Empty query never dumps the file tree.
- `Ctrl+Shift+F` search overlay (names only).
- `Ctrl+1..4` left sections. `Ctrl+,` settings.
- `handle_key` reads `shortcut_map` / BindingTable. Drop lying `g c` hints until prefix mode exists.
- Toast stack top-right, cap 8, paint 3, Esc dismisses newest when no modal.
- Settings persist theme/density/model/bindings to `%APPDATA%\Multiplexer\settings.json`. No secrets.

### Wave 3. Workbench that reviews

- Git tab: path / branch / create-branch form (fields already on Workspace). Reminder uses `reminder_from_list`, not `nth(1)` + `"existing"`.
- Diffs: click selects (not last-turn wash). Mention `@path`. Open in `$VISUAL` / `$EDITOR` / `start`. After TUI exit, reload porcelain. Click row shows `git diff` text for that path (host), not apply.
- Files: Reload, Reveal (copy abs path), Mention, Open external. Select paints. Stop rotating the vec on CycleFile.
- Session: hello/ping; Models / Palette / Help rows; usage "local snapshot only."
- Points: `checkpoint.list` after create; banner "pointer only, files unchanged"; no fabricated `local-N`.
- Cores: kill Job sentence; long-lived `System`; reserved flags survive reload; RAM line.
- MCP: merge inventory by name (Reload must not reset Ready); disable Start when Ready; notice if nothing selected.
- Agents: banner local-only; model + status + msg count; no fake children.
- Activity: reminder, approval, busy, notices, then log. Stable ids.
- Skills: Refresh + hooks section if `hooks.toml` exists. Enable is a local flag. Not run.

### Wave 4. Outlook metaphor + density

- Extract `apps/multiplexer-desktop/src/widgets.rs`. Ban raw `hsla(` in `main.rs`. Light/dark both paint (no leftover dark fills).
- Vertical inspector destinations stay. Toolbars icon-only (Copy is a glyph). Empty center is 2x2 tiles + New chat, not a slogan plus chips forever.
- File titles are leaf names. Clip + nowrap on every row.
- Pop-out inspector: `LayoutForest::detach` + second `open_window` + Dock. Differentiator 5 becomes true.
- Compact density default. Title 48 / rail 48 when the kit lands.

### Wave 5. Later engines (own milestones, do not fake)

Order if the brochure must become true:

1. Killable `ProcessCapture` on the strip (still not PTY).
2. Hidden-git refs, then Revert that touches files, then `checkpoint.diff`.
3. `git.diff` structured hunks on Diffs expand / pop-out.
4. ConPTY + cell grid hosting interactive `grok` **inside** center.
5. `VendoredGrokFactory` / ACP streaming for Chat log (product change).
6. CDP detect/launch, then HAR import, then live waterfall, then `InspectorTab::Har`.
7. Tailscale Serve + tickets + attach.
8. `crates/multiplexer-editor` as pane/pop-out (D4 feature list, not center).
9. Fork `spawn_subagent`, fleet 1-100, live MCP children.

---

## 5. Competitor bar (must-have to not feel basic)

P0 without new engines: fuzzy palette, search overlay, Git create form, diff text preview, Agents rows that look like a list, inspector pop-out, discoverable settings.

P1: toast stack, kill pending cmd, MCP honesty + disable, Grok TUI copy never says embedded, browser stays a launcher.

Do not ship a fake editor, fake HAR, or fake agent tree to close Orca/Cursor gaps.

---

## 6. How parent executes

- One wave at a time. Children write tests and code. Parent runs fmt, clippy, test, mutants.
- First implementation slice after this doc: **Wave 0 + Wave 1** (honesty, dead clicks, hide-to-zero, drag math). That is what the last screenshot and "90% of buttons" complaint are.
- Do not start Wave 5 engines in the same PR as chrome.
- Update stale audits (`01-toolbar`, `02-left-rail`, `03-inspector`, `06-terminal`, `11-controls`) when the matching wave lands, or stamp them obsolete.

---

## 7. Open questions for Justin

1. Hide-left: IconRail (44px) vs Hidden (0). Recommend: title bar cycles to IconRail; pane X is Hidden.
2. Chat log forever `-p`, or Engine C (in-process/ACP) later for tool cards?
3. External editor now (`$VISUAL` / `start`) vs wait for native pane?
4. Persist settings this month (theme/bindings only)?
5. Focus layout (both rails hidden) as Shift+click Reset, or a second palette row?
