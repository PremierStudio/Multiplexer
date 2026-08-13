# 34: Chat, Composer, and Center-Column Visual

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Desktop shell / GPUI center column
**Depends on:** `10-ui-pane-system.md` (Outlook center, design tokens, reduced motion), `15-testing-strategy.md` (headless first, component later)
**Feeds:** desktop `ShellView::center` rewrite, `multiplexer-shell` empty-state module, later markdown slice

This document is consistent with `docs/PLAN-CONTEXT.md` and `docs/DECISIONS.md`. It does not change crate layout (D13), stack (D1), or MVP provider scope (D3). New decisions proposed here are numbered **D90+** in the style of `docs/DECISIONS.md`. They are proposals for the decision log, not locked decisions. If `plan/27` through `plan/33` claim this D-range, remap on merge; the names below are the contract.

**Locked decisions applied:** D1 (Rust + GPUI), D3 (Grok-first copy), D13 (`multiplexer-*` crates; headless chrome lives in `multiplexer-shell`, projection in `apps/multiplexer-desktop`), D21 / D33 (pure chrome helpers are unit + mutation targets; 70% mutation score is the merge floor).

**Relationship to plan/10:** plan/10 specifies the pane *shell* (Outlook three-column, tokens, motion). This doc specifies the *conversation surface* that fills the center leaf today: thread header, transcript bubbles, empty state, busy chrome, and composer. The center remains a splittable region that will later host the editor (`plan/09`). This visual does not claim the whole center forever.

**Parent implement (first code):** `multiplexer-shell` empty-state + message-chrome helpers with headless tests, then rewrite `ShellView::center` in `apps/multiplexer-desktop/src/main.rs`. No cargo in this planning pass.

---

## 1. Problem

The center column is a working control surface with a placeholder look. Today (`apps/multiplexer-desktop/src/main.rs`):

1. **Empty state** is one muted sentence (`empty_center()`). Suggestion prompts are a row of tiny chips parked on the composer, mixed with "dir" and "Copy last". That is not Cursor / Orca / Claude Desktop class.
2. **Bubbles** are unlabeled glass pills. There is no avatar, no hover copy, no thread header. Role chrome is a small "You" / "Agent" line inside the bubble.
3. **Composer** is a short well plus a rectangular ghost "Send" button. There is no circular accent send, no in-composer model pill, no visible-but-disabled attach, no dedicated slash hint bar.
4. **Busy** is a title-bar Stop button and `workspace.busy`. There is no shimmer, no "Grok is working" line, and no elapsed clock.
5. **Markdown** is unparsed plain text with no wrap contract.

The product positioning is "beautiful, blazing-fast." The first thing a user stares at is this column. It has to look finished even while markdown, attachments, and timestamps stay later slices.

---

## 2. Goals and non-goals

### 2.1 Goals (this slice)

- Center column reads as a premium agent product on a 1280-wide window, dark glass theme.
- Empty thread: large mark + **four icon tiles** (not chips).
- Transcript: user right / glass-accent, agent left / avatar glyph + role label, copy-on-hover.
- Busy: shimmer + "Grok is working" + elapsed.
- Composer: taller glass well (min 56), circular accent send with arrow, model pill inside the well, slash hint bar, paperclip visible and disabled ("coming").
- Thread header in the center: title, model, status, interrupt.
- All layout rules that tests can own live in `multiplexer-shell` (no GPUI types).

### 2.2 Non-goals (this slice)

- Markdown parse, syntax highlight, or streaming token paint (later slice, §8).
- Attachments, image paste chips, or drag-and-drop (paperclip is a disabled affordance only).
- Per-message timestamps in the UI (reserve the field, do not paint).
- Virtualized transcript (fine at dozens of messages; virtualize when the list is a measured problem).
- Changing left-rail preview copy (`You:` / `Agent:`) or the inspector.
- Wire-contract changes. Elapsed is not a server field in v1 (§7).
- Light theme, mobile transcript, or a new `multiplexer-ui` crate.

---

## 3. Target layout

