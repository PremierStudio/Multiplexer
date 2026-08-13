# 28: Glass Windows (Acrylic look, native caption kept)

**Status:** Planning (authored for parent implement; pending adversarial review)
**Owner:** Desktop chrome / design system
**Depends on:** `10-ui-pane-system.md` (Outlook layout + design tokens), `00-vision-and-principles.md` (Beautiful), `16-performance.md` (60fps / <16ms)
**Feeds:** `15-testing-strategy.md`, `19-roadmap-and-milestones.md` (Phase 0.4 polish + Phase 2.6 starter), `20-risks-and-open-questions.md`

This document is consistent with `docs/PLAN-CONTEXT.md`. Where a decision is not yet settled,
it is listed under **Open questions** and is **not** decided unilaterally here. New decisions
proposed here are numbered **D77+** in the style of `docs/DECISIONS.md`; they are proposals
for the decision log, not locked decisions.

**Locked decisions applied (D1, D9, D13, D21, D33):** This doc reflects:
- **D1** : Rust + GPUI. Glass is GPU-composited fills, borders, and shadows, not CSS.
- **D9** : Windows-first. The caption contract and Acrylic path are specified against
  GPUI 0.2.2's Windows backend (`apps/multiplexer-desktop` already ships this window).
- **D13** : Tokens live in the desktop crate today (`apps/multiplexer-desktop/src/theme.rs`).
  When `multiplexer-ui` absorbs chrome (plan/10 Phase 2), these tokens move with it.
- **D21 / D33** : Token ranges and layer names are unit-tested, mutation-gated logic.
  Visual goldens are **out of scope for v1**.

**Relationship to plan/10:** plan/10 owns the pane engine, Outlook regions, and the abstract
token groups (`bg.canvas`, `shadow.pane`, …). This doc is the **Windows glass
specialization** of plan/10 §5. It does not change layout geometry, pop-out, or focus. It
changes how those surfaces are *painted* so the DWM blur shows through.

**PARENT_IMPLEMENT.** First code: rewrite `theme.rs` tokens, then rewrite
`glass_pane` / `glass_bar` and wire every surface to a named layer. Do not touch cargo
workspace layout. Do not set `appears_transparent: true`.

---

## 1. Problem statement

The product promise is a beautiful, transparent glass shell. The current desktop already
asks for that (`WindowBackgroundAppearance::Blurred`, `Theme::glass()`, `glass_pane` /
`glass_bar`). Two things still fail the bar:

1. **The caption bug we must not repeat.** GPUI 0.2.2 documents
   `TitlebarOptions.appears_transparent` as *"Should the default system titlebar be hidden
   to allow for a custom-drawn titlebar? (macOS and Windows only)"*. On Windows the flag
   becomes `hide_title_bar`. When it is `true`, `WM_NCCALCSIZE` eats the caption and the
   platform stops using the OS hit-test for move / min / max / close. The window looks
   borderless and **cannot be moved or caption-controlled** unless we ship a full custom
   titlebar with `WindowControlArea` hit testing. We do **not** ship that in this pass.
   The live window already has `appears_transparent: false`. That stays.

2. **The fill is still too solid.** `Theme::glass_strong()` is alpha **0.68**. The approval
   strip is **0.70**. Those reads as painted gray over Acrylic, not glass. Hairline and
   drop-shadow exist, but there is no named inner highlight, no per-surface recipe, and no
   test that alphas stay in the translucent band.

The goal of this pass: **truly transparent glass all over**, with the **native Windows
caption remaining fully usable**.

---

## 2. Non-negotiable window contract

The primary window (and every later pop-out) opens with this contract. Extract it to a
pure function (`window_options(bounds) -> WindowOptions`) so tests can assert it without
opening a HWND.

```
WindowOptions {
    window_bounds: Some(WindowBounds::Windowed(bounds)),
    window_background: WindowBackgroundAppearance::Blurred,
    is_movable: true,
    is_resizable: true,
    is_minimizable: true,
    titlebar: Some(TitlebarOptions {
        title: Some("Multiplexer".into()),
        appears_transparent: false,   // REQUIRED. Never true in v1.
        traffic_light_position: None,
    }),
    ..Default::default()
}
```

