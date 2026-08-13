# 31: Component Kit (headless specs + GPUI projection)

**Status:** Implementation spec (consistent with `docs/PLAN-CONTEXT.md` and `docs/DECISIONS.md`)
**Owner:** Desktop chrome / design system
**Depends on:** `plan/10-ui-pane-system.md` (§5 tokens, §8 components), live `multiplexer-shell`, live `apps/multiplexer-desktop`
**Feeds:** parent rewrite of `apps/multiplexer-desktop/src/widgets.rs`, later snapshot tests in Phase 2.6
**Locked decisions applied:** D1 (Rust + GPUI, not Electron), D13 (consolidated `multiplexer-*` crates), D21 (mutation covers UI logic that is pure), D33 (70% mutation score is the merge floor).

This slice exists so the app stops looking like unlabeled divs. The contract is **headless first**. GPUI is a projection, not the source of truth.

---

## 1. Problem

The desktop binary already paints Outlook chrome (`apps/multiplexer-desktop/src/main.rs`) and a glass token set (`theme.rs`). Controls are catalogued in `apps/multiplexer-desktop/src/controls.rs`. What is missing is a **shared widget vocabulary**:

- Title-bar actions, inspector actions, and composer Send all go through one `ghost_btn` that special-cases `"Stop"` and `"Send"` by label string.
- Thread rows, inspector tabs, chips, and the empty center are ad-hoc `div()` trees with no shared height, hover, selected, or focus rules.
- Empty states are a single muted sentence. No title, no body hierarchy, no action.
- There is no badge/pill tone model, so status colors are invented per call site.

`plan/10` §5 already requires typed design tokens and a component architecture. This doc specifies the **first seven primitives** as pure Rust specs in `multiplexer-shell`, with one GPUI function each in the desktop binary. A new `multiplexer-theme` crate is **not** created in this slice. Color tokens stay in `theme.rs`. Interaction, copy, height, and equality live in the shell so CI stays headless.

`controls.rs` stays the **catalog of which controls exist**. This kit is **how a painted control looks**. Do not merge the two files.

---

## 2. Placement

| Layer | Path | Allowed types |
|---|---|---|
| Headless specs + unit tests | `crates/multiplexer-shell/src/widgets.rs` | Rust only. No `gpui` dependency. |
| Re-exports | `crates/multiplexer-shell/src/lib.rs` | `mod widgets;` plus `pub use widgets::{...}` |
| GPUI projection | `apps/multiplexer-desktop/src/widgets.rs` | One function per spec. Parent writes this file. |
| Color tokens | `apps/multiplexer-desktop/src/theme.rs` | Existing `Theme::*` methods. Map token **names** from the specs. |

D13 names a future `multiplexer-ui` crate. The live tree uses `multiplexer-shell` (pure chrome) plus `apps/multiplexer-desktop` (GPUI). This slice follows the live tree. Do not add a crate.

`first_code` for the parent: `crates/multiplexer-shell/src/widgets.rs` (types, constructors, derived strings, unit tests). Then wire `mod widgets` in `lib.rs`. Then the desktop projection.

---

## 3. Shared tokens

All heights are **integer pixels** (`u16`) so tests never float-compare.

```
pub const HEIGHT_COMPACT: u16 = 32; // ghost, icon, pill, badge, tab
pub const HEIGHT_ROW:     u16 = 36; // list row, drawer header, search field
pub const HEIGHT_COMFORT: u16 = 44; // primary, danger, empty-state action
```

These are the only height tokens in the kit. Title bar (48) and rail collapse (`RAIL_COLLAPSED` = 36 in `workspace.rs`) stay chrome, not widget, constants.

### 3.1 Presence

Empty-after-trim is absent:

```
pub fn present(s: &str) -> bool {
    !s.is_empty() && s.chars().any(|c| !c.is_whitespace())
}
```

Used for icon, hint, subtitle, meta, action_label, placeholder, query, badge text, tab count suffix.

### 3.2 Tone

Shared by `PillSpec` and `BadgeSpec`.

```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tone {
    Neutral,
    Accent,
    Good,
    Warn,
    Danger,
}
```

| Variant | `label()` (exact) | `fill_token()` (exact) | Meaning |
|---|---|---|---|
| `Neutral` | `neutral` | `tone.neutral` | Default chip, idle meta |
| `Accent` | `accent` | `tone.accent` | Selection, model, focus-adjacent |
| `Good` | `good` | `tone.good` | Running well, connected, allowed |
| `Warn` | `warn` | `tone.warn` | Waiting, approval, degraded |
| `Danger` | `danger` | `tone.danger` | Error, deny, interrupt |

`label()` is lowercase English. `fill_token()` is the GPUI mapper key. A mutant that ignores `tone` must change `BadgeSpec::caption()`.

### 3.3 Visual state

Shared resolver for every interactive spec. Hover is **not** stored on the spec (it is pointer state). The resolver takes it as an argument so tests can pin priority.

