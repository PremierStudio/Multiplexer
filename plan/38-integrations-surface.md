# 38: Integrations Surface (Directory of MCP, Skills, Hooks, Git, Models)

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** UI / Shell chrome
**Depends on:** `02-architecture.md`, `05-provider-adapter-layer.md`, `07-checkpointing-and-vcs.md`, `10-ui-pane-system.md`, `17-security-and-secrets.md`, `21-mcp-lifecycle-supervisor.md`, `25-worktree-hooks.md`, `26-mcp-skills-ui.md`
**Feeds:** `15-testing-strategy.md`, `19-roadmap-and-milestones.md`, `20-risks-and-open-questions.md`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context). Where a decision is not yet settled, it is listed under **Open questions** and is
**not** decided unilaterally here. New decisions proposed here are numbered **D143+** in the
style of `docs/DECISIONS.md`; they are proposals for the decision log, not locked decisions.
The D143 start leaves room for `plan/27` through `plan/37` if those docs land in parallel.

**Locked decisions applied (D1, D13, D14, D21, D23, D33):** This doc reflects the locked
decisions from `docs/DECISIONS.md`:
- **D1** : Rust + GPUI. The directory chrome is a GPUI view. The tile *model* is a pure
  projection in `multiplexer-shell` with no GPUI types, matching the existing chrome crate.
- **D13** : consolidated `multiplexer-*` crates. First code lives in `multiplexer-shell`.
  Inventory and lifecycle stay in `multiplexer-mcp`, models in `multiplexer-provider`,
  remotes/worktrees in `multiplexer-worktree`, auth names in `multiplexer-auth`.
- **D14** : OpenRouter / DeepSeek (`ds-flash`) is a config variant of the in-process Grok
  adapter. It is a model tile plus an `openrouter` auth tile, not a fourth provider kind.
- **D21** : mutation-testing scope includes the tile projector, `BrandIcon::from_name`, and
  `filter_tiles`.
- **D23** : secrets session-cache model. Auth tiles show **names only**. No tokens, no
  `op://` resolution, no keychain values, no env contents.
- **D33** : 70% mutation score is the merge floor.

**PARENT_IMPLEMENT.** This doc specifies the surface. The parent implements first code at
`crates/multiplexer-shell/src/integrations.rs`. No cargo work is done by this planner.

**Relationship to plan/26:** plan/26 is the *management* surface (Customize panel: add / edit
/ remove MCP servers, skills, and hooks, plus marketplace browse). This doc is the
*directory*: one beautiful list of everything the workspace is connected to (models,
providers, MCP, skills, hooks, git remotes, worktrees, auth provider *names*). Clicking a
tile opens a detail drawer. Destructive edit still belongs to plan/26. This doc does not
re-specify the supervisor (plan/21), the worktree lifecycle (plan/25), or the model registry
(plan/05). It consumes their read models.

---

## 1. Problem statement

The workspace already *has* the pieces of an integrations picture, but they are scattered
across inspector tabs and text dumps:

1. **Models live in the session tab.** `Workspace.models` and `Workspace.model` are a string
   list and a selected id (`grok`, `grok-4.6`, `fake`). There is no brand tile, no provider
   badge, and no single place that says "this thread is on Grok via the in-process adapter."
2. **MCP is a text list.** `Workspace.mcp: Vec<McpRow>` (name, command, transport) renders as
   `mcp_detail()` lines. The command `npx -y @linear/mcp-server-linear` is unrecognizable as
   Linear. Live plan/21 state (`ready` / `crashed` / `stopped`) is not on the row.
3. **Skills are names only.** `Workspace.skills: Vec<String>` dumps as a newline list. No
   glyph, no source (user vs project), no "invocable" hint.
4. **Git is a status blob.** `Workspace.worktrees` and `Workspace.git_status` live under the
   Git tab. Remotes (origin, upstream) are not a first-class row. There is no brand for
   `github.com` vs a generic host.
5. **Auth is invisible.** `[auth_provider.openrouter]` and friends exist in grok `config.toml`
   (plan/05 §6.1). The chrome never lists them. When we do list them, we must show names
   only (D23). A leaked key in a screenshot is a product bug.
