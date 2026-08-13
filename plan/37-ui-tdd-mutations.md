# 37: UI TDD and Mutations (Theme + Widget Spec)

**Status:** Implementation spec for the parent. Do not run cargo from the authoring subagent.
**Owner:** Parent implementer (`PARENT_IMPLEMENT`)
**Depends on:** `plan/10-ui-pane-system.md` §5 (design tokens), `plan/15-testing-strategy.md`, `docs/CI.md`, `docs/DECISIONS.md` (D13, D21, D33)
**Feeds:** workspace member list, `.cargo/mutants.toml` (already `crates/**`), Phase 2.6 design system
**Expected green command:** `cargo test -p multiplexer-theme -p multiplexer-shell --lib`

This doc is the testing contract for the **new theme crate** and the **pure widget / inspector spec** that must leave the desktop binary. TDD first (RED, then GREEN), then the parent runs cargo-mutants. The parent owns every cargo invocation.

**Locked decisions applied:**

- **D13:** consolidated `multiplexer-*` crates. Theme is `multiplexer-theme`. Widget catalog and inspector spec live in `multiplexer-shell`, not a third crate and not in `apps/`.
- **D21:** mutation scope is all core logic, including pane/chrome spec. Pure token math and widget catalogs are mutation targets. GPUI paint code is not.
- **D33:** 70% mutation kill is the historical merge floor. Library crates in this repo have been pushed to **100% viable kill** and **100% line** (`docs/CI.md`, README). These new surfaces land at that bar, not at 70%.

**Why this split exists.** `apps/**` is excluded from cargo-mutants (`.cargo/mutants.toml`). Today the glass token table lives in `apps/multiplexer-desktop/src/theme.rs` (GPUI `Hsla` / `BoxShadow`). The widget catalog and inspector button spec live in `apps/multiplexer-desktop/src/controls.rs` and `inspector.rs`. Those files already have strong unit tests, but they never see the mutation gate. Extract the pure data. Leave only a GPUI projector in the binary.

---

## 1. Crate split

| Home | Kind | Mutants | What it owns |
|---|---|---|---|
| `crates/multiplexer-theme` | **New** library. No GPUI, no `serde`, no I/O. | **Yes** (whole crate) | HSLA, named token sets, palettes, density spacing, radius, motion, shadow layers |
| `crates/multiplexer-shell` `widgets` | New module, pure | **Yes** (with the rest of shell) | `Surface`, `ControlSpec`, `REQUIRED_IDS`, catalog lookup, shortcut map |
| `crates/multiplexer-shell` `inspector_model` | New module, pure | **Yes** | `InspectorAction`, `InspectorButton`, `tab_buttons`, `inspector_body` |
| `apps/multiplexer-desktop` | GPUI binary | **No** (`apps/**` excluded) | Projector: theme tokens to `gpui::Hsla` / `BoxShadow`. Window paint. Host I/O. Bin tests only |

**Dependency direction.** `multiplexer-shell` does **not** depend on `multiplexer-theme`. Widget ids and inspector copy are structural. Tokens are visual. The desktop binary depends on both and is the only place they meet.

**Do not put in `multiplexer-theme`:** `gpui::*`, window chrome, fonts as platform handles, file loaders, user theme marketplace (plan/09: no marketplace in MVP).

**Do not put in `multiplexer-shell` widgets / inspector_model:** GPUI elements, click handlers, `Theme`, HSLA, layout tree mutations (those stay in `actions.rs` / `workspace.rs`).

**Stays in desktop:** `ShellView`, GPUI `div` trees, `Theme` as a thin facade over `Palette::dark()` (or the active palette), process spawn, clipboard, window options. After the move, `controls.rs` and the logic in `inspector.rs` are deleted from the app. `main.rs` imports catalog types from `multiplexer-shell`.

**Already in shell, out of this extract:** `Workspace`, `InspectorTab`, `ChromeLayout`, `ClientAction`, composer, palette filter, slash, terminal_ui. Those modules keep their existing tests. This plan does not rewrite them.

---

## 2. `multiplexer-theme` surface (pure)

New workspace member: `crates/multiplexer-theme`. Add it to root `Cargo.toml` `members`. Dev-dep: `proptest` (workspace). Runtime deps: none.

