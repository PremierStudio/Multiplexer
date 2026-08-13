# Audit 04: Center chat transcript and empty state

**Surface:** `ShellView::center`, `empty_center`
**Spec:** `plan/34-chat-composer-visual.md` (§3 layout, §4.1-4.4, §4.6 wrap, §5 headless, §6.2 projection, §13 acceptance)
**Code:** `apps/multiplexer-desktop/src/main.rs`
**Helpers:** `crates/multiplexer-shell/src/widgets.rs` (`EmptyStateSpec`, stub `empty_state_tiles`)
**Method:** source read of `center()`, `empty_center()`, and plan/34. No cargo.

## Verdict

The center column is still the placeholder described in plan/34 §1. Transcript and empty state were not rewritten. There is no thread header, no `message_chrome` projection, no copy-on-hover, no elapsed / working row, no wrap contract, no transcript scroll, and no 2x2 tiles. Suggestion prompts remain a chip row on the composer.

None of the §13 acceptance items that belong to transcript or empty state are met.

## Findings

| ID | Severity | Title |
|---|---|---|
| C04-01 | High | Basic bubbles |
| C04-02 | High | No copy-on-hover |
| C04-03 | High | No thread header |
| C04-04 | High | No elapsed / working chrome |
| C04-05 | Medium | No markdown (wrap contract also missing) |
| C04-06 | High | Empty state not using tiles as designed |
| C04-07 | High | Overflow / scroll missing |

---

### C04-01: Basic bubbles

- **Severity:** High
- **Spec:** plan/34 §4.2, §5.1 `message_chrome`, D92
- **Code:** `apps/multiplexer-desktop/src/main.rs` `center()` messages arm (about 1290-1324)
- **Expected:** Project `message_chrome(role)`. User: align end, fill glass-accent, role `"You"`, no avatar. Assistant: align start, surface fill, 24px `"G"` avatar with accent ring, role `"Grok"` (not `"Agent"`). Shared: max width 640, `rounded_xl`, hairline + shadow, pad 12/10, copy-on-hover (C04-02).
- **Actual:** Inline `if user { justify_end } else { justify_start }`. Glass pills with a small role line inside the bubble (`"You"` / `"Agent"`) and raw `m.text`. No `message_chrome` type, no avatar, no avatar column, no `"Grok"` label. Fill colors happen to match the spec hex values, but the row chrome does not.
- **Impact:** Role chrome is a caption inside the pill, the product name never appears on the agent row, and left-rail `Agent:` preview (allowed to stay) leaked into the bubble (forbidden by §4.2).
- **Headless gap:** `crates/multiplexer-shell/src/empty_state.rs` does not exist. No `MessageChrome`, `BubbleAlign`, or `message_chrome(role)`.

---

### C04-02: No copy-on-hover

- **Severity:** High
- **Spec:** plan/34 §4.2 Shared chrome, §6.2
- **Code:** bubble child is only role label + `div().child(m.text)`. `copy_last_message` lives at `main.rs` about 378-388 and is wired from a composer chip ("Copy last") plus catalog id `copy_last_message`.
- **Expected:** 20px copy glyph, top-right of *that* bubble, opacity 0 until hover or keyboard focus. Click copies that message's `text` and flashes `copied`. Palette / `copy_last_message` still copies the last message only.
- **Actual:** No hover handler on the bubble (`hover()` is used on `icon_btn` / resize handles, not messages). No per-message clipboard path. Only last-message copy exists.
- **Impact:** Cannot copy an earlier turn without selecting text from a non-wrapping, non-scrolling stack. Hover affordance specified as first-class chrome is absent.

---

### C04-03: No thread header

