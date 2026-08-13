# 19. Visual density, type, spacing, list item beauty

**Scope:** Live desktop chrome vs `plan/31-component-kit.md` and `plan/35-competitor-visual-bar.md` (type, height, spacing, row beauty). Read-only. No cargo.
**Surfaces read:** `apps/multiplexer-desktop/src/main.rs` helpers (`ghost_btn`, `list_row`, `icon_btn`, `glass_pane`, `chip`, `pill`, `empty_center`, `status_bar`), `apps/multiplexer-desktop/src/theme.rs`, `crates/multiplexer-theme/src/tokens.rs`, `crates/multiplexer-shell/src/widgets.rs`.
**Date:** 2026-08-12

## Verdict

**FAIL.** Glass and Outlook structure are in the right direction. Density, type, and list beauty are not. The window still reads as unlabeled `div()` trees on GPUI `text_sm` (14px). `TypeScale::UI` (13px) exists in tokens and is never painted. Heights are a third system (26 / 28 / 32 / 36 / 44 / 88) that matches neither plan/31 (32 / 36 / 44) nor plan/35 (20 / 28 / 32 / 48 / 56). Empty-state tiles are specified and computed, then ignored. Nine inspector tabs wrap as block chips. The status bar is one muted inventory sentence.

## Finding format

Each finding is a block:

| Field | Meaning |
|---|---|
| **ID** | `Fnn` stable key |
| **Severity** | P0 ship-blocker for the visual bar, P1 density/type miss, P2 polish |
| **Plan** | Exact plan/31 or plan/35 section |
| **Where** | Absolute-path file:line of live evidence |
| **Expected** | Spec number or copy |
| **Live** | What the tree actually paints |
| **Why it looks cheap** | First-two-seconds read |

## FINDINGS: 12

### F01. Chrome is still unlabeled `div` trees (kit not adopted)

- **Severity:** P0
- **Plan:** plan/31 §1, §7, §7.2 (delete `ghost_btn` / `chip` / `empty_center`; one GPUI function per spec in `apps/multiplexer-desktop/src/widgets.rs`)
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:1688-1956`
- **Expected:** `button`, `list_row`, `pill`, `badge`, `tab`, `drawer_header`, `empty_state`, `search_field` project headless specs. Callers pass a spec, not a raw label string.
- **Live:** There is no desktop `widgets.rs`. Paint helpers are still `glass_pane`, `glass_bar`, `empty_center`, `chip`, `icon_btn`, `pill`, `list_row`, `inspector_row_el`, `ghost_btn`. `ghost_btn` still special-cases `"Stop"` and `"Send"` by label (`main.rs:1940-1946`). Inspector tabs, section headers, palette rows, help card, composer well, and status strip are anonymous `div()` stacks. `TabSpec`, `EmptyStateSpec`, `ButtonSpec`, and `empty_state_tiles()` are unused by the painter.
- **Why it looks cheap:** Every control is a bordered box with a string inside. Hover, selected, focus, busy, and height are reinvented per call site, so the window does not look like a kit.

### F02. Missing 13px UI type (whole window is `text_sm`)

- **Severity:** P0
- **Plan:** plan/35 item 9 and §5 `type.ui` 13px / 18lh; captions 11px; body 14px / 20px; mono 12px; **no 16px in chrome**. plan/27 already names `text_sm` as a 14px stand-in.
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:845`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-theme\src\tokens.rs:80-87`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\theme.rs:6-87`
- **Expected:** Chrome (title, rails, tabs, palette rows, buttons) at `TypeScale::UI` (13) / 400 / line-height 18. Section caps and status at `TypeScale::CAPTION` (11). Conversation body at 14 / 20.
- **Live:** Root render sets `.text_sm()` once and never calls `text_size`, `letter_spacing`, `font_weight`, or `line_height`. Theme adapter exposes colors and `panel_radius` only. `TypeScale::UI = 13.0` is tested in `multiplexer-theme` and never mapped into GPUI. Title, rail labels, tabs, buttons, list titles, composer, palette rows, and help title all inherit 14px. Section headers (`THREADS` etc.) are the same size, only fainter (`main.rs:1044-1046`). Conversation bubbles inherit 14px with no 20px line-height.
- **Why it looks cheap:** Linear/Cursor read as 13px compact chrome. This reads as default web type on glass.