```
crates/multiplexer-theme/
  Cargo.toml
  src/lib.rs          // re-exports
  src/color.rs        // Hsla, ThemeError
  src/tokens.rs       // TokenSet, RadiusScale, MotionScale, ShadowLayer
  src/density.rs      // Density, space(step)
  src/palette.rs      // PaletteKind, Palette::{dark, light, high_contrast}
  tests/invariants.rs // property tests (alpha, density monotonic)
```

### 2.1 `Hsla` (`src/color.rs`)

Four `f32` channels in the **unit interval**, matching GPUI `hsla(h, s, l, a)` so the projector is a field copy, not a scale conversion.

```rust
pub struct Hsla { /* private fields */ }

impl Hsla {
    /// Rejects non-finite values and any channel outside `0.0..=1.0`.
    /// Do not clamp. Do not wrap hue. Rejection is what kills range mutants.
    pub fn new(h: f32, s: f32, l: f32, a: f32) -> Result<Self, ThemeError>;

    pub fn h(self) -> f32;
    pub fn s(self) -> f32;
    pub fn l(self) -> f32;
    pub fn a(self) -> f32;
}

pub enum ThemeError {
    OutOfRange { channel: Channel, value: f32 },
    NonFinite { channel: Channel },
}

pub enum Channel { H, S, L, A }
```

`new` is the only constructor. Palette literals call `new(...).expect("static token")` in the palette module, not in tests (tests use `unwrap` on known-good values and `is_err` on known-bad).

### 2.2 `TokenSet` (`src/tokens.rs`)

Named glass chrome plus the plan/10 groups that have numeric meaning today. Every field is a value type. No `gpui`.

**Color fields** (names match the current desktop `Theme` methods, which the projector keeps):

| Field | Dark (authoritative, current desktop) h,s,l,a |
|---|---|
| `glass` | 0.64, 0.16, 0.10, **0.52** |
| `glass_strong` | 0.64, 0.18, 0.12, **0.68** |
| `ink` | 0.64, 0.22, 0.06, **0.35** |
| `hairline` | 0.00, 0.00, 1.00, **0.10** |
| `hairline_bright` | 0.00, 0.00, 1.00, **0.18** |
| `text` | 0.62, 0.08, 0.92, **0.94** |
| `muted` | 0.62, 0.08, 0.72, **0.72** |
| `accent` | 0.58, 0.72, 0.62, **0.95** |
| `good` | 0.38, 0.55, 0.58, **0.95** |
| `send_bg` | 0.00, 0.00, 1.00, **0.11** |
| `danger` | 0.02, 0.68, 0.58, **0.95** |

**Light** (new, fixed literals so tests are not tautologies). Invert lightness, keep hues, keep unit alphas:

| Field | Light h,s,l,a |
|---|---|
| `glass` | 0.64, 0.08, 0.96, 0.72 |
| `glass_strong` | 0.64, 0.10, 0.98, 0.86 |
| `ink` | 0.64, 0.06, 0.94, 0.40 |
| `hairline` | 0.00, 0.00, 0.00, 0.12 |
| `hairline_bright` | 0.00, 0.00, 0.00, 0.22 |
| `text` | 0.62, 0.10, 0.14, 0.94 |
| `muted` | 0.62, 0.08, 0.32, 0.80 |
| `accent` | 0.58, 0.72, 0.42, 0.95 |
| `good` | 0.38, 0.55, 0.38, 0.95 |
| `send_bg` | 0.00, 0.00, 0.00, 0.08 |
| `danger` | 0.02, 0.68, 0.42, 0.95 |

**High contrast:** same hues as dark, but `text.a == 1.0`, `muted.a == 1.0`, `hairline.a == 0.40`, `hairline_bright.a == 0.70`, `glass.a == 0.92`, `glass_strong.a == 1.0`, `ink.a == 1.0`. `accent` / `good` / `danger` alphas stay `0.95` only if contrast against `ink` still differs by `l` of at least `0.40`; if not, raise `l` rather than invent a third hue.

**Radius** (px, `f32`): `sm = 4.0`, `md = 8.0`, `lg = 12.0`. `panel_radius()` returns `lg` (matches desktop `px(12.)`).