### 2.1 Rules (regression-locked)

| Rule | Why |
|---|---|
| `appears_transparent` is **always `false`** | `true` hides the native caption. Move / min / max / close die unless we own hit-testing. |
| `titlebar` is **always `Some(...)`** | GPUI Windows treats `titlebar: None` as `hide_title_bar = true` (`.unwrap_or(true)`). `None` is the same bug. |
| `window_background` is **`Blurred`** | Windows path is `ACCENT_ENABLE_ACRYLICBLURBEHIND`. This is the only real blur we have. |
| Root canvas fill is **low-alpha** (`0.18..0.28`) | An opaque root kills Acrylic. Never `bg` a solid gray / `#111` on the window root. |
| We **do not paint over the OS caption** | Client area starts *below* the caption when `appears_transparent` is false. The glass toolbar is a sibling under it, not a replacement. |
| We **do not** implement custom `WindowControlArea` in this pass | That is the only legal way to hide the caption later. Out of scope. |

### 2.2 What "title" means in the Outlook chrome

plan/10 §2 draws a title bar row of project / branch / model / palette. That row is **our**
toolbar, not the OS caption.

```
┌─────────────────────────────────────────────────────────────┐
│  [OS caption: icon · Multiplexer · min / max / close]       │  ← Windows paints this
├─────────────────────────────────────────────────────────────┤
│  glass toolbar: Chats · path · model · Palette · Help · …   │  ← we paint this UNDER it
├────────────┬──────────────────────────────┬─────────────────┤
│ left rail  │ center                       │ right rail      │
└────────────┴──────────────────────────────┴─────────────────┘
```

The live `title_bar()` at 48px is already in the right place. Keep it as `chrome.toolbar`
(`glass_bar`, full width, `rounded_none`, bottom hairline). Do not try to merge it into
the caption. Do not add a drag region over the OS buttons.

---

## 3. What GPUI 0.2.2 can and cannot do

Verified against `gpui 0.2.2` (`Crate.toml` pin) and the Windows backend in the crates.io
source (`platform.rs`, `platform/windows/window.rs`, `platform/windows/events.rs`).

### 3.1 Can (use these)

- **Whole-window Acrylic blur** via `WindowBackgroundAppearance::Blurred`. On Windows this
  calls `set_window_composition_attribute` with accent state **4**
  (`ACCENT_ENABLE_ACRYLICBLURBEHIND`) and a zero tint. The desktop behind the HWND is
  blurred by DWM. That blur is the "glass".
- **Plain window alpha** via `WindowBackgroundAppearance::Transparent` (accent state 2).
  Not used: no blur, just see-through.
- **Per-element translucent fills** (`hsla(..., a)` with `a < 1.0`) composited in the
  GPU scene. This is how panes become glass: they do **not** blur what is behind *them*,
  they let the *window* blur show through.
- **1px borders** (`border_1` / `border_t_1` / `border_b_1`) with white hairline alphas.
- **Drop shadows** (`BoxShadow { color, offset, blur_radius, spread_radius }`). The
  current `Theme::shadow()` pair (soft dark drop + 1px white edge) is the right idea.
- **Native caption + min / max / close + drag** when `appears_transparent: false`.
- **Element ids** (`div().id(...)`) we already use (`palette`, `help`, `approval-card`).
  Layer names ride these ids.

### 3.2 Cannot (do not pretend)

- **No CSS, no `backdrop-filter`.** There is nothing like
  `backdrop-filter: blur(20px)` on a div. A pane cannot blur the pane behind it.
- **No per-element backdrop blur.** GPUI is a single-pass forward renderer. Shadows
  compute their own alpha; they never sample already-drawn pixels. A 2026 Zed thread
  (`zed-industries/zed#47429`) states this explicitly: whole-window `Blurred` is
  supported; per-element backdrop blur does not exist.
- **No true Windows 11 Mica.** Mica is `DWMWA_SYSTEMBACKDROP_TYPE`
  (`DWMSBT_MAINWINDOW` / `DWMSBT_TRANSIENTWINDOW`). GPUI 0.2.2's Blurred path is
  **Acrylic** (`ACCENT_ENABLE_ACRYLICBLURBEHIND`), not Mica. We market the *look*
  as "Mica/Acrylic". We do not claim the Mica API.
