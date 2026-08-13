# 27: Theme Tokens (Dark-First Glass)

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Desktop design system / `multiplexer-theme`
**Depends on:** `00-vision-and-principles.md`, `02-architecture.md`, `10-ui-pane-system.md`, `15-testing-strategy.md`, `16-performance.md`
**Feeds:** `09-editor.md` (chrome vs syntax split), `13-mobile-app.md` (semantic names only), `19-roadmap-and-milestones.md` (deliverable 2.6)

This document is consistent with `docs/PLAN-CONTEXT.md` and `docs/DECISIONS.md`. Where a
decision is not yet locked, it is listed under **Open questions** and is **not** decided
unilaterally here. New decisions proposed here are numbered **D77+**; they are proposals
for the decision log, not locked decisions.

**Locked decisions applied (D1, D2, D13, D21, D33):**
- **D1:** Rust + GPUI 0.2.2, GPU-rendered. Tokens exist so the desktop shell never invents
  raw colors. The token crate itself does **not** depend on GPUI.
- **D2:** Mobile is Expo / React Native. Mobile may later mirror the *names* in this spec.
  It does not import this crate. No JSON theme export in v1.
- **D13:** consolidated `multiplexer-*` crates. This is `crates/multiplexer-theme`.
- **D21 / D33:** token math, constructors, density, and elevation are core logic. They are
  unit + property + mutation targets. 70% mutation score is the merge floor.

**Relationship to plan/10 §5:** plan/10 sketched a design system (`bg.canvas`, `space.1`…).
This doc is the **authoritative token spec**. Implementers follow the names, values, and
API here. plan/10 remains the pane/shell spec; it should consume these tokens, not redefine
them.

**Relationship to plan/09:** editor *syntax* palettes (tree-sitter scopes) stay in the
editor. This crate is **chrome**: panes, bars, type, space, motion, elevation. A future
editor theme may *read* accent/good/warn/danger from here so status colors match the shell.

---

## 1. Problem statement

`apps/multiplexer-desktop/src/theme.rs` is not a product. It is ten `hsla(...)` helpers,
one radius, and one two-layer shadow. There is no light theme, no density, no elevation
ramp, no type scale, no motion vocabulary, no warn color, no selection, no focus ring,
and no way to test any of it without spinning GPUI.

That is a prototype wash, not a design system. Every new pane will keep inventing
one-off colors (`Theme::send_bg`, `Theme::muted` vs `text_muted`, magic `p_3`) and the
Outlook shell will drift.

The product promise is **dark-first glass**: indigo-slate chrome, translucent panes over
a deep ink canvas, a cool accent, GPU motion, Windows-first. Light must exist as a
second mode, not a weekend invert. Density must be a first-class switch (Comfortable /
Compact). Elevation must be a monotonic glass ramp, not "pick glass or glass_strong."

This doc specifies a **headless** token crate so TDD, cargo-mutants, and CI coverage
gates apply *before* any GPUI pixel is drawn.

---

## 2. Goals and non-goals

### 2.1 Goals (v1)

1. Ship a pure crate `crates/multiplexer-theme` with **no `gpui` dependency**.
2. Represent every color as `HslaTuple { h, s, l, a }` with `f32` components in `[0.0, 1.0]`,
   matching GPUI 0.2.2 `hsla(h, s, l, a)` so the desktop adapter is a field-wise copy.
3. Publish exact shipping values for **dark glass** (the default, the brand).
4. Publish a complete **light** table as `ThemeMode::Light`. Light must differ from dark
   in every semantic color (see tests).
5. Density: `Comfortable` (default) and `Compact`. Compact shrinks the space scale.
6. Elevation `0..=4` with `fn glass(elevation) -> HslaTuple` and **strictly monotonic
   alpha**.
7. Radius `xs/sm/md/lg/xl`, space `4/8/12/16/20/24/32`, type `11/12/13/14/16/20/24`.
8. The full semantic color set listed in §4.1. No more, no fewer, as *required* fields.
9. Motion: durations `fast/normal/slow` and **easing names only**. No GPUI
   `AnimationCurve` in this crate.
10. Named shadows `rest` / `hover` / `float`.
11. Desktop `theme.rs` becomes a thin GPUI adapter (hsla, px, BoxShadow).
12. Tests listed in §8 ship with the crate. They run headless.

### 2.2 Non-goals (v1)

See §11 for the full out-of-scope list. Headline: **no user-uploaded themes**, no theme
marketplace, no CSS/JSON import, no Mica/Acrylic OS backdrop requirement, no syntax
highlighting palette, no high-contrast mode, no third `ThemeMode` for system-follow
(system-follow is a preference that *resolves* to Dark or Light in the adapter).

---

## 3. Design principles

### 3.1 Dark-first glass

Dark glass is the shipping identity. Hue family **0.64** (about 230°) is indigo-slate
chrome. Text sits slightly cooler at **0.62**. Accent is a distinct cyan-blue at **0.58**,
never the same hue as `good` (0.38) or `warn` (0.11) or `danger` (0.02).

Glass is real transparency (`a < 1.0`), not a flat dark grey named "glass." The named
`glass` token **must** satisfy `a < 0.55` in dark mode (test: `dark_tokens_are_transparent_enough`).
Panes stack: `ink` / `bg` canvas, then `glass_ultra` washes, then `glass` panes, then
`glass_strong` bars, then elevation 4 overlays.

### 3.2 Headless tokens, GPU later

`multiplexer-layout` and `multiplexer-shell` already prove the pattern: **no GPUI types
in the pure crate**. `multiplexer-theme` follows that law. Tests and cargo-mutants run
in CI without a window, a GPU, or `gpui` linked.

The desktop binary is the only crate that calls `gpui::hsla`, `px`, `point`, `BoxShadow`.

### 3.3 One vocabulary, two modes

Components never mention raw HSLA. They ask the tokens for `color.text`, `space.s12`,
`radius.md`, `motion.duration_fast_ms`, `shadows.rest`. Switching `ThemeMode` or
`Density` is a single struct rebuild. No component opt-in.

### 3.4 Windows-first, painted glass

v1 **paints** glass with translucent HSLA over an opaque `bg` (and optional `ink` wash).
We do not require DWM Mica, Acrylic, or a transparent OS window in GPUI 0.2.2. If a later
spike enables real window backdrop blur, these alphas are already correct and tests stay
valid. Do not block the token crate on compositor features.

### 3.5 Contrast is an invariant, not a vibe

