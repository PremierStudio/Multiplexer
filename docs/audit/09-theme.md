# 09: Theme tokens vs desktop usage

**FINDINGS:** 11

**Scope:** `crates/multiplexer-theme` vs `apps/multiplexer-desktop` (`theme.rs`, `main.rs`). Specs: `plan/27-theme-tokens.md`, `plan/28-glass-windows.md`.
**Method:** Read-only inventory of every public token and every `hsla(` / `Theme::` call site. No cargo.
**Date:** 2026-08-12

The token crate exists and the desktop adapter delegates most named colors to `ThemeTokens::dark()`. The shipping window still invents raw HSLA, never switches mode or density, and never consumes type, space, motion, or elevation. Glass alphas in the crate are in the translucent band; leftover fills in `main.rs` are not.

---

## Inventory

### Crate surface (`crates/multiplexer-theme`)

| Token / API | Desktop paint? | Adapter? |
|---|---|---|
| `ThemeTokens::dark()` | yes (only path) | `Theme::tokens()` always returns this |
| `ThemeTokens::light()` | **no** | none |
| `with_density` / `Density::Compact` | **no** | none |
| `bg` | **no** | none (`Theme::bg` does not exist) |
| `surface` | yes (composer well) | `Theme::surface()` |
| `surface_raised` | **no** | none |
| `glass` | yes (`glass_pane`) | `Theme::glass()` |
| `glass_strong` | yes (bars + overlay cards) | `Theme::glass_strong()` |
| `glass_ultra` | yes (icons, pills, idle rows) | `Theme::glass_ultra()` |
| `ink` | yes (window root fill) | `Theme::ink()` |
| `text` / `text_muted` / `text_faint` | yes | `text` / `muted` / `faint` |
| `accent` / `accent_muted` | yes | yes (`send_bg` also aliases `accent_muted`) |
| `good` / `warn` / `danger` | yes (badges, agent label, Stop) | yes |
| `hairline` / `hairline_bright` | yes | yes |
| `selection` | yes (tabs, rows, hover) | yes |
| `focus_ring` | **no** | none |
| `space_1`..`space_6` / `space()` | **no** | none |
| `TypeScale::*` | **no** | none |
| `Radius::MD` | yes (`panel_radius`) | yes |
| `Radius::XS/SM/LG/XL` | **no** | none |
| `Motion::*` | **no** | none |
| `Elevation` / `glass_at` | **no** | none |
| Named shadows / fonts / line-height | **not in crate** | `Theme::shadow()` owns literals |

Shell stores `Workspace.settings: UiSettings { mode, density }` (`crates/multiplexer-shell/src/settings.rs`). Desktop never reads those fields and never calls `cycle_mode` / `cycle_density`.

### Hardcoded `hsla(` leftovers

Legal adapter conversion only: `theme.rs:14` (`hsla(t.h, t.s, t.l, t.a)`).

**`theme.rs` still owns values (plan/27 §9 forbids this):**

| Line | Literal | Role |
|---|---|---|
| 74 | `(0.64, 0.30, 0.04, 0.38)` | drop shadow |
| 80 | `(0.00, 0.00, 1.00, 0.07)` | hairline shadow |

**`main.rs` 17 raw fills** (plan/28 §7 step 4: grep `hsla(` and reject chrome `a > 0.55`):

| Line | Literal | Surface | Alpha vs bands |
|---|---|---|---|
| 1021 | `(0, 0, 1, 0.00)` | idle left-rail icon | transparent (ok as content) |
| 1207 | `(0, 0, 1, 0.00)` | idle collapsed inspector tab | transparent |
| 1232 | `(0, 0, 1, 0.03)` | idle inspector tab | below pane band (content wash) |
| 1307 | `(0.58, 0.45, 0.28, 0.55)` | user bubble | pane cap; near `selection` (0.42) |
| 1309 | `(0, 0, 1, 0.06)` | assistant bubble | content wash (plan/28 §6.4 allows) |
| 1424 | `(0.12, 0.45, 0.22, 0.55)` | reminder bar | pane cap; plan/28 wanted 0.40..0.44 |
| **1449** | **`(0.08, 0.55, 0.22, 0.70)`** | **approval card** | **illegal (`a > 0.55`)** |
| 1509 | `(0, 0, 1, 0.05)` | terminal input | content well |
| 1569 | `(0.64, 0.20, 0.04, 0.45)` | palette dim | dim band, but no `dim` token |
| 1592 | `(0, 0, 1, 0.06)` | palette query well | content wash |
| 1607 | `(0.58, 0.40, 0.28, 0.50)` | selected palette row | near `accent_muted` / `selection` |
| 1609 | `(0, 0, 0, 0.00)` | idle palette row | transparent |
| 1636 | `(0.64, 0.20, 0.04, 0.45)` | help dim | same as palette dim |
| 1676 | `(0.58, 0.50, 0.55, 0.28)` | resize hover | plan/28 §6.12 (accent 0.28) |
| 1738 | `(0, 0, 1, 0.06)` | chip fill | content wash |
| 1945 | `(0, 0, 1, 0.07)` | ghost button idle | plan/27 `send_bg` was 0.11 |
| 1949 | `(0.58, 0.35, 0.28, 0.40)` | ghost hover | invented accent wash |