### F03. Height tokens are three systems (32 vs 36 vs everything else)

- **Severity:** P0
- **Plan:** plan/31 §3: `HEIGHT_COMPACT` 32, `HEIGHT_ROW` 36, `HEIGHT_COMFORT` 44. Ghost/Icon 32, Primary/Danger 44, list row / drawer header / search 36. plan/35 items 5 to 11, 16, 17: icon button 32, pill 20, title 48, status 28, icon rail 48, list row 56, composer min 72.
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-shell\src\widgets.rs:57-62,143-149`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:906,1001,1011,1197,1537-1541,1757-1758,1778,1932`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-shell\src\workspace.rs:158`
- **Expected (plan/31):** Three integers only. Specs export `HEIGHT_*`. Send is Primary 44. Search is 36. Rows are 36 even with a subtitle.
- **Expected (plan/35):** Compact chrome at 32, pills at 20, rows at 56, title 48, status 28, rail 48.
- **Live (a third table):**

| Surface | Live px | plan/31 | plan/35 |
|---|---|---|---|
| `ghost_btn` / `icon_btn` | 32 | Ghost/Icon 32; Send should be 44 | 32 (Send 32) |
| `ButtonSpec::height` | Icon 32, **else 36** | Ghost 32, Primary/Danger 44 | 32 |
| `ListRowSpec::height` | 44 / **88** expanded | 36 always | 56 thread row |
| `list_row` / `inspector_row_el` | no `h()`, `py_2` grows | 36 clipped | 56, two lines |
| Title bar | **44** | chrome 48 | 48 |
| Status bar | **26** | (not in kit) | 28 |
| `pill()` | **28** | badge/pill spec 32 | **20** |
| `chip()` | `py_1`, no height | PillSpec 32 | 20 |
| Icon rail column | **44** (`RAIL_COLLAPSED`) | 36 (plan/31 text) | **48** |
| Left rail icon well | **36** | 32 icon | 32×32 rounded-8 |
| Right collapsed tab | 32 | TabSpec 32 | 20 to 24 pill |
| Composer well | `py_2`, no min 72 | Search cousin 36 | min 72 / max 160 |
| Palette query | `py_2`, no 32 | SearchField 36 | 32 |

`widgets.rs` does not define `HEIGHT_COMPACT` / `HEIGHT_ROW` / `HEIGHT_COMFORT` at all.
- **Why it looks cheap:** Adjacent controls do not share a baseline. Title (44) vs icon (32) vs rail well (36) vs pill (28) vs status (26) is the "unlabeled divs" tell.

### F04. List items are not beautiful (no 13/11 pair, no locked height, no hover)

- **Severity:** P0
- **Plan:** plan/35 item 7 (56px, pad 12×8, radius 12, line 1 = 13px title ellipsis, line 2 = 11px muted preview + `status · id`, selected wash + `hairline_bright`, idle `hsla(0,0,1,0.03)`). plan/31 §4.2 (always 36, subtitle clipped inside the row). plan/32 §4.1 (glyph / title / badge / pulse; hover reveal; `rows.rs`).
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:1793-1922`
- **Expected:** One two-line card. Title 13px one line. Preview 11px muted. Status as a glyph or trailing meta, not a third block. Hover lift when not selected. Pulse disc. No raw `thr-N` id on the card (plan/32 §5.1).
- **Live:** `list_row` is three stacked, unclipped children: icon+title+busy ellipsis, then subtitle, then meta. No `h(px(56))` or `h(px(36))`. No `text_size`. No hover. Idle fill is `Theme::glass_ultra()` (alpha 0.20), not 3% white. Radius is `rounded_xl` (22) not 12. Thread meta is `format!("{} · {}", t.status, t.id)` so the id is printed. Busy is the character `…`, not a pulse. `inspector_row_el` is a near-copy that hides meta until expanded and paints a raw badge `div` (`px_1`, `rounded_md`) instead of a 20px or 32px pill. There is no `rows.rs`.
- **Why it looks cheap:** Rows look like three leftover labels, not Linear/Orca fleet cards.

### F05. No dashboard tiles on the empty center