```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisualState {
    Idle,
    Hover,
    Focus,
    Selected,
    Busy,
    Disabled,
}

pub fn resolve_state(
    enabled: bool,
    busy: bool,
    selected: bool,
    focused: bool,
    hovered: bool,
) -> VisualState
```

**Priority (highest first):** `!enabled` → `Disabled`; else `busy` → `Busy`; else `selected` → `Selected`; else `focused` → `Focus`; else `hovered` → `Hover`; else `Idle`.

| State | `fill_token()` | Hover | Focus ring | Cursor |
|---|---|---|---|---|
| Idle | `surface.idle` | yes, if enabled and not busy | no | pointer if interactive |
| Hover | `surface.hover` | (is hover) | no | pointer |
| Focus | `surface.focus` | yes, but focus wins over hover | yes, 1px accent outside | pointer |
| Selected | `surface.selected` | no further brighten | yes only if also focused | pointer |
| Busy | `surface.busy` | no | no | default (not pointer) |
| Disabled | `surface.disabled` | no | no | default |

**Focus ring (words):** a 1px accent hairline drawn **outside** the control bounds when `focused && enabled && !busy`. Keyboard focus only. Mouse down does not by itself show the ring. The headless helper is:

```
pub fn shows_focus_ring(enabled: bool, busy: bool, focused: bool) -> bool {
    enabled && !busy && focused
}
```

**Selected (words):** selected fill plus the bright hairline (`Theme::hairline_bright` in the projection). Selected wins over focus and hover for **fill**. A selected+focused row still shows the focus ring on top of the selected fill.

**Hover (words):** idle fill lifts to `surface.hover`. Disabled and busy ignore hover. Selected ignores hover (no double highlight).

**Busy (words):** keep the kind/tone fill, replace the label via `shown_label()`, ignore clicks. No hover lift. No focus ring.

**Disabled (words):** 40% opacity in the projection, idle fill token, no hover, no ring, not interactive.

```
impl VisualState {
    pub fn fill_token(self) -> &'static str { /* table above */ }
    pub fn is_interactive(self) -> bool {
        !matches!(self, VisualState::Disabled | VisualState::Busy)
    }
}
```

---

## 4. Specs

Every spec derives `Debug, Clone, PartialEq, Eq`. String fields are owned `String`. Constructors take `impl Into<String>`.

Icon names are **string tokens**, not an enum. Empty = no icon. Suggested vocabulary for the first paint: `plus`, `x`, `search`, `chevron-down`, `chevron-right`, `stop`, `send`, `chat`, `folder`, `dot`. Unknown tokens still render as the raw string in tests; the GPUI mapper may fall back to a generic mark.

---

### 4.1 `ButtonSpec`

```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonKind {
    Primary,
    Ghost,
    Danger,
    Icon,
}

pub struct ButtonSpec {
    pub kind: ButtonKind,
    pub label: String,   // visible text. Icon kind may be empty.
    pub hint: String,    // shortcut or tooltip. Empty = hide hint.
    pub icon: String,    // token. Empty = no icon.
    pub enabled: bool,
    pub busy: bool,
}
```

**Exact string fields:** `label`, `hint`, `icon`.

**Constructors (enabled = true, busy = false, unused strings empty):**

| Constructor | kind | Sets |
|---|---|---|
| `ButtonSpec::primary(label, hint)` | Primary | label, hint |
| `ButtonSpec::ghost(label, hint)` | Ghost | label, hint |
| `ButtonSpec::danger(label, hint)` | Danger | label, hint |
| `ButtonSpec::icon(icon, hint)` | Icon | icon, hint; `label` empty |

**Height:**

| kind | height |
|---|---|
| `Ghost`, `Icon` | `HEIGHT_COMPACT` (32) |
| `Primary`, `Danger` | `HEIGHT_COMFORT` (44) |

Title-bar Stop is **Ghost** (32, sits in the 48px bar). Approval Deny and destructive confirms are **Danger** (44). Composer Send is **Primary** (44). Do not special-case the label `"Stop"` or `"Send"` the way `ghost_btn` does today.

**Derived:**

```
impl ButtonSpec {
    pub fn height(self) -> u16 { /* table */ }

    /// Busy text buttons show "Working". Icon kind keeps the icon token.
    pub fn shown_label(&self) -> &str {
        if self.busy && self.kind != ButtonKind::Icon {
            "Working"
        } else {
            self.label.as_str()
        }
    }

    pub fn is_visible(&self) -> bool {
        match self.kind {
            ButtonKind::Icon => present(&self.icon),
            _ => present(&self.label),
        }
    }

    pub fn is_interactive(&self) -> bool {
        self.enabled && !self.busy && self.is_visible()
    }

    pub fn visual_state(&self, focused: bool, hovered: bool) -> VisualState {
        resolve_state(self.enabled, self.busy, false, focused, hovered)
    }

    pub fn caption(&self) -> String {
        let name = match self.kind {
            ButtonKind::Icon if !present(&self.label) => self.icon.as_str(),
            _ => self.shown_label(),
        };
        if present(&self.hint) {
            format!("{name} · {}", self.hint)
        } else {
            name.to_owned()
        }
    }
}
```