---

## Findings

### F1. Hardcoded `hsla` still in `main.rs`

- **Severity:** high
- **Spec:** plan/27 §3.3 / §9.1 ("No color math in the adapter. No tweak alpha at the call site."). plan/28 §4 / §7.4 (components consume tokens; grep `hsla(` and delete illegal chrome).
- **Where:** `apps/multiplexer-desktop/src/main.rs` (17 call sites, table above). Also `theme.rs:74` and `theme.rs:80`.
- **Evidence:** Chrome that plan/28 named (`card.approval`, `card.reminder`, `overlay.*.dim`, selected palette row, resize hover, ghost idle) is still a raw tuple. Several literals are one tweak away from an existing token (`selection` is `(0.58, 0.45, 0.28, 0.42)` vs user bubble `(0.58, 0.45, 0.28, 0.55)`).
- **Why it matters:** The crate cannot enforce bands if the window paints around it. Every new pane will keep inventing fills, which is the drift plan/27 §1 called out.
- **Fix:** Add missing tokens (`dim`, `highlight`, `ghost_fill` / real `send_bg`, warm `glass_emphasis`). Replace every `hsla(` in `main.rs` except the adapter `to_hsla` helper. Move shadow literals into the crate.

### F2. Glass is not transparent enough in practice

- **Severity:** high
- **Spec:** plan/28 §1 / §4.1 / §13 (pane `0.28..0.55`, canvas `0.18..0.28`, hairline `0.08..0.16`, no chrome `a > 0.55`). plan/27 §3.1 (`glass.a < 0.55`, real transparency).
- **Where:** `main.rs:1449` (approval `0.70`), `main.rs:1424` (reminder `0.55`), `main.rs:1307` (user bubble `0.55`), `tokens.rs:235-236` (`glass_strong.a = 0.50` used for bars *and* overlay cards), `tokens.rs:248` (`hairline_bright.a = 0.18`), `main.rs:843` stacked under `glass_pane` / `glass_bar`.
- **Evidence:** Crate dark `glass.a = 0.36` passes `a < 0.55` and is close to plan/28's `0.34`. The live window still fails the product look:
  1. Approval fill `0.70` is the exact illegal number plan/28 §1 and §6.10 called out. Still there.
  2. Reminder sits at the pane cap (`0.55`); plan/28 §6.11 asked `0.40..0.44`.
  3. Title / status / terminal / overlay cards all use `glass_strong` at `0.50`. plan/28 split that into `glass_bar` `0.42` and `glass_overlay` `0.50`.
  4. Root is `Theme::ink()` (`a = 0.22`) plus pane `0.36` plus row `glass_ultra` `0.20` plus selection `0.42`. Acrylic has to punch through a stack, not one wash.
  5. No inner 1px highlight child (plan/28 §5.1 step 3). Without the lit lip, translucent fills read as painted slabs.
  6. `hairline_bright.a = 0.18` is over the `0.16` hairline cap.
- **Why it matters:** Window options are already correct (`Blurred`, `appears_transparent: false` at `main.rs:2054-2065`). The caption contract is fine. The fill recipe is what still looks solid.
- **Fix:** Kill approval `0.70` and reminder `0.55` first. Split bar vs overlay tokens. Paint the 1px highlight. Drop `hairline_bright` to `<= 0.16`. Stop stacking extra washes on the canvas.

### F3. Unused color tokens: `bg`, `surface_raised`, `focus_ring`