**Motion** (ms, `u16`, plan/10 §5.3): `fast = 120`, `medium = 200`, `slow = 320`.

**Shadow** (dark, two layers, matches desktop `Theme::shadow`):

1. color `(0.64, 0.30, 0.04, 0.45)`, offset `(0.0, 10.0)`, blur `28.0`, spread `-4.0`
2. color `(0.0, 0.0, 1.0, 0.04)`, offset `(0.0, 1.0)`, blur `0.0`, spread `0.0`

Light/high-contrast shadows: same geometry, recolored from that palette's `ink` / `hairline`. Tests pin **geometry** independently of color so a mutant that zeros blur or flips spread still dies.

```rust
pub struct ShadowLayer {
    pub color: Hsla,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
}

impl TokenSet {
    pub fn colors(&self) -> impl Iterator<Item = Hsla>; // every color field, stable order
    pub fn panel_radius(&self) -> f32;                  // == radius.lg
    pub fn shadows(&self) -> &[ShadowLayer];            // len == 2
}
```

### 2.3 `Density` (`src/density.rs`)

Plan/10 spacing is a 4px base scale (`space.1` ... `space.8`). Density scales the base. **Do not clamp** an out-of-range step; return `None`.

```rust
pub enum Density { Compact, Comfortable, Roomy }

impl Density {
    pub const fn all() -> [Density; 3];
    pub const fn base_px(self) -> u16;           // 3, 4, 6
    pub fn space(self, step: u8) -> Option<u16>; // Some(base * step) iff step in 1..=8
    pub fn rail_gap(self) -> u16;                // space(2).unwrap()
    pub fn pane_pad(self) -> u16;                // space(3).unwrap()
}
```

| Density | `base_px` |
|---|---|
| Compact | **3** |
| Comfortable | **4** (plan/10 default) |
| Roomy | **6** |

`Comfortable` is `Default`.

### 2.4 `Palette` (`src/palette.rs`)

```rust
pub enum PaletteKind { Dark, Light, HighContrast }

impl PaletteKind {
    pub const fn all() -> [PaletteKind; 3];
}

pub struct Palette { kind: PaletteKind, tokens: TokenSet }

impl Palette {
    pub fn dark() -> Self;
    pub fn light() -> Self;
    pub fn high_contrast() -> Self;
    pub fn of(kind: PaletteKind) -> Self;
    pub fn kind(&self) -> PaletteKind;
    pub fn tokens(&self) -> &TokenSet;
}
```

`SystemFollow` is a **host** concern (read OS, then pick `Dark` or `Light`). It does not live in this crate. A mutant that adds a fourth kind fails `PaletteKind::all().len() == 3`.

---

## 3. `multiplexer-shell` widgets + inspector_model (pure)

Move, do not copy. Desktop tests move with the code so there is one catalog.

### 3.1 `src/widgets.rs`

Lift `apps/multiplexer-desktop/src/controls.rs` almost as-is:

- `Surface` (10 variants, `all()` order is Outlook order)
- `ControlSpec` `{ id, surface, label, shortcut, action }`
- `REQUIRED_IDS` (39 ids, current checklist)
- `all_controls`, `control_by_id`, `controls_on`, `no_dead_labels`, `shortcut_map`
- `is_live` / `ControlSpec::is_live`

Keep the current 39-id table, surface pins, and shortcut map. Those literals are the mutation-hard oracle.

Re-export from `lib.rs`:

```rust
pub use widgets::{
    all_controls, control_by_id, controls_on, no_dead_labels, shortcut_map,
    ControlSpec, Surface, REQUIRED_IDS,
};
```

### 3.2 `src/inspector_model.rs`

Lift the **pure** half of `apps/multiplexer-desktop/src/inspector.rs`:

- `InspectorAction` (RefreshCores, RefreshMcp, RefreshGit, CreateCheckpoint, RevertCheckpoint, CycleModel, CopySession, RunGitStatus, NewWorktreeHint)
- `InspectorButton` `{ label, hint, action }`
- `tab_buttons(tab: InspectorTab) -> Vec<InspectorButton>`
- `inspector_body(ws: &Workspace, session_id: Option<&str>) -> String`