- **No inset `BoxShadow`.** `gpui::BoxShadow` has no `inset` field. An "inner
  highlight" is a **child 1px strip** or a **top hairline**, not `box-shadow: inset`.
- **No painting into the OS caption** while `appears_transparent` is false. Content
  that overlaps the caption is clipped / covered by DWM.
- **No screenshot / visual-snapshot tests in v1.** Headless GPUI on CI does not
  give us a stable Acrylic plate (DWM is host-dependent). Token + layer asserts only.

### 3.3 The honest recipe

**Mica/Acrylic look = window `Blurred` + low-alpha cool fills + 1px white hairline +
inner top highlight + soft drop shadow.**

If a surface needs more contrast, raise its fill *inside* `0.28..0.55`. Never "fix"
contrast by slamming alpha to 0.70+ or by switching the window to `Opaque`.

---

## 4. Token system (first code)

All glass numbers live in `apps/multiplexer-desktop/src/theme.rs`. Components consume
tokens, never raw `hsla(...)` for chrome fills. Message bubbles, chips, and ghost
buttons may keep local alphas, but they must still sit inside the published bands
(see §6).

### 4.1 Bands (tested invariants)

| Band | Range | Used for |
|---|---|---|
| `canvas.alpha` | **0.18 .. 0.28** | Window root wash. Lets Acrylic read. |
| `pane.alpha` | **0.28 .. 0.55** | Every glass panel / bar / card fill. |
| `hairline.alpha` | **0.08 .. 0.16** | 1px white edges. |
| `dim.alpha` | **0.40 .. 0.50** | Modal overlay scrim (palette, help). |
| `text.alpha` | **≥ 0.90** | Primary text on glass. |
| `muted.alpha` | **≥ 0.68** | Secondary text. Readable, not chalk. |

Anything outside the band is a failing test, not a "tweak".

### 4.2 Concrete dark tokens (v1 values)

Hue stays the current cool indigo (`h ≈ 0.64`) so we do not restyle the product, only
the *opacity*. Values below are the rewrite targets. Current live numbers that violate
the band are called out.

| Token | Target `hsla` | Notes |
|---|---|---|
| `canvas` (today `ink`) | `(0.64, 0.22, 0.06, **0.22**)` | Was `0.35`. Still a wash, less muddy. |
| `glass` (pane) | `(0.64, 0.16, 0.11, **0.34**)` | Was `0.52`. Mid-band. Rails + center. |
| `glass_bar` | `(0.64, 0.18, 0.12, **0.42**)` | Replaces `glass_strong` at `0.68`. |
| `glass_overlay` | `(0.64, 0.18, 0.13, **0.50**)` | Palette / help cards. Top of band. |
| `glass_emphasis` | `(0.58, 0.40, 0.22, **0.44**)` | Selected thread, on-tab, approval tint. |
| `hairline` | `(0.00, 0.00, 1.00, **0.10**)` | 10%. Keep. |
| `hairline_bright` | `(0.00, 0.00, 1.00, **0.14**)` | Was `0.18` (over the 16% cap). |
| `highlight` | `(0.00, 0.00, 1.00, **0.12**)` | Inner top 1px. New. |
| `dim` | `(0.64, 0.20, 0.04, **0.45**)` | Live overlay scrim is already 0.45. Keep. |
| `text` | `(0.62, 0.08, 0.92, **0.94**)` | Keep. |
| `muted` | `(0.62, 0.08, 0.72, **0.72**)` | Keep. |
| `accent` / `good` / `danger` | keep current | Status / CTA, not chrome fill. |
| `panel_radius` | `px(12)` | Keep. Bars that span the window use `rounded_none`. |

`glass_strong()` remains as a **deprecated alias** of `glass_bar()` for one commit if
call sites need it, then it is deleted. Its 0.68 alpha must not survive.

### 4.3 Elevation tokens

```
shadow.pane     = dark (0.64, 0.30, 0.04, 0.40)  offset (0, 8)  blur 24  spread -4
                  + white (0, 0, 1, 0.05)        offset (0, 1)  blur  0  spread  0
shadow.overlay  = dark (0.64, 0.30, 0.04, 0.50)  offset (0, 16) blur 36  spread -4
                  + white (0, 0, 1, 0.06)        offset (0, 1)  blur  0  spread  0
```

