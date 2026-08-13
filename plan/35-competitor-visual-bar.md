# 35: Competitor Visual Bar (chrome parity)

> **Status:** Authoritative plan doc for *visual and chrome* parity. Consistent with `docs/PLAN-CONTEXT.md`. If any statement here conflicts with PLAN-CONTEXT, PLAN-CONTEXT wins and the conflict should be flagged in `plan/20-risks-and-open-questions.md`.
>
> **Purpose:** Define the visual bar we must beat. Not feature-complete versus every competitor this sprint. Visual and chrome parity: glass, icon rail, list rows, pills, overlays, density, type. This is the input the parent can implement now in `multiplexer-ui` / `apps/multiplexer-desktop`.
>
> **Locked decisions applied:** **D1** (Rust + GPUI), **D6** (Multiplexer.dev brand), **D7** (Orca *feature* baseline is match-all across Phases 1-5; this doc is the *visual* floor, not that feature list), **D8** (MVP = Phases 1-4), **D9/D35** (Windows-primary, conditional on the Phase-0 spike). See `docs/DECISIONS.md`.

**Evidence rule:** No invented screenshots. Traits below are taken from public product copy, settings docs, help articles, and well-known chrome conventions those apps document or ship. Where a pixel value is a Multiplexer *target* (not a competitor measurement we verified), it is marked **(spec)**.

**Scope cut:** We do not ship Cursor's full IDE, Linear's issue tracker, Warp's block terminal, or ChatGPT's consumer chat this sprint. We *do* make Multiplexer look like it belongs in that set when a Windows developer opens the window.

---

## 1. What "visual bar" means

`plan/01` is the *capability* bar (Orca worktrees, Design Mode, HAR, and so on). This doc is the *look* bar.

A Windows developer who also uses Cursor, Claude Desktop, Warp, Linear, or ChatGPT desktop will judge Multiplexer in the first two seconds: title bar, glass, icon rail, row density, type size, pills, overlays. If those feel cheap (Electron-flat, 16px chrome type, 44px touch rows, no blur, no icon rail), the product loses before a single agent turn.

The floor:

| Layer | We must look like |
|---|---|
| Window | Native GPU window with platform blur, not a painted rectangle |
| Chrome | Icon rail + Outlook list + compact title/status, not a web header |
| Density | Linear/Cursor compact (13px UI, 4px grid), not consumer-chat airy |
| Controls | 32px icon buttons, 20px pills, 12px radii, hairline borders |
| Overlays | Centered glass palette/help, dimmed canvas, Esc to dismiss |
| Type | 13px UI, 14px conversation, 12px mono labels. No 16px chrome. |

The existing shell already has the right *direction* (`WindowBackgroundAppearance::Blurred`, `Theme::glass()`, 48px title bar, 32px `ghost_btn`, 12px panel radius, 520px palette). This doc locks the remaining numbers and the Windows acrylic vs mac vibrancy split.

---

## 2. Per-competitor visual / UX traits we must match

Five traits each. These are chrome and interaction, not feature checklists.

### 2.1 Orca (onorca.dev): ADE chrome we sit next to

Orca is the strongest *product* competitor (`plan/01` §3). Its marketing page and settings docs describe a dense, split-first workspace. We match the *feel* of that chrome even when a given Orca feature is still later-phase.

1. **Split-anything workspace, tabs as first-class chrome.** Public copy: "Arrange agents, terminals, browsers, diffs, and files into split panes." The homepage mock lists tab titles such as `Terminal 2` and `localhost:3000` over the working surface. **Match:** every surface lives in a pane frame with a thin tab strip (22px **(spec)**), not a floating web card.
2. **Left list is a fleet, not a chat history.** Public copy groups projects, worktrees, branch names, agent counts (`5 agents`), and running rows with elapsed time (`49m`, `3h`). **Match:** thread/agent rows show title + one muted preview line + a status glyph/time. Not a single untitled "New chat".
3. **Status strip with usage and connection pills.** Public copy shows model/account (`Claude Code`, `Opus · Claude Max`), path, `SSH Connected`, and usage chips (`58% 5h`). Settings expose a Resource Manager status-bar toggle. **Match:** a 28px **(spec)** status bar with connection, model, and run-state pills.
4. **Appearance is a product setting, not a theme file.** Settings reference: Theme, accent color, density, UI font, UI zoom, editor font override, minimap, status-bar toggles. **Match:** density and type are tokens the user can feel immediately (compact default). Full settings UI can follow; the *default* must already be compact.
5. **Filter chips and state glyphs on agent surfaces.** Agent Dashboard docs: removable filter chips, "Clear all filters", card header with agent icon + conversation name + state glyph. **Match:** inspector/activity filters are pills, not dropdown walls; running/waiting/error is a 8px **(spec)** glyph, not a paragraph.