`InspectorTab` stays in `workspace.rs` (already exported, already tested). This module uses it.

Exact `tab_buttons` oracle (order is part of the contract):

| Tab | Buttons `(label, action)` |
|---|---|
| Session | `("Model", CycleModel)`, `("Copy", CopySession)` |
| Resources | `("Reload", RefreshCores)` |
| Mcp | `("Reload", RefreshMcp)` |
| Checkpoints | `("New", CreateCheckpoint)`, `("Revert", RevertCheckpoint)` |
| Git | `("Reload", RefreshGit)`, `("Status", RunGitStatus)`, `("New WT", NewWorktreeHint)` |
| Terminal | `[]` |
| Skills | `[]` |

`inspector_body` is a pure match onto existing `Workspace` detail methods. A mutant that swaps two arms dies only if the test compares against the **named method**, not against `inspector_body` twice.

Re-export from `lib.rs`:

```rust
pub use inspector_model::{inspector_body, tab_buttons, InspectorAction, InspectorButton};
```

### 3.3 Desktop after the move

- Delete `apps/multiplexer-desktop/src/controls.rs`.
- Delete the catalog / body logic from `inspector.rs`. Either delete the file and import from shell in `main.rs`, or leave a 5-line re-export module with **no** `#[cfg(test)]` (tests now live in the library).
- `theme.rs` becomes a projector:

```rust
fn to_hsla(c: multiplexer_theme::Hsla) -> gpui::Hsla {
    gpui::hsla(c.h(), c.s(), c.l(), c.a())
}
```

`Theme::glass()` etc. call `Palette::dark().tokens()` (until a theme switch lands). `Theme::shadow()` maps `ShadowLayer` to `gpui::BoxShadow`. `Theme::panel_radius()` is `px(tokens.panel_radius())`.

- Optional **bin-only** test in desktop `theme.rs`: `Theme::glass()` channels equal `Palette::dark().tokens().glass`. This is a projector smoke test. It is **not** a mutation target.

---

## 4. TDD sequence (parent)

Do not implement production code before the matching test exists. Confirm RED (compile fail or assertion fail for the right reason), then GREEN, then refactor.

1. Add empty `multiplexer-theme` crate + workspace member. `lib.rs` is a doc comment only.
2. Write `color.rs` tests first (`new` accepts unit interval, rejects `< 0`, `> 1`, `NaN`, `INFINITY` on **each** channel). Implement `Hsla`.
3. Write `tokens.rs` / `palette.rs` tests with the **literal tables in §2.2**. Implement `TokenSet` + three palettes.
4. Write `density.rs` tests (`base_px`, `space(1..=8)`, `space(0)` / `space(9)` are `None`). Implement `Density`.
5. Write `tests/invariants.rs` property tests (§6). They must fail if a palette ships `a = 1.1` or Compact spacing is not strictly below Roomy.
6. `cargo test -p multiplexer-theme --lib` green (parent).
7. Add `widgets.rs` / `inspector_model.rs` **tests first** by moving the desktop modules' `#[cfg(test)]` blocks, then the production items. Strengthen inspector tests to exact `Vec` equality (§5.3).
8. Point `lib.rs` re-exports. Delete desktop copies. Rewire `main.rs`.
9. `cargo test -p multiplexer-theme -p multiplexer-shell --lib` green (parent).
10. Parent runs clippy, then cargo-mutants on the new crate and on shell, then llvm-cov (§7).

---

## 5. Exact test modules and mutation-hard assertions

Co-located unit tests sit in `#[cfg(test)] mod tests` at the bottom of each module (plan/15 §2.1). Property tests live in `crates/multiplexer-theme/tests/invariants.rs`.

**Tautologies are a gate failure.** Forbidden:

- `assert_eq!(f(), f())`
- `assert_eq!(x, x)` / `assert!(x == x)`
- `assert!(result.is_ok())` without reading the `Ok` payload against a literal
- `assert!(s.contains(""))`
- `assert_ne!` between values that have different types or can never be equal
- Calling a function and asserting only that it did not panic
- Debug-format-only assertions
- A test that writes a value then reads the same binding back with no second observer

Every assertion below is chosen because a cargo-mutants operator flip, literal swap, or arm delete makes it fail.