`shown_label` is the exact literal `Working` (capital W, no ellipsis). A mutant that leaves the original label when `busy` fails `busy_button_shows_working`.

**Hover / selected / focus (words):**

- Buttons are never `Selected`. `resolve_state` is called with `selected = false`.
- Ghost idle fill is `surface.idle` (today: white at 0.07) with `Theme::hairline_bright` border.
- Primary idle fill is `tone.accent`. Text is the on-accent light token (`Theme::text`).
- Danger idle fill is `tone.danger`. Text is `Theme::text`.
- Icon is 32×32, no hint text painted inside the hit target (hint is tooltip / caption only).
- Hover: Ghost lifts to `surface.hover`. Primary/Danger lift to a brighter cut of the same tone (projection: existing `hover(|s| s.bg(hsla(0.58, 0.35, 0.28, 0.40)))` family). Icon hover matches Ghost.
- Focus: 1px accent ring outside. Primary already uses accent fill; the ring must still appear so focus is not invisible.
- Disabled: not interactive, 40% opacity, no hover.
- Busy: `shown_label()` is `Working` for text kinds; clicks ignored.

**Unit tests (this spec):** `button_kind_selects_height`, `busy_button_shows_working`, `disabled_button_is_not_interactive`, `icon_button_visible_from_icon_not_label`, `ghost_and_primary_are_not_equal`.

---

### 4.2 `ListRowSpec`

```
pub struct ListRowSpec {
    pub id: String,
    pub icon: String,        // empty = no leading icon
    pub title: String,       // required for visibility
    pub subtitle: String,    // empty = single-line row
    pub meta: String,        // right-side status / id. Empty = hide.
    pub badge: Option<BadgeSpec>,
    pub selected: bool,
    pub busy: bool,
    pub expandable: bool,
    pub expanded: bool,
    pub children_count: usize,
}
```

**Exact string fields:** `id`, `icon`, `title`, `subtitle`, `meta`. Badge text lives on `badge`.

**Constructor:** `ListRowSpec::new(id, title)` fills the rest with empty / `None` / false / 0.

**Height:** always `HEIGHT_ROW` (36), even when `subtitle` is present. Subtitle is one muted line clipped inside 36. Multi-line previews are out of scope (virtualization in `plan/10` §8.3 can revisit).

**Derived:**

```
impl ListRowSpec {
    pub fn height(&self) -> u16 { HEIGHT_ROW }

    pub fn is_visible(&self) -> bool { present(&self.title) }

    pub fn is_open(&self) -> bool { self.expandable && self.expanded }

    /// expanded is ignored when not expandable.
    pub fn disclosure(&self) -> &'static str {
        if !self.expandable {
            ""
        } else if self.expanded {
            "collapse"
        } else {
            "expand"
        }
    }

    pub fn child_suffix(&self) -> String {
        if self.expandable {
            self.children_count.to_string()
        } else {
            String::new()
        }
    }

    pub fn visual_state(&self, focused: bool, hovered: bool) -> VisualState {
        resolve_state(true, self.busy, self.selected, focused, hovered)
    }

    pub fn caption(&self) -> String {
        let mut c = self.title.clone();
        if present(&self.subtitle) {
            c = format!("{c} · {}", self.subtitle);
        }
        if present(&self.meta) {
            c = format!("{c} · {}", self.meta);
        }
        if let Some(badge) = &self.badge {
            c = format!("{c} · {}", badge.caption());
        }
        c
    }
}
```

Rows have no `enabled` flag in this slice (lists are always enabled). Busy still wins over selected for `VisualState` (spinner on the current thread).

**Hover / selected / focus (words):**

- Idle: fill `surface.idle` (today: white at 0.03), hairline at 0.04.
- Hover (not selected): fill `surface.hover`.
- Selected: fill `surface.selected` (today: `hsla(0.58, 0.35, 0.22, 0.45)`) and `hairline_bright` border. Hover does not change a selected row.
- Focus: same fill rules, plus the 1px accent ring when the row is the keyboard active descendant.
- Busy: keep selected fill if `selected`, otherwise idle; show a trailing busy mark; not a pointer cursor.
- Disclosure: only if `expandable`. Chevron-right when collapsed, chevron-down when `is_open()`. `children_count` paints only when expandable.
- Clicking the row body selects. Clicking the disclosure toggles expand. That split is a GPUI hit-target concern; the spec only exposes `disclosure()`.

**Required test:** `selected_row_is_not_equal_idle`.

```
let idle = ListRowSpec { selected: false, ..base };
let selected = ListRowSpec { selected: true, ..base };
assert_ne!(idle, selected);
assert_eq!(idle.visual_state(false, false), VisualState::Idle);
assert_eq!(selected.visual_state(false, false), VisualState::Selected);
assert_ne!(idle.visual_state(false, false), selected.visual_state(false, false));
assert_eq!(idle.height(), HEIGHT_ROW);
assert_eq!(selected.height(), HEIGHT_ROW);
```