Dark: `text.l - bg.l >= 0.70`. Light: `bg.l - text.l >= 0.70`. `text_faint` is decorative
and may fail that guard. Status hues are separated so accent is never a stand-in for good
or danger. This is an L-channel proxy, not a full WCAG sRGB implementation (out of v1).

### 3.6 Motion is a budget

Durations live in the 120 / 200 / 320 ms band from plan/10 §5.3 so they fit the <16 ms
input-latency budget (plan/16): animation *values* interpolate, input is never blocked.
Easing is a name. The adapter maps names to GPUI curves later. `reduce_motion` is a
boolean on the motion struct, default `false`; the adapter sets it from the OS.

---

## 4. Token taxonomy

All values below are **normative** for v1. Changing a shipping number is a spec change,
not a drive-by tweak.

### 4.1 Semantic colors (required fields)

| Field | Role |
|---|---|
| `bg` | Opaque window canvas. Glass composites over this. |
| `surface` | Opaque lifted region (lists, inspector body) when glass is wrong. |
| `surface_raised` | One step above `surface` (selected row fill, raised card). |
| `glass` | Default pane fill. Dark `a < 0.55`. Equals elevation 2. |
| `glass_strong` | Bars, composer chrome, title/status. Equals elevation 3. |
| `glass_ultra` | Most transparent wash (scrims, idle overlays). Equals elevation 0. |
| `ink` | Deepest wash / brand black. May be translucent. |
| `text` | Primary foreground. |
| `text_muted` | Secondary labels, timestamps, section caps. |
| `text_faint` | Disabled, placeholders, decorative. |
| `accent` | Interactive brand (active tab, links, focus companion). |
| `accent_muted` | Accent at rest (idle pill, unread wash). |
| `good` | Success, running-healthy, apply-hunk. |
| `warn` | Waiting, degraded, approval needed. |
| `danger` | Error, reject, destructive. |
| `hairline` | Default 1 px border. |
| `hairline_bright` | Hovered / focused border. |
| `selection` | Text and row selection fill. |
| `focus_ring` | Keyboard focus outline. Distinct from `hairline`. |

No other *required* color fields in v1. Adapter-only aliases (`send_bg`, old `muted`)
are defined in §9.3 and must not leak into this crate.

### 4.2 Elevation (0..=4)

Elevation is a glass *alpha ramp*, not a z-index integer in the layout tree.

| Level | Use | Dark alpha | Named alias |
|---|---|---|---|
| 0 | Ultra wash, idle overlay, empty ghost slot | 0.22 | `glass_ultra` |
| 1 | Recessed pane, collapsed rail | 0.36 | (unnamed) |
| 2 | Default pane (`glass_pane` today) | 0.52 | `glass` |
| 3 | Title bar, status bar, composer | 0.68 | `glass_strong` |
| 4 | Palette, popover, modal, popped chrome | 0.84 | (unnamed) |

`ThemeTokens::glass(elevation)` returns the tuple for that level. Alpha is **strictly
increasing** with elevation in both modes (`elevation_monotonic_alpha`). Hue stays in
the chrome family; only S/L/A step.

Invalid levels (`5`, wrapping) are rejected: `Elevation::try_from(u8)` returns `Err`.
A `saturating` constructor clamps to 0..=4 for UI code that must not fail; both paths
are tested so mutants cannot collapse them.

### 4.3 Density

| Density | Intent |
|---|---|
| `Comfortable` | Default. Space scale is exactly 4 / 8 / 12 / 16 / 20 / 24 / 32. |
| `Compact` | Power-user. Every space step is `<=` Comfortable, and at least `s4` and `s32` are strictly smaller. |

Density does **not** change colors, type, radius, motion, or shadows in v1. That keeps
`density_compact_shrinks_space` a clean assertion and stops mutants from "fixing" compact
by also shrinking type.

### 4.4 Radius

Shared across modes and densities. Units are CSS-like px, stored as `f32`.