### 5.1 `multiplexer_theme::color::tests`

| Test | Hard assertion (kills) |
|---|---|
| `unit_interval_round_trips` | `Hsla::new(0.25, 0.5, 0.75, 1.0).unwrap()` getters equal **those four literals**, not each other |
| `zero_and_one_are_accepted` | `new(0.0, 0.0, 0.0, 0.0)` and `new(1.0, 1.0, 1.0, 1.0)` succeed; `a()` is `0.0` vs `1.0` |
| `rejects_below_zero_each_channel` | `new(-0.01, 0.0, 0.0, 0.0)` is `Err` with `channel == Channel::H`; same for S, L, A with the negative in **that** slot only |
| `rejects_above_one_each_channel` | `new(1.01, 0.0, 0.0, 0.0)` is `Err` on `H`; same pattern for S, L, A |
| `rejects_nan_and_inf` | `NAN`, `INFINITY`, `NEG_INFINITY` on A (and one other channel) are `NonFinite` |
| `error_names_the_bad_channel` | swapping H vs A in the error would fail: bad A reports `Channel::A` and the **exact** rejected `value` |

Do not implement `new` as clamp. A clamp mutant (`1.01 -> 1.0`) would survive if tests only checked "some color comes back."

### 5.2 `multiplexer_theme::tokens::tests` and `palette::tests`

| Test | Hard assertion |
|---|---|
| `dark_glass_matches_shipped_chrome` | `dark.glass == Hsla::new(0.64, 0.16, 0.10, 0.52).unwrap()` (all four channels) |
| `dark_table_is_complete` | One assert per remaining dark field against §2.2. A mutant that changes `send_bg.a` from `0.11` to `0.10` dies (`0.10` is `hairline.a`) |
| `dark_related_tokens_differ` | `glass.a < glass_strong.a`; `hairline.a < hairline_bright.a`; `ink.l < glass.l`; `text.l > muted.l`; `send_bg != hairline`; `accent != good`; `good.h != danger.h` |
| `light_text_is_darker_than_canvas` | `light.text.l < light.glass.l` and `light.text.l < 0.5` |
| `dark_text_is_lighter_than_canvas` | `dark.text.l > dark.glass.l` and `dark.text.l > 0.5` |
| `light_table_is_complete` | Every light field equals §2.2 (literals, not `dark` inverted in the test) |
| `high_contrast_opaque_text` | `hc.text.a == 1.0`, `hc.muted.a == 1.0`, `hc.ink.a == 1.0`, `hc.hairline.a == 0.40` |
| `palettes_are_not_the_same_set` | `dark.tokens().text != light.tokens().text`; `dark.kind() == PaletteKind::Dark` |
| `palette_kind_all_is_three_in_order` | `all() == [Dark, Light, HighContrast]`; `of(k).kind() == k` for each |
| `panel_radius_is_lg_twelve` | `radius.sm == 4.0`, `md == 8.0`, `lg == 12.0`, `panel_radius() == 12.0`, `sm < md`, `md < lg` |
| `motion_is_plan10` | `fast == 120`, `medium == 200`, `slow == 320`, and `fast < medium < slow` |
| `dark_shadow_geometry` | `shadows().len() == 2`; layer0 `offset_y == 10.0`, `blur == 28.0`, `spread == -4.0`; layer1 `blur == 0.0`, `offset_y == 1.0`; layer0 color is `(0.64, 0.30, 0.04, 0.45)` |
| `colors_iter_length` | `colors().count() == 11` (the eleven named color fields). A dropped field dies. |

### 5.3 `multiplexer_theme::density::tests`

| Test | Hard assertion |
|---|---|
| `bases_are_3_4_6` | `Compact.base_px() == 3`, `Comfortable == 4`, `Roomy == 6` (exact, not only ordered) |
| `default_is_comfortable` | `Density::default() == Comfortable` |
| `space_is_base_times_step` | `Comfortable.space(1) == Some(4)`, `space(8) == Some(32)`, `Compact.space(8) == Some(24)`, `Roomy.space(1) == Some(6)` |
| `space_rejects_zero_and_nine` | `space(0) == None`, `space(9) == None`, `space(255) == None` for **each** density |
| `rail_gap_and_pane_pad` | `Comfortable.rail_gap() == 8`, `pane_pad() == 12`; Compact `6` / `9`; Roomy `12` / `18` |
| `all_is_three_in_order` | `[Compact, Comfortable, Roomy]` |