A mutant that drops `selected` from `PartialEq` or from `visual_state` dies here.

**More tests:** `expandable_row_ignores_expanded_when_not_expandable`, `busy_selected_row_is_busy_state`, `list_row_caption_includes_subtitle_and_meta`.

---

### 4.3 `PillSpec` / `BadgeSpec`

Same field shape. **Two distinct structs** (not a type alias) so a pill cannot be passed where a badge is required.

```
pub struct PillSpec {
    pub tone: Tone,
    pub text: String,
}

pub struct BadgeSpec {
    pub tone: Tone,
    pub text: String,
}
```

**Exact string fields:** `text` on each. Tone is the enum, not a string field.

**Height:** `HEIGHT_COMPACT` (32) for both. A badge painted **inside** a 36px row is still a 32-tall spec; the projection vertically centers it.

**Derived (identical math, separate impls):**

```
impl BadgeSpec {
    pub fn height(&self) -> u16 { HEIGHT_COMPACT }
    pub fn is_visible(&self) -> bool { present(&self.text) }
    pub fn caption(&self) -> String {
        format!("{} · {}", self.tone.label(), self.text)
    }
}

impl PillSpec {
    pub fn height(&self) -> u16 { HEIGHT_COMPACT }
    pub fn is_visible(&self) -> bool { present(&self.text) }
    pub fn caption(&self) -> String {
        format!("{} · {}", self.tone.label(), self.text)
    }
}
```

Pills are the composer chips (`What can you do?`, `Summarize this repo`). Badges are status on a row (`running`, `error`, model name). Pills are clickable in GPUI; badges are not. Headless, neither stores `enabled` / `selected`.

**Hover / selected / focus (words):**

- Badge: no hover lift, no focus ring, no pointer. Fill is `tone.*`. Text is `Theme::text`.
- Pill: idle fill is a dim cut of `tone.*` (Neutral uses `surface.idle`). Hover lifts to `surface.hover` while keeping the tone border. Focus shows the 1px accent ring. No selected state in this slice (chips are fire-and-forget).
- Warn/Danger text must remain readable on the dark glass canvas (WCAG AA, `plan/10` §5.2). Projection uses `Theme::text`, not a dark ink.

**Required test:** `badge_tone_changes_label`.

```
let good = BadgeSpec { tone: Tone::Good, text: "ready".into() };
let danger = BadgeSpec { tone: Tone::Danger, text: "ready".into() };
assert_eq!(good.text, danger.text);
assert_ne!(good.tone, danger.tone);
assert_ne!(good.caption(), danger.caption());
assert_eq!(good.caption(), "good · ready");
assert_eq!(danger.caption(), "danger · ready");
assert_ne!(good, danger);
```

A mutant that formats `caption()` from `text` only dies here.

**More tests:** `pill_caption_uses_same_tone_vocab`, `empty_badge_text_is_not_visible`, `pill_and_badge_with_same_fields_are_not_the_same_type` (compile-time: separate structs; runtime: compare captions only).

---

### 4.4 `TabSpec`

```
pub struct TabSpec {
    pub id: String,
    pub icon: String,       // empty = text-only tab
    pub label: String,      // required
    pub selected: bool,
    pub count: Option<u32>, // None = hide count. Some(0) paints "0".
}
```

**Exact string fields:** `id`, `icon`, `label`.

**Height:** `HEIGHT_COMPACT` (32).

**Derived:**

```
impl TabSpec {
    pub fn height(&self) -> u16 { HEIGHT_COMPACT }

    pub fn is_visible(&self) -> bool { present(&self.label) }

    pub fn count_suffix(&self) -> String {
        match self.count {
            Some(n) => n.to_string(),
            None => String::new(),
        }
    }

    pub fn caption(&self) -> String {
        match self.count {
            Some(n) => format!("{} · {n}", self.label),
            None => self.label.clone(),
        }
    }

    pub fn visual_state(&self, focused: bool, hovered: bool) -> VisualState {
        resolve_state(true, false, self.selected, focused, hovered)
    }
}
```

Inspector tabs today: `Session`, `Cores`, `MCP`, `Points`, `Git`, `Term`, `Skills`. `id` matches `ControlSpec` ids where they exist (`tab_session`, `tab_cores`, …). `count` is unused on those seven until MCP/core counts are wired; the spec still implements `Some(0)` vs `None`.

**Hover / selected / focus (words):**

- Idle: fill `surface.idle` (today: white at 0.03), no border.
- Hover: `surface.hover`.
- Selected: fill `surface.selected` (today: `hsla(0.58, 0.40, 0.28, 0.50)`). No hover lift on top.
- Focus: selected/idle fill plus 1px accent ring.
- Count, when `Some`, paints as a Neutral badge to the right of `label`. It is not a separate hit target.

**Tests:** `selected_tab_is_not_equal_idle`, `tab_count_none_hides_suffix`, `tab_count_zero_shows_zero`.

---

### 4.5 `DrawerHeaderSpec`

The left-rail `CHATS` strip and any future section header.