The second stop is an **outer** 1px light edge (GPUI cannot do inset). The **inner**
highlight is a separate child (see §5).

### 4.4 Layer names (tested catalog)

A `GlassLayer` enum is the visual twin of `controls::Surface`. Every painted chrome
region has exactly one layer name. Tests assert the catalog is complete and that
each layer's fill token is inside the pane band (canvas and dim have their own bands).

| Layer name | Surface | Fill token | Shape |
|---|---|---|---|
| `window.canvas` | root | `canvas` | full window, no radius, no hairline |
| `chrome.toolbar` | TitleBar | `glass_bar` | full width, `rounded_none`, bottom hairline |
| `rail.left` | LeftRail | `glass` | `panel_radius`, full hairline, pane shadow |
| `rail.center` | Center | `glass` | same |
| `rail.right` | RightRail | `glass` | same |
| `drawer.terminal` | TermStrip | `glass_bar` | full width, `rounded_none`, top hairline |
| `chrome.status` | (status strip) | `glass_bar` | full width, `rounded_none`, top hairline |
| `overlay.palette.dim` | Palette | `dim` | absolute full window |
| `overlay.palette.card` | Palette | `glass_overlay` | 520px, radius, bright hairline, overlay shadow |
| `overlay.help.dim` | HelpOverlay | `dim` | absolute full window |
| `overlay.help.card` | HelpOverlay | `glass_overlay` | 560px, same chrome as palette card |
| `card.approval` | ApprovalCard | `glass_emphasis` | full width, bottom hairline, no radius |
| `card.reminder` | ReminderBar | `glass_emphasis` (warm hue allowed) | full width, bottom hairline |

`controls::Surface` stays the *input* catalog (what the user can click). `GlassLayer`
is the *paint* catalog (what the GPU draws). A surface may produce more than one
layer (palette = dim + card).

---

## 5. The glass primitive

Two builders, owned by `theme.rs` (or a sibling `glass.rs` if `theme.rs` gets crowded).
`main.rs` stops defining local `glass_pane` / `glass_bar`.

### 5.1 Shared stack (every glass surface except canvas and dim)

1. **Fill** at the layer's token alpha (`0.28..0.55`).
2. **Hairline** `border_1` (or a single edge on full-width bars) in white `0.08..0.16`.
3. **Inner highlight**: a 1px-tall child docked to the **top inside** edge, `bg(highlight)`,
   `rounded` matching the parent, ignored by hit-testing. This is the "lit lip" Acrylic
   panels have. Because `BoxShadow` has no inset, this child is mandatory.
4. **Drop shadow** from `shadow.pane` (or `shadow.overlay` on floating cards).
5. **Radius** `panel_radius` on floating / column panes; `rounded_none` on window-spanning
   bars so they do not show corner gaps against the caption or the window edge.

`glass_pane()` = steps 1-5, all four borders, radius, pane shadow.
`glass_bar()` = steps 1-5, one joining edge only (toolbar = bottom, drawer/status = top),
no radius, pane shadow optional (bars can skip the heavy drop so they sit flush).

### 5.2 What we refuse

- A second `Blurred` window per pane (would create extra HWNDs and break the caption).
- Fake frosted bitmaps / noise textures as a blur stand-in.
- Opaque `#1a1a1a` "glass" that ignores `window_background`.
- Painting a 32-40px custom caption and setting `appears_transparent: true` "just to
  look more native". That is the bug.

---

## 6. Recipe per surface

### 6.1 Window canvas (`window.canvas`)

- Root `div` in `ShellView::render`: `.size_full().bg(Theme::canvas())`.
- No border, no shadow, no radius.
- Children are the Outlook column. Padding around the three rails (`p_2` + `gap_1`)
  stays: the gap is where Acrylic shows between panes. Do not collapse it to zero.

### 6.2 Glass toolbar under the OS caption (`chrome.toolbar`)

- Native caption remains. We never cover it.
- 48px `glass_bar`, `rounded_none`, `border_b_1` hairline, inner highlight on its own
  top edge (just under the caption). That highlight is what makes the toolbar read as
  a shelf, not a second titlebar.