- **Severity:** medium
- **Spec:** plan/27 §4.1 (required fields) and §7.8 (window fill is `color.bg`; keyboard focus is `color.focus_ring`, not `hairline_bright` alone; selected row is `surface_raised`).
- **Where:** `crates/multiplexer-theme/src/tokens.rs:140-157`. Adapter: `apps/multiplexer-desktop/src/theme.rs` (no `bg`, `surface_raised`, or `focus_ring` methods).
- **Evidence:** Composer focus uses `Theme::accent()` (`main.rs:1380-1384`). Terminal focus uses `Theme::accent()` (`main.rs:1511-1515`). Selected rows use `Theme::selection()` (`main.rs:1815-1818`), never `surface_raised`. Window root uses `Theme::ink()` (`main.rs:843`), never `bg`. Dark `bg` (`a = 0.18`) is a dead canvas token.
- **Why it matters:** Required fields that nothing paints are untested in the product. Focus is not distinct from accent. Canvas vs ink (plan/27 opaque `bg` vs plan/28 wash) is unresolved because `bg` is never applied.
- **Fix:** Expose the three tokens on the adapter. Root fill: `bg` or document that plan/28 canvas *is* `ink` and delete `bg`. Selected rows: `surface_raised` or drop the field. Focused wells: 1px `focus_ring`.

### F4. Density is unused

- **Severity:** medium
- **Spec:** plan/27 §4.3 / §4.5 (`Comfortable` vs `Compact` shrinks space). plan/27 §9.4 (`ThemeTokens::new(mode, Density::Compact)` rebuilds the theme).
- **Where:** Crate: `tokens.rs:35-38, 125-132, 182-186`. Shell: `crates/multiplexer-shell/src/settings.rs:9, 31-36` and `workspace.rs:290`. Desktop: no reads.
- **Evidence:** `Theme::tokens()` is `ThemeTokens::dark()` (Comfortable only). Compact values (`2, 6, 8, 12, 16, 24`) are never passed to `px()`. Layout spacing is GPUI scale helpers (`p_2`, `p_3`, `p_4`, `px_3`, `gap_1`..`gap_3` at ~60 sites in `main.rs`). `UiSettings::cycle_density` is unit-tested and never dispatched from the window. `settings_open` stays `false`.
- **Why it matters:** Compact cannot change a single gap. The space scale is dead weight plus a false settings API.
- **Fix:** Thread `workspace.settings.density` into `Theme::tokens()`. Replace `p_*` / `gap_*` / `px_3` chrome padding with `space_N`. Wire a settings or palette action to `cycle_density`.

### F5. Light theme is unused

- **Severity:** medium
- **Spec:** plan/27 §2.1 goal 4 and §6.2 (complete light table). plan/27 §9.2 (`Theme::light()`). plan/36 §4.10 (settings theme flip).
- **Where:** `tokens.rs:178-180, 258-286`. `theme.rs:9-11`. `settings.rs:8, 24-28`.
- **Evidence:** Adapter has no `Theme::light()`. `Theme::tokens()` hardcodes `ThemeTokens::dark()`. `UiSettings.mode` defaults to `Dark` and `cycle_mode` is never called from `apps/multiplexer-desktop`. Light glass (`0.42`) / light text (`l = 0.14`) never reach GPUI. plan/28 §8.4 explicitly deferred light glass; plan/27 still required the table *and* the adapter switch.
- **Why it matters:** `light_differs_from_dark` only proves the crate. The product is dark-only. Settings can store Light and still paint Dark (plan/36 already warned).
- **Fix:** `Theme::tokens()` reads `workspace.settings.mode`. Add `Theme::light()` as plan/27 §9.2. Settings overlay (or a palette row) calls `cycle_mode` and `cx.notify()`.

### F6. Type scale is unused

- **Severity:** medium
- **Spec:** plan/27 §4.6 (`11/12/13/14/16/20/24`; caps = `xs`, body = `base`, empty title = `xl`, composer = `lg`). Fonts: Segoe UI Variable / Cascadia Mono.
- **Where:** `tokens.rs:78-100`. Desktop: `main.rs:845` is the only type size call (`.text_sm()` on the root).
- **Evidence:** Section caps (`section.label().to_ascii_uppercase()`, `main.rs:1045-1046`) use `Theme::faint()` with default `text_sm`. Empty-state title (`main.rs:1720`) is not `TypeScale::DISPLAY` (20). Composer (`main.rs:1372-1399`) is not `TypeScale::TITLE` (16). Status bar (`main.rs:1537-1554`) is not `TypeScale::SMALL` (12). No `FontNames`, no line-height. Crate names (`CAPTION/SMALL/UI/BODY/TITLE/DISPLAY/HERO`) also do not match the spec names (`xs/sm/md/base/lg/xl/xxl`), even though the numbers match.
- **Why it matters:** One GPUI size for chrome, titles, caps, and status. Switching density cannot tighten type (v1 says it should not), but even Comfortable cannot express the scale.
- **Fix:** Adapter helpers `Theme::type_px(TypeScale::*)` and font families. Caps `xs`, status `sm`, body `base`, composer/pane titles `lg`, empty title `xl`.