- **Severity:** High
- **Spec:** plan/34 §4.1, §6.2 `center` tree, D95
- **Code:** `center()` (about 1266-1329) starts with `glass_pane` then a flex-1 transcript. Title, model, status, and Stop are not painted in the center. `selected_thread()` is used only to choose empty vs message list. Window title bar (about 902-964) holds the model pill and Stop/Idle, which is the *window* chrome, not the 44px center strip.
- **Expected:** Always-on 44px glass strip at the top of the center pane: title from `selected_thread().title` (ellipsis, default "New chat"), quiet model pill (`ClientAction::CycleModel`), status from `thread.status` + `workspace.busy` (idle muted, running accent, error danger, busy appends the same elapsed string as the working row), danger Stop ghost when busy (`ClientAction::Interrupt`, catalog id `stop`). Not a second catalog id.
- **Actual:** No `thread_header` child. Center never shows thread title. Model and Stop stay in the window title bar only.
- **Impact:** Switching threads does not identify the conversation in the pane the user stares at. Interrupt is not on the conversation surface.

---

### C04-04: No elapsed / working chrome

- **Severity:** High
- **Spec:** plan/34 §4.3, §6.3, D90
- **Code:** `ShellView` (about 54-70) has `last_core_sample: Instant` and `pending_turn`, not `busy_started: Option<Instant>`. `center()` never reads `workspace.busy`. `send()` (about 437-466) starts a turn with no clock. `pump()` (about 548-641) already calls `window.request_animation_frame()` while `pending_turn` is live (the tick path §6.3 wanted).
- **Expected:** When `workspace.busy`: 2px shimmer under the thread header (static accent bar if reduced motion); working row after the last message (or under the empty mark) from `working_copy(elapsed_secs)` (`Grok is working · {n}s` / `{m}m {s}s`). Header status uses the same formatter. Host Instant only; do not put a clock on `Workspace`.
- **Actual:** No shimmer, no working row, no `working_copy`, no `busy_started`. Busy is a title-bar Stop/Idle swap plus `workspace.busy` on agent list rows.
- **Headless gap:** `working_copy(u64)` is unspecified in code. No tests for `0` / `12` / `60` / `72`.
- **Impact:** A live turn has no in-transcript progress. Elapsed cannot be shown without the host Instant that was never added.

---

### C04-05: No markdown (wrap contract also missing)

- **Severity:** Medium (parser is a later slice, D94 / §8). The wrap contract in §4.6 is in scope now and is also missing.
- **Spec:** plan/34 §4.2 body, §4.6, §8, D94
- **Code:** `row.child(div().max_w(px(640.0))....child(div().child(m.text)))` (about 1300-1323)
- **Expected (this slice):** v1 is plain text. Word wrap at `max_width_px` (640). Long tokens (URLs, hashes) break rather than overflow the pane. Newlines in `ChatMessage.text` are hard breaks. Spaces are not collapsed. No inline styles, lists, or headings. Backticks paint as literals until the later markdown module.
- **Expected (later slice, specified now):** `multiplexer-shell::markdown` enum (`Paragraph`, `CodeBlock`, `InlineCode`); fenced blocks monospaced + filled surface; no syntax colors in the first markdown cut.
- **Actual:** Raw `m.text` in a nested `div`. No wrap / overflow-wrap / whitespace style. No hard-break of `\n`. No markdown module. `ChatMessage` is `{ role, text }` only (`workspace.rs`).
- **Impact:** A long assistant reply or a URL can blow the 640px bubble and the pane (see C04-07). A later markdown parse has no wrap contract to hang on.

---

### C04-06: Empty state not using tiles as designed

- **Severity:** High
- **Spec:** plan/34 §4.4, §5.1 `empty_state_tiles` / `SuggestionTile`, D91, §13
- **Code:**
  - `empty_center()` about 1706-1726
  - empty branch `Some(t) if t.messages.is_empty() => vec![empty_center()]` and `None => vec![empty_center()]` (about 1288-1289, 1327)
  - composer chip row about 1338-1365
  - stub `empty_state_tiles()` in `crates/multiplexer-shell/src/widgets.rs` 178-186