- Controls stay the current ghost buttons (Chats, Palette, Help, Stop, Inspector).
- Do not add window-control glyphs. Do not claim the top 8px as a drag region.

### 6.3 Left rail (`rail.left`)

- `glass_pane`, width from `chrome.occupied_left()`.
- Collapsed strip uses the same pane (just narrower). Do not switch to an opaque strip.
- Selected thread chip uses `glass_emphasis` + `hairline_bright`, not a solid navy.
- Hover on a thread: fill alpha +0.06, still ≤ 0.55.

### 6.4 Center (`rail.center`)

- `glass_pane`, `flex_1`, `min_w_0`.
- Conversation column is transparent to the pane. User bubbles may use `glass_emphasis`;
  assistant bubbles stay a faint white (`0.06..0.10`), which is *content* not chrome
  (allowed below the pane band because it sits *on* glass, not *as* glass).
- Composer field: white `0.08` fill, `hairline_bright` idle, `accent` when focused.

### 6.5 Right rail (`rail.right`)

- Same primitive as the left rail.
- Inspector tabs: idle white `0.04`, selected `glass_emphasis`.
- Tab body text stays `muted`. No extra opaque well behind the inspector text.

### 6.6 Bottom terminal drawer (`drawer.terminal`)

- `glass_bar` at the current 108px, `rounded_none`, `border_t_1`, inner highlight on
  the top edge (reads as a sliding drawer lip).
- Input well: white `0.05..0.08`, accent border when `Focus::Terminal`.
- This is the pop-up terminal from plan/10 §2.4. When that drawer later animates
  (`motion.medium`, 200ms), the recipe does not change, only height.

### 6.7 Status strip (`chrome.status`)

- 26px `glass_bar`, `rounded_none`, `border_t_1`. Quieter than the drawer: no extra
  shadow. Muted text.

### 6.8 Palette overlay (`overlay.palette.dim` + `.card`)

- Dim: absolute, full window, `Theme::dim()` at **0.45** (legal: 0.40-0.50). Click
  dim to close (already wired).
- Card: `glass_overlay` (0.50), 520px, `panel_radius`, `hairline_bright`,
  `shadow.overlay`, inner highlight.
- Query well and selected row stay low-alpha (selected = `glass_emphasis`).
- Do not raise the card to 0.68 to "make it readable". Raise text contrast, not fill.

### 6.9 Help overlay (`overlay.help.dim` + `.card`)

- Same dim token and same card recipe as the palette (560px). One primitive, two
  callers. Esc / click-dim already close it.

### 6.10 Approval card (`card.approval`)

- Today: `hsla(0.08, 0.55, 0.22, 0.70)` (illegal). Rewrite to `glass_emphasis` with a
  **warm** hue (`h ≈ 0.08`) at alpha **0.44**, bottom hairline, inner highlight.
- Allow / Deny remain ghost buttons. Danger stays on the Deny/Stop *button*, not on
  the whole strip.
- Still a bar under the toolbar, not a modal. If we later promote it to a modal,
  it reuses the overlay dim+card pair.

### 6.11 Reminder bar (`card.reminder`)

- Same recipe as approval, warm hue (`h ≈ 0.12`) at 0.40-0.44. Today 0.55 is the
  top of the pane band; drop it slightly so it does not compete with approval.

### 6.12 Resize handles

- Idle: fully transparent (Acrylic shows). Hover: accent at **0.28** (floor of the
  pane band). Width stays 7px.

---

## 7. Implementation sequence (parent)

Do this in order. Each step is a compiling desktop crate with tests green.

1. **Tokens + tests in `theme.rs` (first code).** Publish the table in §4.2. Add
   `GlassLayer`, `alpha_of`, `fill_for(layer)`, `band_for(layer)`. Co-located
   `#[cfg(test)]` as specified in §8. No GPUI window required.
2. **Move `glass_pane` / `glass_bar` onto the token module.** They take a
   `GlassLayer` (or a small `GlassKind::{Pane, Bar, OverlayCard}`) and apply the
   stack in §5. `main.rs` only calls them.
3. **Wire every surface** in §6 to a layer name / id. Palette and help keep their
   existing ids (`palette`, `help`) and gain child ids (`palette-card`, `help-card`)
   if needed.