### F7. Elevation / `glass_at` unused, and the ramp is not the spec

- **Severity:** medium
- **Spec:** plan/27 §4.2 / §6.1 (table `0.22, 0.36, 0.52, 0.68, 0.84`; `glass(0)==glass_ultra`, `glass(2)==glass`, `glass(3)==glass_strong`; `try_from_u8` rejects 5).
- **Where:** `tokens.rs:42-64, 188-192`. No desktop call.
- **Evidence:** `glass_at` is `glass.a + 0.07 * index`, clamped to `0.78`. Dark `glass.a = 0.36`, so the live ramp is `0.36, 0.43, 0.50, 0.57, 0.64`. That means:
  - `glass_at(Base)` is `0.36`, not `glass_ultra` (`0.20`).
  - `glass_at(Raised)` is `0.50`, not `glass` (`0.36`).
  - `glass_at(Overlay)` is `0.57`, not `glass_strong` (`0.50`), and `0.57` is already over the plan/28 pane cap.
  - `glass_at(Float)` is `0.64`, well over `0.55`.
  - Variants are `Base/Sunken/Raised/Overlay/Float`, not `Zero..=Four`. No `try_from_u8` / `saturating`.
  - Crate test `elevation_monotonic_alpha` uses `>=`, so a flat ramp survives.
- **Why it matters:** The one API that was supposed to pick pane vs bar vs overlay is both unused and wrong. If someone wires it tomorrow, Float glass is more opaque than the approval bug.
- **Fix:** Replace the formula with the §6.1 / §6.2 tables. Alias named glass to elevations. Paint rails at 2, bars at 3, palette card at 4. Kill the `>=` test.

### F8. Space and motion unused

- **Severity:** medium
- **Spec:** plan/27 §4.5 (space `4/8/12/16/20/24/32`), §4.7 (motion `120/200/320`, easing names). plan/10 drawer motion later uses `motion.medium` 200ms.
- **Where:** `tokens.rs:102-132, 158-163`. Desktop: GPUI `p_*` / `gap_*` only. Zero `Motion` references under `apps/`.
- **Evidence:** Comfortable space in the crate is `4, 8, 12, 16, 24, 32` (steps 1..6). Spec also has `s20 = 20`. There is no 20px step. Motion in the crate is `90 / 160 / 240` with string easings `"ease-out"` / `"ease-in-out"`, not `120 / 200 / 320` and `EasingName`. No GPUI animation reads these numbers. `reduce_motion` does not exist.
- **Why it matters:** Density (F4) cannot land until space is the padding source. Drawer / overlay motion will invent durations the same way chrome invented HSLA.
- **Fix:** Expose `space_*` as `px()` on the adapter. Add `s20`. Correct motion to 120/200/320 and an `EasingName` enum. Do not animate until those values are the only durations.

### F9. Adapter still owns values (`shadow`, `send_bg`)

- **Severity:** medium
- **Spec:** plan/27 §9 ("After this crate lands, [`theme.rs`] must not own values."). §4.8 named shadows `rest/hover/float`. §9.2 `send_bg` is `hsla(0,0,1,0.11)`, not a crate field.
- **Where:** `theme.rs:56-58, 71-86`.
- **Evidence:** `Theme::shadow()` hardcodes a two-layer stack (drop `a = 0.38`, blur 32, spread -6). Plan/27 rest is drop `a = 0.45`, blur 28, spread -4. Plan/28 `shadow.pane` is drop `a = 0.40`, blur 24, offset y 8. Hover and float stacks do not exist. Overlay cards (`main.rs:1583, 1651`) reuse the same rest shadow. `Theme::send_bg()` returns `accent_muted` `(0.58, 0.40, 0.32, 0.55)`, a saturated brand wash, not the spec's white `0.11` ghost.
- **Why it matters:** Shadows and the Send button cannot follow mode. Light theme (F5) would keep a dark drop. Send reads as a filled accent chip.
- **Fix:** Put `NamedShadows` in the crate. Adapter copies fields only. Restore `send_bg` as a white ghost or add `ghost_fill` and use it for chips, ghost idle, and Send.

### F10. Crate taxonomy drifts from plan/27