### 5.4 `multiplexer_shell::widgets::tests`

Move the desktop `controls.rs` tests. Keep these mutation-hard pins (already written; do not weaken):

| Test | Hard assertion |
|---|---|
| `all_required_ids_present` | `REQUIRED_IDS.len() == 39`, `all_controls().len() == 39`, every required id is present, `spec.action == spec.id`, `is_live` |
| `required_ids_live_on_their_surfaces` | The 39 `(id, Surface)` pins; `action == id` |
| `shortcuts_cover_palette` | `ctrl-k` and `ctrl-p` map to `command_palette`, **not** `palette_run` / `help`; palette surface has exactly `palette_filter`, `palette_run` |
| `no_empty_actions` / `no_dead_labels` | Empty and whitespace `id`/`label`/`action` are not live |
| `surfaces_nonempty` | `Surface::all().len() == 10` and the **per-surface counts**: TitleBar 5, LeftRail 3, Center 5, Composer 3, RightRail 15, TermStrip 2, Palette 2, HelpOverlay 1, ApprovalCard 2, ReminderBar 1 |
| `surface_match_is_exhaustive` | Tag map `0..9` in `all()` order |
| `shortcut_map_has_required_bindings` | `len() == 11`, each chord listed, `ctrl-shift-k` is `None` |
| `shortcut_targets_are_live_handlers` | Keys unique; `escape -> close_overlay` and `control_by_id("close_overlay")` is `None` |
| `control_by_id_known_and_unknown` | `send` equals the full `ControlSpec` literal; `""`, `"nope"`, `"SEND"`, `"send "` are `None`; `help != help_close` |
| `control_ids_are_unique` | sort+dedup length unchanged |
| `visible_labels_match_current_chrome` | Exact label strings (`"Chats"`, `"What can you do?"`, …) |
| `controls_on_does_not_leak_other_surfaces` | Composer ids `["send", "newline", "paste"]` only |
| `actions_are_snake_case_handler_names` | `[a-z0-9_]+`, starts with a letter, `action == id` |

### 5.5 `multiplexer_shell::inspector_model::tests`

Replace `any(|b| …)` with **exact vectors**. `any` leaves swap-order and extra-button mutants alive.

| Test | Hard assertion |
|---|---|
| `tab_buttons_exact_for_every_tab` | For each `InspectorTab`, `tab_buttons(tab)` equals the §3.2 list (labels **and** actions **and** hints **and** length). Terminal and Skills are `is_empty()` |
| `session_buttons_are_model_then_copy` | `[0].action == CycleModel`, `[1].action == CopySession`, `len() == 2`. `CycleModel != CopySession` |
| `git_buttons_are_three` | Reload / Status / New WT in that order |
| `inspector_body_matches_named_workspace_method` | After setting `ws.inspector`, `inspector_body(&ws, sid) == ws.session_detail(sid)` (and the six other methods). Compare to the **other** tab's method and `assert_ne!` so a swapped match arm dies |
| `inspector_body_session_none_vs_some` | `None` contains `"(none yet)"`; `Some("sess-1")` contains `"sess-1"` and is not equal to the `None` body |
| `inspector_action_tags_are_distinct` | Exhaustive `match` tag `0..8` on `InspectorAction::all()` (add `all()` if missing). Length 9 |

Keep a workspace fixture (`Workspace::new("demo", "grok")`) so body tests observe real copy, not empty strings.

### 5.6 What desktop bin tests may do

At most:

- Projector: `to_hsla(dark.glass)` has the same four floats as `Hsla::new(0.64, 0.16, 0.10, 0.52)`.
- Smoke: `REQUIRED_IDS.len() == all_controls().len()` (already in `ShellView::new`; keep it).

No mutant run on `apps/multiplexer-desktop`. Do not add a lib target to the desktop package to "get coverage."

---

## 6. Property tests