4. **Delete illegal alphas.** Grep `apps/multiplexer-desktop/src` for `hsla(` and
   reject any chrome fill `a > 0.55` or hairline `a > 0.16`. Content chips / ghost
   idle (`0.06`-`0.11`) are allowed.
5. **Leave `WindowOptions` as they are** except to extract `window_options()` for
   the contract test. Do **not** flip `appears_transparent`. Do **not** set
   `titlebar: None`.
6. **Stop.** No light theme, no custom caption, no GPUI fork, no screenshot lab.

`controls.rs` does not need a visual change. If a new id is added for the toolbar
itself, keep `Surface::all().len() == 10` unless a new *control* appears.

---

## 8. TDD (v1: headless token + layer asserts)

No screenshot tests. No "render this PNG and diff it". Acrylic is DWM-host-dependent
and would flake on CI. The suite is pure Rust over tokens and names, matching how
`controls.rs` already tests the control catalog.

### 8.1 Unit (co-located on `theme.rs`)

| Test | Asserts |
|---|---|
| `canvas_alpha_in_wash_band` | `Theme::canvas().a` in `0.18..0.28` |
| `pane_fills_in_glass_band` | `glass`, `glass_bar`, `glass_overlay`, `glass_emphasis` each in `0.28..0.55` |
| `hairlines_in_white_band` | `hairline`, `hairline_bright`, `highlight` each `l == 1.0` (white) and `a` in `0.08..0.16` |
| `dim_in_modal_band` | `Theme::dim().a` in `0.40..0.50` |
| `text_stays_readable` | `text.a >= 0.90`, `muted.a >= 0.68` |
| `every_layer_has_a_fill` | `GlassLayer::all()` is non-empty; each maps to a fill; names are unique kebab-case |
| `layer_names_are_stable` | exact set from §4.4 (so a rename is a deliberate test edit) |
| `window_options_keep_native_caption` | extracted `window_options()` has `appears_transparent == false`, `titlebar.is_some()`, `window_background == Blurred`, `is_movable && is_minimizable && is_resizable` |
| `titlebar_none_is_forbidden` | documented constant / helper never returns `titlebar: None` |

### 8.2 Property (small, cheap)

- For every `GlassLayer`, `band_for(layer).contains(fill_for(layer).a)`.
- Hairline tokens always have `h == 0.0 && s == 0.0` (white, not tinted gray).
- `GlassLayer::all().len()` equals the number of names in the golden list.

### 8.3 Mutation

`theme.rs` is in scope for cargo-mutants (D21). Surviving mutants that bump an alpha
out of band, drop `appears_transparent: false`, or delete a layer name must be
killed by §8.1-8.2. Merge floor remains **70%** (D33) on this logic.

### 8.4 Explicitly out of v1

- GPUI element screenshots / goldens (plan/10 §9.2 visual snapshots wait for Phase 2.6).
- Comparing DWM blur radius across machines.
- Light-theme glass.
- Custom caption hit-testing.

Component tests that already exist (`controls.rs`, `inspector.rs`) stay green and
are not rewritten for paint.

---

## 9. Accessibility and performance

- **Contrast:** primary text stays ≥ 0.90 alpha on a ≤ 0.55 pane. That is the WCAG
  lever we control without measuring the user's wallpaper. Status colors (`accent`,
  `good`, `danger`) stay high-chroma and high-alpha.
- **Reduced motion:** this pass adds no animation. When the terminal drawer later
  slides (plan/10 §5.3), it respects `prefers-reduced-motion`.
- **Frame budget:** extra 1px highlight children are a handful of quads. They must
  not allocate per frame beyond the existing element tree. No blur shaders of our
  own (plan/16: input < 16ms).
- **Pop-out windows (later):** each new HWND uses the same `window_options()` so a
  detached pane does not regress the caption bug.

---

## 10. Roadmap slot

This is **Phase 0.4 chrome polish**, not a new MVP phase. It lands on the existing
`multiplexer-desktop` binary before Phase 2.6's full design system.

