# 29: Icon System (ChromeGlyph + BrandIcon)

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Desktop chrome / Design system
**Depends on:** `02-architecture.md`, `10-ui-pane-system.md`, `13-mobile-app.md`, `26-mcp-skills-ui.md`
**Feeds:** `15-testing-strategy.md`, `19-roadmap-and-milestones.md`, `apps/multiplexer-desktop`, `crates/multiplexer-shell`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context) and with `docs/DECISIONS.md` (the locked decisions). Where a decision is not yet
settled, it is listed under **Open questions** and is **not** decided unilaterally here. New
decisions proposed here are numbered **D77+** in the style of `docs/DECISIONS.md`; they are
proposals for the decision log, not locked decisions.

**Locked decisions applied (D1, D6, D13, D21, D33):** This doc reflects the locked
decisions from `docs/DECISIONS.md`:
- **D1** : Rust + GPUI 0.2.2. Chrome glyphs are GPUI primitives. Brand marks are raster
  `img()` assets, not an Electron icon font and not a runtime SVG kit.
- **D6** : Multiplexer.dev is the product brand. First-party marks (Multiplexer, Grok) are
  ours. Third-party marks stay attribution-only and are never presented as our brand.
- **D13** : consolidated `multiplexer-*` crates. The headless catalog lives in
  `multiplexer-shell` (no GPUI types). The GPUI projection lives in
  `apps/multiplexer-desktop` until `multiplexer-ui` exists.
- **D21 / D33** : name matching, alias tables, asset-path helpers, and row resolvers are
  core logic. They are unit + property + mutation targets. 70% mutation score is the
  merge floor.