```
pub struct DrawerHeaderSpec {
    pub title: String,        // "Chats", "Inspector"
    pub icon: String,         // empty ok
    pub subtitle: String,     // empty = hide
    pub meta: String,         // "3" or "running"; empty = hide
    pub action_label: String, // "New"; empty = no trailing action
    pub action_hint: String,  // "+"
    pub collapsed: bool,
    pub busy: bool,
}
```

**Exact string fields:** `title`, `icon`, `subtitle`, `meta`, `action_label`, `action_hint`.

**Height:** `HEIGHT_ROW` (36).

**Derived:**

```
impl DrawerHeaderSpec {
    pub fn height(&self) -> u16 { HEIGHT_ROW }

    pub fn is_visible(&self) -> bool { present(&self.title) }

    pub fn has_action(&self) -> bool { present(&self.action_label) }

    pub fn action_button(&self) -> Option<ButtonSpec> {
        if !self.has_action() {
            return None;
        }
        let mut b = ButtonSpec::ghost(self.action_label.clone(), self.action_hint.clone());
        b.busy = self.busy;
        Some(b)
    }

    pub fn disclosure(&self) -> &'static str {
        if self.collapsed { "expand" } else { "collapse" }
    }

    pub fn shown_subtitle(&self) -> &str {
        if self.collapsed { "" } else { self.subtitle.as_str() }
    }

    pub fn caption(&self) -> String {
        let mut c = self.title.clone();
        if present(self.shown_subtitle()) {
            c = format!("{c} · {}", self.shown_subtitle());
        }
        if present(&self.meta) {
            c = format!("{c} · {}", self.meta);
        }
        c
    }

    pub fn visual_state(&self, focused: bool, hovered: bool) -> VisualState {
        resolve_state(true, self.busy, false, focused, hovered)
    }
}
```

When `collapsed` is true: hide `subtitle`, keep `title` and `meta`, chevron-right. The header itself is hoverable (whole 36px row). The trailing action is a nested Ghost button and uses Button rules.

**Hover / selected / focus (words):**

- Idle: transparent (sits on the glass pane). Title uses `Theme::muted` (today: `CHATS`).
- Hover: `surface.hover` on the full header row.
- No selected state (the selected **row** beneath is the selection).
- Focus: 1px accent ring on the header when it is the keyboard target (collapse / expand).
- Busy: disclosure frozen, action button `shown_label()` becomes `Working`.

**Tests:** `drawer_header_height_is_row`, `collapsed_header_hides_subtitle`, `drawer_header_action_is_ghost_32`, `drawer_header_without_action_label_has_no_action`.

---

### 4.6 `EmptyStateSpec`

```
pub struct EmptyStateSpec {
    pub title: String,
    pub body: String,
    pub action: Option<ButtonSpec>,
}
```

**Exact string fields:** `title`, `body`. The action, when present, carries its own `label` / `hint` / `icon`.

**Height:** the block is not a single token. The **action**, when present, is forced to Primary 44:

```
impl EmptyStateSpec {
    pub fn action_height(&self) -> Option<u16> {
        self.action.as_ref().map(|b| {
            let mut primary = b.clone();
            primary.kind = ButtonKind::Primary;
            primary.height()
        })
    }

    pub fn has_action(&self) -> bool {
        self.action.as_ref().is_some_and(|b| b.is_visible())
    }

    pub fn action_or_primary(&self) -> Option<ButtonSpec> {
        self.action.clone().map(|mut b| {
            b.kind = ButtonKind::Primary;
            b
        })
    }

    pub fn is_visible(&self) -> bool { present(&self.title) }

    pub fn caption(&self) -> String {
        if present(&self.body) {
            format!("{} · {}", self.title, self.body)
        } else {
            self.title.clone()
        }
    }
}
```

Today's `empty_center()` is one muted sentence and no button. Replace it with:

| Field | First paint copy (exact) |
|---|---|
| `title` | `No messages` |
| `body` | `Start a chat, open the palette (Ctrl+K), or run a command in the terminal strip.` |
| `action` | `ButtonSpec::primary("New thread", "Ctrl+N")` |

**Hover / selected / focus (words):**

- The title/body block is not interactive and has no hover or ring.
- The action uses Primary button rules (hover lift, focus ring, `Working` when busy).
- No selected state.

**Required test:** `empty_state_has_action`.

```
let ready = EmptyStateSpec {
    title: "No messages".into(),
    body: "Start a chat.".into(),
    action: Some(ButtonSpec::primary("New thread", "Ctrl+N")),
};
assert!(ready.has_action());
assert_eq!(ready.action.as_ref().unwrap().label, "New thread");
assert_eq!(ready.action_or_primary().unwrap().kind, ButtonKind::Primary);
assert_eq!(ready.action_or_primary().unwrap().height(), HEIGHT_COMFORT);

let none = EmptyStateSpec { title: "Empty".into(), body: String::new(), action: None };
assert!(!none.has_action());

let dead = EmptyStateSpec {
    title: "Empty".into(),
    body: String::new(),
    action: Some(ButtonSpec::ghost("", "")),
};
assert!(!dead.has_action());
```