- **Severity:** P1
- **Plan:** plan/31 §4.6 `EmptyStateSpec` (title / body / Primary 44 action). plan/34 §4.4: 2×2 tiles, min 148×76, not chips. plan/35 §2.1 item 5 (agent dashboard cards). `empty_state_tiles()` and `integration_tiles()` already exist in shell.
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:1288-1327,1706-1726,1338-1364`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-shell\src\widgets.rs:162-186`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-shell\src\integrations.rs:18-72`
- **Expected:** Empty thread paints `EmptyStateSpec` plus four tiles (`What can you do?`, `Summarize this repo`, `git status`, `List project files` / `Run the tests`). Integration catalog tiles (model / MCP / skill / worktree) have a projector.
- **Live:** `empty_center()` is a sparkle glyph, the sentence `A control surface for your agents`, and a faint shortcut string. No title/body hierarchy. No Primary action. `empty_state_tiles()` is never called from desktop. `integration_tiles` is never imported. Suggestions sit on the **composer** as five `chip()` helpers, including an extra `Copy last` that is not in the tile catalog. Tiles hide-when-transcript-starts is inverted: chips stay after messages exist.
- **Why it looks cheap:** Empty state is a muted slogan. Competitors open on a dashboard of starting points.

### F06. Nine inspector tabs wrap as block chips

- **Severity:** P1
- **Plan:** plan/31 §4.4 listed seven tabs (`Session`, `Cores`, `MCP`, `Points`, `Git`, `Term`, `Skills`) as `TabSpec` height 32. plan/35 item 18: wrapping **pill** row, 20 to 24px, 4px gap, 8px inset, active accent wash. Not a 32px button row.
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-shell\src\workspace.rs:32-85`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:1220-1241`
- **Expected:** Seven (or a curated subset) as 20px pills, or an overflow. `TabSpec` drives selected fill and optional count badge.
- **Live:** `InspectorTab::all()` is **9** (`Session`, `Cores`, `MCP`, `Points`, `Git`, `Term`, `Skills`, `Files`, `Activity`). Open right rail paints every label in `.flex().flex_wrap().gap_1()` as `px_2().py_1().rounded_lg()` blocks: `"◎ Session"` style `format!("{} {}", t.glyph(), t.label())`. No `h(px(20))`, no radius 999, no `TabSpec`, no count. On a 220 to 300px rail this wraps to two or three ragged rows of unlabeled chips. Collapsed right rail then switches language to 32px icon wells (`main.rs:1197`).
- **Why it looks cheap:** The inspector header is a wrapping word cloud, not a pill strip.

### F07. Status bar is a 26px inventory sentence

- **Severity:** P1
- **Plan:** plan/35 item 17 and §2.1 item 3 / §2.2 item 5. 28px, 11px type, 8px pad, 8px gap. Left connection pill, center focus hint (`Enter send · Ctrl+K palette`), right run-state pill (`Theme::good` / `accent` / `danger`). Single line, no wrap.
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:1537-1554`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-shell\src\status.rs:41-54`
- **Expected:** Three clusters of 20px pills. Connection, hint, idle/running/waiting/error.
- **Live:** `glass_bar().h(px(26.0)).px_3()` with one muted `flex_1` child. Copy is `status_line`: `idle · 1 chats · 0 mcp · 0 cores · 0 cp · 0 wt`. Flash appends `   ·   {extra}` into the same string. No pills. No 11px. No connection token. No good/accent/danger run-state. Session id is stored on `ClientStatus` and not shown (correct), but neither is model, path, or SSH/local.
- **Why it looks cheap:** It is a debug footer, not Orca/Cursor status chrome.

### F08. Pills and chips miss the 20px grammar

- **Severity:** P1
- **Plan:** plan/35 item 10 (20px, pad 8×0, radius 999, 11px). plan/31 §4.3 `PillSpec` / `BadgeSpec` height 32, distinct structs, `Tone` fill. plan/35 item 15: title bar is pills, not a sentence (directionally started).
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:1728-1790,1895-1907`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-shell\src\widgets.rs:65-88`
- **Expected:** One pill primitive. Model / branch / connection / run-state / inspector tabs / composer chips share height and radius. Badges inside rows are 20 to 32 tall, vertically centered.
- **Live:** `pill()` is 28px, `rounded_lg`, muted text, no tone. `chip()` has no height and a 6% white fill. Inspector badges are `px_1().rounded_md()` on the tone color. `PillSpec` does not exist. `BadgeSpec` has `tone` + `text` and no `height()` / `caption()`. Title-bar project/branch/model use `pill()` (better than the old concatenated sentence) but at the wrong size and with square-ish corners.
- **Why it looks cheap:** Three chip languages in one window.

### F09. Icon rail and section headers miss compact metrics

- **Severity:** P1
- **Plan:** plan/35 items 5 and 8. Rail 48px, six destinations max, selected 32×32 rounded-8 well. Headers 11px / 500 / letter-spacing +0.6 / `Theme::muted()` / pad 12×8. plan/31 §4.5 `DrawerHeaderSpec` height 36, action Ghost 32.
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:1000-1060`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-shell\src\workspace.rs:88-127,158`
- **Expected:** 48px icon column. Destinations: Chats, Agents, Files, Git, Search, Settings. `CHATS` strip via `DrawerHeaderSpec`.
- **Live:** `RAIL_COLLAPSED = 44`. Left icon column is `w(px(44.0))` with `h(px(36.0))` wells, `rounded_lg`, four destinations only (`Threads`, `Agents`, `Files`, `Activity`). Header is `section.label().to_ascii_uppercase()` in `Theme::faint()` at inherited 14px, no tracking, no 500 weight. New/Del are 32px `icon_btn`s, not a drawer-header Ghost. No `DrawerHeaderSpec`.
- **Why it looks cheap:** The rail is a stub of text-sized hits, not a VS Code / ChatGPT icon rail. `THREADS` looks like a leftover label.

### F10. Composer and Send still ignore both height contracts

- **Severity:** P1
- **Plan:** plan/31 §4.1: Send is `ButtonSpec::primary("Send", "Enter")` height 44. Do not special-case the label. plan/35 item 16: well min 72 / max 160, Send 32, chips 20px on an 8px gap row. Approval Deny is Danger 44 (plan/31).
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\main.rs:1330-1404,1460-1465,1924-1956`
- **Expected:** One well with a typed Send. Approval Allow/Deny are Primary/Danger 44, not Ghost 32.
- **Live:** Composer is `px_3().py_2().rounded_xl()` with no min-height. Send is `ghost_btn("Send", "↵")` at 32, which trips the `"Send"` fill branch (`Theme::send_bg()`). Placeholder still packs keyboard hints into the well (`Message Grok…  Enter send …`). Approval Allow/Deny are the same `ghost_btn` at 32 with generic idle fill. Terminal Run/Clear are also `ghost_btn` 32 with a hint painted **inside** the hit target (plan/35 item 6: strip dual-label on icon-only).
- **Why it looks cheap:** Send looks like another toolbar ghost, not a CTA. The well is a one-line field, not a 72px dock.