```
┌─ thread header (44px) ──────────────────────────────────────────┐
│  Title (ellipsis)          [grok]   running · 12s     [ Stop ]  │
│  ════════ shimmer 2px (busy only) ════════════════════════════  │
├─────────────────────────────────────────────────────────────────┤
│  transcript (flex-1, min-h 0, pad 16, gap 12)                   │
│                                                                 │
│  empty:                                                         │
│           [ large Multiplexer mark ]                            │
│           What should we build?                                 │
│           Four things to start.                                 │
│     ┌────────────┐  ┌────────────┐                              │
│     │ ✦  What…   │  │ ▤  Summar… │                              │
│     └────────────┘  └────────────┘                              │
│     ┌────────────┐  ┌────────────┐                              │
│     │ ⎇  Git …   │  │ ⚗  Run …   │                              │
│     └────────────┘  └────────────┘                              │
│                                                                 │
│  messages:                                                      │
│   [G] Grok                                          You         │
│   ┌─────────────────┐                    ┌─────────────────┐    │
│   │ agent body      │                    │ user body       │    │
│   │           [copy]│                    │           [copy]│    │
│   └─────────────────┘                    └─────────────────┘    │
│                                                                 │
│   Grok is working · 12s     (busy row after last message)       │
│                                                                 │
├─ slash hint bar (22px, only when draft is a slash or idle hint) ┤
│  /new  start a new chat                                         │
├─ composer well (min 56) ────────────────────────────────────────┤
│  [📎]  Message Grok…                                 [grok ▾] [➤]│
└─────────────────────────────────────────────────────────────────┘
```

The composer chips that currently sit above the well **go away**. Their four catalog prompts become empty-state tiles. "Copy last" is no longer a chip; it is per-bubble hover plus the existing `copy_last_message` action.

---

## 4. Visual spec

Tokens come from `apps/multiplexer-desktop/src/theme.rs` and plan/10 §5. New named helpers may be added on `Theme` (bubble user fill, send circle, tile fill). Do not invent a second token file.

### 4.1 Thread header (always on)

A 44px glass strip at the top of the center pane, not the window title bar.

| Slot | Source | Paint |
|---|---|---|
| Title | `selected_thread().title` | Primary text, one line, ellipsis. Default "New chat". |
| Model | `workspace.model` | Quiet pill, same visual language as the in-composer model pill. Click cycles (`ClientAction::CycleModel`). |
| Status | `thread.status` + `workspace.busy` | `idle` muted, `running` accent, `error` danger. When busy, append the same elapsed string as the working row. |
| Interrupt | `workspace.busy` | Danger "Stop" ghost. Hidden when idle. Same handler as title-bar Stop (`ClientAction::Interrupt`, catalog id `stop`). |

Header is a second *projection* of existing actions. Do not add a second catalog id for Stop or Cycle model.

### 4.2 Message bubbles

`message_chrome(role)` is the headless contract. GPUI projects it.

| Role | Align | Fill | Avatar | Role label |
|---|---|---|---|---|
| `User` | End (right) | Glass-accent: `hsla(0.58, 0.45, 0.28, 0.55)` + `Theme::hairline()`, existing user bubble | None | `"You"` |
| `Assistant` | Start (left) | Surface: `hsla(0.0, 0.0, 1.0, 0.06)` + hairline | 24px circle, accent ring, glyph `"G"` | `"Grok"` |

Shared chrome:

- Max width 640px (`message_chrome(role).max_width_px == 640.0`).
- Radius `rounded_xl`, `Theme::shadow()`.
- Pad 12 / 10.
- Body is v1 plain text (§8). Wrap at the bubble width. Preserve `\n`. Do not collapse spaces.
- **Copy on hover:** a 20px copy glyph in the bubble's top-right, opacity 0 until hover (or keyboard focus). Click copies *that* message's `text`. Palette / `copy_last_message` still copies the last message.
- Timestamp: not painted. Later optional under the role label (`text-xs`, muted). Do not add `created_at` to `ChatMessage` in this slice.