**Relationship to plan/10 and plan/26:** plan/10 owns the pane chrome and design tokens.
plan/26 owns the Customize panel and MCP/skills rows. This doc owns the *icon language*
those surfaces paint: a first-party `ChromeGlyph` set drawn in GPUI primitives, plus a
small curated `BrandIcon` set vendored from [dashboardicons.com](https://dashboardicons.com)
 / [homarr-labs/dashboard-icons](https://github.com/homarr-labs/dashboard-icons). It does
not re-specify layout, tokens, or config writes.

**PLAN-CONTEXT note:** `docs/PLAN-CONTEXT.md` currently lists plan docs through `plan/26`.
This file is a new chapter. Adding it to that list is a docs-maintenance edit, not a
product decision.

---

## 1. Problem statement

The Phase 0.4 desktop shell is already a glass Outlook layout (`Theme` in
`apps/multiplexer-desktop/src/theme.rs`) with live inspector tabs, MCP inventory, and
skills lists. Those rows and chrome controls are still *text*. MCP names such as
`github` or `linear` render as a string. Inspector tabs are four-letter labels.
Title-bar actions are ghost buttons with words.

That is fine for a headless model. It is not a control surface. Two gaps:

1. **No chrome glyph language.** Close, search, play, plug, sparkle, and the rest of the
   shell vocabulary must be drawn by us, in GPUI, at 16px and 20px, without bitmaps and
   without shipping a 2,000-icon font.
2. **No brand language.** MCP servers and skills are identified by the brands they wrap.
   Users recognize GitHub, Linear, Docker, Slack. They do not recognize a generic plug
   next to every row. The industry language for this is dashboard-icons: kebab-case
   slugs, SVG/PNG/WEBP, and `-light` variants for dark UIs.

The trap is to submodule the whole 1,800-icon collection, fetch from the CDN at runtime,
or try to paint SVG inside GPUI 0.2.2. None of those are acceptable. This doc specifies
the small, tested, vendored path.

---

## 2. Why this is a first-class system

1. **Recognition is the product.** An MCP row that shows the GitHub mark is instantly
   scannable. A row that shows `github  [stdio]` is a log line. plan/26's Customize
   panel and the current inspector MCP/Skills tabs only become a dashboard when the
   brand language is real.
2. **Chrome must stay first-party.** Brand packs are the wrong source for Close, Chevron,
   Play, and Search. Those glyphs define Multiplexer, not GitHub. They are drawn in
   primitives so they pick up `Theme::text` / `Theme::accent` and stay crisp at any DPI.
3. **GPUI 0.2.2 is raster-honest.** `img()` of a PNG is the supported path. SVG in this
   GPUI version is not a safe default. We do not add `resvg` / `usvg` just to paint
   16 brand marks.
4. **License and trademarks are not a footnote.** dashboard-icons is Apache-2.0
   (copyright Homarr Labs). Apache-2.0 §6 does not grant trademark rights. Product
   names stay with their owners. We vendor a *small curated set*, keep `THIRD_PARTY_ICONS.md`,
   and never claim endorsement.

---

## 3. Design goals

1. **Two layers, one resolver.** `ChromeGlyph` for shell chrome. `BrandIcon` for
   recognized product marks. MCP/Skills rows call one function and get one or the other.
2. **Headless catalog, GPUI projection.** Enums, aliases, `from_name`, `all()`, and
   asset-path helpers live in `multiplexer-shell` with no GPUI types. Draw recipes and
   `img()` live in the desktop binary.
3. **Curated vendor, not a submodule.** A pin table of ~20 slugs. PNG 64px and 128px
   committed under `apps/multiplexer-desktop/assets/brands/`. No git submodule of the
   1,800-icon tree. No runtime CDN fetch.
4. **Dark UI first.** The shipping theme is dark glass. Prefer dashboard-icons `-light`
   variants (example: `github-light`). Fall back to the unsuffixed color PNG only when
   a `-light` file does not exist.
5. **TDD at inception.** `BrandIcon::from_name("github") == Some(GitHub)`, unknown
   names return `None`, `ChromeGlyph::all().len()` is exact, asset paths are deterministic.
6. **Honest attribution.** `THIRD_PARTY_ICONS.md` at the repo root ships with the
   product. First-party Grok/Multiplexer marks are listed separately and are not
   claimed as dashboard-icons.

---

## 4. Proposed architecture

### 4.1 Placement

```
crates/multiplexer-shell/src/icons.rs     headless catalog (no GPUI)
apps/multiplexer-desktop/src/icons.rs     GPUI draw + img() + components
apps/multiplexer-desktop/assets/brands/   vendored PNG 64 / 128
THIRD_PARTY_ICONS.md                      attribution (repo root)
scripts/vendor-brand-icons.ps1            optional re-vendor helper (not runtime)
```

`multiplexer-shell` already owns `McpRow`, `InspectorTab`, and the workspace model, and
it is GPUI-free by crate contract (`Cargo.toml`: "No GPUI types."). The icon catalog
belongs there so CI stays headless. The desktop binary already projects `Workspace`
into glass panes; it is the first (and only, for this slice) painter.

`plan/02` still names a future `multiplexer-ui` crate. When that crate appears, the
GPUI half of this system moves with the rest of the shell view. The headless catalog
does not move.

### 4.2 Runtime vs vendor

```
MCP / skill / tab name
        │
        ▼
 BrandIcon::from_name(name)     (pure, tested)
        │
   Some(brand) ──────────────► BrandBadge  (img() of vendored PNG)
        │
       None
        │
        ▼
 kind == Mcp   ► ChromeGlyph::Plug
 kind == Skill ► ChromeGlyph::Sparkle
 kind == Tab   ► InspectorTab::glyph()
        │
        ▼
 ChromeGlyph::paint(size)       (GPUI primitives, no bitmap)
```

The CDN is a *vendor source*, not a runtime dependency:

- SVG: `https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/svg/<name>.svg`
- PNG 512: `https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/<name>.png`
- Dark-UI variant: append `-light` before the extension (`github-light.png`)
- GitHub raw fallback: `https://raw.githubusercontent.com/homarr-labs/dashboard-icons/main/png/<name>.png`

The shipping binary never hits those URLs. A re-vendor script may. Missing assets fail
the desktop compile or a catalog unit test, not a user session.

### 4.3 Why PNG, not SVG

GPUI 0.2.2 (`apps/multiplexer-desktop` already pins this) paints rasters through
`img()`. SVG support in this version is not a contract we will bet the chrome on.
Dashboard-icons PNG sources are 512px tall. We downsample once, offline, to 64px and
128px, and let `img()` scale inside the 28 / 32 / 36 tiles. That keeps the asset
folder small and the GPU path boring.

Chrome glyphs are the opposite: they tint, they animate with hover, and they must
stay sharp at 16px. Those are primitive paths, not PNGs.

### 4.4 First implementation slice

Parent implementer starts here, in this order:

1. `crates/multiplexer-shell/src/icons.rs` plus co-located unit tests (catalog only).
2. `apps/multiplexer-desktop/assets/brands/` (curated PNG 64/128, dark-UI `-light`).
3. `apps/multiplexer-desktop/src/icons.rs` (`IconButton` / `IconTile` / `BrandBadge`,
   glyph paint).
4. Wire MCP and Skills inspector rows to `icon_for_mcp` / `icon_for_skill`.
5. `THIRD_PARTY_ICONS.md`.

No new crate. No GPUI in `multiplexer-shell`. No submodule.

---

## 5. Key design decisions (proposed D77+)

### D77. Two-layer icon language (PROPOSED)

- **Decision:** Shell chrome uses `ChromeGlyph` (GPUI primitives, no bitmap). Product
  marks use `BrandIcon` (vendored PNG via `img()`). One resolver picks the layer.
- **Rationale:** Mixing brand packs into Close/Search/Play makes the chrome look like
  a homepage dashboard. Mixing primitive glyphs into GitHub/Linear makes brands look
  like clip art. The split is the product.

### D78. Curated vendor, never the full collection (PROPOSED)

- **Decision:** Vendor a pin table of roughly 20 slugs as 64px and 128px PNG. Do not
  git-submodule `homarr-labs/dashboard-icons`. Do not fetch icons at runtime.
- **Rationale:** The upstream tree is 1,800+ icons and grows. A submodule would bloat
  clone time, confuse license attribution, and invite "just add them all." A pin table
  is reviewable and mutation-testable.

### D79. Headless catalog in `multiplexer-shell` (PROPOSED)

- **Decision:** `ChromeGlyph`, `BrandIcon`, `from_name`, `all()`, asset-path helpers,
  and row resolvers live in `crates/multiplexer-shell/src/icons.rs` with zero GPUI
  types. Desktop (or later `multiplexer-ui`) only paints.
- **Rationale:** Matches the existing shell contract and keeps name-matching under
  D21 mutation scope without a GPU.

### D80. MCP / Skills row rule is mandatory (PROPOSED)

- **Decision:** An MCP row whose name matches a `BrandIcon` **must** show that brand.
  Otherwise it shows `ChromeGlyph::Plug`. A Skills row whose name matches a
  `BrandIcon` **must** show that brand. Otherwise it shows `ChromeGlyph::Sparkle`.
- **Rationale:** This is the only user-visible reason to vendor brands in Phase 0.4.
  A generic plug on `github` is a bug, not a fallback we will "get to later."

### D81. Attribution file is part of the feature (PROPOSED)

- **Decision:** Ship `THIRD_PARTY_ICONS.md` at the repo root in the same change that
  vendors the first PNG. List source, license, trademark disclaimer, and every
  vendored slug.
- **Rationale:** Apache-2.0 redistribution plus trademark honesty. The file is the
  NOTICE-equivalent for this slice.

### D82. Dark-UI `-light` PNGs, first-party Grok (PROPOSED)

- **Decision:** Dark glass uses dashboard-icons `-light` files when they exist.
  `BrandIcon::Grok` is a first-party asset, not claimed as dashboard-icons, even if
  a similar slug appears upstream later.
- **Rationale:** Upstream documents `-light` for dark backgrounds. Grok/xAI marks
  are product-identity, not a third-party dashboard tile.

---

## 6. ChromeGlyph

### 6.1 Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChromeGlyph {
    Chat,
    Agent,
    Folder,
    Git,
    Terminal,
    Cpu,
    Plug,
    Flag,
    Search,
    Plus,
    Close,
    Chevron,
    Play,
    Stop,
    Copy,
    Settings,
    Sparkle,
    Layout,
    Browser,
    Diff,
    Activity,
    Palette,
}
```

`ChromeGlyph::all()` returns this exact order as `[ChromeGlyph; 22]`. Length 22 is a
tested invariant. Adding a variant is a catalog change: update `all()`, both draw
recipes, the inspector/title-bar map, and the unit test.

No bitmaps. No font-awesome. No dashboard-icons slug for these.

### 6.2 Sizes

| Slot | Canvas | Stroke | Used inside |
|---|---|---|---|
| Compact | 16px | 1.5px | `IconButton` / `IconTile` at 28 |
| Default | 20px | 1.75px | `IconButton` / `IconTile` at 32 and 36 |

The 20px recipe is the 16px recipe scaled by `20/16` unless a row in §6.3 says
"optical." Optical means: keep stroke, snap joins to whole pixels, do not scale a
1.5px stroke to 1.875.

Coordinate space: origin top-left, y down, glyph centered in the canvas. Stroke cap
round, join round, color `Theme::text` (or `Theme::accent` when the parent says so).
Fill, when used, is the same color at full opacity unless noted.

### 6.3 Draw recipes (16px). 20px = scale unless marked optical.

Each recipe is simple geometry a GPUI `canvas` / path painter can emit. If a glyph is
too dense at 16px after one honest attempt, use the documented fallback. Do not invent
a third language.

| Glyph | 16px recipe (canvas 0..16) | 20px | Fallback if the path is illegible |
|---|---|---|---|
| **Chat** | Rounded rect `(2,3)..(14,12)` r=2. Triangle tail `(4,12)..(7,12)..(3,15)`. | scale | none expected |
| **Agent** | Circle center `(8,5.5)` r=2.5. Shoulder arc from `(3.5,13.5)` through `(8,10)` to `(12.5,13.5)`. | scale | none expected |
| **Folder** | Body rect `(2,6)..(14,13)`. Tab `(2,4)..(7.5,4)..(8.5,6)..(2,6)`. | scale | none expected |
| **Git** | Circles r=1.5 at `(4,4)`, `(12,4)`, `(8,12)`. Lines between those centers. | optical: keep r=1.75 | none expected |
| **Terminal** | Rounded rect `(2,3)..(14,13)` r=1.5. Chevron `>` as two segments through `(4.5,6)..(6.5,8)..(4.5,10)`. Underscore `(8,10.5)..(11.5,10.5)`. | scale | none expected |
| **Cpu** | Square `(4.5,4.5)..(11.5,11.5)`. Eight pin ticks, 1.5px, mid-edge outward. Inner square inset 2px, no fill. | optical | none expected |
| **Plug** | Prongs `(6,3)..(6,7)` and `(10,3)..(10,7)`. Body rounded rect `(4.5,7)..(11.5,13.5)` r=1.5. | scale | none expected |
| **Flag** | Pole `(4,3)..(4,14)`. Triangle `(4,3)..(13,7)..(4,11)`. | scale | none expected |
| **Search** | Circle center `(7,7)` r=4. Handle `(10,10)..(14,14)`. | optical: r=5 on 20 | none expected |
| **Plus** | H `(3,8)..(13,8)`. V `(8,3)..(8,13)`. | scale | none expected |
| **Close** | Diagonals `(4,4)..(12,12)` and `(12,4)..(4,12)`. | scale | none expected |
| **Chevron** | Polyline `(6,4)..(11,8)..(6,12)` (points right). Parent may rotate 180° for left. | scale | none expected |
| **Play** | Triangle `(5,3.5)..(14,8)..(5,12.5)`. | scale | none expected |
| **Stop** | Square `(4.5,4.5)..(11.5,11.5)` filled. | scale | none expected |
| **Copy** | Back rect `(3,5)..(11,14)`. Front rect `(5,2)..(13,11)`. Front wins (drawn last). | scale | none expected |
| **Settings** | Regular hexagon, center `(8,8)`, radius 6. Inner circle r=2. | optical | U+2699 `⚙` at 13px / 16px, color `Theme::text` |
| **Sparkle** | 4-point star, center `(8,8)`, long radius 6, short radius 2. Optional second star at `(12,4)` radius 2. | optical: drop the second star if it muddies | U+2726 `✦` |
| **Layout** | Outer rounded rect `(2,3)..(14,13)` r=1.5. Vertical split `(7,3)..(7,13)`. | scale | none expected |
| **Browser** | Outer rounded rect `(2,3)..(14,13)` r=1.5. Top bar `(2,3)..(14,6.5)`. Three dots r=0.7 at x=4, 6, 8 on the bar. | optical: dots r=0.9 | U+25A2 `▢` |
| **Diff** | Two rounded rects `(3,2.5)..(13,7)` and `(3,9)..(13,13.5)`. Plus tick on the top, minus tick on the bottom, 4px wide, centered. | optical | `+` / `-` stacked in the 16/20 box |
| **Activity** | Polyline `(2,11)..(5,8)..(8,12)..(11,5)..(14,7)`. | scale | none expected |
| **Palette** | Circle center `(8,8)` r=5.5, missing a 50° bite at bottom-right (thumb hole). Three fill dots r=1.1 at 10 o'clock, 1 o'clock, 7 o'clock. | optical: 2 dots if 3 collide | U+25D0 `◐` |

`Settings`, `Sparkle`, `Browser`, and `Palette` are the only allowed Unicode fallbacks.
A fallback is a documented last resort after the geometry is tried, not a shortcut for
the first commit. Tests still enumerate the variant; they do not snapshot pixels.

### 6.4 Paint API (desktop only)

```rust
// apps/multiplexer-desktop/src/icons.rs
fn paint_glyph(glyph: ChromeGlyph, px: GlyphPx, color: Hsla) -> impl IntoElement;
enum GlyphPx { Px16, Px20 }
```

Implementation is a `match` that builds a `canvas` (or a small stack of `div`s for the
rectilinear glyphs: Plus, Close, Stop, Layout). No `include_bytes!` on this path.

### 6.5 Chrome assignments (existing Phase 0.4 surfaces)

| Surface (today) | Control | Glyph |
|---|---|---|
| Title bar | Chats toggle | `Chat` |
| Title bar | Inspector toggle | `Layout` |
| Title bar | Stop | `Stop` |
| Title bar | Command palette | `Search` |
| Left rail | New thread | `Plus` |
| Left rail | Delete thread | `Close` |
| Composer | Send | `Play` |
| Inspector tab Session | | `Agent` |
| Inspector tab Cores | | `Cpu` |
| Inspector tab MCP | | `Plug` |
| Inspector tab Points | | `Flag` |
| Inspector tab Git | | `Git` |
| Inspector tab Term | | `Terminal` |
| Inspector tab Skills | | `Sparkle` |
| Palette / help close | | `Close` |
| Approval allow / deny | | `Play` / `Close` (deny stays danger-tinted) |

`InspectorTab::glyph(self) -> ChromeGlyph` lives in the shell catalog so the desktop
cannot drift.

---

## 7. BrandIcon

### 7.1 Enum and pin table

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrandIcon {
    Grok,
    GitHub,
    Linear,
    Cloudflare,
    Docker,
    Node,
    Rust,
    Windows,
    Git,
    Slack,
    Notion,
    Discord,
    Postgres,
    Redis,
    VsCode,
    Kubernetes,
    OpenAi,
    Tailscale,
    OnePassword,
    Atlassian,
}
```

`BrandIcon::all()` returns this exact order as `[BrandIcon; 20]`. Length 20 is a tested
invariant. The first eleven (Grok through Notion) are the required v1 set from the
product brief. The next nine are the allowed expansion for MCP names we already expect
(Tailscale from plan/23, Atlassian, Postgres, and so on). If a slug 404s at vendor
time, drop that variant from v1 rather than inventing a file. Update `all().len()` in
the same commit.

| Variant | dashboard-icons slug | Dark-UI file stem | Aliases (`from_name` matches any) |
|---|---|---|---|
| Grok | *(first-party, not upstream)* | `grok` | `grok`, `xai`, `x-ai`, `x.ai` |
| GitHub | `github` | `github-light` | `github`, `gh`, `github-mcp-server`, `server-github` |
| Linear | `linear` | `linear-light` | `linear` |
| Cloudflare | `cloudflare` | `cloudflare-light` | `cloudflare`, `cf` |
| Docker | `docker` | `docker-light` | `docker` |
| Node | `nodejs` | `nodejs-light` | `node`, `nodejs`, `node-js` |
| Rust | `rust` | `rust-light` | `rust`, `cargo` |
| Windows | `windows-11` | `windows-11-light` | `windows`, `windows-11`, `win32`, `win` |
| Git | `git` | `git-light` | `git` |
| Slack | `slack` | `slack-light` | `slack` |
| Notion | `notion` | `notion-light` | `notion` |
| Discord | `discord` | `discord-light` | `discord` |
| Postgres | `postgresql` | `postgresql-light` | `postgres`, `postgresql`, `pg` |
| Redis | `redis` | `redis-light` | `redis` |
| VsCode | `visual-studio-code` | `visual-studio-code-light` | `vscode`, `vs-code`, `visual-studio-code`, `code` |
| Kubernetes | `kubernetes` | `kubernetes-light` | `kubernetes`, `k8s` |
| OpenAi | `openai` | `openai-light` | `openai`, `chatgpt` |
| Tailscale | `tailscale` | `tailscale-light` | `tailscale` |
| OnePassword | `1password` | `1password-light` | `1password`, `onepassword`, `op` |
| Atlassian | `atlassian` | `atlassian-light` | `atlassian`, `jira`, `confluence` |

Slug column is the *unsuffixed* dashboard-icons name. Dark-UI file stem is what we
actually vendor for the shipping theme. If `<stem>.png` 404s, try the unsuffixed
`<slug>.png` and record that choice in `THIRD_PARTY_ICONS.md`. If both 404, the
variant is not in v1.

`BrandIcon::Grok` files are drawn or exported by us (`grok-64.png`, `grok-128.png`).
They do not come from the CDN and they do not use a `-light` suffix unless we author
one.

### 7.2 `from_name`

```rust
impl BrandIcon {
    pub fn from_name(name: &str) -> Option<Self>;
}
```

Normalization, in order:

1. Trim. Lowercase. Empty string → `None`.
2. Strip a leading `@scope/` (npm-style `@modelcontextprotocol/server-github` keeps
   `server-github`).
3. Replace `_` and whitespace with `-`. Collapse repeated `-`.
4. Strip a trailing `.exe` / `.cmd` / `.bat` / `.js` / `.mjs` (command-ish names).
5. Exact match against the alias table (every cell in §7.1).
6. Token match: split on `-` and `/`. If **any** token equals an alias, that brand
   wins. First match in `BrandIcon::all()` order wins when two brands collide.
7. Otherwise `None`. Never panic. Never default to Grok.

Examples:

| Input | Result |
|---|---|
| `github` | `Some(GitHub)` |
| `GitHub` | `Some(GitHub)` |
| `github-mcp-server` | `Some(GitHub)` |
| `@modelcontextprotocol/server-github` | `Some(GitHub)` |
| `linear` | `Some(Linear)` |
| `not-a-real-brand` | `None` |
| `""` | `None` |
| `mcp-generic` | `None` |

`from_name` returning `None` is success. The row resolver then picks a glyph.

### 7.3 Asset path helper

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrandPx { Px64, Px128 }

impl BrandIcon {
    /// Relative to `apps/multiplexer-desktop/`.
    pub fn asset_rel_path(self, px: BrandPx, dark_ui: bool) -> &'static str;
    pub fn slug(self) -> &'static str;
    pub fn file_stem(self, dark_ui: bool) -> &'static str;
}
```

Rules:

- Paths are POSIX-style relative strings, always starting with `assets/brands/`.
- Dark UI + brand that has a `-light` stem: `assets/brands/github-light-64.png`.
- Dark UI + first-party Grok: `assets/brands/grok-64.png`.
- Light UI (not shipped in this slice, but the helper must be defined): drop
  `-light`, e.g. `assets/brands/github-64.png`.
- `BrandPx::Px128` swaps `64` for `128`.

These strings are `&'static str` from a table, not `format!`, so mutation tests can
kill a swapped suffix. A unit test asserts the GitHub / Grok / 64 / 128 / dark / light
matrix.

Desktop loads with `include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", path))`
or a `rust-embed` / `include_str` equivalent. Missing files fail compile. Do not
read from disk at runtime.

### 7.4 Vendor procedure

1. For each non-Grok row in §7.1, GET
   `https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/<file-stem>.png`.
2. On 404, GET the unsuffixed `png/<slug>.png`. On a second 404, drop the variant.
3. Downsample 512 → 64 and 512 → 128 (lanczos, keep alpha). Write
   `assets/brands/<stem>-64.png` and `assets/brands/<stem>-128.png`.
4. Record the exact CDN URL, HTTP date, and chosen stem in `THIRD_PARTY_ICONS.md`.
5. Commit the PNGs. Do not commit the 512 sources.

`scripts/vendor-brand-icons.ps1` is optional sugar for a later refresh. It is not
required to land the first catalog. The first slice may download by hand, as long as
the pin table and the attribution file match the files on disk.

Never vendor the SVG tree. Never vendor WEBP. Never vendor wordmark variants
(`*-wordmark-light`).

---

## 8. Component contracts (desktop)

All three sit on the existing glass tokens (`Theme::glass`, `Theme::ink`,
`Theme::hairline`, `Theme::hairline_bright`, `Theme::text`, `Theme::muted`).
They are GPUI elements in `apps/multiplexer-desktop/src/icons.rs`.

### 8.1 Shared geometry

| Prop | 28 | 32 | 36 |
|---|---|---|---|
| Hit / tile size | 28×28 | 32×32 | 36×36 |
| Corner radius | 8 | 10 | 12 |
| Inner glyph | 16 | 20 | 20 |
| Inner brand PNG | 16 (from 64 asset) | 20 (from 64 asset) | 24 (from 64 or 128) |

Fill: `Theme::glass` at rest. Hover: `Theme::glass_strong` plus `Theme::hairline_bright`
border (1px). Pressed: `Theme::ink`. Disabled: fill unchanged, glyph/PNG at 0.35
opacity, `cursor` default not pointer.

No drop shadow on 28. `Theme::shadow` is allowed on 36 tiles only (palette / marketplace
cards later).

### 8.2 `IconButton`

Clickable chrome. Used in the title bar, composer, inspector tool row, approval card.

```text
IconButton {
    glyph: ChromeGlyph,
    size: 28 | 32 | 36,     // default 28
    tint: text | accent | danger | muted,
    tooltip: SharedString,  // required (matches today's hint strings)
    enabled: bool,
    on_click: /* existing ShellView action */
}
```

Must be keyboard-reachable when the parent surface already handles the shortcut.
The button itself does not bind keys; `controls.rs` stays the catalog.

Hover is a *visual* state only. Tests at the catalog layer assert size enums and
the glyph-to-action pairing. GPUI component tests (later, plan/10 §9) snapshot the
28/32/36 boxes.

### 8.3 `IconTile`

Non-primary tile: collapsed rail icons, inspector tab chips, palette row leading
mark when the row is a command (not a brand).

```text
IconTile {
    glyph: ChromeGlyph,
    size: 28 | 32 | 36,     // default 32
    selected: bool,         // selected uses Theme::accent fill at 0.50 (same as today's tab chip)
    label: Option<SharedString>,  // if Some, tile is icon+label; if None, icon only
}
```

Collapsed left rail (`RAIL_COLLAPSED = 36`) uses `IconTile` at 36, icon only.
Inspector tabs may keep their text labels and add a 16px glyph in front.

### 8.4 `BrandBadge`

The only component that calls `img()` on a brand PNG.

```text
BrandBadge {
    icon: BrandIcon,
    size: 28 | 32 | 36,     // default 28 on rows, 32 on marketplace cards
    dark_ui: bool,          // always true in this slice
}
```

Inner image is contain-fit, never cover-cropped. No extra brand-color wash. The
glass tile behind the PNG is what makes a black-on-transparent `-light` mark
readable.

If the include-bytes path is missing, the build fails. There is no runtime
fallback to a glyph inside `BrandBadge`. The *resolver* is what falls back to a
glyph, by returning `RowIcon::Glyph` instead of constructing a badge.

### 8.5 `RowIcon` projection

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowIcon {
    Brand(BrandIcon),
    Glyph(ChromeGlyph),
}

pub fn icon_for_mcp(name: &str) -> RowIcon {
    match BrandIcon::from_name(name) {
        Some(b) => RowIcon::Brand(b),
        None => RowIcon::Glyph(ChromeGlyph::Plug),
    }
}

pub fn icon_for_skill(name: &str) -> RowIcon {
    match BrandIcon::from_name(name) {
        Some(b) => RowIcon::Brand(b),
        None => RowIcon::Glyph(ChromeGlyph::Sparkle),
    }
}
```

These two functions are the D80 contract. They live in `multiplexer-shell`. The
desktop MCP list and Skills list **must** call them. Painting the name as text
without a leading `RowIcon` is a failed implementation of this plan.

---

## 9. MCP and Skills rows

Today (`workspace.rs`):

- `McpRow { name, command, transport }` renders as
  `"{name}  [{transport}]\n  {command}"`.
- Skills render as a newline-joined `Vec<String>` of names.

This slice does not change those structs. It changes the *projection*:

```
[BrandBadge or Plug]  name          [transport]
                      command
```

```
[BrandBadge or Sparkle]  skill-name
```

Matching is on `McpRow.name` and the skill name string, **not** on `command`.
A server named `search` that happens to run `npx github-mcp` does not become
GitHub. A server named `github` that runs `uvx something` does.

plan/26's Customize panel, when it lands, reuses `icon_for_mcp` / `icon_for_skill`
for the registry list. It does not grow a second alias table.

---

## 10. Attribution: `THIRD_PARTY_ICONS.md`

Created at the repo root in the same change as the first vendored PNG. Outline:

```markdown
# Third-party icons

## dashboard-icons (Homarr Labs)

- Source: https://github.com/homarr-labs/dashboard-icons
- Browse: https://dashboardicons.com
- License: Apache License 2.0 (see upstream LICENSE)
- Copyright: (c) 2024 Bjorn Lammers, Meier Lukas, Thomas Camlong and Homarr Labs
- CDN used to vendor: https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/<name>.png

We vendor a small curated subset as 64px and 128px PNG under
`apps/multiplexer-desktop/assets/brands/`. We do not submodule the collection.

### Trademarks

All product names, trademarks, and registered trademarks are the property of
their respective owners. Icons are used for identification only and do not
imply endorsement. Apache-2.0 does not grant trademark rights.

### Vendored slugs

| BrandIcon | File stems | Upstream slug | Notes |
|---|---|---|---|
| GitHub | github-light-64 / 128 | github | dark-UI -light variant |
| ... | ... | ... | ... |

## First-party marks

| File | Owner | Notes |
|---|---|---|
| grok-64.png, grok-128.png | Multiplexer / fair-use product identity | NOT from dashboard-icons |
```

Copy the Apache-2.0 LICENSE text by reference (link), not by pasting the whole
document. Keep the trademark paragraph verbatim in spirit (identification only,
no endorsement).

---

## 11. Testing (TDD at inception)

Name matching and path helpers are core logic (D21). They land *before* the GPUI
paint. Mutation score on `crates/multiplexer-shell/src/icons.rs` is gated at 70%
(D33).

### 11.1 Unit (co-located + `crates/multiplexer-shell/tests/`)

Required cases (these are the acceptance tests named in the brief):

```rust
assert_eq!(BrandIcon::from_name("github"), Some(BrandIcon::GitHub));
assert_eq!(BrandIcon::from_name("not-a-real-brand"), None);
assert_eq!(ChromeGlyph::all().len(), 22);
assert_eq!(
    BrandIcon::GitHub.asset_rel_path(BrandPx::Px64, true),
    "assets/brands/github-light-64.png"
);
```

Also required, because mutants will otherwise survive:

- `from_name("GitHub")` and `from_name("GITHUB")` match.
- `from_name("")` and `from_name("   ")` are `None`.
- `from_name("github-mcp-server")` and `from_name("@modelcontextprotocol/server-github")`
  are `Some(GitHub)`.
- `from_name("linear") == Some(Linear)`.
- `from_name("grok") == Some(Grok)` and `from_name("xai") == Some(Grok)`.
- Every `BrandIcon::all()` entry has a unique `slug()`.
- `BrandIcon::all().len() == 20` (or the dropped-slug count if a 404 removed one;
  the test uses the enum length, not a magic 20, *and* a second test that the
  required v1 eleven are present).
- `ChromeGlyph::all()` contains each variant exactly once (no duplicates).
- `asset_rel_path(Px128, true)` for GitHub ends with `github-light-128.png`.
- `asset_rel_path(Px64, false)` for GitHub is `assets/brands/github-64.png`.
- `asset_rel_path(_, _)` for Grok never contains `-light`.
- Every path starts with `assets/brands/` and ends with `.png`.
- `icon_for_mcp("github") == RowIcon::Brand(GitHub)`.
- `icon_for_mcp("unknown-server") == RowIcon::Glyph(Plug)`.
- `icon_for_skill("github") == RowIcon::Brand(GitHub)`.
- `icon_for_skill("triage") == RowIcon::Glyph(Sparkle)`.
- `InspectorTab::Mcp.glyph() == ChromeGlyph::Plug`.
- `InspectorTab::Skills.glyph() == ChromeGlyph::Sparkle`.
- `InspectorTab::all()` glyphs are 7 distinct assignments (tabs may share a glyph
  only if a test names that share; default is one unique glyph per tab).

### 11.2 Property

- For any `proptest` string, `BrandIcon::from_name` returns `Option` and never
  panics.
- For any string, `icon_for_mcp` / `icon_for_skill` return a `RowIcon`.
- If `from_name(s) == Some(b)` then `icon_for_mcp(s) == RowIcon::Brand(b)` and
  `icon_for_skill(s) == RowIcon::Brand(b)`.
- If `from_name(s) == None` then the MCP glyph is always `Plug` and the skill
  glyph is always `Sparkle`.
- `asset_rel_path` for every `(BrandIcon, BrandPx, bool)` is unique per triple
  except Grok's dark/light collapse (Grok dark and light share a stem; assert
  that explicitly).

### 11.3 Asset existence

A desktop-side test (or a `build.rs` assertion) that every `asset_rel_path(*, true)`
for variants in `BrandIcon::all()` resolves to a file under
`apps/multiplexer-desktop/assets/brands/`. This is what stops a catalog merge
without the PNG.

### 11.4 Mutation

`cargo-mutants` over `multiplexer-shell` `icons.rs`. Survivors that flip
`from_name("github")` to `None`, swap Plug/Sparkle, or rewrite a path suffix
must be killed by §11.1 / §11.2. Glyph *paint* is not a mutation target (GPU).

### 11.5 Component / e2e (follow-on)

When plan/10's headless GPUI harness is in place:

- `IconButton` 28/32/36 hit boxes.
- MCP inspector: a workspace with `McpRow { name: "github", .. }` shows a brand
  badge, not a plug.
- Skills inspector: `"triage"` shows a sparkle.

Not a blocker for the catalog merge.

---

## 12. Implementation order

1. **Catalog (red → green).** Add `crates/multiplexer-shell/src/icons.rs`. Export
   from `lib.rs`. Write the §11.1 tests first. Implement enums, `from_name`,
   `all()`, path helper, `icon_for_*`, `InspectorTab::glyph`.
2. **Assets.** Vendor the required-eleven PNGs (plus any expansion slugs that
   200). Add first-party `grok-64.png` / `grok-128.png`. Add
   `THIRD_PARTY_ICONS.md`.
3. **Desktop paint.** `apps/multiplexer-desktop/src/icons.rs`: `paint_glyph`,
   `IconButton`, `IconTile`, `BrandBadge`. Use `Theme` tokens. Wire title-bar
   Stop / palette / new-thread as the first three `IconButton`s so the chrome
   is visible without touching MCP.
4. **Rows.** Replace the MCP and Skills inspector text-only lists with a row
   that leads with `RowIcon`. Keep the existing copy (`name`, `transport`,
   `command`) to the right of the icon.
5. **Gate.** `fmt` → `clippy -D warnings` → shell unit+property → mutants on
   `icons.rs` → desktop compile (assets present).

---

## 13. Non-goals

- Submoduling or vendoring the full 1,800-icon tree.
- Runtime CDN fetches, icon caches, or "download missing brands on first sight."
- SVG in the GPUI tree, `resvg`, icon fonts, or emoji as the primary chrome.
- A user-facing icon picker or "set a custom icon on this MCP server" (plan/26
  may add that later; it would still resolve through `BrandIcon::from_name`).
- Light-theme polish beyond the path helper knowing how to drop `-light`.
- Mobile raster packaging (plan/13). Expo can reuse the same PNGs and the same
  alias table later; this slice does not add a TS codegen step.
- Replacing `controls.rs` labels with icons-only. Labels stay. Icons lead.

---

## 14. Open questions / risks

These are flagged, not decided here:

1. **Slug 404s.** `windows-11`, `visual-studio-code`, `1password`, `openai`,
   `tailscale` may use a different upstream name. The vendor step records the
   actual slug or drops the variant. The required eleven must ship; the
   expansion nine may shrink.
2. **Grok mark legal.** Whether we draw a geometric stand-in or export an
   official xAI-permitted mark is a branding question (D6). Until that is
   answered, a simple first-party spark-mark PNG is enough to keep
   `BrandIcon::Grok` off the dashboard-icons attribution list.
3. **GPUI `img()` + `include_bytes!` ergonomics.** If 20 × 2 files make the
   binary noisy, a single `assets/brands.rs` map generated from the catalog is
   allowed. The public helper still returns the same `&'static str` paths.
4. **When `multiplexer-ui` appears.** Move only the GPUI file. Do not move the
   catalog.
5. **plan/26 marketplace cards.** They should use `BrandBadge` at 32 or 36.
   Out of scope for the first slice beyond keeping the component capable of
   those sizes.
6. **HiDPI.** 128px assets exist so a 36 tile on a 2x display is not mush.
   We are not shipping 256px.

**Flagged consistency note:** this doc is consistent with PLAN-CONTEXT (Rust +
GPUI, Windows-first desktop, TDD at inception, no Electron) and with plan/10
(design tokens, pane chrome) and plan/26 (MCP/skills rows consume the resolver).
If D1 (stack) or D13 (crate layout) flips, §4 and §8 must be revisited.

---

## 15. Parent implementer

**WRITE / FIRST CODE**

| Path | What |
|---|---|
| `crates/multiplexer-shell/src/icons.rs` | `ChromeGlyph`, `BrandIcon`, `from_name`, `all`, asset helper, `icon_for_mcp`, `icon_for_skill` |
| `apps/multiplexer-desktop/src/icons.rs` | GPUI paint, `IconButton`, `IconTile`, `BrandBadge` |
| `apps/multiplexer-desktop/assets/brands/` | curated PNG 64/128, dark-UI `-light`, first-party Grok |
| `THIRD_PARTY_ICONS.md` | attribution |

Start with the shell catalog and its tests. Assets and GPUI paint follow the
green catalog. Do not add a crate. Do not submodule dashboard-icons.