### F11. Theme adapter has no type or size tokens (density default is Comfortable)

- **Severity:** P2
- **Plan:** plan/35 §5 token lock (`type.ui`, `type.caption`, `type.body`, `type.mono`, `icon.rail` 48, `row.thread` 56, `pill.h` 20, `status.h` 28). plan/35 item 11: compact default. plan/31 §7.1: `Theme::surface_idle_control()`, `surface_idle_row()`, `Theme::warn()`.
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\apps\multiplexer-desktop\src\theme.rs`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\crates\multiplexer-theme\src\tokens.rs:35-38,104-132,166-176,218-225`
- **Expected:** Desktop Theme methods (or token fields) for 13 / 11 / 14 / 12 type and the size constants above. Default density Compact so space steps are 2 / 6 / 8 / 12.
- **Live:** `Theme` maps glass, ink, hairline, text, muted, accent, good, warn, danger, selection, `panel_radius` (12), shadow. No type, no row height, no pill height, no rail width, no status height. `ThemeTokens::default()` is **Dark + Comfortable** (space 4 / 8 / 12 / 16). Compact exists and is unused. Warn exists and is only used on inspector badges, not status pills.
- **Why it looks cheap:** Painters invent px literals, so 32 vs 36 cannot converge.

### F12. plan/31 and plan/35 disagree on the numbers (parent will pick wrong)