**More tests:** `empty_state_caption_joins_title_and_body`, `empty_state_forces_primary_action`.

---

### 4.7 `SearchFieldSpec`

Palette filter and any later file search.

```
pub struct SearchFieldSpec {
    pub query: String,
    pub placeholder: String,
    pub hint: String,    // "Ctrl+K"
    pub icon: String,    // "search"; empty = no leading icon
    pub focused: bool,
    pub enabled: bool,
    pub busy: bool,
}
```

**Exact string fields:** `query`, `placeholder`, `hint`, `icon`.

**Height:** `HEIGHT_ROW` (36). Not 32 (too tight for a caret) and not 44 (that is a CTA).

**Derived:**

```
impl SearchFieldSpec {
    pub fn height(&self) -> u16 { HEIGHT_ROW }

    pub fn showing_placeholder(&self) -> bool { !present(&self.query) }

    pub fn shown_text(&self) -> &str {
        if self.showing_placeholder() {
            self.placeholder.as_str()
        } else {
            self.query.as_str()
        }
    }

    pub fn is_interactive(&self) -> bool { self.enabled && !self.busy }

    pub fn visual_state(&self, hovered: bool) -> VisualState {
        resolve_state(self.enabled, self.busy, false, self.focused, hovered)
    }

    pub fn caption(&self) -> String {
        let text = self.shown_text();
        if present(&self.hint) {
            format!("{text} · {}", self.hint)
        } else {
            text.to_owned()
        }
    }
}
```

Palette first paint: `placeholder = "Filter commands"`, `hint = "Ctrl+K"`, `icon = "search"`.

**Hover / selected / focus (words):**