Agent row layout: avatar (top-aligned) + column(role label, bubble). User row: column(role label right-aligned, bubble), no avatar.

Do not use "Agent" in the bubble label. Left-rail `thread_preview` may keep `Agent:` (out of scope).

### 4.3 Streaming / busy

Visible whenever `workspace.busy` is true (send through `push_assistant` / `mark_error` / `mark_interrupted`).

1. **Shimmer bar:** 2px under the thread header, accent gradient sliding left to right on a 1.2s loop. `prefers-reduced-motion` (plan/10 §5.3): static accent bar, no slide.
2. **Working row:** after the last transcript message (or under the empty mark if a send is in flight with no assistant text yet). Copy from `working_copy(elapsed_secs)`:
   - `0..=59` → `Grok is working · {n}s`
   - `>= 60` → `Grok is working · {m}m {s}s` (example: 72 → `Grok is working · 1m 12s`)
3. Composer send is inert while busy (already true in `ShellView::send`). The circular send paints muted.
4. Interrupt stays available in the thread header and the title bar.

Elapsed clock: **host-only Instant**. See §7.

### 4.4 Empty state

Shown when the selected thread exists and `messages.is_empty()` **and** `!workspace.busy`.

1. Large mark: 56px Multiplexer glyph (reuse banner mark if already in-tree; otherwise a 56px rounded square with a bold "M", accent fill at 0.18 alpha). Centered.
2. Title: `What should we build?` (primary, ~18px).
3. Subtitle: `Pick a starting point, or type below.` (muted).
4. **Four tiles**, 2×2 grid, gap 10, each tile min 148×76, max width shared. Not chips. Not the current `chip()` helper.

`empty_state_tiles()` returns exactly these four, in this order, with these catalog ids:

| id | icon key | title | subtitle | on click |
|---|---|---|---|---|
| `chip_what` | `spark` | What can you do? | Capabilities of this session | `set_draft` + `Send` |
| `chip_summarize` | `repo` | Summarize this repo | Read the tree and describe it | `set_draft` + `Send` |
| `chip_git_status` | `branch` | Git status | Porcelain in the terminal | `run_shell("git status")` |
| `chip_test` | `flask` | Run the tests | `cargo test` in this workspace | `set_draft("Run the tests")` + `Send` |

`chip_test` is a **send**, not a silent `cargo test`. The agent owns the test run. This matches the catalog label already in `controls.rs` (`"Run the tests"`) and replaces the current desktop "dir" chip.

Tiles hide as soon as the first message lands. They do not persist under a transcript. They do not sit on the composer.

Icon keys are headless enums. GPUI maps them to simple vector marks (no emoji dependency).

### 4.5 Composer

Replace the short well + rectangular Send + chip row.

**Well**

- Glass fill `hsla(0.0, 0.0, 1.0, 0.06)`, hairline; focused hairline is `Theme::accent()`.
- **Min height 56px.** Grows with wrapped draft, cap 8 lines (~168px), then the draft clips (scroll later).
- Pad 12 left, 8 right, 8 vertical.
- One row when the draft is a single line: `[paperclip] [draft flex-1] [model pill] [send circle]`.
- Multi-line: draft uses the full width above a 36px footer that holds paperclip, model pill (left) and send (right).

**Placeholder**

- `composer_placeholder()` → `Message Grok…`
- Shown only when `draft.is_empty()`. Keyboard hints do **not** live inside the placeholder.

**Send**

- Circular, 36px, `Theme::accent()` fill, arrow glyph (right-pointing). Not a ghost rectangle, not the word "Send" as the primary mark.
- Accessible name remains "Send" (`controls` id `send`).
- Muted (`Theme::send_bg()`, no accent) when draft is empty or `workspace.busy`.
- Click / Enter: existing `send()`. Shift+Enter: newline (unchanged).

**Model pill (inside the well)**

- Compact pill: current `workspace.model`, muted chevron.
- Click: `ClientAction::CycleModel` (same as inspector / header).
- Not a new catalog id.

**Paperclip**