- **Severity:** P2 (spec lock, not a paint bug)
- **Plan:** plan/31 §3 and §4 vs plan/35 §4 items 6, 7, 10, 16
- **Where:** `C:\Users\gollum\Development\PremierStudio\Multiplexer\plan\31-component-kit.md:47-51,194-201,285-287,389-387`; `C:\Users\gollum\Development\PremierStudio\Multiplexer\plan\35-competitor-visual-bar.md:147-157,165`
- **Expected:** One locked table before the parent rewrite.
- **Conflict:**

| Token | plan/31 | plan/35 |
|---|---|---|
| Ghost / icon button | 32 | 32 (agree) |
| Primary / Send | **44** | **32** |
| Pill / badge / tab | **32** | **20** (tab selected 24) |
| List row | **36**, subtitle clipped | **56**, two lines |
| Search / drawer header | 36 | (not numbered; query well 32) |
| Title bar | 48 (chrome, not kit) | 48 |
| Icon rail collapsed | 36 (plan/31 prose) | **48** (live is 44) |

Live `ButtonSpec::height` (36 for every non-icon) and `ListRowSpec::height` (44 / 88) match **neither**. Until this is locked, implementing plan/31 kit heights will miss the competitor bar, and implementing plan/35 56px rows will fail plan/31 tests that pin `HEIGHT_ROW == 36`.
- **Why it looks cheap:** The tree already hedged by inventing 44 / 28 / 26. That hedge is visible.

## plan/35 §4 checklist (density / type / list only)

| # | Item | Status |
|---|---|---|
| 1 | Blurred window + ink + `p_2`/`gap_1` | Pass |
| 2 | `glass_pane` 12px / hairline / shadow | Pass (keep) |
| 3 | Bars use `glass_strong` | Partial (title/status/overlays yes; composer well is `Theme::surface()`) |
| 5 | 48px icon rail, 32×32 wells, six destinations | Fail (44 / 36 / four) |
| 6 | 32px icon buttons, 16px glyph | Partial (32px hit; glyph is a character, not 16px) |
| 7 | 56px list rows, 13/11 type | Fail (F04) |
| 8 | 11px letter-spaced section headers | Fail (F09) |
| 9 | 13px UI type | Fail (F02) |
| 10 | 20px pills | Fail (F08) |
| 11 | Compact default (title 48, status 28, row 56, composer 72) | Fail (title 44, status 26, Comfortable density) |
| 12 | Hairlines on rows, shadows only on panes | Partial (rows have hairline **and** no row shadow, good; idle fill is glass_ultra not 3% white) |
| 16 | Composer min 72, Send 32, 20px chips | Fail (F10) |
| 17 | Status 28px, three pill clusters | Fail (F07) |
| 18 | Inspector tabs 20 to 24px pills | Fail (F06: nine wrapping blocks) |

## What is already correct (do not restyle)

- `WindowBackgroundAppearance::Blurred`, root `Theme::ink()`, workspace `p_2` / `gap_1`.
- `glass_pane` / `glass_bar` as chrome (plan/31 §7.2: keep these).
- Title bar already uses project / branch / model clusters instead of one muted sentence.
- Palette 520 / top 80 / dimmer `hsla(0.64, 0.20, 0.04, 0.45)` / 12 rows.
- Help card 560 / `p_4`.
- `icon_btn` 32×32 is the right hit size; promote it, do not grow it.
- Cool-ink / accent-58 color family. This audit is not a color restyle.

## Suggested parent lock (so F03 / F12 stop drifting)

Use plan/35 as the **paint** numbers and keep plan/31 as the **state machine** (Tone, VisualState, captions):

- Compact 32: icon, ghost, Send, search well.
- Pill 20 (selected tab 24).
- Row 56 (two lines, 13 / 11). Collapsed inspector rows may stay 36 only if they are single-line.
- Comfort 44: empty-state Primary, approval Allow/Deny only.
- Title 48, status 28, rail 48.
- `TypeScale::UI` 13 on all chrome. Caption 11 on headers, pills, status.

Then delete `ghost_btn`, `chip`, `empty_center`, and the duplicate `inspector_row_el`. Paint tiles from `empty_state_tiles()`. Stop wrapping nine tabs: six visible pills plus overflow, or a 22px strip with a chevron.

## FINDINGS

**12**