### 2.2 Cursor: IDE chrome we sit next to

Cursor is a VS Code fork. We are not forking VS Code. We still have to look like a serious editor shell, because that is the muscle memory of the Windows audience.

1. **Icon activity rail.** Cursor (and VS Code) put primary destinations on a slim icon rail. The rail can be vertical or horizontal; the *trait* is icon-first navigation with a selected-state pill behind the icon. **Match:** a 48px **(spec)** vertical icon rail on the far left (Windows-first). Collapsed list rails must become this, not a 36px text stub.
2. **Composer as a dedicated, rounded dock.** Agent input is a distinct surface (not a terminal prompt pretending to be chat). **Match:** composer is a 12px-radius **(spec)** glass well, 12px padding **(spec)**, Send is a 32px control, hint text muted 11px.
3. **Tab bar + breadcrumbs, not a single page.** Editor chrome is a 32px **(spec)** tab strip and a 22px **(spec)** breadcrumb. **Match:** center pane has a tab/breadcrumb row even when only one file/thread is open, so the shell never looks like a marketing landing page.
4. **Model / mode as a header pill.** The active model and agent mode sit in chrome, one click to change. **Match:** title bar carries `project · branch · model` as pills, not a concatenated muted string.
5. **Dense status bar.** Language, branch, errors, agent state live on a 22-28px bar. **Match:** 28px **(spec)** status bar, 11px type, no wrapping.

### 2.3 Claude Desktop: conversation chrome we sit next to

Help-center and product tours describe Chat / Cowork / Code as top-level modes, a conversation list, and an Artifacts split pane. The 2026 Code-on-desktop writeups add drag-any-panel, integrated terminal, and a rebuilt diff viewer.

1. **Mode tabs in the primary chrome.** Desktop tours: Chat, Cowork, and Code across the top. **Match:** the title bar (or a 36px **(spec)** mode row under it) has explicit destinations (Chats, Build, Inspector). Not buried in a hamburger.
2. **Conversation list with quiet grouping.** Left rail lists conversations; Artifacts have their own sidebar section. **Match:** section header (`CHATS`) is 11px, letter-spaced, muted. Rows are selectable glass, not flat grey.
3. **Artifact / preview split.** Artifacts open in a dedicated side pane; composer stays at the bottom so follow-ups stay in context (Claude Artifacts UX notes). **Match:** inspector/right rail is the artifact column; composer never leaves the center bottom.
4. **Large, obvious composer.** Consumer Claude uses a wide rounded input. We take the *clarity*, not the airy padding. **Match:** composer min-height 72px **(spec)**, max ~160px before internal scroll, 13px input type.
5. **Warm, low-noise surfaces.** Claude is not IDE-black. **Match:** dark default stays cool-ink (`Theme::ink` / `glass`), but hover/selected use a single accent wash (existing `hsla(0.58, …)` family). No rainbow chrome.

### 2.4 Warp: terminal chrome we sit next to

Warp docs cover a command palette, theme picker, and window opacity + blur. The product is an IDE-like terminal: tabs, blocks, a prominent input bar.

1. **Window opacity + blur as a first-class look.** Warp settings expose opacity and background blur. **Match:** the window *is* blurred (`WindowBackgroundAppearance::Blurred`). Glass panes are translucent tints over that blur, not opaque `#111` slabs.
2. **Command palette as a centered glass overlay.** Warp palette is the global search for actions, workflows, settings. **Match:** `Ctrl+K` / `Ctrl+P` opens a 520px **(spec)** overlay, 80px from the top, dimmed canvas, 12 listed rows.
3. **Input bar is a surface, not a caret on raw black.** Warp's prompt is a rounded dock with helper chips (AI, workflows). **Match:** terminal strip and chat composer share the same well: 12px radius, hairline, 8px inner pad.
4. **Block-shaped output.** Warp groups a command + its output into a rounded block. **Match:** agent tool-call cards (Read / Update / shell) are 8px-radius **(spec)** blocks with 12px padding, not raw log lines.
5. **Tab + split chrome on a terminal.** Warp is not a single pty. **Match:** the bottom terminal strip has a 28px **(spec)** handle row (Run, Clear, focus hint) and room for a tab label.