- **Severity:** medium
- **Spec:** plan/27 §6 tables, §7 API (modules, `ColorTokens`, `Elevation::try_from_u8`, mandated test names).
- **Where:** `crates/multiplexer-theme/src/{lib.rs,tokens.rs}` (two files, not eight modules). No `tests/` harness, no proptest.
- **Evidence:**
  - Radius: crate `4/8/12/16/22` vs spec `4/6/8/12/20`. `panel_radius` uses `Radius::MD` (12), which happens to equal spec `lg`, so the name is wrong even when the pane looks right.
  - Motion: `90/160/240` vs `120/200/320`.
  - Type names: `CAPTION..HERO` vs `xs..xxl`.
  - Space: 1..6, missing 20px.
  - No `ColorTokens` nest, so `tokens.glass` vs `fn glass(e)` is the parse trap §7.5 warned about (they shipped `glass_at` instead).
  - Mandated test `dark_tokens_are_transparent_enough` is named `dark_glass_is_transparent_enough`. Missing: `dark_preserves_existing_desktop_glass`, `elevation_try_from_rejects_five`, contrast tests, shadow-weight test, all five properties.
  - Dark shipping numbers do not match §6.1 (`glass` 0.36 vs 0.52, `ink` 0.22 vs 0.35, `bg` 0.18 vs opaque 1.00). Those moves track plan/28 Acrylic more than plan/27 "preserve existing desktop." Neither spec is applied consistently.
- **Why it matters:** Two plans plus a third live table. Implementers cannot tell which number is source of truth. Mutation gates on the wrong names will not catch the mutants plan/27 §8.4 listed.
- **Fix:** Pick one table (recommend plan/28 alphas + plan/27 names/API). Rebuild the crate modules and mandated tests to match. Record the plan/27 vs plan/28 canvas conflict (`bg` opaque vs canvas wash) in DECISIONS.

### F11. plan/28 glass primitives never landed

- **Severity:** medium
- **Spec:** plan/28 §4.4 `GlassLayer` catalog, §5 `glass_pane` / `glass_bar` owned by `theme.rs`, §7.2 (`main.rs` only calls them), §8 unit tests on bands and `window_options()`.
- **Where:** `main.rs:1688-1704` still defines local `glass_pane` / `glass_bar`. No `GlassLayer`. No `highlight`. No `dim`. No `canvas` alias. No `window_options()` helper (options are inlined at `main.rs:2054`).
- **Evidence:** `glass_pane` is fill + hairline + rest shadow + `Radius::MD`. `glass_bar` is fill + `border_color` only (no guaranteed edge, no radius skip is explicit beyond callers passing `rounded_none`). Missing: inner highlight child, `GlassKind::{Pane, Bar, OverlayCard}`, layer ids (`window.canvas`, `chrome.toolbar`, `rail.left`, …). Adapter test is a single `adapter_glass_is_translucent` (`theme.rs:94-98`). None of the §8.1 names exist (`canvas_alpha_in_wash_band`, `pane_fills_in_glass_band`, `window_options_keep_native_caption`).
- **Why it matters:** F1 and F2 keep coming back because there is no layer catalog to hang tokens on. Caption is safe only by copy-paste in `main`, not by a tested helper.
- **Fix:** Extract `window_options()`. Move builders into `theme.rs` taking a `GlassLayer`. Add the §8.1 tests. Then F1's grep becomes a one-line CI gate.

---

## What is already fine

- Window contract: `WindowBackgroundAppearance::Blurred`, `appears_transparent: false`, `titlebar: Some(...)`, movable / resizable / minimizable (`main.rs:2054-2065`). Matches plan/28 §2.
- Crate is GPUI-free (plan/27 §3.2). Desktop is the only `hsla` / `px` / `BoxShadow` owner in intent.
- Dark `glass.a = 0.36 < 0.55`. `glass_ultra < glass < glass_strong`. `ink.a = 0.22` sits in the canvas wash band.
- Accent / good / warn / danger are distinct hues and are used for badges (`main.rs:1896-1900`).
- Status colors are not used as full-strip fills except Stop (`Theme::danger()` on the ghost when `label == "Stop"`).
- `Theme::faint()`, `warn()`, `selection()`, `surface()`, `glass_ultra()` are real call sites, not dead adapter wrappers.

---

## Suggested order

1. Delete illegal `hsla` (F1), especially approval `0.70` (F2).
2. Land `GlassLayer` + highlight + `window_options()` tests (F11).
3. Wire `UiSettings.mode` / `density` into `Theme::tokens()` (F4, F5).
4. Consume `bg` / `focus_ring` / `surface_raised` / type / space (F3, F6, F8).
5. Replace `glass_at` with the elevation table (F7) and move shadows into the crate (F9).
6. Reconcile plan/27 vs plan/28 numbers in one decision (F10).