File: `crates/multiplexer-theme/tests/invariants.rs`. Use `proptest` with the workspace dep. Persist failures under `tests/invariants.proptest-regressions`.

### 6.1 Token alpha (and every channel) in `0.0..=1.0`

```rust
proptest! {
    #[test]
    fn constructed_alpha_stays_in_unit(a in 0.0f32..=1.0) {
        let c = Hsla::new(0.5, 0.5, 0.5, a).unwrap();
        prop_assert!((0.0..=1.0).contains(&c.a()));
        prop_assert_eq!(c.a(), a);
    }

    #[test]
    fn negative_alpha_is_rejected(a in -8.0f32..0.0) {
        prop_assert!(Hsla::new(0.5, 0.5, 0.5, a).is_err());
    }

    #[test]
    fn alpha_above_one_is_rejected(a in 1.0f32..8.0) {
        prop_assume!(a > 1.0);
        prop_assert!(Hsla::new(0.5, 0.5, 0.5, a).is_err());
    }
}
```

Plus a **non-proptest** sweep (property over the finite catalog, not a generator):

```rust
#[test]
fn every_builtin_token_channel_is_unit() {
    for kind in PaletteKind::all() {
        for color in Palette::of(kind).tokens().colors() {
            for ch in [color.h(), color.s(), color.l(), color.a()] {
                assert!((0.0..=1.0).contains(&ch), "{kind:?} {ch}");
            }
        }
    }
}
```

`prop_assert_eq!(c.a(), a)` is what kills a mutant that clamps a valid `0.3` to `0.0` or replaces `a` with `1.0`.

### 6.2 Density spacing is strictly monotonic

Two independent orders: **step** (within one density) and **density** (across Compact / Comfortable / Roomy).

```rust
proptest! {
    #[test]
    fn spacing_grows_with_step(step in 1u8..8) {
        for d in Density::all() {
            let lo = d.space(step).unwrap();
            let hi = d.space(step + 1).unwrap();
            prop_assert!(lo < hi);
            prop_assert_eq!(hi - lo, d.base_px());
        }
    }

    #[test]
    fn spacing_grows_with_density(step in 1u8..=8) {
        let c = Density::Compact.space(step).unwrap();
        let m = Density::Comfortable.space(step).unwrap();
        let r = Density::Roomy.space(step).unwrap();
        prop_assert!(c < m && m < r);
        prop_assert_eq!(c, 3 * step as u16);
        prop_assert_eq!(m, 4 * step as u16);
        prop_assert_eq!(r, 6 * step as u16);
    }
}
```

`hi - lo == base_px()` kills `*` replaced by `+` and `step` replaced by `1`. Exact `3/4/6 * step` kills a mutant that assigns Compact the Comfortable base.

Also unit-test `space(8) < space(9)` is not a thing: `space(9)` is `None`, so monotonicity is only claimed on `1..=8`.

---

## 7. Parent cargo commands (order, single thread)

Parent only. Same order as `docs/CI.md`, narrowed to these packages. **One test thread** so failures are readable and mutant reruns are deterministic.

```text
1. fmt
   cargo fmt --check

2. clippy (deny warnings)
   cargo clippy -p multiplexer-theme -p multiplexer-shell --all-targets -- -D warnings

3. unit + property   (this is expected_cmd, plus the thread pin)
   cargo test -p multiplexer-theme -p multiplexer-shell --lib -- --test-threads=1

4. mutation (100% viable kill; after GREEN only)
   CARGO_INCREMENTAL=0 cargo mutants -p multiplexer-theme --in-place --timeout 30 --jobs 1
   CARGO_INCREMENTAL=0 cargo mutants -p multiplexer-shell --in-place --timeout 30 --jobs 1

5. coverage (100% line on these libs)
   cargo llvm-cov -p multiplexer-theme -p multiplexer-shell --lib --fail-under-lines 100 -- --test-threads=1
```

During the extract, a faster mutants loop on just the new shell modules is allowed:

```text
CARGO_INCREMENTAL=0 cargo mutants -p multiplexer-shell --file src/widgets.rs --file src/inspector_model.rs --in-place --timeout 30 --jobs 1
```

The merge gate is still **the whole** `-p multiplexer-shell` crate. Existing shell modules must stay at 100% viable kill.