### 2.5 Linear: density and type we sit next to

Linear is not a coding-agent control surface. It is the density and type bar for "this is professional software." Public design notes (community `DESIGN.md`, Refero style extract, Linear's own Liquid Glass writeup) are explicit.

1. **13px UI type, compact tracking.** Documented scale includes Linear Text 13px / 500 for buttons and Linear Mono 13px. Body in-product is 13-14px, captions 12px. **Match:** chrome UI is **13px / 400-500**, line-height 18px **(spec)**. Captions 11px. Never 16px in rails or title bar.
2. **4px base, 8-12px padding, 6px and 12px radii.** Refero extract: compact density, 4px base, 6px and 12px radii, 8-12px paddings, 0.5px hairlines. **Match:** 4px grid, row pad 12x8 **(spec)**, `radius.sm` 6px, `radius.md` 12px (already `Theme::panel_radius`). Hairline is 1px at 10% white on GPU (0.5px disappears on Windows DPI).
3. **Filter chips and status pills as the vocabulary.** Issues, cycles, and agents are labeled with small pills, not badges the size of buttons. **Match:** pill height 20px **(spec)**, horizontal pad 8px, full-pill radius, 11px type.
4. **Command palette is how you navigate.** `Cmd/Ctrl+K` is Linear's spine. **Match:** same overlay grammar as Warp/Cursor: one box, fuzzy list, shortcut hint on the right in muted 11px.
5. **Glass with a ProKit discipline.** Linear's Liquid Glass essay (2025-10-21): take translucency and depth, refuse refraction that hurts dense UIs, own the material, raise contrast when the OS asks. **Match:** glass is blur + tint + hairline + one specular edge. No fake refraction. Increase Contrast / reduce-transparency falls back to opaque `glass_strong`.

### 2.6 ChatGPT desktop: consumer glass we sit next to

The official Windows app is a companion window: slim sidebar (icons + conversation list), centered composer, model picker. Community reports of "sidebar icons vanish" confirm the icon-rail pattern. OpenAI's desktop is the consumer glass bar; we take the *chrome*, not the airy marketing layout.

1. **Slim icon + conversation rail.** New chat, search, and recents live on the left; the rail can collapse to icons. **Match:** icon rail 48px **(spec)**; list rail 248px default (already `ChromeLayout::left_width`). Collapse is icons, not a vertical word.
2. **Header model / GPT picker as a pill.** The active model is a single control in the header. **Match:** model is a 20-24px pill in the 48px title bar, not a settings dive.
3. **Centered composer with attachment chips.** Composer is the focal control; extras are chips (attach, voice, tools). **Match:** composer well + 20px chips on a 8px gap row above or inside the well.
4. **Companion-window proportions.** ChatGPT desktop is used *beside* other apps. Default window in our shell is already 1360×860, min 920×620. **Match:** keep that. Do not launch maximized into a VS Code clone.
5. **Translucent caption, with a solid fallback.** Windows glass apps fail in public (Codex desktop: sidebar/title bar going transparent when maximized). **Match:** blur is an enhancement. If DWM composition is off, RDP, or contrast mode is on, chrome paints opaque. Never "disappearing icons on a see-through bar."

---

## 3. Windows acrylic vs macOS vibrancy

We ship **Windows first** (D9 / D35). Glass is designed for **Windows 11 Acrylic**, then remapped to macOS vibrancy. Do not design for mac and "see how it looks on Windows."

### 3.1 Two different materials

| | Windows 11 Acrylic (ship target) | macOS vibrancy (later) |
|---|---|---|
| OS primitive | DWM backdrop blur / Acrylic brush (tint + blur + faint noise) | `NSVisualEffectView` materials (`sidebar`, `headerView`, `hudWindow`) |
| GPUI hook today | `WindowBackgroundAppearance::Blurred` (already set in `apps/multiplexer-desktop`) | Same enum; GPUI maps it to vibrancy |
| What the user sees | Desktop / windows behind, heavily frosted, *our* cool-ink tint on top | More of the wallpaper, stronger color bleed, less noise |
| Failure modes | Uncomposited sessions, RDP, older DWM, maximized transparent-caption bugs | Reduce Transparency, Increase Contrast, Stage Manager edge cases |
| Noise | Acrylic's grain is a feature; do not add a second noise overlay | No extra noise |
| Safe alpha for pane glass | **0.52** pane / **0.68** bar (current `Theme::glass` / `glass_strong`) | Can drop toward 0.35 / 0.50 later; *not* the Windows default |

**Mica vs Acrylic.** Mica tints from the wallpaper with little blur (Settings, Explorer). Acrylic is the frosted-glass flyout (Start, context menus). Multiplexer chrome is **Acrylic-like**: we need blur so editor/terminal content does not show through the rails as readable text. Do not request Mica for sidebars.

### 3.2 Windows-first rules (implement now)

1. Keep `WindowBackgroundAppearance::Blurred`.
2. Set the title bar transparent (`TitlebarOptions.appears_transparent = true`) so Acrylic shows in the 48px caption. Draw our own 48px `glass_bar` and Windows caption-button reserve (right 140px **(spec)** so Minimize/Maximize/Close never sit on pills).
3. Every pane is `Theme::glass()` (alpha 0.52) over the blurred window. The canvas behind panes is `Theme::ink()` (alpha 0.35) so the wallpaper is a hint, not a second UI.
4. **Do not** drop pane alpha below 0.45 on Windows. Linear/Claude-style "almost clear" glass fails WCAG on busy wallpapers.
5. Overlays (palette, help) dim the canvas with `hsla(0.64, 0.20, 0.04, 0.45)` (already in `palette_overlay`) and paint `glass_strong` (alpha 0.68) on the card.
6. **Fallback:** if blur is unavailable or `prefers-reduced-transparency` / high contrast is on, set window background to opaque and swap `glass`/`glass_strong` to alpha 1.0 at the same RGB. Icons and type must not depend on blur for contrast.
7. Hairline is `hsla(0, 0, 1, 0.10)` idle and `0.18` on selected/bright (already `Theme::hairline*`). That is the Acrylic edge, not a 2px dark border.

### 3.3 macOS later (do not block Windows)

When mac ships: same token *names*, different alphas, `traffic_light_position` inset 12px from the left so the icon rail starts after the lights. Vibrancy material `sidebar` for rails, `hudWindow` for palette. No refraction shader. This is a remap, not a second design system.

---

## 4. Multiplexer "must look like this" checklist (20)

Parent implements these now against `apps/multiplexer-desktop` + `crates/multiplexer-shell` chrome. Each item is a concrete target. **(spec)** means Multiplexer-owned number.

### Glass and window

1. **Blurred window + ink canvas.** `WindowBackgroundAppearance::Blurred`. Root canvas `Theme::ink()`. Workspace padding 8px (`p_2`) and gap 4px (`gap_1`), already the right scale. No opaque full-window fill that kills Acrylic.
2. **Glass panes, not flat panels.** Left rail, center, right rail, terminal strip use `glass_pane()`: 12px radius, `Theme::glass()` fill, 1px `hairline`, existing two-stop shadow (0/10/28/−4 dark + 0/1/0/0 4% white). Keep these numbers.
3. **Glass bars stronger than panes.** Title bar, status bar, composer well, overlay cards use `Theme::glass_strong()` (alpha 0.68) so controls stay readable on Acrylic.
4. **Transparent caption on Windows.** `appears_transparent: true`, custom 48px title bar, 140px **(spec)** trailing reserve for system caption buttons. Drag region is the empty middle of the title bar.

### Icon rail and lists

5. **48px icon rail.** Far-left rail is 48px **(spec)** wide (replace `RAIL_COLLAPSED = 36`). Six destinations max: Chats, Agents, Files, Git, Search, Settings. Selected item is a 32×32 **(spec)** rounded-8 well at accent wash `hsla(0.58, 0.35, 0.22, 0.45)`.
6. **32px icon buttons.** Every chrome button (title-bar ghosts, rail icons, composer Send, terminal Run) is **32px tall**. Icon glyph 16px **(spec)**. Horizontal pad 12px when the button has a label. Current `ghost_btn` height stays; strip the dual-label+hint when the control is icon-only.
7. **List rows 56px.** Thread/agent rows: height 56px **(spec)**, pad 12×8, radius 12, 1px hairline. Line 1: 13px primary title, one line, ellipsis. Line 2: 11px muted preview + `status · id`. Selected = accent wash + `hairline_bright`. Idle fill `hsla(0,0,1,0.03)`.
8. **Section headers.** `CHATS` / inspector section labels: 11px **(spec)**, 500 weight, letter-spacing +0.6px, `Theme::muted()`, pad 12×8. Not 13px bold.

### Pills, type, density

9. **13px UI type.** All chrome (title, rails, tabs, palette rows, buttons) is **13px / 400**, line-height 18px **(spec)**. Medium (500) only on the selected rail label and primary buttons. Conversation body 14px / 20px. Mono labels 12px. **No 16px in chrome.**
10. **Pills are 20px.** Model, branch, connection, run-state, inspector tabs, composer chips: height 20px **(spec)**, pad 8×0, radius 999, 11px type. Selected inspector tab may grow to 24px **(spec)** but must stay a pill, not a block button.
11. **Compact density default.** 4px grid. Title bar 48, status bar 28, tab strip 22, breadcrumb 22, icon rail 48, list row 56, composer min 72. Comfortable / touch densities are a later setting (Orca has density; we do not build the setting this sprint).
12. **Hairlines, not drop shadows on rows.** Rows and pills use 1px 10% white. Shadows are reserved for panes, popovers, and the palette card (`Theme::shadow` only).

### Overlays and chrome surfaces

13. **Palette overlay.** Width 520px, top offset 80px, radius 12, pad 12 (`p_3`), `glass_strong`, `hairline_bright`, existing shadow. Query well 32px tall, 8px radius, 6% white fill. Max 12 rows. Row pad 8×4, selected accent wash. Dimmer `hsla(0.64, 0.20, 0.04, 0.45)`. Click-dim and Esc close. Shortcut hint right-aligned 11px muted.
14. **Help / modal overlay.** Same dimmer. Card 560px, pad 16 (`p_4`), 12px radius. One title at 13px 500, body 13px muted. No second visual language for modals.
15. **Title bar content is pills, not a sentence.** Replace the single muted `project · model · connection` string with three 20px pills plus icon-rail toggles. Left cluster: Chats toggle. Center: project, branch (when known), model. Right cluster (before the 140px caption reserve): Palette, Help, Stop (danger fill, only when busy), Inspector toggle.
16. **Composer well.** 12px radius, 12px pad, `glass_strong`, hairline. Min height 72px, max 160px then scroll. Send = 32px. Chip row (when present) 20px pills, 8px gap, 8px above the textarea.

### Status, tabs, motion, fallback

17. **Status bar 28px.** 11px type, 8px horizontal pad, 8px gap. Left: connection pill. Center: focus hint (`Enter send · Ctrl+K palette`). Right: run-state pill (idle / running / waiting / error) using `Theme::good` / `accent` / `danger`. Single line, no wrap.
18. **Inspector tabs are pills.** Right-rail tabs stay a wrapping pill row (already), 20-24px, 4px gap, 8px inset. Active = accent wash. Do not promote them to a 32px button row.
19. **Motion budget.** Rail collapse, overlay fade, terminal slide: 120-200ms (`motion.fast` / `motion.medium` from `plan/10`). No bounce. `prefers-reduced-motion` skips all of it. Input never waits on animation (`plan/16`).
20. **Opaque fallback.** High contrast, reduced transparency, or failed DWM blur: window opaque, `glass`/`glass_strong` alpha 1.0, hairline darkened to 18% on a solid ink fill. Icon rail and pills must still meet WCAG AA. This is the ChatGPT/Codex "transparent chrome ate my icons" bug we refuse to ship.

---

## 5. Token lock (implement against `theme.rs`)

Keep the existing HSLA family. Add the missing *size* tokens; do not invent a second color story.

| Token | Value **(spec)** | Already in tree? |
|---|---|---|
| `glass` | `hsla(0.64, 0.16, 0.10, 0.52)` | Yes |
| `glass_strong` | `hsla(0.64, 0.18, 0.12, 0.68)` | Yes |
| `ink` | `hsla(0.64, 0.22, 0.06, 0.35)` | Yes |
| `hairline` / `hairline_bright` | white 10% / 18% | Yes |
| `text` / `muted` | 92% @ 0.94 / 72% @ 0.72 | Yes |
| `accent` / `good` / `danger` | existing | Yes |
| `radius.sm` / `radius.md` | 6px / 12px | md yes (`panel_radius`) |
| `type.ui` | 13px / 18lh | **No** (lock now) |
| `type.caption` | 11px / 14lh | **No** |
| `type.body` | 14px / 20lh | **No** |
| `type.mono` | 12px | **No** |
| `icon.rail` | 48px | **No** (`RAIL_COLLAPSED` is 36) |
| `icon.button` | 32px | Yes (`ghost_btn`) |
| `icon.glyph` | 16px | **No** |
| `row.thread` | 56px | **No** |
| `pill.h` | 20px | **No** |
| `title.h` | 48px | Yes |
| `status.h` | 28px | **No** |
| `overlay.w` / `overlay.top` | 520 / 80 | Yes |
| `caption.reserve` | 140px (Windows) | **No** |

System UI font on Windows: **Segoe UI Variable** (fallback Segoe UI). Mono: the user's terminal/editor font, default Cascadia Mono / Cascadia Code. Do not ship Inter as a bundled UI face this sprint.

---

## 6. Mapping to the current shell (gap list)

So the parent does not re-litigate layout:

| Already correct | Change now |
|---|---|
| Blurred window background | Transparent caption + 140px caption reserve |
| `glass_pane` / `glass_bar` / shadow | Apply `type.ui` 13px everywhere in chrome |
| 48px title bar, 32px `ghost_btn`, 12px radius | Title bar becomes pills, not one muted string |
| Left 248 / right 300, clamp ranges | `RAIL_COLLAPSED` 36 → 48 icon rail with 32px icons |
| Thread rows exist (pad 12, radius xl) | Fix height 56, 13/11 type, status glyph |
| Inspector tabs as small chips | Tokenize as 20px pills |
| Palette 520 / top 80 / dimmer | Add 11px shortcut column, 32px query well |
| Composer + Send | 72px min well, chip row |
| Status line in the terminal strip | Promote a real 28px status bar (item 17) |

Do not restyle colors. The cool-ink / accent-58 family is the brand. The miss is density, type, icon rail, pills, and Windows caption glass.

---

## 7. Non-goals this sprint

- Feature parity with Orca / Cursor / Claude Code / Warp (that is `plan/01` + `plan/19`).
- Linear Liquid Glass refraction, SDF lighting, or a custom shader. Hairline + blur + tint is enough.
- macOS vibrancy tuning, traffic-light insets, or a second token file.
- User-facing density/zoom/font pickers (Orca has them; we ship the compact default first).
- Horizontal activity bar (Cursor's current default). We ship a **vertical** 48px rail on Windows.
- New screenshots, marketing mocks, or a Figma source of truth. The numbers in §4 are the source of truth.

---

## 8. Open questions (not decided here)

- Whether GPUI's `Blurred` appearance on Windows 11 is true Acrylic or a weaker DWM blur. If the fallback in item 20 is what we actually get, keep the opaque path honest and file a GPUI follow-up. Do not fake blur with a dark gradient.
- Light theme token remap (required by `plan/10` §5.2) is out of this sprint's visual bar. Dark + Acrylic is the Windows-first look.
- Icon set (Lucide vs Fluent vs custom 16px) is unresolved. The rail *metrics* are not.

---

## References

- `docs/PLAN-CONTEXT.md`: shared plan context; differentiator #5 (pop-out pane UI), Windows-first.
- `docs/DECISIONS.md`: D1, D6, D7, D8, D9, D35.
- `plan/00-vision-and-principles.md`: Beautiful / Clean / Windows-first.
- `plan/01-competitive-analysis.md`: capability bar (Orca). This doc does not replace it.
- `plan/10-ui-pane-system.md`: Outlook layout, tokens, motion, palette.
- `apps/multiplexer-desktop/src/theme.rs`: live glass tokens.
- `crates/multiplexer-shell/src/workspace.rs`: `ChromeLayout`, `RAIL_COLLAPSED`.
- Orca settings reference (Theme, accent, density, UI font, UI zoom, status-bar toggles): https://www.onorca.dev/docs/settings
- Orca product chrome (worktree list, tabs, usage pills): https://www.onorca.dev/
- Linear Liquid Glass / ProKit stance (translucency without refraction): https://linear.app/now/linear-liquid-glass
- Claude Desktop modes and Artifacts split-pane behavior: Claude Help Center (Artifacts; Desktop install).
- Warp: command palette docs; in-app opacity + blur controls (documented in Warp theme/UI writing).
- GPUI: `WindowBackgroundAppearance::Blurred` (macOS vibrancy vs Windows DWM blur). Windows composition fallback is mandatory.