- Visible, 28px ghost, left of the draft (or left of the footer).
- **Disabled.** Cursor default, 0.45 opacity.
- Hover tooltip: `coming`.
- Catalog: add id `attach` on `Surface::Composer` (see §6). Handler is a no-op that flashes `coming` (reuse `ShellView.flash`). Do not open a file picker.

**Slash hint bar**

- 22px row *above* the well, not inside it.
- If `parse_slash(&draft)` is `Some(cmd)`: accent text `/{token}  {slash_hint(cmd)}`.
- Else: muted caption `Enter send · Shift+Enter newline · / for commands`.
- Empty draft still shows the muted caption (this is where the old placeholder hints go).

### 4.6 Markdown (later slice, specified now)

v1 stays **plain text**. The wrap contract is in force now so a later parse cannot change bubble width:

- Word wrap at `max_width_px`. Long tokens (URLs, hashes) break rather than overflow the pane.
- Newlines in `ChatMessage.text` are hard breaks.
- No inline styles, no lists, no headings in v1.

**Later slice (not this PR):**

- Fenced ` ``` ` blocks: monospaced font, filled surface (`hsla(0.0, 0.0, 1.0, 0.04)`), 8px pad, hairline, no syntax colors in the first markdown cut.
- Inline `` `code` ``: monospaced, 4px pad, same fill.
- Headings, lists, links after code blocks.
- Streaming: append into the last assistant bubble; do not remount on every token.

Do not add a markdown crate in this slice.

---

## 5. Headless model (TDD)

New module: `crates/multiplexer-shell/src/empty_state.rs`. Re-export from `lib.rs`. No GPUI types. No `Instant`. No `Workspace` mutation (read-only helpers), except tile click remains a host concern.

### 5.1 API

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileIcon {
    Spark,
    Repo,
    Branch,
    Flask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileAction {
    SendPrompt,
    RunShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuggestionTile {
    pub id: &'static str,
    pub icon: TileIcon,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub payload: &'static str,
    pub action: TileAction,
}

pub fn empty_state_tiles() -> [SuggestionTile; 4];

pub fn composer_placeholder() -> &'static str;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BubbleAlign {
    Start,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MessageChrome {
    pub align: BubbleAlign,
    pub role_label: &'static str,
    pub show_avatar: bool,
    pub avatar_glyph: Option<&'static str>,
    pub copy_on_hover: bool,
    pub max_width_px: f32,
}

pub fn message_chrome(role: Role) -> MessageChrome;

pub fn working_copy(elapsed_secs: u64) -> String;

pub fn idle_composer_hint() -> &'static str;

pub const COMPOSER_MIN_HEIGHT: f32 = 56.0;
pub const BUBBLE_MAX_WIDTH: f32 = 640.0;
pub const ATTACH_TOOLTIP: &str = "coming";
```

Tile payloads (locked):

| id | action | payload |
|---|---|---|
| `chip_what` | `SendPrompt` | `What can you do?` |
| `chip_summarize` | `SendPrompt` | `Summarize this repo` |
| `chip_git_status` | `RunShell` | `git status` |
| `chip_test` | `SendPrompt` | `Run the tests` |