- Idle: fill `surface.idle` (today composer: white at 0.06), border `hairline_bright`.
- Hover (unfocused): `surface.hover`.
- Focus: border becomes `Theme::accent` (today's composer rule). Fill stays idle. Focus ring is the accent border itself; do **not** draw a second outer ring (a double ring looks like a bug on a 36px field).
- Headless: `shows_focus_ring` is **false** for search. Focus is the accent border token, not the outer ring helper.
- Busy: keep the last `query`, ignore keystrokes, no pointer.
- Disabled: 40% opacity, placeholder still visible, not interactive.
- No selected state.

Add a dedicated helper so the exception is testable:

```
pub fn search_uses_outer_focus_ring() -> bool { false }
```

**Tests:** `search_empty_shows_placeholder`, `search_height_is_row`, `search_focus_does_not_use_outer_ring`, `busy_search_is_not_interactive`.

---

## 5. Module surface

`crates/multiplexer-shell/src/widgets.rs` is the only new shell file. `lib.rs` gains:

```
mod widgets;

pub use widgets::{
    present, resolve_state, search_uses_outer_focus_ring, shows_focus_ring,
    BadgeSpec, ButtonKind, ButtonSpec, DrawerHeaderSpec, EmptyStateSpec,
    ListRowSpec, PillSpec, SearchFieldSpec, TabSpec, Tone, VisualState,
    HEIGHT_COMFORT, HEIGHT_COMPACT, HEIGHT_ROW,
};
```

Do not re-export constructors separately; they live on the types.

`Cargo.toml` of `multiplexer-shell` does **not** gain `gpui`.

---

## 6. Required unit tests

Co-located `#[cfg(test)] mod tests` in `widgets.rs`. Names in **bold** are the merge-gate names from the brief. Every test asserts both a positive and a negative so cargo-mutants cannot flip a single comparison.

| Test | Must kill |
|---|---|
| **`selected_row_is_not_equal_idle`** | Dropping `selected` from `PartialEq` or `visual_state` |
| **`badge_tone_changes_label`** | `caption()` ignoring `tone` |
| **`empty_state_has_action`** | `has_action()` always true/false; empty label treated as present |
| `button_kind_selects_height` | Ghost/Icon not 32, Primary/Danger not 44 |
| `busy_button_shows_working` | Busy keeps original label |
| `disabled_button_is_not_interactive` | `enabled` ignored |
| `icon_button_visible_from_icon_not_label` | Icon visibility keyed off `label` |
| `ghost_and_primary_are_not_equal` | `kind` dropped from `PartialEq` |
| `expandable_row_ignores_expanded_when_not_expandable` | `is_open()` == `expanded` |
| `busy_selected_row_is_busy_state` | Selected wins over busy |
| `list_row_caption_includes_subtitle_and_meta` | caption is title only |
| `pill_caption_uses_same_tone_vocab` | Pill `label()` drift vs Badge |
| `empty_badge_text_is_not_visible` | whitespace-only text is visible |
| `selected_tab_is_not_equal_idle` | same class of bug as rows |
| `tab_count_none_hides_suffix` | `None` formats as `"0"` or `"None"` |
| `tab_count_zero_shows_zero` | `Some(0)` treated as `None` |
| `drawer_header_height_is_row` | header uses 32 or 44 |
| `collapsed_header_hides_subtitle` | subtitle still in `shown_subtitle` |
| `drawer_header_action_is_ghost_32` | action is Primary/44 |
| `drawer_header_without_action_label_has_no_action` | empty label still Some |
| `empty_state_caption_joins_title_and_body` | body dropped |
| `empty_state_forces_primary_action` | kind left as Ghost |
| `search_empty_shows_placeholder` | empty query shows `""` |
| `search_height_is_row` | search is 32 |
| `search_focus_does_not_use_outer_ring` | search uses `shows_focus_ring` |
| `busy_search_is_not_interactive` | busy still accepts input |
| `resolve_state_priority_is_strict` | order swap (disabled vs busy vs selected vs focus vs hover) |
| `height_tokens_are_32_36_44` | constants edited |
| `tone_labels_are_stable` | `Good` prints `ok` / `success` |
| `present_rejects_whitespace` | `"   "` counts as present |

`resolve_state_priority_is_strict` pins the six outcomes with explicit tuples. Include at least:

```
assert_eq!(resolve_state(false, true, true, true, true), VisualState::Disabled);
assert_eq!(resolve_state(true, true, true, true, true), VisualState::Busy);
assert_eq!(resolve_state(true, false, true, true, true), VisualState::Selected);
assert_eq!(resolve_state(true, false, false, true, true), VisualState::Focus);
assert_eq!(resolve_state(true, false, false, false, true), VisualState::Hover);
assert_eq!(resolve_state(true, false, false, false, false), VisualState::Idle);
assert_ne!(VisualState::Selected, VisualState::Idle);
assert_ne!(VisualState::Focus, VisualState::Hover);
```

---

## 7. GPUI mapping (parent writes)

File: `apps/multiplexer-desktop/src/widgets.rs`.

**One function per component.** No generic `widget(spec: enum)`. Callers in `main.rs` pass a spec and a click closure.

| Function | Consumes | Height | Notes |
|---|---|---|---|
| `button(spec, id, cx, on_click)` | `ButtonSpec` | `spec.height()` | Replaces `ghost_btn` |
| `list_row(spec, cx, on_click, on_toggle)` | `ListRowSpec` | 36 | Replaces thread `div` tree |
| `pill(spec, id, cx, on_click)` | `PillSpec` | 32 | Replaces `chip` |
| `badge(spec)` | `BadgeSpec` | 32 | Status on a row; no click |
| `tab(spec, cx, on_click)` | `TabSpec` | 32 | Replaces inspector tab `div` |
| `drawer_header(spec, cx, on_toggle, on_action)` | `DrawerHeaderSpec` | 36 | Replaces `CHATS` strip |
| `empty_state(spec, cx, on_action)` | `EmptyStateSpec` | action 44 | Replaces `empty_center` |
| `search_field(spec, id, cx, on_click)` | `SearchFieldSpec` | 36 | Palette filter + composer cousin |

Each function:

1. Reads `height()`, `visual_state(...)`, `fill_token()`, `caption()` / `shown_label()` from the spec.
2. Maps `fill_token()` through a local table onto `Theme::*` (see §7.1).
3. Applies hover **only** when `visual_state(...).is_interactive()`.
4. Sets `ElementId` from `id` / `spec.id` / `spec.label` so existing click tests keep a stable target.
5. Does not invent labels. If `hint` is present on a Ghost/Primary/Danger button, paint it muted to the right of `shown_label()`, matching today's `ghost_btn`.

Hover closures stay in GPUI. The headless spec never stores `hovered`.

### 7.1 Token → Theme map (dark glass, current `theme.rs`)

| Token | Theme method / construction |
|---|---|
| `surface.idle` | `hsla(0.0, 0.0, 1.0, 0.07)` (buttons) or `0.03` (rows/tabs). Parent may use two helpers: `Theme::surface_idle_control()` and `Theme::surface_idle_row()`. |
| `surface.hover` | `hsla(0.58, 0.35, 0.28, 0.40)` (existing button hover) |
| `surface.focus` | idle fill plus accent **border** |
| `surface.selected` | `hsla(0.58, 0.35, 0.22, 0.45)` (rows) / `hsla(0.58, 0.40, 0.28, 0.50)` (tabs) |
| `surface.busy` | selected fill if selected, else idle |
| `surface.disabled` | idle at 0.40 opacity |
| `tone.neutral` | `hsla(0.0, 0.0, 1.0, 0.06)` (today's chip) |
| `tone.accent` | `Theme::accent()` |
| `tone.good` | `Theme::good()` |
| `tone.warn` | new `Theme::warn()` = `hsla(0.10, 0.70, 0.58, 0.95)` (amber, AA on ink) |
| `tone.danger` | `Theme::danger()` |

Add `Theme::warn()` in the same parent pass. Do not add light-theme mappings here (still dark-only).

### 7.2 Adoption (stop painting unlabeled divs)

| Current call site | Becomes |
|---|---|
| `ghost_btn("Chats", "Hide"/"Show", ...)` | `button(&ButtonSpec::ghost("Chats", ...), ...)` |
| `ghost_btn("Palette", "Ctrl+K", ...)` | Ghost 32 |
| `ghost_btn("Help", "F1", ...)` | Ghost 32 |
| `ghost_btn("Stop", "Ctrl+.", ...)` | Ghost 32 (not Danger) |
| `ghost_btn("Send", "Enter", ...)` | `ButtonSpec::primary("Send", "Enter")` |
| `ghost_btn("Allow"/"Deny", ...)` | Primary / Danger 44 |
| inspector `tab_buttons` → `ghost_btn` | Ghost 32, labels from `InspectorButton` |
| thread row `div` in `left_rail` | `ListRowSpec { id: t.id, title, subtitle: preview, meta: status, selected }` |
| inspector tab `div` | `TabSpec { id: control id, label: t.label(), selected }` |
| `chip("What can you do?", ...)` | `PillSpec { tone: Neutral, text: "What can you do?" }` |
| `empty_center()` | `EmptyStateSpec` in §4.6 |
| `CHATS` header + New/Del | `DrawerHeaderSpec { title: "Chats", action_label: "New", action_hint: "+" }` plus a second Ghost for Del |
| palette query field | `SearchFieldSpec { placeholder: "Filter commands", hint: "Ctrl+K", icon: "search", focused: focus == Palette }` |

Delete `ghost_btn`, `chip`, and `empty_center` from `main.rs` once the projection file exists. Keep `glass_pane` / `glass_bar` (those are chrome, not widgets).

`controls.rs` `REQUIRED_IDS` does not change. Parent builds a `ButtonSpec` from `ControlSpec.label` + `ControlSpec.shortcut`.

---

## 8. What this is not

- Not a new `multiplexer-theme` crate.
- Not GPUI types in `multiplexer-shell`.
- Not a restyle of the editor, Ghostty terminal, or HAR waterfall.
- Not a change to `ControlSpec`, `Workspace`, or the wire contract.
- Not light theme, motion tokens, or snapshot goldens (those stay Phase 2.6 / `plan/10` §9.2).
- Not virtualized lists (reuse later; rows are still 36).
- Not a second source of action names. Actions stay on `ClientAction` / `ControlSpec.action`.

---

## 9. Testing and CI

Headless tests in `multiplexer-shell` run in the **unit** gate. They are a mutation target (D21). Merge floor is 70% killed on `widgets.rs` (D33).

Suggested cargo-mutants focus (parent runs, this doc does not):

```
cargo mutants -p multiplexer-shell --file src/widgets.rs
```

GPUI component tests (headless harness, `plan/10` §9.1) come **after** the projection file exists. First paint does not add screenshot goldens.

Coverage: every public function in `widgets.rs` is called from a unit test. `resolve_state` and `present` are the highest-value mutants.

---

## 10. Parent implementation order

1. Add `crates/multiplexer-shell/src/widgets.rs` with tokens, enums, specs, helpers, and the tests in §6.
2. `mod widgets` + `pub use` in `lib.rs`.
3. `cargo test -p multiplexer-shell widgets -- --nocapture` (parent).
4. `cargo mutants -p multiplexer-shell --file src/widgets.rs` until the floor holds.
5. Add `apps/multiplexer-desktop/src/widgets.rs` (one function per spec).
6. Add `Theme::warn()` and the idle/hover helpers if the map in §7.1 needs them.
7. Replace `ghost_btn` / `chip` / `empty_center` / tab and row divs in `main.rs`.
8. `cargo clippy -p multiplexer-shell -- -D warnings` and the desktop clippy equivalent.

No behavior change to send / interrupt / palette / approval. Visual only, plus the empty-state **New thread** action (already `ClientAction::NewThread`).

---

## 11. Open questions

None that block this slice. Locked here:

- Specs live in `multiplexer-shell`, not a theme crate.
- Heights are 32 / 36 / 44 as specified.
- Title-bar Stop is Ghost 32. Composer Send is Primary 44. Approval Deny is Danger 44.
- Search focus is an accent **border**, not the outer ring used by buttons and rows.
- Pill and Badge are distinct structs that share `Tone`.

If Phase 2 later extracts `multiplexer-theme`, move only `Tone`, `VisualState`, `HEIGHT_*`, and `fill_token()` strings. Leave the specs in the shell (they are chrome copy, not paint).

---

## 12. Consistency

- `plan/10` §5 tokens: this kit **names** the tokens; `theme.rs` **paints** them.
- `plan/10` §8: panes stay components; these seven widgets are the atoms those panes compose.
- `plan/19` item 2.6 (design system, snapshot-tested): this is the Phase 0.4 / Phase 1 foothold. Snapshots wait for 2.6.
- `plan/15` / D21 / D33: pure specs are unit + mutation. GPUI projection is the later component gate.
- `apps/multiplexer-desktop/src/controls.rs`: catalog unchanged.

---

PARENT_IMPLEMENT
files: plan/31-component-kit.md
first_code: crates/multiplexer-shell/src/widgets.rs (headless specs + tests)