- **Expected:** Shown when the selected thread exists, `messages.is_empty()`, and `!workspace.busy`. Large 56px Multiplexer mark, title `What should we build?`, subtitle `Pick a starting point, or type below.`, then a 2x2 grid of four tiles (min 148x76, gap 10), not chips, not `chip()`. Tiles from `empty_state_tiles()`:

  | id | icon | title | action |
  |---|---|---|---|
  | `chip_what` | Spark | What can you do? | SendPrompt |
  | `chip_summarize` | Repo | Summarize this repo | SendPrompt |
  | `chip_git_status` | Branch | Git status | RunShell `git status` |
  | `chip_test` | Flask | Run the tests | SendPrompt `Run the tests` |

  Tiles hide when the first message lands. They do not sit on the composer. Catalog ids stay; they name tiles. The "dir" / files chip is removed. `Copy last` is not a tile.
- **Actual `empty_center()`:** Centered sparkle glyph + muted tagline `A control surface for your agents` + faint keyboard hints `Ctrl+K palette   F1 help   Ctrl+\` terminal`. No 56px mark, no specified title/subtitle, no tiles.
- **Actual suggestions:** Five `chip()` pills parked on the composer, always visible (including after messages exist): "What can you do?", "Summarize this repo", "git status", "List project files" (switches inspector/files, not a send), "Copy last". Fourth chip is not `chip_test` / "Run the tests" even though `controls.rs` catalogs `chip_test` with that label.
- **Stub helper:** `empty_state_tiles()` returns `[&'static str; 4]` titles including `"List project files"`, not `[SuggestionTile; 4]` with ids, icons, subtitles, payloads, and `TileAction`. Desktop does not import or project it. `EmptyStateSpec::chat()` ("Start a session" / "New chat") is a plan/31 leftover and is also unused by `empty_center()`.
- **Impact:** Empty thread is a slogan, not a starting surface. Catalog `chip_test` has no matching paint. Composer chip row is the opposite of D91.

---

### C04-07: Overflow / scroll missing

- **Severity:** High
- **Spec:** plan/34 §3 transcript (`flex-1`, `min-h 0`, pad 16, gap 12), §4.6 wrap (long tokens break rather than overflow), §2.2 (virtualize later; dozens of messages must still be usable)
- **Code:** transcript wrapper about 1280-1328: `div().flex_1().min_h_0().p_4().flex().flex_col().gap_3().children(...)`. Desktop `main.rs` has no `overflow_y_scroll` / `overflow_x_scroll` / `overflow` at all (confirmed by search). Bubble body has no wrap/break style (C04-05).
- **Expected:** Transcript is the flex-1 region and must scroll when messages exceed the pane. Virtualization is a non-goal, but a window of messages still has to be reachable. Bubble width is capped at 640px *and* text wraps / breaks inside that width. `\n` is a hard break.
- **Actual:** `flex_1` + `min_h_0` allow the column to shrink; without overflow-y scroll the children clip. Messages are a plain `Vec` mapped into the column. No scroll, no sticky-to-bottom, no overflow-x guard. A long unwrapped token can push past the bubble and the pane.
- **Contrast:** `icon_btn` already uses `hover()`. Terminal strip uses `visible_tail` to cap log paint. Transcript has neither a scroller nor a tail window.
- **Impact:** A thread longer than the viewport loses earlier turns. A single long line can break the center layout. This is independent of later virtualization.

---

## Headless contract (blocking the rewrite)

plan/34 `first_code` step 1 is `crates/multiplexer-shell/src/empty_state.rs`. It is not in the tree. Missing API: `TileIcon`, `TileAction`, `SuggestionTile`, `empty_state_tiles() -> [SuggestionTile; 4]`, `message_chrome`, `working_copy`, `composer_placeholder`, `idle_composer_hint`, `COMPOSER_MIN_HEIGHT`, `BUBBLE_MAX_WIDTH`, `ATTACH_TOOLTIP`. Required unit names in §5.2 are absent.

`widgets.rs::empty_state_tiles()` is a four-string stub that does not satisfy §5.1 and is not consumed by `center()`.

## Out of scope for this file

Composer well (min 56, circular send, in-well model pill, disabled paperclip, slash / idle hint bar) is specified in the same plan but is the composer surface, not the transcript. Noted only because the chip row there is the misplaced empty-state tiles (C04-06).

## FINDINGS: 7
