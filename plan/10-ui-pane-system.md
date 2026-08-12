# 10 — UI Pane System

**Status:** Draft (consistent with `docs/PLAN-CONTEXT.md`)
**Owner:** Multiplexer planning fan-out
**Scope:** The dockable/splittable/pop-out pane engine, the Outlook-style layout, the design system, command palette, keyboard-first workflows, component architecture, and component testing.

**Locked decisions applied:** D21 (mutation-testing scope — layout engine is a unit + property + mutation target), D33 (70% mutation score is the merge floor). See `docs/DECISIONS.md`.

---

## 1. Overview & Goals

The pane system is the **shell** of Multiplexer — the frame that holds every other surface (editor, terminal, browser, HAR, files, diff, agent activity, chat). It is one of the ten core differentiators (differentiator #5: *"Powerful pop-out pane UI"*) and is the primary vehicle for the design principles *Beautiful*, *Clean*, *Powerful UI*, and *Blazing fast*.

The pane system must deliver, in one coherent engine:

1. The **user's explicit layout spec** (Outlook-style left sidebar, center build pane, multi-purpose right bar, optional pop-up terminal, pop-out windows).
2. A **dockable / resizable / splittable** pane engine with **per-project saved layouts**.
3. **Pop-out windows** — detach any pane to its own OS window, re-dock, preserve layout across windows.
4. A **design system** — tokens, dark/light themes, smooth GPU animations.
5. A **command palette** — keyboard-first search across commands, panes, files, agents.
6. **Keyboard-first** workflows — keybindings and Vim-mode editor.
7. A **component architecture** — fine-grained reactive state, virtualized lists.
8. **Component testing** — GPUI element/component tests + pane-layout snapshot tests.

This doc is the *shell*; the surfaces that live inside panes (editor, terminal, browser, HAR) are specified in their own plan docs (`plan/09`, `plan/08`, `plan/11`, `plan/12`). We define the pane *containers*, *lifecycle*, *layout model*, and *interaction* here, and treat pane *content* as pluggable components.

---

## 2. The Layout Spec (user's explicit design)

The default workspace is a **three-column + optional bottom** arrangement, Outlook-style. Every region is a pane that can be split, resized, hidden, or popped out.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Title bar:  project  ·  branch  ·  model  ·  run/stop  ·  palette (⌘K)     │
├────────────┬───────────────────────────────────────────────┬─────────────────┤
│ LEFT       │  CENTER — BUILD PANE                          │ RIGHT BAR       │
│ SIDEBAR    │                                               │ (multi-purpose) │
│ (Outlook)  │  ┌─────────────────────────────────────────┐  │  Tabs:          │
│            │  │  Agent conversation (thread)            │  │  [Browser]     │
│ Chats      │  │  ────────────────────────────────────── │  │  [HAR]         │
│ Threads    │  │  Editor / diff view                     │  │  [Files]       │
│ Projects   │  │  (split vertically or horizontally)     │  │  [Diff]        │
│ Agents     │  └─────────────────────────────────────────┘  │  [Terminal]    │
│ Activity   │                                               │  [Agent act.]  │
│            │                                               │  [Model info]  │
│  ◀ collapse│                                               │  ▶ collapse    │
├────────────┴───────────────────────────────────────────────┴─────────────────┤
│  BOTTOM: optional pop-up terminal (slides up from the bottom edge)            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Left sidebar (Outlook-style)

- A vertical rail of **sections**: **Chats / Threads**, **Projects**, **Agents**, **Activity**.
- Each section is a **virtualized list** (see §7.3) — thousands of threads/agents scroll at 60fps.
- **Collapsible**: the whole sidebar collapses to a slim icon rail (and can be fully hidden). Sections can be reordered, and individual sections can be collapsed to headers.
- The sidebar is itself a pane: it can be popped out, split, or docked to the right instead of the left (layout is user-configurable, not hard-coded).
- **Threads** show: title, model badge, last-message preview, unread/attention indicator, status (running / waiting-approval / idle / error).

### 2.2 Center — the build pane

- The **main agent conversation** (the thread you are actively working in) plus the **editor** and **diff view**.
- The center is a **splittable** region: conversation | editor side-by-side, or editor on top / diff below. Splits are arbitrary and recursive (see §3).
- This is where inline diff-apply, LSP, multi-cursor, and Vim mode live (delegated to `plan/09`).
- The center pane is the default focus target on launch.

### 2.3 Right bar (multi-purpose, swappable tabs)

- A **tabbed** pane hosting: **Browser** (system-browser preview via CDP — `plan/11`), **HAR profiler** (`plan/12`), **Files**, **Diff**, **Terminal**, **Agent activity**, **Model info**.
- Tabs are **swappable**: drag to reorder, drag a tab out to pop it into its own window, drag a tab into the center or left to re-dock it.
- Only one tab is visible at a time by default, but the right bar can be **split** so two tabs are visible simultaneously (e.g. Browser + HAR side by side — the natural pairing for debugging).
- The right bar is collapsible like the left sidebar.

### 2.4 Bottom — pop-up terminal

- An **optional** terminal that **slides up** from the bottom edge (Ghostty-class terminal — `plan/08`).
- Toggled by a keybinding (e.g. `` Ctrl+` ``) or from the palette; animates up/down smoothly.
- It is a **pane** like any other: it can be popped out, split, or converted into a permanent bottom dock.

### 2.5 Pop-out (every pane → its own OS window)

- **Every** pane — sidebar section, build pane, right-bar tab, bottom terminal — can be **detached** into its own native OS window.
- Detached windows are **layout-preserving**: the pane's position, size, and content survive the round-trip (detach → re-dock).
- Multi-window is a first-class state, not a hack: the layout tree spans windows (see §4).

---

## 3. The Pane / Layout Engine

### 3.1 Layout tree model

The workspace layout is a **binary tree of split nodes and leaf panes**:

```
LayoutTree
├── SplitNode { axis: Horizontal | Vertical, ratio: f32, children: [Node, Node] }
├── LeafNode { pane: PaneId, tab_group: Option<TabGroup> }
└── Root { window: WindowId, child: Node }
```

- **SplitNode** — divides its region along an axis at a normalized ratio (0.0–1.0). Recursive, so any pane can be split arbitrarily ("split-anything panes" — Orca baseline bar).
- **LeafNode** — holds a single pane, or a **tab group** (multiple panes stacked with a tab bar, e.g. the right bar).
- **Window** — each OS window owns a root node. The **primary window** holds the default three-column layout; **pop-out windows** hold a single detached subtree.

The tree is **pure and serializable** — it contains only structural data (ids, axes, ratios, tab order), never live objects. This is what makes layout persistence, pop-out, and snapshot testing trivial.

### 3.2 Pane registry & content

- A **`PaneRegistry`** maps `PaneId → PaneDescriptor` where the descriptor declares: `kind` (editor / terminal / browser / har / files / diff / chat / agent-activity / model-info), a **factory** that builds the GPUI element, and **capabilities** (can-split, can-pop-out, can-tab, min-size).
- Panes are **content-agnostic containers**: the engine manages geometry, focus, resize handles, tab bars, and lifecycle; the content component renders inside.
- A pane's **content state** (e.g. which file the editor shows, which thread the chat shows) is stored separately from the layout tree, keyed by `PaneId`, so detach/re-dock and layout restore never lose content.

### 3.3 Rendering in GPUI

- The layout tree is **projected into a GPUI element tree** each frame. Because the tree is small (dozens of nodes) and GPUI diffs elements, re-rendering on layout change is cheap.
- **Resize handles** are GPUI interactive elements that mutate `ratio` on the ancestor `SplitNode`; drag is throttled to the frame budget (<16ms) and the change is applied as a single state update.
- **Splitting** inserts a new `SplitNode` and a new `LeafNode`; **closing** removes a leaf and collapses a single-child split back to its child (no degenerate one-child splits).
- **Focus** is a first-class value: exactly one pane has keyboard focus; focus follows the layout tree (a `focus_path` from root to leaf), so it survives splits and window changes.

### 3.4 Saved layouts (per-project)

- Layouts are **per-project**: opening project A restores its pane arrangement; project B has its own.
- A layout snapshot = the serialized `LayoutTree` + pane content descriptors + window geometry. Stored as JSON in the project's local config (`.multiplexer/layout.json`, gitignored) and in the server-side read model for sync to the mobile app.
- **Layout presets**: built-in named layouts (e.g. *"Debug"* = center + right bar with Browser+HAR split; *"Focus"* = center only; *"Review"* = center + right-bar Diff). Users can save custom presets.
- **Restore semantics**: on launch, the primary window restores the last layout for the opened project; panes whose content is unavailable (e.g. a closed thread) restore as empty placeholders rather than erroring.

---

## 4. Pop-out Windows

### 4.1 Detach

- Dragging a pane/tab off the edge (or `⌘⇧D` / palette "Pop out pane") **detaches** it: the subtree rooted at that leaf is moved into a **new OS window**.
- The new window gets its own `Root`; the original location becomes an **empty placeholder** (a ghost slot) so the layout tree shape is preserved and re-docking is deterministic.
- Detach is a **pure tree transformation**: `detach(window, path) → (new_window_root, ghost_leaf)`.

### 4.2 Re-dock

- Dragging a window's content back over a pane edge (or palette "Dock pane") **re-attaches** it: the ghost slot is replaced by the returned subtree, and the window closes if it becomes empty.
- Re-dock is the inverse transformation and is **layout-preserving** — the pane returns to its prior position, size, and content.

### 4.3 Layout preservation across windows

- Because the layout tree is **pure and serializable**, the entire multi-window workspace is one forest of trees that can be saved and restored atomically.
- **Window identity** is stable: a detached window keeps its `WindowId` and geometry across layout saves, so restoring a session reopens the same set of windows in the same positions.
- **Content follows the pane, not the window**: `PaneId`-keyed content state means a pane keeps its thread/file/browser session whether it lives in the primary window or a pop-out.

### 4.4 Multi-window constraints

- **Single process**: all windows share one process and one server runtime (the native binary). Pop-out windows are GPUI windows in the same app, not separate processes — this keeps state trivially shared and memory low.
- **Focus arbitration**: only one window has active keyboard focus; the command palette and keybindings operate on the focused window's pane.
- **Windows-first**: GPUI's multi-window support is exercised first on Windows; macOS/Linux follow.

---

## 5. Design System

### 5.1 Design tokens

All visual values are **design tokens** — a single source of truth consumed by every component. Tokens are typed (Rust enums/structs) so misuse is a compile error, not a runtime surprise.

| Token group | Examples |
|---|---|
| **Color** | `bg.canvas`, `bg.surface`, `bg.surfaceHover`, `bg.surfaceActive`, `border.subtle`, `text.primary`, `text.secondary`, `text.muted`, `accent`, `accentHover`, `status.running`, `status.waiting`, `status.error`, `status.success` |
| **Spacing** | `space.1`…`space.8` (4px base scale), `sidebar.width`, `rightbar.width`, `pane.gap` |
| **Radius** | `radius.sm`, `radius.md`, `radius.lg` (pane corners, buttons, inputs) |
| **Typography** | `font.ui` (system UI), `font.mono` (editor/terminal), sizes `type.xs`…`type.xl`, weights, line-heights |
| **Motion** | `motion.fast` (120ms), `motion.medium` (200ms), `motion.slow` (320ms), easing curves |
| **Elevation** | `shadow.pane`, `shadow.popover`, `shadow.window` (for pop-out affordance) |

### 5.2 Dark / light themes

- **Two built-in themes** (dark default, light) plus a **system-follow** mode. Each theme is a full token mapping; components reference tokens, never raw colors, so theming is exhaustive and automatic.
- Theme switching is **instant and animated** (cross-fade via GPUI) and is a single state change — no component opt-in required.
- Contrast is tuned for accessibility (WCAG AA on text/background pairs); status colors are distinguishable in both themes.

### 5.3 Smooth animations (GPU-rendered)

- All motion is **GPU-composited** in GPUI — no layout thrash. Targets: 60fps+, input latency <16ms.
- **Animated interactions**: sidebar collapse/expand, bottom terminal slide-up, tab-swap, pane pop-out/dock transition, theme cross-fade, list item enter/exit.
- **Motion budget**: animations are short (120–320ms) and respect `prefers-reduced-motion` (a token-driven "reduce motion" flag that disables non-essential animation).
- **Performance is a design constraint**: animations never block input; a running animation yields to the frame budget (see `plan/16`).

### 5.4 "Beautiful, clean, powerful"

- **Beautiful**: crisp typography, deliberate spacing, subtle elevation, GPU-smooth motion.
- **Clean**: progressive disclosure — calm by default, complexity revealed on demand; every pane earns its place; no redundant chrome.
- **Powerful**: dense, keyboard-first surfaces (palette, Vim, splits) that reward power users without cluttering the default view.

---

## 6. Command Palette

### 6.1 Scope

A **keyboard-first, fuzzy-searchable** palette (invoked with `⌘K` / `Ctrl+K`) that searches across **four namespaces**:

| Namespace | Examples |
|---|---|
| **Commands** | "New thread", "Run agent", "Pop out pane", "Toggle terminal", "Switch theme", "Save layout", "Open diff" |
| **Panes** | "Focus editor", "Show HAR", "Open files pane", "Split right", "Close pane" |
| **Files** | Open any file in the workspace (native search — Orca baseline) |
| **Agents / threads** | Jump to a running agent, a thread, an approval request, a subagent |

### 6.2 Behavior

- **Fuzzy matching** across all namespaces with ranked results; keyboard navigation (↑↓, Enter, Esc).
- **Actions are real commands**: palette items invoke the same command system as keybindings and menus (single `Command` registry — see §7.2), so there is exactly one path for any action.
- **Context-aware**: results are filtered by the focused pane (e.g. in the editor, file results rank higher; in the right bar, pane commands rank higher).
- **Async results**: file/agent search is debounced and virtualized; the palette stays responsive with thousands of candidates.
- **Extensible**: provider adapters and future plugins register commands into the same registry, so the palette grows with the product.

---

## 7. Keyboard-First

### 7.1 Keybinding system

- A **central keybinding map** (JSON, user-editable) mapping chords → commands. Defaults follow platform conventions (Windows `Ctrl`, macOS `⌘`).
- **Context-sensitive**: bindings are scoped to a pane kind (editor vs terminal vs palette vs global), so `Ctrl+W` means "close tab" in a tab group and "delete word" in the editor.
- **Discoverability**: the palette shows the keybinding for every command; a "Show all keybindings" command lists the full map.
- **Vim-mode editor** is a first-class keybinding context (delegated to `plan/09`), but the pane system guarantees the editor's keybindings never leak into global navigation and vice versa.

### 6.2 Core navigation bindings (defaults)

| Action | Default |
|---|---|
| Command palette | `Ctrl+K` |
| Toggle left sidebar | `Ctrl+B` |
| Toggle right bar | `Ctrl+Shift+B` |
| Toggle bottom terminal | `` Ctrl+` `` |
| Split pane right / down | `Ctrl+\` / `Ctrl+Shift+\` |
| Focus next / prev pane | `Ctrl+Tab` / `Ctrl+Shift+Tab` |
| Pop out focused pane | `Ctrl+Shift+D` |
| Dock focused window | `Ctrl+Shift+E` |
| Close pane / tab | `Ctrl+W` |

---

## 8. Component Architecture

### 8.1 Composition

- **Panes are components**: each pane kind is a GPUI component (`EditorPane`, `TerminalPane`, `BrowserPane`, `HarPane`, `FilesPane`, `DiffPane`, `ChatPane`, `AgentActivityPane`, `ModelInfoPane`).
- **The engine is a set of components**: `Workspace` (root), `WindowRoot`, `SplitView`, `TabGroup`, `PaneFrame` (chrome: title bar, resize handle, tab bar, pop-out button), `Sidebar`, `BottomTerminal`, `CommandPalette`.
- **Composition over inheritance**: panes implement a `Pane` trait (descriptor, factory, focus handling, serialization); the engine composes them without knowing their internals.

### 8.2 State management (fine-grained, reactive)

- **Fine-grained reactive state** via GPUI's reactive model: each pane holds a small, typed `Model<T>`; components subscribe only to the slices they render. A keystroke in the editor re-renders the editor's visible lines, not the whole window.
- **The layout tree is one reactive model** (`Model<LayoutForest>`); structural changes (split, resize, detach) update it and re-render only affected splits.
- **Content state is separate from layout state** (`PaneId → Model<PaneContent>`), so detach/re-dock and layout restore never lose content and never re-render unrelated panes.
- **Server state** (threads, agents, activity) flows in from the event-sourced read model over the JSON-RPC contract (`plan/04`, `plan/06`) and is projected into pane models; the UI is a pure view of server truth, not a second source of truth.

### 8.3 Virtualized lists (chats / threads / agents)

- The left sidebar's **Chats/Threads**, **Agents**, and **Activity** lists are **virtualized**: only visible rows are materialized as GPUI elements; scrolling recycles rows.
- Virtualization is **row-height-aware** (fixed or measured) and supports **variable-height** rows (multi-line previews) via a measured offset index.
- **Live updates** (a new subagent spawns, a thread's status changes) patch the visible window of rows without rebuilding the list — critical for "dozens of concurrent subagents" (performance target).
- The same virtualized list primitive is reused for the palette's file/agent results and the HAR waterfall (`plan/12`).

---

## 9. Component Testing

### 9.1 GPUI element / component tests

- Each pane component and engine component has **co-located `#[cfg(test)]` tests** that render the component in a headless GPUI test harness and assert on the resulting element tree / state.
- **Behavioral tests**: split inserts a `SplitNode`; resize changes `ratio`; close collapses a single-child split; detach produces a ghost + new window root; re-dock restores the subtree.
- **Focus tests**: focus follows the layout path; focus survives split/close/detach; exactly one pane holds focus.
- **Reactive tests**: a content-state change re-renders only the subscribed pane (assert via render-count instrumentation).

### 9.2 Snapshot tests for pane layouts

- Because the layout tree is **pure and serializable**, layout snapshots are first-class test artifacts.
- **Structural snapshots**: serialize the `LayoutTree` to a canonical JSON and snapshot it — a split/resize/detach sequence produces a deterministic, diffable snapshot.
- **Visual snapshots**: render a pane layout in the headless harness and snapshot the rendered output (GPUI supports deterministic rendering for snapshotting); golden files catch unintended visual regressions in the design system.
- **Round-trip tests**: serialize → deserialize → re-render must be identity for every layout (property-based with proptest over random split/resize/detach sequences — see `plan/15`).

### 9.3 Integration with the test pipeline

- Component tests run in the **component** CI gate (after unit+property+mutation, before integration/e2e — per PLAN-CONTEXT §Testing).
- Snapshot goldens are committed and reviewed like code; a design-token change that shifts every snapshot is visible as one intentional diff, not silent drift.
- E2E (`plan/15`) drives the real app headlessly to verify pane interactions end-to-end (split, pop-out, palette, theme switch).

### 9.4 Layout-engine unit tests (D21)

The layout engine is **pure and serializable** — detach/re-dock are pure tree transformations over structural data (ids, axes, ratios, tab order), never live objects. This makes it the ideal target for unit + property + mutation testing (D21). The transformation functions get co-located `#[cfg(test)]` unit tests:

- **`detach(window, path)`** — produces a new window root + a ghost placeholder leaf; the original location becomes a ghost slot; the tree shape is preserved.
- **`re_dock(window, ghost_path, subtree)`** — replaces the ghost slot with the returned subtree; closes the window if it becomes empty.
- **`split(node, axis, ratio)`** — inserts a new `SplitNode` + `LeafNode`; the new child is placed correctly along the axis at the given ratio.
- **`collapse(node)`** — removes a leaf and collapses a single-child split back to its child (no degenerate one-child splits).
- **`focus_routing(tree, focus_path)`** — focus follows the layout tree from root to leaf; exactly one pane holds focus; focus survives split/close/detach.

Each test asserts on the resulting tree structure (node kinds, axes, ratios, ids, tab order) and on invariants (no one-child splits, exactly one focus, ghost slots present after detach).

### 9.5 Layout-engine property tests (proptest) (D21)

Because the transformations are pure, they are property-tested with proptest over random split/resize/detach/re-dock sequences:

- **Detach-then-re-dock is identity:** for any layout and any valid pane path, `re_dock(detach(layout, path))` restores the original subtree — position, size, and content survive the round-trip.
- **`focus_path` is always a valid root→leaf path:** for any reachable layout, the focus path resolves to exactly one leaf pane and never dangles after split/close/detach.
- **Serialize → deserialize → re-render is identity:** for any layout, serializing to canonical JSON, deserializing, and re-rendering yields the identical tree (round-trip identity, per §9.2).

These property tests run in the **unit+property** CI gate (see §9.7).

### 9.6 Layout-engine mutation testing (D21)

The layout engine is a **mutation-testing target** (cargo-mutants), consistent with D21 (mutation testing applies to all core logic, including the pane system's layout engine) and D33 (70% mutation score is the **merge floor**):

- Run cargo-mutants against the layout-engine crate; the **≥70% mutation score** gate applies (the merge floor, per D33 — the bar may rise over time).
- Mutants target the transformation functions and invariants above: detach/re-dock, split/collapse, focus routing, and serialization round-trip. A surviving mutant that breaks an invariant (e.g. a one-child split, a lost focus path, a non-identity round-trip) must be killed by the unit/property tests.
- Mutation runs in the **mutation** CI gate (see §9.7).

### 9.7 CI gate for pane-system changes (D21)

Pane-system changes must pass the **full gate chain** before merge, consistent with `plan/15` and PLAN-CONTEXT §Testing:

**fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e → coverage**

- **fmt / clippy:** formatting and lint (warnings denied) for the pane-system crates.
- **unit+property:** the layout-engine unit tests (§9.4) and proptest properties (§9.5).
- **mutation:** cargo-mutants with **≥70% mutation score** (merge floor, D33) on the layout engine (§9.6).
- **integration:** real core + mock agent, asserting on the read model (layout persistence/sync).
- **component:** GPUI element/component tests + pane-layout snapshot tests (§9.1–9.2).
- **e2e:** headless drive of the real app for pane interactions (split, pop-out, palette, theme switch) — merge gate + nightly (D32).
- **coverage:** ≥85% line, ≥80% branch.

All green before merge; no blind CI.

---

## 10. Open Questions

Per PLAN-CONTEXT, these are pending user decisions and must not be decided unilaterally. This doc references them where relevant; they are tracked in `plan/20-risks-and-open-questions.md`.

1. **Stack (OQ1):** Rust + GPUI is assumed throughout this doc (recommended). If Electron+React were chosen, the entire pane engine, design system, and component-testing strategy here would be rewritten — this doc is contingent on GPUI.
2. **Orca baseline scope (OQ7):** "Split-anything panes" and "native search" are Orca baseline features this doc assumes in full. Whether the MVP ships the *complete* split/pop-out surface or a defensible subset is open.
3. **Editor scope (OQ4):** The center build pane hosts the editor; whether the MVP ships the full native editor or a lighter one affects how much of the center-pane split surface is exercised early.
4. **Layout persistence location:** Per-project layouts are stored locally (`.multiplexer/layout.json`) and mirrored to the server read model for mobile sync. Whether layout sync to mobile is MVP or deferred is open.
5. **Multi-window on non-Windows:** Pop-out windows are Windows-first (consistent with Windows-first shipping); macOS/Linux multi-window timing follows the platform rollout.
6. **Default layout preset:** Whether the default workspace is exactly the three-column Outlook layout above, or a slightly different default (e.g. right bar hidden by default), is a UX decision deferred to the user.

---

*Next: `plan/11-system-browser-integration.md` — detect/import installed browsers, launch/authorize, drive via CDP, no bundled Chromium.*