| Plan | How this doc plugs in |
|---|---|
| `plan/19` §2 deliverable 0.4 | GPUI shell is no longer a blank pane; it is the glass Outlook chrome. Exit criterion adds: native caption usable + token tests green. |
| `plan/19` §4 deliverable 2.6 | Absorbs these tokens into the shared `multiplexer-ui` theme. Snapshot tests may start *then*. They do not start here. |
| `plan/10` §5 | `bg.canvas` / `bg.surface` / `shadow.pane` get these concrete Windows values. |
| `plan/15` | Adds the theme unit+property+mutation targets in §8. No new e2e. |

---

## 11. Key design decisions (proposed D77+)

These are proposals for `docs/DECISIONS.md`. They are **not** locked.

### D77. Native caption stays; `appears_transparent` is false (PROPOSED)
- **Decision:** Every Multiplexer HWND keeps `titlebar: Some(TitlebarOptions { appears_transparent: false, .. })`. Custom-drawn captions and `titlebar: None` are forbidden until a later decision ships `WindowControlArea` hit-testing for move / min / max / close.
- **Rationale:** GPUI 0.2.2 on Windows *hides* the system titlebar when the flag is true. That is how we lost drag and the caption buttons. Glass is not worth a dead window.

### D78. Whole-window Acrylic + layered fills, not per-pane blur (PROPOSED)
- **Decision:** `WindowBackgroundAppearance::Blurred` is the only blur. Surfaces are translucent fills (`0.28..0.55`) over that plate. We do not fork GPUI for backdrop-filter or Mica.
- **Rationale:** Matches what the crate can actually do. Fake-solid gray is the failure mode we are fixing.

### D79. Published alpha bands are test-gated (PROPOSED)
- **Decision:** Canvas `0.18..0.28`, pane `0.28..0.55`, hairline `0.08..0.16`, dim `0.40..0.50`. Violations fail CI.
- **Rationale:** The 0.68 `glass_strong` / 0.70 approval fills happened because nothing enforced translucency.

### D80. No screenshot tests for glass in v1 (PROPOSED)
- **Decision:** Glass v1 is asserted by token alphas and layer names only.
- **Rationale:** Acrylic is a DWM effect. Pixel goldens would be host-specific and would not catch the caption bug. Headless names + numbers will.

---

## 12. Open questions

Per PLAN-CONTEXT, these are not decided here.

1. **Custom caption later?** If we ever want a flush, caption-less window, it is a
   separate project: `appears_transparent: true` **plus** `WindowControlArea::Drag /
   Min / Max / Close` hit regions that match the Windows 11 caption buttons. Not this
   pass.
2. **True Mica?** Would need a GPUI fork (`DWMWA_SYSTEMBACKDROP_TYPE`). Deferred.
   Acrylic + this recipe is the Windows-first look.
3. **Light theme glass?** plan/10 §5.2 still promises light + system-follow. Light
   glass would invert fills (white `0.28..0.45` over Blurred) and needs its own token
   table. Out of scope here; dark only.
4. **Where tokens live after Phase 2.** Today: `apps/multiplexer-desktop/src/theme.rs`.
   plan/10 / D13 want `multiplexer-ui`. Move when that crate exists, not now.

---

## 13. Acceptance

This pass is done when:

- [ ] `appears_transparent == false` and `titlebar.is_some()` on every window options
      helper, covered by a unit test.
- [ ] `window_background == Blurred`.
- [ ] The OS caption still moves, minimizes, maximizes, and closes the live window
      (manual on Windows; the automated guard is the options test).
- [ ] The glass toolbar sits **under** the caption, not over it.
- [ ] Every layer in §4.4 is painted through `glass_pane` / `glass_bar` / dim+card.
- [ ] No chrome fill alpha outside `0.28..0.55`; canvas in `0.18..0.28`; dim in
      `0.40..0.50`; hairlines in `0.08..0.16`.
- [ ] Inner 1px highlight is present on panes, bars, and overlay cards.
- [ ] `theme.rs` unit + property tests from §8 are green. No screenshot fixtures
      added.
- [ ] `Theme::glass_strong` at 0.68 and the approval 0.70 fill are gone.

---

*Next implementation step (parent): rewrite `apps/multiplexer-desktop/src/theme.rs`
tokens + tests, then replace `glass_pane` / `glass_bar` in `main.rs`.*