6. **No directory aesthetic.** Competitors either hide this in settings or dump TOML. The
   bar we want is [dashboardicons.com](https://dashboardicons.com): a curated grid of 48px
   brand tiles, one glance, filterable, each tile a status pill away from a drawer.

The result is that "what is this workspace connected to?" is a scavenger hunt. This doc
makes it a first-class directory.

---

## 2. Why a directory, not another settings page

1. **Integrations are the product surface.** MCP servers, skills, remotes, and models are
   how Multiplexer *feels* powerful. A settings form is for editing. A directory is for
   orientation: what is on, what is sick, what brand is this.
2. **dashboardicons is the visual language users already know.** Homarr / Homepage / Dashy
   all render service logos as a 48px tile plus a label. We reuse that language (slug,
   48px, light/dark variants) without becoming a homepage dashboard. See §5.
3. **One projector, two placements.** The same `Vec<TileSpec>` can fill a right-rail tab
   (Phase 0.4 chrome we already have) or a left-rail Outlook section (plan/10 §2.1). We
   do not invent two models.
4. **The drawer is the expand.** Click is not a modal and not a tab switch to a wall of
   TOML. Click expands a detail drawer on the selected tile: status, subtitle, safe
   fields, and a "Customize" affordance that later routes into plan/26.
5. **It is testable as a pure function.** `integration_tiles(&Workspace) -> Vec<TileSpec>`
   and `filter_tiles(query)` are headless. GPUI only paints. That matches
   `multiplexer-shell` today (`palette.rs`, `workspace.rs`).

---

## 3. Design goals

1. **One directory for every connection.** Models / providers, MCP inventory, skills,
   hooks, git remotes, worktrees, auth provider names.
2. **Brand tiles, 48px.** Known brands resolve to a `BrandIcon` slug (dashboardicons
   kebab-case). Unknowns fall back to a category glyph. Never a blank square.
3. **Status pill on every tile.** Selected / ready / crashed / stopped / installed /
   present / idle. Pills are an enum, not free text, so the GPUI theme (`good` / `danger`
   / `muted`) can map them.
4. **Click opens a detail drawer (expand).** Selecting a tile sets `selected_tile` and
   fills a drawer body. Clicking the same tile again collapses. Esc collapses.
5. **Filter is first-class.** A query box filters the directory the way `filter_items`
   filters the palette: case-insensitive substring on id, name, kind, status, subtitle.
6. **No secrets.** Auth tiles are names. Remote tiles show host, never userinfo. MCP
   `env` / `headers` never appear in `TileSpec` or the drawer.
7. **TDD at the projector.** Tests lock `integration_tiles` and `filter_tiles` before any
   GPUI work.

---

## 4. Placement: right-tab first, left-section later

Both placements are specified. They share one tile model.

### 4.1 Right-tab (ships first)

Add `InspectorTab::Integrations` to the existing right rail.

```
InspectorTab::all() becomes 8:
  Session, Resources, Mcp, Checkpoints, Git, Terminal, Skills, Integrations
label: "Apps"
hint / palette: "g a"
```

The tab body is no longer a string dump. It is:

```
┌─ Integrations ─────────────────────────────┐
│  [filter tiles…                    ]       │
│                                            │
│  ┌────┐  Grok 4.6              [active]    │
│  │ 48 │  model · grok-4.6                  │
│  └────┘                                    │
│  ┌────┐  Linear                [ready]     │
│  │ 48 │  mcp · npx @linear…                │
│  └────┘                                    │
│  ┌────┐  origin                [present]   │
│  │ 48 │  remote · github.com               │
│  └────┘                                    │
│                                            │
│  ─ drawer (when a tile is selected) ─      │
│  Linear                                    │
│  status  ready                             │
│  kind    mcp                               │
│  command (safe)  npx -y @linear/…          │
│  [Customize…]                              │
└────────────────────────────────────────────┘
```

Why the right tab ships first:

- The current desktop already has a right inspector (`apps/multiplexer-desktop/src/inspector.rs`).
- MCP, Skills, and Git tabs already exist as *partial* views of this directory. The Apps
  tab is the unified view; the older tabs stay as focused filters (they can later become
  `filter_tiles` presets: `kind:mcp`, `kind:skill`, `kind:git`).
- The left rail is still the Outlook chat list. Stealing it in Phase 0.4 would fight the
  thread-picker muscle memory.

Existing tests that assert `InspectorTab::all().len() == 7` must be updated when the
variant is added. That chrome wiring is a follow-up in the same crate, not a blocker for
the first-code projector.

### 4.2 Left-section (Phase 2 Outlook sections)

Plan/10 §2.1 already names left-rail sections: Chats / Threads, Projects, Agents,
Activity. Add **Integrations** as a section that can replace the chat list:

```
pub enum LeftSection {
    Chats,
    Integrations,
}
```

Same `integration_tiles` / `filter_tiles` / `selected_tile` drawer. The left rail is
248px by default, which is enough for a one-column tile list (48px icon + name + pill).
A two-column Homarr-style grid is reserved for a popped-out Integrations pane, not the
collapsed rail.

### 4.3 Drawer behavior (both placements)

- Click tile T: if `selected_tile == Some(T)` then clear it (collapse), else set it
  (expand).
- Drawer content is `tile_detail(&TileSpec, &Workspace) -> String` in the headless
  model. GPUI later replaces the string with labeled rows.
- Drawer never includes secret-looking values (reuse `SecretRef::looks_like_plaintext`
  as a reject rule if a field leaks through).
- "Customize" is a later host action that opens the plan/26 editor for that id. First
  code does not implement the editor.

---

## 5. dashboardicons aesthetic

We copy the *visual contract*, not the website.

| Rule | Value |
|---|---|
| Tile mark | 48×48 px |
| Slug | kebab-case (`github`, `slack`, `linear`, `cloudflare`) |
| Catalog | [homarr-labs/dashboard-icons](https://github.com/homarr-labs/dashboard-icons) |
| CDN (docs / browse only) | `https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/svg/{slug}.svg` |
| Dark chrome | prefer `{slug}-light` when the catalog has a light variant |
| Light chrome | prefer `{slug}` or `{slug}-dark` |
| Unknown brand | category glyph, never a broken image |
| Runtime | **no network fetch on the hot path or in tests** |

The shell crate stores a `BrandIcon` enum plus a `slug()` string. The desktop later
resolves slug to a vendored SVG (or a GPUI vector glyph). Tests assert slugs and
glyphs only. Shipping a live jsDelivr fetch from the desktop would make CI and
air-gapped Windows installs depend on a third-party CDN. That is out of scope.

Trademark note (dashboard-icons LICENSE): logos identify the service. They do not
imply endorsement. We do not recolor official marks except to pick the catalog's
own `-light` / `-dark` file.

---

## 6. Proposed architecture

### 6.1 Data flow

```
Workspace (headless, multiplexer-shell)
  models, model
  mcp: Vec<McpRow>
  skills: Vec<String>
  hooks: Vec<String>            // additive, default empty
  remotes: Vec<RemoteRow>       // additive, default empty
  worktrees, selected_worktree
  auth_providers: Vec<String>   // additive, names only
        │
        │  integration_tiles(&Workspace)
        ▼
  Vec<TileSpec>     48px BrandIcon + name + status + kind
        │
        │  filter_tiles(tiles, query)
        ▼
  visible tiles
        │
        │  click → selected_tile: Option<String>
        ▼
  tile_detail(...)  drawer body (no secrets)
        │
        ▼
  GPUI (later)  /  inspector_body (string, now)
```

The projector is a pure function of `Workspace`. The host (desktop / server) is
responsible for filling `mcp` from `multiplexer_mcp::load_user_mcp_inventory`,
`skills` from `merge_skill_rows`, remotes from `git remote -v` (host only, never
userinfo), and `auth_providers` from `[auth_provider.*]` **keys**. The projector
does not open files.

### 6.2 First-code types

File: `crates/multiplexer-shell/src/integrations.rs`

`multiplexer-shell` currently depends only on `multiplexer-layout`. Keep it that
way. Do not take a dep on `multiplexer-mcp` or `multiplexer-auth` for the
projector. Map from the `Workspace` fields that already exist, plus the three
additive lists below.

```rust
/// Kind of connection this tile represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileKind {
    Model,
    Provider,
    Mcp,
    Skill,
    Hook,
    Remote,
    Worktree,
    Auth,
}

impl TileKind {
    pub fn as_str(self) -> &'static str { /* model, provider, mcp, ... */ }
    pub fn glyph(self) -> &'static str {
        // fallback mark when BrandIcon is Generic
        match self {
            Self::Model | Self::Provider => "◆",
            Self::Mcp => "⬡",
            Self::Skill => "✎",
            Self::Hook => "⚡",
            Self::Remote | Self::Worktree => "⎇",
            Self::Auth => "⚷",
        }
    }
}

/// Lifecycle / presence pill. Mapped from workspace facts, not from secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileStatus {
    /// Currently selected model, or selected worktree.
    Active,
    /// Supervisor ready (plan/21), or a configured remote with a host.
    Ready,
    /// Configured, not currently selected / not yet spawned.
    Idle,
    /// Inventory present (skill, hook, auth name, mcp without live state).
    Present,
    /// Supervisor crashed / failed.
    Crashed,
    /// Supervisor stopped, or worktree prunable.
    Stopped,
}

impl TileStatus {
    pub fn as_str(self) -> &'static str { /* active, ready, idle, ... */ }
    pub fn label(self) -> &'static str { /* Active, Ready, Idle, ... */ }
}

/// dashboardicons slug or a generic fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrandIcon {
    Linear,
    Cloudflare,
    Github,
    Slack,
    Git,
    Grok,
    OpenRouter,
    Fake,
    Generic,
}

impl BrandIcon {
    /// Map a command, model id, remote host, or auth name to a brand.
    pub fn from_name(raw: &str) -> Self { /* §7 table */ }

    /// kebab-case catalog slug. `generic` means "use the kind glyph".
    pub fn slug(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Cloudflare => "cloudflare",
            Self::Github => "github",
            Self::Slack => "slack",
            Self::Git => "git",
            Self::Grok => "x",          // no grok slug; X/xAI family
            Self::OpenRouter => "openrouter",
            Self::Fake => "generic",
            Self::Generic => "generic",
        }
    }
}

/// One directory row. No secret fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileSpec {
    pub id: String,          // stable: "{kind}:{name}"
    pub kind: TileKind,
    pub name: String,
    pub subtitle: String,    // safe: transport, host, model id
    pub status: TileStatus,
    pub icon: BrandIcon,
}

/// One git remote as the host fills it. URL is host-only (no userinfo, no path
/// required). Empty host is allowed and becomes Generic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRow {
    pub name: String,      // origin, upstream
    pub host: String,      // github.com, git.example.dev
}
```

Additive `Workspace` fields (default empty, so existing `Workspace::new` tests
keep passing once the constructor is updated):

```rust
// in Workspace
pub remotes: Vec<RemoteRow>,
pub auth_providers: Vec<String>,   // names only: "openrouter", "grok"
pub hooks: Vec<String>,            // hook names from inventory
pub selected_tile: Option<String>, // TileSpec.id
pub tile_query: String,
```

Setters: `set_remotes`, `set_auth_providers`, `set_hooks`, `set_tile_query`,
`select_tile`, `toggle_tile`. None of these accept a secret.

### 6.3 Projector: `integration_tiles`

```rust
/// Project the workspace into a stable, ordered directory.
///
/// Order (always this order, so the list is predictable):
///   1. models          (Workspace.models)
///   2. provider kinds  (derived: grok / fake from model ids)
///   3. mcp             (Workspace.mcp)
///   4. skills          (Workspace.skills)
///   5. hooks           (Workspace.hooks)
///   6. remotes         (Workspace.remotes)
///   7. worktrees       (Workspace.worktrees)
///   8. auth names      (Workspace.auth_providers)
pub fn integration_tiles(ws: &Workspace) -> Vec<TileSpec>;
```

Rules:

| Source | `id` | `name` | `subtitle` | `status` | `icon` |
|---|---|---|---|---|---|
| `ws.models` | `model:{id}` | display (`grok-4.6` stays `grok-4.6`) | `model` | `Active` if `id == ws.model`, else `Idle` | `from_name(id)` |
| derived provider | `provider:grok` or `provider:fake` | `grok` / `fake` | `provider` | `Active` if any selected model maps to it | `Grok` / `Fake` |
| `ws.mcp` | `mcp:{name}` | server name | `mcp · {transport}` | `Present` until live state is on the row | `from_name(name + " " + command)` |
| `ws.skills` | `skill:{name}` | skill name | `skill` | `Present` | `from_name(name)`, else `Generic` |
| `ws.hooks` | `hook:{name}` | hook name | `hook` | `Present` | `Generic` (kind glyph) |
| `ws.remotes` | `remote:{name}` | remote name | `remote · {host}` | `Ready` if host non-empty, else `Idle` | `from_name(host)` else `Git` |
| `ws.worktrees` | `worktree:{i}` | last path segment | `worktree` | `Active` if `selected_worktree == Some(i)`, else `Present` | `Git` |
| `ws.auth_providers` | `auth:{name}` | name | `auth` | `Present` | `from_name(name)` |

Provider derivation (D14):

- If any model id contains `fake` (case-insensitive), emit one `provider:fake` tile.
- If any model id contains `grok`, `xai`, or is `ds-flash`, emit one `provider:grok`
  tile. `ds-flash` is **not** a separate provider kind.
- If `ws.models` is empty, still emit a provider tile for `ws.model`.

Do not emit duplicate ids. Within a kind, preserve workspace order.

`McpRow` today has no live status. First code therefore marks every MCP tile
`Present`. When the host later copies plan/21 `LifecycleState` onto the row
(additive `McpRow.status: Option<String>`), map:

| `LifecycleState` | `TileStatus` |
|---|---|
| `Ready` | `Ready` |
| `Spawned` | `Idle` |
| `Crashed {..}` / `Failed` | `Crashed` |
| `Stopped` | `Stopped` |

That mapping is specified now so the host does not invent a second vocabulary.
First code tests lock `Present` for MCP without a status field.

### 6.4 Filter: `filter_tiles`

Mirror `palette::filter_items`. Empty query returns the input (cloned) in the
same order. Non-empty query is a case-insensitive substring over `id`, `name`,
`subtitle`, `kind.as_str()`, `status.as_str()`, and `icon.slug()`.

```rust
/// Case-insensitive substring filter. Empty query returns `tiles` unchanged.
pub fn filter_tiles(tiles: &[TileSpec], query: &str) -> Vec<TileSpec>;
```

A convenience wrapper matches the prompt shape `filter_tiles(query)` for the
workspace:

```rust
pub fn filter_workspace_tiles(ws: &Workspace, query: &str) -> Vec<TileSpec> {
    filter_tiles(&integration_tiles(ws), query)
}
```

Whitespace-only queries trim to empty (so `"   "` is the full catalog). This
must be tested: a mutant that skips `trim()` would keep the catalog hidden.

### 6.5 Drawer helpers

```rust
pub fn toggle_tile(ws: &mut Workspace, id: &str) -> bool;
pub fn tile_detail(tile: &TileSpec) -> String;
```

`toggle_tile` sets `ws.selected_tile` to `Some(id)` or `None` if it was already
that id. Returns whether the field changed. Unknown ids are still selectable
(the drawer then shows the id and "unknown tile"); this keeps the chrome from
fighting a stale click after a refresh.

`tile_detail` is a short, secret-free block:

```
{name}
kind     {kind}
status   {status.label}
icon     {slug}
{subtitle}
```

No command env, no headers, no `op://` strings, no keychain names beyond the
already-public auth *provider* name.

### 6.6 Crate wiring (first code)

In `crates/multiplexer-shell/src/lib.rs`:

```rust
mod integrations;
pub use integrations::{
    filter_tiles, filter_workspace_tiles, integration_tiles, tile_detail, toggle_tile,
    BrandIcon, RemoteRow, TileKind, TileSpec, TileStatus,
};
```

Re-export `RemoteRow` next to `McpRow` from `workspace` as well if the
desktop host wants a single import path.

Palette follow-up (not first code, but specified): add a `PaletteItem` for
`SelectTab(InspectorTab::Integrations)` with id `apps`, hint `g a`.

---

## 7. `BrandIcon::from_name` mapping table

`from_name` is the mutation-critical function. It takes any haystack (model id,
MCP name + command, remote host, auth name) and returns a brand.

### 7.1 Algorithm

1. Lowercase the input.
2. Treat `@scope/pkg`, `npx`, `uvx`, `cmd /c`, URL prefixes (`https://`,
   `http://`, `git@`), and path separators as separators. The matcher walks
   tokens and also searches the raw lowercased haystack for the needles below.
3. First match in **table order** wins. Table order is brand-specific needles
   before generic ones (`github` before `git`).
4. If nothing matches, return `Generic`.

Needles are matched as substrings of the lowercased haystack, except single
token needles (`gh`, `x`) which must match a whole token (split on
`/`, `\`, whitespace, `@`, `:`, `.` is *not* a split for `github.com`: the
host `github.com` still contains the substring `github`).

`gh` is special: match only as a whole token so `high` does not become GitHub.

### 7.2 Table (required, tested)

| Brand | Needles (lowercase) | Typical inputs that must hit |
|---|---|---|
| `Linear` | `linear`, `@linear/` | `npx -y @linear/mcp-server-linear`, `npx @linear`, `linear`, `mcp.linear.app` |
| `Cloudflare` | `cloudflare`, `@cloudflare/` | `npx @cloudflare/mcp`, `cloudflare`, `workers.dev` *only if* `cloudflare` also appears; `workers.dev` alone is **not** enough (too generic) |
| `Github` | `github`, `githubusercontent`, token `gh` | `npx @modelcontextprotocol/server-github`, `github`, `github.com`, `git@github.com:org/repo.git`, token `gh` |
| `Slack` | `slack`, `@slack/` | `npx @slack/mcp`, `slack`, `slack.com` |
| `Grok` | `grok`, `grok-4.6`, `grok-4`, `xai` | `grok`, `grok-4.6`, `xai-grok-shell` |
| `OpenRouter` | `openrouter`, `ds-flash` | `openrouter`, `ds-flash`, `[auth_provider.openrouter]` name |
| `Fake` | whole token `fake` | `fake`, model id `fake` |
| `Git` | `gitlab`, `bitbucket`, `git.` host, token `git` | leftover remotes that did not hit GitHub |
| `Generic` | (none) | `mystery`, empty string, `npx -y @acme/thing` |

Notes that tests must lock:

- `npx -y @linear/mcp-server-linear` is `Linear`, not `Generic`. The `@linear/`
  needle is why we do not require the server name to be exactly `linear`.
- `npx @modelcontextprotocol/server-github` is `Github`. The word `github` sits
  in the package name.
- `git@github.com:acme/app.git` is `Github`, not `Git`. `github` is checked
  before `git`.
- `origin` with host `github.com` is `Github`. `origin` with host
  `git.internal.dev` is `Git`.
- `high-priority` is **not** `Github` (`gh` is whole-token only).
- `ds-flash` is `OpenRouter` (the *model* tile). The *provider* tile for that
  model is still `provider:grok` (D14). Both tiles appear.
- Empty string and whitespace-only are `Generic`.
- Matching is case-insensitive: `NPX @Linear/MCP` is `Linear`.

### 7.3 Suggested extra needles (same function, later rows)

These are not required for first-code green, but the function should be easy
to extend without changing the algorithm. Add them behind the required table
once tests for the required set are green:

| Brand | Needles |
|---|---|
| `Github` | `ghcr.io` |
| `Slack` | `slack-sdk` |
| (future `Notion`) | `notion` |
| (future `Sentry`) | `sentry` |

Do not add a brand without a dashboardicons slug (or an explicit Generic
fallback) and a unit test.

---

## 8. Key design decisions (proposed D143+)

These are proposals for `docs/DECISIONS.md`. They are **not** locked.

### D143. One tile model, two placements (PROPOSED)
- **Decision:** `TileSpec` + `integration_tiles` + `filter_tiles` is the only
  integrations directory model. The right-tab and the left-section are views
  of that model.
- **Rationale:** Plan/10 already treats pane content as pluggable. Two models
  would fork filter, status, and icon logic.

### D144. Right-tab ships first (PROPOSED)
- **Decision:** `InspectorTab::Integrations` (label `Apps`) is the first
  shipping placement. `LeftSection::Integrations` lands with plan/10 Outlook
  sections in Phase 2.
- **Rationale:** The Phase 0.4 chrome already has a right inspector. The left
  rail is the chat list. Unifying MCP / Skills / Git as one directory does
  not require stealing the Outlook rail.

### D145. `BrandIcon::from_name` is a static table (PROPOSED)
- **Decision:** Brand resolution is a pure function over a haystack with a
  closed, tested needle table. No network, no filesystem, no fuzzy ML.
- **Rationale:** Mutation-testable. Deterministic. Works offline. The
  dashboardicons catalog is a *paint* source, not a *lookup* source.

### D146. Auth tiles are names only (PROPOSED)
- **Decision:** `Workspace.auth_providers` is `Vec<String>` of
  `[auth_provider.<name>]` keys. Tiles, drawers, and logs never include
  `env_key` values, `op://` refs, or keychain material. D23 still applies.
- **Rationale:** A directory screenshot must be safe to share. Presence of
  an auth *name* is not a secret. The value behind it is.

### D147. Click expands a drawer, not a modal (PROPOSED)
- **Decision:** Selecting a tile sets `selected_tile` and expands a detail
  drawer in the same pane. Re-click or Esc collapses. No modal, no new window.
- **Rationale:** A modal fights the Outlook layout and the <16ms input budget.
  Expand-in-place matches the "directory then inspect" pattern.

### D148. Projector stays in `multiplexer-shell` (PROPOSED)
- **Decision:** First code is `crates/multiplexer-shell/src/integrations.rs`.
  No GPUI types. No new crate. No dep on `multiplexer-mcp`.
- **Rationale:** Matches `palette.rs` / `workspace.rs`. Headless CI stays
  headless. The desktop binary remains a thin painter.

---

## 9. Security considerations

1. **No secrets in `TileSpec`.** If a host accidentally puts a token in
   `McpRow.command` or `RemoteRow.host`, `tile_detail` must still not *add*
   secret fields. `from_name` only reads the haystack for needles; it does
   not echo it into the icon. Prefer host-side stripping of userinfo from
   remotes (`https://user:token@github.com/org/repo` becomes host
   `github.com`) before the row reaches `Workspace`.
2. **Auth names are public identifiers.** `openrouter` and `grok` are config
   keys. They are safe. `env_key = "OPENROUTER_API_KEY"` is a name of an env
   var, not the value, but we still do **not** put it on the tile. Names of
   env vars are a small leak of setup; skip them.
3. **MCP commands on the drawer.** Showing `npx -y @linear/mcp-server-linear`
   is acceptable (it is already in `config.toml`). Showing `env` / `headers`
   is not. First-code `tile_detail` uses `subtitle`, not the raw command, for
   MCP (transport only). The host may later add a "safe command" field that
   is argv0 + package, never env.
4. **Registry / CDN.** The desktop must not fetch dashboardicons on startup.
   Vendoring a small SVG subset (linear, cloudflare, github, slack, git, x)
   is a later assets task. Until then, paint the kind glyph.
5. **Audit.** Selecting a tile is not a sensitive event. Opening Customize
   (plan/26) is. This surface does not write config.

---

## 10. Testing strategy

TDD at inception. Write the tests in `integrations.rs` (`#[cfg(test)]`)
**first**, then the functions. Follow the shell crate's existing style:
many asserts per test, explicit `assert_ne!` on neighboring states, so
cargo-mutants has nowhere to hide.

### 10.1 Unit: `BrandIcon::from_name`

Table-driven. Each required row in §7.2 is a case. Also:

- empty, whitespace, `mystery` => `Generic`
- case fold: `NPX @Linear/MCP` => `Linear`
- order: `git@github.com:acme/app.git` => `Github` (not `Git`)
- whole-token `gh`: `gh` => `Github`, `high` => `Generic`
- `ds-flash` => `OpenRouter`
- `grok-4.6` => `Grok`
- `fake` => `Fake`
- slug() for every variant is the kebab string in §6.2
- `Generic.slug()` is `"generic"` and is **not** a catalog fetch

Kill mutants that swap two needles, drop `to_lowercase`, or make `gh` a
substring match.

### 10.2 Unit: `integration_tiles`

Build a `Workspace::new("demo", "grok-4.6")` and fill:

```
models: ["grok", "grok-4.6", "fake"]
mcp:    Linear (npx @linear/...), gh (npx ...server-github)
skills: ["review"]
hooks:  ["fmt"]
remotes: origin / github.com
worktrees: ["C:/src/demo", "C:/src/demo-wt"]
auth_providers: ["openrouter", "grok"]
```

Assert:

- order is the §6.3 kind order
- selected model tile is `Active`, others `Idle`
- `provider:grok` and `provider:fake` both appear (fake is in the catalog)
- Linear MCP icon is `BrandIcon::Linear`, id is `mcp:Linear` (or the row name)
- origin icon is `Github`, subtitle contains `github.com` and does not contain
  `@` or `http`
- worktree tile names are last path segments (`demo`, `demo-wt`)
- auth tiles are `Present`, names only, no `op://`
- empty workspace (`Workspace::new("p", "m")` with no extras) still yields at
  least the current model tile and one provider tile
- ids are unique

### 10.3 Unit: `filter_tiles`

- empty query and `"   "` return the full list, same order
- `"linear"` returns the Linear MCP tile (and nothing from `fake` unless the
  name also matches)
- `"mcp"` returns every `TileKind::Mcp` (matches kind)
- `"ready"` matches status `as_str`
- `"github"` matches a remote whose host is `github.com` via subtitle or icon
  slug
- unknown query returns `[]`
- filter does not reorder: relative order of hits equals input order

### 10.4 Unit: drawer

- `toggle_tile` first time sets `selected_tile`
- second time on the same id clears it
- toggling a different id replaces, does not stack
- `tile_detail` contains name, kind, status, slug
- `tile_detail` never contains `op://`, `Bearer`, or a 21+ char token-shaped
  string

### 10.5 Property (proptest)

- For any `Workspace` (strategy: short strings, 0..8 models, 0..8 mcp rows,
  0..8 skills / hooks / remotes / worktrees / auth names), every tile id is
  unique, every `from_name` result is a defined enum variant, and
  `filter_tiles(&tiles, "") == tiles`.
- For any query, every returned tile matches the query on at least one of
  the documented fields (the filter cannot invent hits).
- `filter_tiles(filter_tiles(tiles, q), q) == filter_tiles(tiles, q)`
  (idempotent).

Keep the strategy alphabetic / short so it does not generate secret-shaped
blobs. Reject strings that `SecretRef::looks_like_plaintext` would reject,
or simply cap generated strings at 20 chars.

### 10.6 Mutation

cargo-mutants over `integrations.rs` and the `Workspace` setters for remotes /
auth / hooks. Gate: ≥85% line, ≥80% branch, ≥70% killed (D21, D33). Highest
value mutants:

- needle order (`git` before `github`)
- `trim()` on the query
- `Active` vs `Idle` for the selected model
- D14: emitting an `openrouter` *provider* kind (must not)

### 10.7 Component / e2e (later, not first code)

- GPUI snapshot of the Apps tab with three brand tiles and an open drawer.
- E2E: filter `linear`, click the tile, drawer shows `Linear` and `ready` /
  `present`, screenshot contains no env values.

---

## 11. Implementation sequence (parent)

1. **Red.** Add `crates/multiplexer-shell/src/integrations.rs` with the types
   and `#[cfg(test)]` cases from §10.1 to §10.4. Wire `mod integrations` in
   `lib.rs`. Tests fail because the functions are missing or stubbed.
2. **Green.** Implement `BrandIcon::from_name`, `integration_tiles`,
   `filter_tiles`, `filter_workspace_tiles`, `toggle_tile`, `tile_detail`.
   Add the additive `Workspace` fields with empty defaults and setters.
3. **Refactor.** Share the lowercase / token-split helper so `from_name` and
   `filter_tiles` do not drift. Do not add GPUI.
4. **Chrome follow-up (same crate, separate commit).** Add
   `InspectorTab::Integrations`, palette row, `inspector.rs` body that
   renders `filter_workspace_tiles` as text (name + status) until GPUI
   tiles exist. Update the 7-tab tests to 8.
5. **Desktop follow-up.** Paint 48px glyphs. Vendor the six slugs. Drawer
   as a real expand. No CDN fetch.
6. **Plan/26 join.** Drawer "Customize" routes to the Customize panel for
   that id.

First code stops at step 3.

---

## 12. Open questions / risks

These are flagged, not decided here:

1. **Grok brand mark.** dashboardicons has no first-party `grok` slug. This
   doc maps Grok to `x` (X/xAI family) and still paints a `◆` glyph until we
   vendor a Multiplexer-owned Grok mark. Whether we commission a mark or keep
   the glyph is a branding call (D6).
2. **OpenRouter slug.** If the catalog has no `openrouter` file, first code
   still returns that slug and the painter falls back to the kind glyph. Do
   not block the projector on catalog coverage.
3. **Whether older MCP / Skills / Git tabs stay.** This doc keeps them as
   focused views. Collapsing them into Apps-only is a later UX call.
4. **Live MCP status on `McpRow`.** First code uses `Present`. Promoting
   plan/21 state onto the row needs a small `McpRow` additive field and a
   host copy. Not a projector redesign.
5. **Remote userinfo stripping.** The host must strip credentials before
   `set_remotes`. Whether we also add a belt-and-suspenders sanitizer in
   `set_remotes` (drop anything before `@` in the host field) is open. A
   sanitizer is cheap and mutation-testable; recommended but not required
   for first code.
6. **Mobile.** The paired app (plan/13) should eventually show the same
   directory over the wire contract. No new RPC is specified here. A later
   `integrations.list` method would return `TileSpec` JSON. Flagged for
   plan/04 if we want it in MVP.

**Flagged consistency note:** this doc is consistent with PLAN-CONTEXT
(server-centric runtime, GPUI desktop, D14 OpenRouter-as-config, D23 no
plaintext secrets) and with plan/26 (directory vs Customize). If D13 crate
layout or D1 stack flips, §6 and §11 must be revisited.

---

*Next: parent implements `crates/multiplexer-shell/src/integrations.rs` per §11
steps 1 to 3. See `plan/19-roadmap-and-milestones.md` for Phase 2 slotting of
the MCP/skills UI this directory sits on.*