`composer_placeholder()` is exactly `Message Grok…` (ellipsis character U+2026, matching today's string prefix).

`idle_composer_hint()` is exactly `Enter send · Shift+Enter newline · / for commands`.

`message_chrome`:

- `Role::User` → `align: End`, `role_label: "You"`, `show_avatar: false`, `avatar_glyph: None`, `copy_on_hover: true`, `max_width_px: 640.0`.
- `Role::Assistant` → `align: Start`, `role_label: "Grok"`, `show_avatar: true`, `avatar_glyph: Some("G")`, `copy_on_hover: true`, `max_width_px: 640.0`.

`working_copy` formatting is specified in §4.3. Never returns an empty string. Always starts with `Grok is working`.

### 5.2 Required unit tests (co-located)

Names are the contract the parent must land first:

1. `empty_state_tiles_returns_four`
   - `empty_state_tiles().len() == 4`
   - ids `["chip_what", "chip_summarize", "chip_git_status", "chip_test"]`
   - titles match §4.4
   - icons are four distinct `TileIcon` values
   - `chip_git_status.action == RunShell` and payload `git status`
   - the other three are `SendPrompt`
2. `composer_placeholder_is_message_grok`
   - equals `Message Grok…`
   - does not contain `Enter` or `/help` (hints moved out)
3. `message_chrome_user_aligns_end`
   - `message_chrome(Role::User).align == BubbleAlign::End`
   - label `You`, no avatar
4. `message_chrome_agent_aligns_start`
   - `message_chrome(Role::Assistant).align == BubbleAlign::Start`
   - label `Grok`, avatar `Some("G")`, `copy_on_hover`
5. `working_copy_formats_seconds_and_minutes`
   - `0` → `Grok is working · 0s`
   - `12` → `Grok is working · 12s`
   - `60` → `Grok is working · 1m 0s`
   - `72` → `Grok is working · 1m 12s`
6. `attach_tooltip_is_coming`
   - `ATTACH_TOOLTIP == "coming"`
7. `composer_min_height_is_56`
   - `COMPOSER_MIN_HEIGHT == 56.0`

Mutation targets: the `match` on `Role`, the four-tile table, and the minute/second split in `working_copy`. A mutant that swaps user/agent alignment or drops a tile must die.

### 5.3 What does *not* go on `Workspace`

Do not add `elapsed`, `Instant`, or `busy_started_at` to `Workspace` in this slice. `Workspace` stays `Clone + PartialEq` without a clock. See §7.

---

## 6. Desktop projection and catalog

### 6.1 Files the parent rewrites

| File | Change |
|---|---|
| `crates/multiplexer-shell/src/empty_state.rs` | New. API in §5. |
| `crates/multiplexer-shell/src/lib.rs` | `mod empty_state;` and re-exports. |
| `apps/multiplexer-desktop/src/main.rs` | Rewrite `center()`, replace `empty_center` / `chip` usage, add thread header + busy row + new composer. Host `busy_started: Option<Instant>`. |
| `apps/multiplexer-desktop/src/theme.rs` | Optional: `bubble_user()`, `send_circle()`, `tile_bg()`. Only if `center()` would otherwise inline more raw `hsla`. |
| `apps/multiplexer-desktop/src/controls.rs` | Add `attach` (see below). Do not rename the four `chip_*` ids. |

`first_code` order: shell module + tests green, then desktop `center()` consumes the helpers. Do not restyle the left rail, inspector, or terminal strip in the same pass.

### 6.2 `center()` structure

```
center
├── thread_header          // 44px, always
├── shimmer                // 2px, busy only
├── transcript (flex-1)
│     ├── empty_state      // mark + 2x2 tiles
│     └── or messages.map(project message_chrome)
│     └── busy_row         // working_copy(host_elapsed)
├── slash_hint_bar         // parse_slash or idle_composer_hint
└── composer_well          // min 56, attach + draft + model + send
```

Project `message_chrome(m.role)` instead of inlining `if user { justify_end }`. Project `empty_state_tiles()` instead of five ad-hoc `chip()` calls.

Per-bubble copy: on hover click, `cx.write_to_clipboard` that message's text and set `flash` to `copied`. Keep `copy_last_message` for the last message / palette.

### 6.3 Host elapsed

On `ShellView`:

```text
busy_started: Option<Instant>
```

- Set to `Some(Instant::now())` when `send()` actually starts a turn (`workspace.busy` becomes true).
- Clear on `push_assistant`, `mark_error`, `mark_interrupted`, and when `ignore_turn` consumes a late result.
- `elapsed_secs = busy_started.map(|t| t.elapsed().as_secs()).unwrap_or(0)`
- `pump()` already calls `window.request_animation_frame()` while `pending_turn` is live. That is enough to refresh the label once per frame. Do not add a workspace tick.

### 6.4 Control catalog delta

Keep the four `chip_*` ids on `Surface::Center`. They now name tiles, not chips. Labels stay as they are in `controls.rs` today.

Add one live control:

| id | surface | label | shortcut | action |
|---|---|---|---|---|
| `attach` | `Composer` | `Attach` | None | `attach` |

Handler: no-op + flash `coming`. Do not enable a picker.

Counts that tests pin today (`REQUIRED_IDS.len() == 39`, Composer == 3, Center == 5) **will move**:

- `REQUIRED_IDS.len() == 40`
- Composer controls: `send`, `newline`, `paste`, `attach` (4)
- Center stays 5 (`chip_*` × 4 + `copy_last_message`)

Update every length assertion in `controls.rs` in the same desktop commit. Do not leave `attach` uncatalogued (plan/10: no dead unlabeled chrome).

Do not add catalog ids for the model pill, thread-header Stop, or per-bubble copy. Those are second projections.

Remove the desktop "dir" chip. It is not in the catalog and must not return as a fifth tile.

---

## 7. Elapsed: tick field or host-only? (proposed)

**Proposal (D90): host-only Instant. No workspace tick field in v1.**

| Option | Pros | Cons |
|---|---|---|
| `Workspace.busy_elapsed_secs` ticked by the host | Headless tests can assert a number on the struct | Every one-second tick dirties `Workspace` (`PartialEq` snapshots, extra notifies). Clock is still host-driven. Not on the wire. |
| `Workspace.busy_started_ms: Option<u64>` | Pure "when did busy start" | Needs a time source to format. Still not shared with mobile unless we put it on the wire. |
| **Host `Instant` + `working_copy(secs)` (choose this)** | `Workspace` stays a session model. Formatter is the testable unit. `pump()` already frames while busy. | Desktop-only clock. Two clients would not share one elapsed without a later server timestamp. |

v1 is a single desktop host. Elapsed is presentation, not orchestration. Putting a clock on `Workspace` would make every inspector notify a side effect of a chat spinner.

**Later (not this slice):** if mobile or a popped-out window needs the same clock, add `turn_started_at` (server ISO time) on the thread read model and compute elapsed from the server clock. That is a `multiplexer-wire` change and belongs with plan/04, not here.

Header status and the working row must use the **same** `working_copy(elapsed_secs)` so they cannot drift.

---

## 8. Markdown later slice (contract only)

Track as a follow-up, not a second module in `first_code`:

1. Keep `ChatMessage.text` as the source string.
2. Add `multiplexer-shell::markdown` that returns a small enum (`Paragraph`, `CodeBlock { lang, body }`, `InlineCode`) with no HTML.
3. GPUI: paragraphs use the wrap rules in §4.6; code blocks use monospaced fill. No highlighting in the first markdown cut.
4. Component snapshot of one user bubble + one agent bubble that contains a fenced block.

Until that lands, a message that contains backticks paints them as literals.

---

## 9. Testing

### 9.1 Unit (this slice, merge gate)

`empty_state.rs` tests in §5.2. Run with the existing `multiplexer-shell` unit job. No GPUI.

### 9.2 Mutation

`cargo-mutants` on `crates/multiplexer-shell/src/empty_state.rs` only, after the unit tests exist. Kill:

- tile count `4` → `3` / `5`
- swapped `BubbleAlign`
- `working_copy` minute branch deleted
- `composer_placeholder` emptied

Merge floor 70% (D33) on this module.

### 9.3 Component / e2e (not first_code)

- Component: render `center` empty vs one user + one agent message; assert tile count 4, user row `justify_end`, agent avatar present, composer min height 56. Deferred until the GPUI test harness in plan/10 §9 is wired for the desktop binary.
- E2E: click `chip_what` → user bubble appears, composer clears. Deferred to the existing e2e gate (D32).

Do not block this visual slice on GPUI component tests that do not exist yet. Headless chrome is the gate.

### 9.4 Controls tests

Update `apps/multiplexer-desktop/src/controls.rs` tests in the same commit as `attach`. Parent must not land the paperclip without the catalog bump.

---

## 10. Implementation order (`first_code`)

1. Add `empty_state.rs` with the types and functions in §5. Tests from §5.2. Re-export.
2. Rewrite `ShellView::center`:
   - thread header
   - empty state from `empty_state_tiles()`
   - bubbles from `message_chrome`
   - busy row from `working_copy`
   - composer well min 56, circular send, model pill, disabled paperclip, slash / idle hint bar
   - delete `empty_center()` and the chip row
3. Host `busy_started` + catalog `attach`.
4. Manual look on Windows: empty thread, one send, busy shimmer, hover copy, slash `/new`.

Do not restyle rails. Do not touch `grok -p` / session start.

---

## 11. Proposed decisions (D90+)

### D90. Elapsed is host-only (PROPOSED)

Desktop holds `Option<Instant>`. `working_copy(u64)` is the shared formatter. No tick field on `Workspace`. No wire field in v1.

### D91. Empty state is four tiles, not chips (PROPOSED)

`empty_state_tiles()` is the only suggestion source. Composer chip row is removed. Catalog ids `chip_*` remain, now naming tiles.

### D92. Agent bubble label is "Grok" (PROPOSED)

Grok-first (D3). Do not paint the raw model id as the role label in v1. Model id lives on the header pill and the in-composer pill.

### D93. Attach is visible and disabled (PROPOSED)

Paperclip ships as chrome with tooltip `coming`. No picker, no paste-image chips, no drag-and-drop.

### D94. Markdown is a later slice (PROPOSED)

v1 is wrapping plain text. Code-block monospaced fill is specified (§4.6, §8) and not implemented here.

### D95. Center header is a second projection (PROPOSED)

Title / model / status / Stop live in the center header. Actions reuse `CycleModel` and `Interrupt`. No duplicate catalog ids.

---

## 12. Open questions / risks

Flagged, not decided here beyond the proposals above:

1. **Agent label vs model id.** D92 proposes the literal `Grok`. If custom models (`ds-flash`) should change the bubble label, that is a one-line change to `message_chrome` later.
2. **`chip_test` as send vs shell.** This doc sends the prompt to Grok. A user who wanted a raw `cargo test` still has the terminal strip.
3. **Shimmer cost.** A 2px animated bar must stay inside the 16ms input budget (plan/16). Reduced-motion falls back to a static bar.
4. **Catalog length churn.** Adding `attach` breaks several `controls.rs` length pins. That is expected and must ship with the visual.
5. **D-number collision.** D90+ may overlap sibling plan docs 27 through 33. Remap on review; the behavior names (D90 through D95 titles) are the contract.
6. **Large mark asset.** If `docs/banner.svg` is too wide for a 56px mark, use the "M" fallback rather than blocking on a new illustration.

**Consistency:** Outlook center (plan/10 §2.2) still holds. This is the conversation half of that pane, not a new pane kind. Server-centric runtime is unchanged: the desktop still calls `send()` / `interrupt()` the same way. No secrets, no wire change, no provider change.

---

## 13. Acceptance

This slice is done when:

- [ ] `empty_state_tiles()` returns 4 tiles with the locked ids and actions.
- [ ] `composer_placeholder()` is `Message Grok…` with no keyboard hints inside it.
- [ ] `message_chrome(Role::User).align` is `End` and `message_chrome(Role::Assistant).align` is `Start`.
- [ ] Empty center shows a large mark + 2×2 tiles; the tiny chip row is gone.
- [ ] Composer well is at least 56px; send is a circular accent arrow; model pill sits in the well; paperclip is visible, disabled, tooltip `coming`.
- [ ] Busy shows shimmer + `Grok is working · {elapsed}`; elapsed comes from host `Instant`.
- [ ] Center thread header shows title, model, status, and Stop when busy.
- [ ] Transcript is still plain text, wrapped to 640px.
- [ ] `controls` catalog includes `attach` and still includes the four `chip_*` ids.

---

*Next: parent implement (`empty_state.rs` + `ShellView::center`). Markdown fill lands as a follow-up slice, not in `first_code`.*