| Name | px | Use |
|---|---|---|
| `xs` | 4 | Chips, tiny badges |
| `sm` | 6 | Inputs, ghost buttons |
| `md` | 8 | Menus, inner cards |
| `lg` | 12 | Panes (today's `Theme::panel_radius`) |
| `xl` | 20 | Palette, modal shell |

Radius is monotonic: `xs < sm < md < lg < xl`.

### 4.5 Space (Comfortable)

| Name | px |
|---|---|
| `s4` | 4 |
| `s8` | 8 |
| `s12` | 12 |
| `s16` | 16 |
| `s20` | 20 |
| `s24` | 24 |
| `s32` | 32 |

Compact (normative):

| Name | px |
|---|---|
| `s4` | 2 |
| `s8` | 6 |
| `s12` | 8 |
| `s16` | 12 |
| `s20` | 16 |
| `s24` | 20 |
| `s32` | 24 |

### 4.6 Type scale

Shared across modes and densities. Sizes are px.

| Name | px | Use |
|---|---|---|
| `xs` | 11 | Section caps ("CHATS"), badges |
| `sm` | 12 | Status bar, meta |
| `md` | 13 | Secondary body |
| `base` | 14 | Default UI body (today's `text_sm` stand-in) |
| `lg` | 16 | Composer, pane titles |
| `xl` | 20 | Empty-state title |
| `xxl` | 24 | Rare display (hello frame, modal title) |

Line-height multipliers (not font sizes): `tight = 1.20`, `normal = 1.35`, `loose = 1.50`.

Font *names* (adapter maps to GPUI `Font`):

| Role | Windows-first family | Fallback |
|---|---|---|
| `ui` | `Segoe UI Variable` | `Segoe UI` |
| `mono` | `Cascadia Mono` | `Consolas` |

This crate stores the names as `&'static str`. It does not load fonts.

### 4.7 Motion

| Token | Value |
|---|---|
| `duration_fast_ms` | 120 |
| `duration_normal_ms` | 200 |
| `duration_slow_ms` | 320 |
| `easing_standard` | `EaseOut` |
| `easing_emphasized` | `EaseInOut` |
| `easing_enter` | `EaseOut` |
| `easing_exit` | `EaseIn` |
| `reduce_motion` | `false` by default |

`EasingName` is an enum. **No GPUI types.** `fast < normal < slow` is a test.

### 4.8 Named shadows

Each name is **two layers**: a colored drop + a 1 px hairline highlight (the current
`Theme::shadow()` recipe). Offsets, blur, spread are px `f32`.

| Name | Intent |
|---|---|
| `rest` | Default pane / bar at rest |
| `hover` | Lifted pane, hovered card |
| `float` | Palette, popover, detached window chrome |

Blur is strictly increasing: `rest.blur < hover.blur < float.blur` on layer 0.
Offset-y is non-decreasing. This is how `shadow_float_is_heavier_than_rest` kills
"swap the names" mutants.

---

## 5. Color science and HSLA convention

### 5.1 Unit HSLA (GPUI 0.2.2)

GPUI 0.2.2 `hsla(h, s, l, a)` takes four `f32` values in **unit range**:

- `h`: hue as a fraction of 360°. `0.64 ≈ 230.4°` (indigo). `1.0` and `0.0` are the same hue.
- `s`: saturation `[0, 1]`.
- `l`: lightness `[0, 1]`.
- `a`: alpha `[0, 1]`.

`HslaTuple` is a pure copy of that convention. The adapter is:

```rust
gpui::hsla(t.h, t.s, t.l, t.a)
```

Do **not** store degrees (0..360) or 0..255 channels. That would force a conversion
and break the "field-wise copy" law.

### 5.2 Type

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HslaTuple {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

impl HslaTuple {
    pub const fn new(h: f32, s: f32, l: f32, a: f32) -> Self {
        Self { h, s, l, a }
    }

    pub fn clamp(self) -> Self { /* each channel to [0, 1] */ }

    pub fn components(self) -> (f32, f32, f32, f32) {
        (self.h, self.s, self.l, self.a)
    }

    pub fn approx_eq(self, other: Self, eps: f32) -> bool { /* max abs diff <= eps */ }
}
```

Shipping constructors emit already-clamped literals. `clamp` exists so malformed
callers cannot take the process down; tests assert shipping tokens do not need it.

`PartialEq` is bitwise on `f32`. Visual tests use `approx_eq` with `eps = 1e-6`.

### 5.3 Why not `gpui::Hsla` in the crate

1. Headless CI: `gpui` pulls wgpu, windowing, and a heavy build graph.
2. Mutation and proptest stay milliseconds.
3. Mobile / docs / snapshots can depend on names without linking a desktop renderer.
4. The current `theme.rs` already treats HSLA as four floats. We are extracting that
   truth, not wrapping GPUI.

### 5.4 Contrast proxy

v1 does **not** implement sRGB relative luminance. We assert:

- Dark: `text.l - bg.l >= 0.70` and `text.a >= 0.90`.
- Light: `bg.l - text.l >= 0.70` and `text.a >= 0.90`.
- `accent.h`, `good.h`, `warn.h`, `danger.h` pairwise differ by `>= 0.05`.

A full WCAG module is a later crate concern (open question Q3).

---

## 6. Exact token tables

HSLA written as `(h, s, l, a)`. These numbers are the shipping source of truth.

Hue notes for the dark family (so reviewers can see the brand, not a random palette):

- Chrome / glass / ink / bg: **0.64**
- Text: **0.62**
- Accent / selection / focus: **0.58**
- Good: **0.38**
- Warn: **0.11**
- Danger: **0.02**
- Neutral hairlines: **0.00** hue, **0.00** sat (white at low alpha)

### 6.1 Dark glass (shipping default)

Preserve today's desktop look. `glass`, `glass_strong`, `ink`, `hairline`,
`hairline_bright`, `text`, `text_muted` (was `muted`), `accent`, `good`, `danger`
match `apps/multiplexer-desktop/src/theme.rs` as of this spec. New tokens fill the
gaps without shifting the already-shipped chrome.

| Token | H | S | L | A | Notes |
|---|---|---|---|---|---|
| `bg` | 0.64 | 0.24 | 0.045 | 1.00 | Opaque canvas. New. Window fill should use this, not `ink`. |
| `surface` | 0.64 | 0.18 | 0.090 | 1.00 | Opaque lifted. |
| `surface_raised` | 0.64 | 0.16 | 0.125 | 1.00 | Selected row / raised card. |
| `glass` | 0.64 | 0.16 | 0.10 | 0.52 | **Existing.** `a < 0.55`. Elevation 2. |
| `glass_strong` | 0.64 | 0.18 | 0.12 | 0.68 | **Existing.** Elevation 3. |
| `glass_ultra` | 0.64 | 0.14 | 0.08 | 0.22 | New. Elevation 0. More transparent than `glass`. |
| `ink` | 0.64 | 0.22 | 0.06 | 0.35 | **Existing.** Wash, not the opaque canvas. |
| `text` | 0.62 | 0.08 | 0.92 | 0.94 | **Existing.** |
| `text_muted` | 0.62 | 0.08 | 0.72 | 0.72 | **Existing** `Theme::muted`. |
| `text_faint` | 0.62 | 0.08 | 0.62 | 0.48 | New. |
| `accent` | 0.58 | 0.72 | 0.62 | 0.95 | **Existing.** |
| `accent_muted` | 0.58 | 0.40 | 0.48 | 0.55 | New. |
| `good` | 0.38 | 0.55 | 0.58 | 0.95 | **Existing.** |
| `warn` | 0.11 | 0.72 | 0.58 | 0.95 | New. Amber. Same L/A as good/danger. |
| `danger` | 0.02 | 0.68 | 0.58 | 0.95 | **Existing.** |
| `hairline` | 0.00 | 0.00 | 1.00 | 0.10 | **Existing.** |
| `hairline_bright` | 0.00 | 0.00 | 1.00 | 0.18 | **Existing.** |
| `selection` | 0.58 | 0.55 | 0.50 | 0.28 | New. Accent-tinted wash. |
| `focus_ring` | 0.58 | 0.80 | 0.66 | 0.90 | New. Brighter than accent. |

Dark elevation ramp (`ThemeTokens::dark().glass(e)`):

| Elevation | H | S | L | A |
|---|---|---|---|---|
| 0 | 0.64 | 0.14 | 0.08 | 0.22 |
| 1 | 0.64 | 0.15 | 0.09 | 0.36 |
| 2 | 0.64 | 0.16 | 0.10 | 0.52 |
| 3 | 0.64 | 0.18 | 0.12 | 0.68 |
| 4 | 0.64 | 0.20 | 0.14 | 0.84 |

Invariants: `glass(0) == glass_ultra`, `glass(2) == glass`, `glass(3) == glass_strong`.

Dark shadows (layer 0 = drop, layer 1 = hairline highlight):

| Name | Layer | Color | offset_x | offset_y | blur | spread |
|---|---|---|---|---|---|---|
| `rest` | 0 | (0.64, 0.30, 0.04, 0.45) | 0 | 10 | 28 | -4 |
| `rest` | 1 | (0.00, 0.00, 1.00, 0.04) | 0 | 1 | 0 | 0 |
| `hover` | 0 | (0.64, 0.32, 0.04, 0.52) | 0 | 14 | 36 | -4 |
| `hover` | 1 | (0.00, 0.00, 1.00, 0.06) | 0 | 1 | 0 | 0 |
| `float` | 0 | (0.64, 0.34, 0.03, 0.60) | 0 | 22 | 48 | -6 |
| `float` | 1 | (0.00, 0.00, 1.00, 0.08) | 0 | 1 | 0 | 0 |

`rest` layer 0 matches today's `Theme::shadow()` drop. Hover and float are new, heavier.

### 6.2 Light (second ThemeMode)

Light is **frosted paper**, not an automatic invert. Glass stays translucent. Text is
dark indigo. Accent/good/warn/danger darken (lower L) so they hold contrast on paper.

| Token | H | S | L | A |
|---|---|---|---|---|
| `bg` | 0.62 | 0.08 | 0.96 | 1.00 |
| `surface` | 0.62 | 0.10 | 0.98 | 1.00 |
| `surface_raised` | 0.62 | 0.08 | 1.00 | 1.00 |
| `glass` | 0.62 | 0.12 | 0.98 | 0.48 |
| `glass_strong` | 0.62 | 0.10 | 0.96 | 0.72 |
| `glass_ultra` | 0.62 | 0.10 | 1.00 | 0.22 |
| `ink` | 0.64 | 0.18 | 0.18 | 0.08 |
| `text` | 0.64 | 0.22 | 0.14 | 0.92 |
| `text_muted` | 0.64 | 0.12 | 0.32 | 0.72 |
| `text_faint` | 0.64 | 0.10 | 0.42 | 0.48 |
| `accent` | 0.58 | 0.70 | 0.42 | 0.95 |
| `accent_muted` | 0.58 | 0.35 | 0.55 | 0.45 |
| `good` | 0.38 | 0.58 | 0.38 | 0.95 |
| `warn` | 0.10 | 0.78 | 0.42 | 0.95 |
| `danger` | 0.01 | 0.72 | 0.44 | 0.95 |
| `hairline` | 0.64 | 0.10 | 0.20 | 0.12 |
| `hairline_bright` | 0.64 | 0.12 | 0.18 | 0.20 |
| `selection` | 0.58 | 0.50 | 0.60 | 0.22 |
| `focus_ring` | 0.58 | 0.75 | 0.42 | 0.90 |

Light elevation ramp:

| Elevation | H | S | L | A |
|---|---|---|---|---|
| 0 | 0.62 | 0.10 | 1.00 | 0.22 |
| 1 | 0.62 | 0.11 | 0.99 | 0.34 |
| 2 | 0.62 | 0.12 | 0.98 | 0.48 |
| 3 | 0.62 | 0.10 | 0.96 | 0.72 |
| 4 | 0.62 | 0.10 | 0.94 | 0.88 |

Light shadows (dark drop, not a white glow pretending to be depth):

| Name | Layer | Color | offset_x | offset_y | blur | spread |
|---|---|---|---|---|---|---|
| `rest` | 0 | (0.64, 0.20, 0.20, 0.12) | 0 | 8 | 20 | -2 |
| `rest` | 1 | (0.00, 0.00, 1.00, 0.60) | 0 | 1 | 0 | 0 |
| `hover` | 0 | (0.64, 0.22, 0.18, 0.16) | 0 | 12 | 28 | -2 |
| `hover` | 1 | (0.00, 0.00, 1.00, 0.70) | 0 | 1 | 0 | 0 |
| `float` | 0 | (0.64, 0.24, 0.16, 0.22) | 0 | 18 | 40 | -4 |
| `float` | 1 | (0.00, 0.00, 1.00, 0.80) | 0 | 1 | 0 | 0 |

`light_differs_from_dark` compares every semantic color field with `!=` (and
`approx_eq` must be false at `eps = 1e-6`). Shadows and elevation ramps also differ.

### 6.3 Shared scales (both modes)

Type, radius, motion durations, easing names: identical for dark and light.

Space: chosen by `Density`, not by `ThemeMode`.

---

## 7. Rust API sketch

Crate: `crates/multiplexer-theme`.
Workspace member. `default-features = false`. Dependencies: **none** on the lib
target. Dev-deps: `proptest`. Optional `serde` feature if prefs persistence wants
to serialize `ThemeMode` / `Density` (not required to land the crate).

No `gpui`. No `windows`. No filesystem.

### 7.1 Layout

```
crates/multiplexer-theme/
  Cargo.toml
  src/
    lib.rs          // re-exports
    hsla.rs         // HslaTuple
    elevation.rs    // Elevation, ThemeError
    density.rs      // Density, SpaceScale, RadiusScale
    motion.rs       // MotionTokens, EasingName
    shadow.rs       // ShadowLayer, NamedShadows
    typography.rs   // TypeScale, FontNames, LineHeight
    color.rs        // ColorTokens
    tokens.rs       // ThemeTokens, ThemeMode
  tests/
    tokens.rs       // required integration-style tests
    elevation.rs
    density.rs
    contrast.rs
```

Co-located `#[cfg(test)]` modules on each file cover constructors and error paths.
`tests/` holds the five mandated tests plus property suites so cargo-mutants has a
second crate-level harness (same pattern as `multiplexer-layout`).

### 7.2 Enums and newtypes

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    pub fn is_dark(self) -> bool { matches!(self, Self::Dark) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Density {
    Comfortable,
    Compact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Elevation {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
}

impl Elevation {
    pub const MIN: u8 = 0;
    pub const MAX: u8 = 4;

    pub fn try_from_u8(level: u8) -> Result<Self, ThemeError> { /* 0..=4 or Err */ }
    pub fn saturating(level: u8) -> Self { /* clamp 0..=4 */ }
    pub fn as_u8(self) -> u8 { self as u8 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EasingName {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeError {
    ElevationOutOfRange { got: u8 },
}
```

`ThemeMode` has **two** variants. System-follow is not a variant (D77).

### 7.3 Scales

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpaceScale {
    pub s4: f32,
    pub s8: f32,
    pub s12: f32,
    pub s16: f32,
    pub s20: f32,
    pub s24: f32,
    pub s32: f32,
}

impl SpaceScale {
    pub fn comfortable() -> Self { /* 4,8,12,16,20,24,32 */ }
    pub fn compact() -> Self { /* 2,6,8,12,16,20,24 */ }
    pub fn get(self, step: SpaceStep) -> f32 { /* match */ }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceStep { S4, S8, S12, S16, S20, S24, S32 }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadiusScale {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

impl RadiusScale {
    pub fn standard() -> Self { /* 4,6,8,12,20 */ }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeScale {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub base: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}

impl TypeScale {
    pub fn standard() -> Self { /* 11,12,13,14,16,20,24 */ }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineHeight {
    pub tight: f32,   // 1.20
    pub normal: f32,  // 1.35
    pub loose: f32,   // 1.50
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontNames {
    pub ui: &'static str,
    pub ui_fallback: &'static str,
    pub mono: &'static str,
    pub mono_fallback: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionTokens {
    pub duration_fast_ms: u16,
    pub duration_normal_ms: u16,
    pub duration_slow_ms: u16,
    pub easing_standard: EasingName,
    pub easing_emphasized: EasingName,
    pub easing_enter: EasingName,
    pub easing_exit: EasingName,
    pub reduce_motion: bool,
}

impl MotionTokens {
    pub fn standard() -> Self { /* 120, 200, 320, names as §4.7 */ }
    pub fn duration_ms(self, speed: MotionSpeed) -> u16 { /* match */ }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionSpeed { Fast, Normal, Slow }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowLayer {
    pub color: HslaTuple,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowStack {
    pub drop: ShadowLayer,
    pub hairline: ShadowLayer,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NamedShadows {
    pub rest: ShadowStack,
    pub hover: ShadowStack,
    pub float: ShadowStack,
}
```

### 7.4 Colors and ThemeTokens

Semantic colors live in `ColorTokens` so `ThemeTokens::glass(elevation)` does not
collide with a field named `glass` at the call site (`tokens.glass` vs `tokens.glass(e)`
is a Rust parse trap; see §7.5).

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorTokens {
    pub bg: HslaTuple,
    pub surface: HslaTuple,
    pub surface_raised: HslaTuple,
    pub glass: HslaTuple,
    pub glass_strong: HslaTuple,
    pub glass_ultra: HslaTuple,
    pub ink: HslaTuple,
    pub text: HslaTuple,
    pub text_muted: HslaTuple,
    pub text_faint: HslaTuple,
    pub accent: HslaTuple,
    pub accent_muted: HslaTuple,
    pub good: HslaTuple,
    pub warn: HslaTuple,
    pub danger: HslaTuple,
    pub hairline: HslaTuple,
    pub hairline_bright: HslaTuple,
    pub selection: HslaTuple,
    pub focus_ring: HslaTuple,
}

impl ColorTokens {
    pub fn dark() -> Self { /* §6.1 table */ }
    pub fn light() -> Self { /* §6.2 table */ }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeTokens {
    pub mode: ThemeMode,
    pub density: Density,
    pub color: ColorTokens,
    pub radius: RadiusScale,
    pub space: SpaceScale,
    pub typography: TypeScale,
    pub line_height: LineHeight,
    pub fonts: FontNames,
    pub motion: MotionTokens,
    pub shadows: NamedShadows,
}

impl ThemeTokens {
    pub fn dark() -> Self {
        Self::new(ThemeMode::Dark, Density::Comfortable)
    }

    pub fn light() -> Self {
        Self::new(ThemeMode::Light, Density::Comfortable)
    }

    pub fn new(mode: ThemeMode, density: Density) -> Self { /* assemble */ }

    /// Glass fill for an elevation. Alpha is strictly monotonic in `elevation`.
    pub fn glass(&self, elevation: Elevation) -> HslaTuple { /* ramp for mode */ }
}
```

### 7.5 Why `color.glass` and `fn glass(elevation)`

The user-facing API is `ThemeTokens::dark()`, `ThemeTokens::light()`, and
`fn glass(elevation) -> HslaTuple`. Named pane glass is `tokens.color.glass`.

Do **not** put a field `pub glass: HslaTuple` on `ThemeTokens`. In Rust,
`tokens.glass(e)` would then try to call the field. Nesting colors is the clean fix.

### 7.6 `lib.rs` surface

Public surface is small and explicit:

```rust
//! Headless design tokens for Multiplexer chrome.
//!
//! No GPUI types live here. The desktop binary maps [`HslaTuple`] with
//! `gpui::hsla(h, s, l, a)`.

mod color;
mod density;
mod elevation;
mod hsla;
mod motion;
mod shadow;
mod tokens;
mod typography;

pub use color::ColorTokens;
pub use density::{Density, RadiusScale, SpaceScale, SpaceStep};
pub use elevation::{Elevation, ThemeError};
pub use hsla::HslaTuple;
pub use motion::{EasingName, MotionSpeed, MotionTokens};
pub use shadow::{NamedShadows, ShadowLayer, ShadowStack};
pub use tokens::{ThemeMode, ThemeTokens};
pub use typography::{FontNames, LineHeight, TypeScale};
```

Keep `pub use` 1:1 with the types above. Do not glob-export.

### 7.7 Workspace wiring (implementer checklist)

When the crate is created (not in this planning change):

1. Add `"crates/multiplexer-theme"` to root `Cargo.toml` `members`.
2. Add `multiplexer-theme = { path = "crates/multiplexer-theme" }` under
   `[workspace.dependencies]`.
3. Depend from `apps/multiplexer-desktop` only. `multiplexer-shell` stays
   GPUI-free and does **not** need this crate for Phase 0 chrome state.
4. Edition / license / repository from workspace keys, same as
   `crates/multiplexer-layout`.

### 7.8 What components call

| Surface | Token |
|---|---|
| Window fill | `color.bg` (migrate off `Theme::ink()` as the canvas) |
| Optional brand wash | `color.ink` over `bg` |
| Default pane | `glass(Elevation::Two)` + `color.hairline` + `shadows.rest` + `radius.lg` |
| Title / status / composer | `glass(Elevation::Three)` or `color.glass_strong` |
| Collapsed rail | `glass(Elevation::One)` |
| Palette / popover | `glass(Elevation::Four)` + `shadows.float` + `radius.xl` |
| Primary text | `color.text` |
| Caps / meta | `color.text_muted`, type `xs` |
| Placeholder | `color.text_faint` |
| Active tab, send (ready) | `color.accent` |
| Idle accent wash | `color.accent_muted` |
| Healthy / apply | `color.good` |
| Approval / wait | `color.warn` |
| Error / reject | `color.danger` |
| Selection | `color.selection` |
| Keyboard focus | `color.focus_ring` (1 px), not `hairline_bright` alone |
| Ghost button fill | adapter alias `send_bg` (white 0.11). Not a crate field. |

---

## 8. TDD

TDD at inception (plan/15, D21, D33). Write the tests in this section **first**. The
constructors exist to make them pass. cargo-mutants on this crate is in scope.

### 8.1 Mandated tests (must exist with these names)

These five names are part of the contract with the parent implementer.

#### `dark_tokens_are_transparent_enough`

```rust
#[test]
fn dark_tokens_are_transparent_enough() {
    let t = ThemeTokens::dark();
    assert!(t.color.glass.a < 0.55);
    assert!(t.color.glass_ultra.a < t.color.glass.a);
    assert!(t.glass(Elevation::Two).a < 0.55);
    assert_eq!(t.glass(Elevation::Two), t.color.glass);
}
```

Kills: "make glass opaque", "swap ultra and glass", "elevation 2 != named glass".

#### `light_differs_from_dark`

```rust
#[test]
fn light_differs_from_dark() {
    let d = ThemeTokens::dark();
    let l = ThemeTokens::light();
    assert_ne!(d.mode, l.mode);
    assert_ne!(d.color, l.color);
    assert_ne!(d.color.bg, l.color.bg);
    assert_ne!(d.color.text, l.color.text);
    assert_ne!(d.color.glass, l.color.glass);
    assert_ne!(d.color.ink, l.color.ink);
    assert_ne!(d.shadows, l.shadows);
    // Light text is darker (lower L) than dark text.
    assert!(l.color.text.l < d.color.text.l);
}
```

Kills: `light()` returning `dark()`, shared color struct, invert-by-comment-only.

#### `density_compact_shrinks_space`

```rust
#[test]
fn density_compact_shrinks_space() {
    let comfy = ThemeTokens::new(ThemeMode::Dark, Density::Comfortable);
    let compact = ThemeTokens::new(ThemeMode::Dark, Density::Compact);
    assert_eq!(comfy.space.s4, 4.0);
    assert_eq!(comfy.space.s32, 32.0);
    assert!(compact.space.s4 < comfy.space.s4);
    assert!(compact.space.s32 < comfy.space.s32);
    for (c, k) in comfy.space.steps().zip(compact.space.steps()) {
        assert!(k <= c);
    }
    // Density does not recolor.
    assert_eq!(comfy.color, compact.color);
}
```

Add `SpaceScale::steps(&self) -> impl Iterator<Item = f32>` so this is not seven
copy-pasted asserts that mutants can delete one of.

#### `elevation_monotonic_alpha`

```rust
#[test]
fn elevation_monotonic_alpha() {
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        let t = ThemeTokens::new(mode, Density::Comfortable);
        let mut prev = -1.0;
        for level in 0..=4 {
            let e = Elevation::try_from_u8(level).unwrap();
            let a = t.glass(e).a;
            assert!(a > prev, "mode={mode:?} elev={level}");
            prev = a;
        }
        assert_eq!(t.glass(Elevation::Zero), t.color.glass_ultra);
        assert_eq!(t.glass(Elevation::Two), t.color.glass);
        assert_eq!(t.glass(Elevation::Three), t.color.glass_strong);
    }
}
```

Kills: flat ramp, reversed ramp, named aliases drifting from the function.

#### `accent_not_equal_good`

```rust
#[test]
fn accent_not_equal_good() {
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        let c = ThemeTokens::new(mode, Density::Comfortable).color;
        assert_ne!(c.accent, c.good);
        assert_ne!(c.accent, c.warn);
        assert_ne!(c.accent, c.danger);
        assert_ne!(c.good, c.warn);
        assert_ne!(c.good, c.danger);
        assert_ne!(c.warn, c.danger);
        assert!((c.accent.h - c.good.h).abs() >= 0.05);
    }
}
```

Kills: reuse accent for status, collapse the status set to one hue.

### 8.2 Additional unit tests (crate must ship)

| Test | Assertion |
|---|---|
| `hsla_shipping_tokens_in_unit_range` | Every channel of every semantic color, both modes, in `[0, 1]`. |
| `dark_preserves_existing_desktop_glass` | Dark `glass/glass_strong/ink/text/accent/good/danger/hairline` match §6.1 exactly. |
| `comfortable_space_matches_spec` | `4,8,12,16,20,24,32`. |
| `compact_space_matches_spec` | `2,6,8,12,16,20,24`. |
| `radius_is_monotonic` | `xs < sm < md < lg < xl` and `lg == 12.0`. |
| `type_scale_matches_spec` | `11,12,13,14,16,20,24`. |
| `type_scale_identical_across_modes` | Dark type == light type. |
| `motion_fast_lt_normal_lt_slow` | `120 < 200 < 320`. |
| `shadow_float_is_heavier_than_rest` | `float.drop.blur > hover.drop.blur > rest.drop.blur`. |
| `elevation_try_from_rejects_five` | `try_from_u8(5)` is `Err`; `saturating(5) == Four`; `saturating` != `try_from`. |
| `elevation_try_from_accepts_zero_through_four` | All five Ok. |
| `dark_text_contrasts_bg` | `text.l - bg.l >= 0.70`. |
| `light_text_contrasts_bg` | `bg.l - text.l >= 0.70`. |
| `focus_ring_not_equal_hairline` | Both modes. |
| `reduce_motion_defaults_false` | `MotionTokens::standard().reduce_motion == false`. |
| `theme_mode_is_dark_helper` | `Dark.is_dark()`, `!Light.is_dark()`. |

### 8.3 Property tests (`proptest`)

| Property | Forall |
|---|---|
| `glass_alpha_increases_with_elevation` | mode in {Dark, Light}, levels `a < b` in 0..=4 ⇒ `glass(a).a < glass(b).a` |
| `hsla_clamp_idempotent` | four floats ⇒ `clamp(clamp(x)) == clamp(x)` and channels in `[0,1]` |
| `density_never_grows_space` | mode, each step: compact <= comfortable |
| `saturating_elevation_in_range` | any `u8` ⇒ `saturating(n).as_u8() <= 4` |
| `new_mode_matches_constructor` | `new(Dark, d).color == dark().color` when `d` is Comfortable |

Do not property-test exact shipping literals. Those are unit tests. Properties protect
the *laws*.

### 8.4 Mutation notes

High-value mutants this suite must kill:

- Flip a comparison (`<` to `<=` on elevation alpha: use strict `<` and distinct alphas).
- Return `Dark` from `light()`.
- Copy Comfortable into Compact.
- Set `glass.a = 1.0`.
- Reuse `accent` for `good`.
- `try_from_u8` treating 5 as Four (the saturating/try split).
- Swap shadow names.
- Change `panel` radius `lg` from 12 to 8 (assert `== 12.0`).

Target: ≥85% line, ≥80% branch, ≥70% mutation killed on this crate (D21/D33).

### 8.5 What is *not* tested here

- GPUI snapshot pixels. That is a desktop component test after the adapter lands.
- OS `prefers-reduced-motion` probing. Adapter integration.
- Font availability on the machine.
- Theme switch cross-fade. plan/10 / plan/16.

---

## 9. Desktop `theme.rs` becomes a thin GPUI adapter

Today: `apps/multiplexer-desktop/src/theme.rs` owns values and types (`Hsla`,
`BoxShadow`, `Pixels`). After this crate lands, it **must not** own values.

### 9.1 Mapping law

```
HslaTuple { h, s, l, a }  →  gpui::hsla(h, s, l, a)
f32 px                    →  gpui::px(v)
ShadowLayer               →  gpui::BoxShadow { color, offset: point(px(x), px(y)),
                                               blur_radius: px(blur),
                                               spread_radius: px(spread) }
ShadowStack               →  vec![drop, hairline]
```

No color math in the adapter. No "tweak alpha by 0.02" at the call site. If a value
is wrong, change the table in `multiplexer-theme` and the tests.

### 9.2 Target shape

```rust
//! GPUI adapter over `multiplexer-theme`. No shipping literals live here.

use gpui::{hsla, point, px, BoxShadow, Hsla, Pixels};
use multiplexer_theme::{
    Elevation, HslaTuple, NamedShadows, ShadowStack, ThemeTokens,
};

pub fn to_hsla(c: HslaTuple) -> Hsla {
    hsla(c.h, c.s, c.l, c.a)
}

pub fn to_px(v: f32) -> Pixels {
    px(v)
}

pub fn to_box_shadows(stack: ShadowStack) -> Vec<BoxShadow> {
    fn layer(l: multiplexer_theme::ShadowLayer) -> BoxShadow {
        BoxShadow {
            color: to_hsla(l.color),
            offset: point(px(l.offset_x), px(l.offset_y)),
            blur_radius: px(l.blur),
            spread_radius: px(l.spread),
        }
    }
    vec![layer(stack.drop), layer(stack.hairline)]
}

#[derive(Clone)]
pub struct Theme {
    pub tokens: ThemeTokens,
}

impl Theme {
    pub fn dark() -> Self { Self { tokens: ThemeTokens::dark() } }
    pub fn light() -> Self { Self { tokens: ThemeTokens::light() } }

    pub fn glass(&self) -> Hsla { to_hsla(self.tokens.color.glass) }
    pub fn glass_at(&self, e: Elevation) -> Hsla { to_hsla(self.tokens.glass(e)) }
    pub fn glass_strong(&self) -> Hsla { to_hsla(self.tokens.color.glass_strong) }
    pub fn glass_ultra(&self) -> Hsla { to_hsla(self.tokens.color.glass_ultra) }
    pub fn ink(&self) -> Hsla { to_hsla(self.tokens.color.ink) }
    pub fn bg(&self) -> Hsla { to_hsla(self.tokens.color.bg) }
    pub fn text(&self) -> Hsla { to_hsla(self.tokens.color.text) }
    pub fn muted(&self) -> Hsla { to_hsla(self.tokens.color.text_muted) }
    pub fn accent(&self) -> Hsla { to_hsla(self.tokens.color.accent) }
    pub fn good(&self) -> Hsla { to_hsla(self.tokens.color.good) }
    pub fn warn(&self) -> Hsla { to_hsla(self.tokens.color.warn) }
    pub fn danger(&self) -> Hsla { to_hsla(self.tokens.color.danger) }
    pub fn hairline(&self) -> Hsla { to_hsla(self.tokens.color.hairline) }
    pub fn hairline_bright(&self) -> Hsla { to_hsla(self.tokens.color.hairline_bright) }
    pub fn panel_radius(&self) -> Pixels { to_px(self.tokens.radius.lg) }
    pub fn shadow(&self) -> Vec<BoxShadow> { to_box_shadows(self.tokens.shadows.rest) }
    pub fn shadow_named(&self, s: &NamedShadows) -> /* pick */ Vec<BoxShadow> { /* ... */ }

    /// Legacy ghost-button fill. Not a semantic token. Keep out of the crate.
    pub fn send_bg(&self) -> Hsla { hsla(0.0, 0.0, 1.0, 0.11) }
}
```

`send_bg` stays an adapter alias so `main.rs` does not churn on day one. A later pass
replaces it with `color.surface_raised` or a real `ghost_fill` token (open question Q2).

### 9.3 Migration (three commits, not one bomb)

1. **Land `multiplexer-theme` + tests.** Desktop unchanged. CI green on the new crate.
2. **Adapter delegates, statics remain as `Theme::glass()` → `Theme::dark().glass()`**
   for one cycle so `main.rs` still compiles. Mark statics `#[deprecated]` or just
   reimplement them as `ThemeTokens::dark()` wrappers.
3. **Thread `Theme` (or `ThemeTokens`) through `ShellView`.** Window fill uses `bg`
   (opaque), not `ink` as the only canvas. Delete static wrappers once call sites
   take `&self.theme`.

Do not redesign the Outlook layout in the same change. Tokens first.

### 9.4 Where the live `Theme` lives

`ShellView` owns `theme: Theme`. A later command (`Switch theme`, plan/10 palette)
rebuilds it (`Theme::light()` / `Theme::dark()`) and `cx.notify()`. Density is the
same rebuild with `ThemeTokens::new(mode, Density::Compact)`.

System-follow: adapter reads the OS appearance once at startup and on the
`WM_SETTINGCHANGE` / GPUI appearance callback, then picks `Dark` or `Light`. The
token crate never sees `System`.

### 9.5 Component tests after the adapter

Desktop / `multiplexer-ui` (when that crate exists) snapshot:

- Dark pane uses `glass.a < 0.55` (read back from the element style if GPUI exposes it;
  otherwise assert the `Theme` handed to the element).
- Light snapshot differs from dark (image or style hash).
- Compact shrinks a known gap (left rail padding).

Those tests are **not** part of `multiplexer-theme`. They are listed so Phase 2.6
(plan/19) has an acceptance hook.

---

## 10. How this sits in the workspace

```
crates/multiplexer-theme     pure tokens (this spec)
crates/multiplexer-layout    pure layout tree (already GPUI-free)
crates/multiplexer-shell     pure chrome state (already GPUI-free)
apps/multiplexer-desktop     GPUI adapter + projection
```

`multiplexer-theme` does not depend on layout or shell. Layout/shell do not depend
on theme. The desktop binary is the composition root for both trees **and** tokens.
That keeps headless tests isolated and matches D13's "consolidated crates, clear
ownership."

plan/19 deliverable **2.6 Design system** consumes this crate. The crate itself can
land in Phase 1 (it is test-only value until the adapter switches) without waiting
for the editor.

Mobile (D2) may copy the *names* into a TS theme later. No codegen in v1.

---

## 11. Out of scope (v1)

Explicitly **not** in this crate or in the first adapter pass:

1. **User-uploaded themes.** No file drop, no JSON/TOML theme, no URL import, no
   "paste a Zed theme." v1 is two hand-tuned modes. A marketplace is a product, not
   a token table.
2. **Theme marketplace / community packs.**
3. **Runtime theme editor** (sliders for hue). Prefs may *toggle* mode and density
   only.
4. **High-contrast mode.** plan/09 mentions it for the editor. Not a `ThemeMode`
   here. If we add it, it is a third constructed table plus its own contrast tests.
5. **`ThemeMode::System`.** Resolver lives in the adapter.
6. **Syntax / tree-sitter palettes.** plan/09.
7. **Terminal ANSI / 256 / truecolor tables.** plan/08. The terminal may *borrow*
   `text` / `bg` / `accent` later; it does not live here.
8. **OS backdrop materials** (Mica, Acrylic, vibrant, blur-behind). Painted glass only.
9. **GPUI animation playback.** Names and durations only.
10. **Full WCAG sRGB contrast engine.**
11. **Per-pane theme overrides.**
12. **Animated theme interpolation in the token crate** (no `lerp` of two
    `ThemeTokens` in v1). The adapter may cross-fade at the GPUI layer.
13. **serde as a required dependency.** Optional feature only.
14. **Changing Outlook layout, pane engine, or editor** in the same change.
15. **i18n of font picks** beyond the Windows-first family + fallback.

If a future spec wants user themes, it must: schema-validate every required field,
re-run the five mandated tests as *acceptance against the loaded table*, and reject
palettes that fail `glass.a < 0.55` for a theme that claims to be dark glass. That
is v2+.

---

## 12. Proposed decisions (D77+)

### D77. Two ThemeModes only; system-follow is a preference (PROPOSED)

`ThemeMode` is `Dark | Light`. Dark is the default shipping mode. An OS
"follow system" switch lives in desktop prefs and resolves to one of those two
before `ThemeTokens::new` is called.

### D78. `multiplexer-theme` is GPUI-free (PROPOSED)

Tokens are `HslaTuple` and `f32` px. The only legal GPUI conversion is the desktop
adapter. Matches `multiplexer-layout` / `multiplexer-shell`.

### D79. User-uploaded themes are out of v1 (PROPOSED)

No import path, no marketplace. Two constructed tables plus density.

### D80. Dark glass numbers preserve the current desktop chrome (PROPOSED)

`glass`, `glass_strong`, `ink`, `text`, `text_muted`, `accent`, `good`, `danger`,
`hairline`, `hairline_bright`, and the rest drop-shadow are the values already in
`theme.rs`. New tokens extend; they do not restyle the shipping window in the first
PR.

---

## 13. Open questions

Q1. Should Compact also tighten `radius.lg` (panes) from 12 to 8? v1 says no
    (colors/type/radius stay put). Revisit if Compact feels like "same chrome,
    smaller gaps" is not enough.

Q2. Promote `send_bg` (`hsla(0,0,1,0.11)`) to a semantic `ghost_fill`, or keep it
    as an adapter alias forever?

Q3. When do we replace the L-channel contrast proxy with real WCAG against an
    assumed opaque `bg` backing? Not v1.

Q4. Does the editor syntax theme take `accent/good/warn/danger` from this crate
    in Phase 2.1, or stay fully independent until a third pass?

Q5. Optional `serde` feature on `ThemeMode` + `Density` for `~/.multiplexer/ui.toml`,
    or store those two enums in an existing prefs crate later?

None of these block landing `crates/multiplexer-theme` with the tables and tests
in this doc.

---

## 14. Implementation order (for the parent)

1. Create `crates/multiplexer-theme` with the types in §7 and **empty** constructors.
2. Write the five mandated tests plus `elevation_try_from_rejects_five`. They fail.
3. Fill `ColorTokens::dark/light`, elevation ramps, space, radius, type, motion,
   shadows from §6. Tests pass.
4. Add property tests and the rest of §8.2. Run cargo-mutants on this crate only.
5. Wire the workspace member. Depend from `apps/multiplexer-desktop`.
6. Rewrite `theme.rs` as the adapter (§9). Keep static wrappers one cycle.
7. Switch `ShellView` window fill to `bg`. Do not restyle the world.
8. Stop. Layout, inspector, and editor work stay out of this change.

---

## 15. Consistency notes

- Does not contradict D1 (GPUI UI), D13 (crate names), D21/D33 (mutation), D2
  (mobile is a different stack).
- plan/10 §5.1 examples (`bg.canvas`, `space.1`) are superseded by the names here.
  Implementers of the pane system use `color.bg`, `space.s4`, `radius.lg`.
- plan/09 syntax themes remain a separate table. No marketplace here either.
- plan/16 motion budget: 120 to 320 ms, input never blocked.
- plan/19 item 2.6 is the design-system milestone this crate unblocks.

---

PARENT_IMPLEMENT
files: plan/27-theme-tokens.md
first_code: crates/multiplexer-theme
tests: dark_tokens_are_transparent_enough, light_differs_from_dark, density_compact_shrinks_space, elevation_monotonic_alpha, accent_not_equal_good, hsla_shipping_tokens_in_unit_range, dark_preserves_existing_desktop_glass, comfortable_space_matches_spec, compact_space_matches_spec, radius_is_monotonic, type_scale_matches_spec, type_scale_identical_across_modes, motion_fast_lt_normal_lt_slow, shadow_float_is_heavier_than_rest, elevation_try_from_rejects_five, elevation_try_from_accepts_zero_through_four, dark_text_contrasts_bg, light_text_contrasts_bg, focus_ring_not_equal_hairline, reduce_motion_defaults_false, theme_mode_is_dark_helper, glass_alpha_increases_with_elevation (proptest), hsla_clamp_idempotent (proptest), density_never_grows_space (proptest), saturating_elevation_in_range (proptest), new_mode_matches_constructor (proptest)