**Do not** pass `--test-threads` to cargo-mutants (it is not a cargo-mutants flag). `--jobs 1` is the mutants parallelism pin.

**Do not** run mutants on `multiplexer-desktop`. `examine_globs = ["crates/**/*.rs"]` already excludes `apps/**`.

**Bar:** 70% is the historical floor (D33). These crates ship at **100% viable kill, zero survivors**, and **100% line**. A survivor is fixed by a stronger assertion, never by `exclude_re` unless the mutant is a proven no-op (document it in `.cargo/mutants.toml` the way `Containment::close` is documented).

---

## 8. Mutation targets (what must die)

High-value mutants the suite above is written to kill:

| Mutant class | Killer |
|---|---|
| `Hsla::new` drops one bound (`a > 1` kept, `a < 0` dropped) | per-channel below-zero / above-one tests |
| `Hsla::new` clamps instead of rejecting | reject tests + `prop_assert_eq!(c.a(), a)` |
| Swap `glass` / `glass_strong` alphas | exact literals + `glass.a < glass_strong.a` |
| `send_bg.a` `0.11` -> `0.10` | exact table + `send_bg != hairline` |
| `Density` bases `3,4,6` -> `4,4,6` | exact bases + `c < m < r` + `3 * step` |
| `space` range `1..=8` -> `0..=8` or `1..=9` | `space(0)` / `space(9)` are `None` |
| `*` -> `+` in `base * step` | `Comfortable.space(8) == Some(32)` and `hi - lo == base` |
| `tab_buttons` Session arms swapped | exact `[CycleModel, CopySession]` order |
| `tab_buttons` Git drops New WT | `len() == 3` + exact third action |
| `inspector_body` Terminal arm calls `skills_detail` | `assert_eq!(body, ws.terminal_detail())` and `assert_ne!(body, ws.skills_detail())` |
| `REQUIRED_IDS` loses an id | `len() == 39` and contains-each |
| `control_by_id` becomes case-insensitive | `"SEND"` is `None` |
| Shortcut `ctrl-p` retargeted to help | `shortcut_action("ctrl-p") == Some("command_palette")` and `!= Some("help")` |
| Shadow `spread` `-4` -> `4` | exact geometry pin |
| `PaletteKind::all` drops HighContrast | `len() == 3` and array equality |

---

## 9. Out of scope

- GPUI component / snapshot / e2e gates (plan/15 §3). Those stay later, on the binary, not on these libs.
- Theme marketplace, user CSS, OKLCH conversion, runtime theme files.
- Moving `InspectorTab` or `Workspace` detail formatters. Only the **router** and **button catalog** move.
- Changing the 39-id product checklist. This plan relocates it so mutants can see it.
- Running cargo from any authoring subagent.

---

## 10. Parent implement checklist

```text
[ ] crates/multiplexer-theme added to workspace members
[ ] Tests written before production items (RED observed)
[ ] cargo fmt --check
[ ] cargo clippy -p multiplexer-theme -p multiplexer-shell --all-targets -- -D warnings
[ ] cargo test -p multiplexer-theme -p multiplexer-shell --lib -- --test-threads=1
[ ] cargo mutants -p multiplexer-theme  (100% viable kill, --jobs 1)
[ ] cargo mutants -p multiplexer-shell  (100% viable kill, --jobs 1)
[ ] cargo llvm-cov -p multiplexer-theme -p multiplexer-shell --lib --fail-under-lines 100
[ ] Desktop projector only; controls.rs / inspector logic gone from apps/
[ ] No GPUI types in either library crate
[ ] No em dash in new comments or docs
```

---

**PARENT_IMPLEMENT**

- **files:** `plan/37-ui-tdd-mutations.md` (this spec), then `crates/multiplexer-theme/**`, `crates/multiplexer-shell/src/widgets.rs`, `crates/multiplexer-shell/src/inspector_model.rs`, `crates/multiplexer-shell/src/lib.rs`, `Cargo.toml`, `apps/multiplexer-desktop/src/{main.rs,theme.rs}` (delete `controls.rs` / inspector logic)
- **expected_cmd:** `cargo test -p multiplexer-theme -p multiplexer-shell --lib`
