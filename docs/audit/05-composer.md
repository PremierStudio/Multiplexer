# 05 Composer audit (draft, send, slash, chips, focus)

**Scope:** Center composer well, draft editing, send, slash commands, suggestion chips, keyboard focus.
**Against:** `plan/34-chat-composer-visual.md` (§4.4 empty tiles, §4.5 composer, §5 headless API, §6 desktop projection).
**Sources:** `apps/multiplexer-desktop/src/main.rs` (`center`, `handle_key`, `send`, `handle_slash`, `draft_display`), `apps/multiplexer-desktop/src/controls.rs`, `crates/multiplexer-shell/src/slash.rs`, `crates/multiplexer-shell/src/composer.rs`, `crates/multiplexer-shell/src/widgets.rs`, `crates/multiplexer-shell/src/workspace.rs`.
**Not run:** cargo (this pass is read-only).

**FINDINGS: 10**

Finding format used below:

```
### C-NN Short title
- **Severity:** P0 | P1 | P2
- **Where:** path, symbol, lines
- **Spec:** plan/34 section
- **Evidence:** what the code does
- **Impact:** user or contract effect
- **Fix:** smallest correct change
```

---

## Findings

### C-01 Fake composer is a painted string, not a GPUI input

- **Severity:** P0
- **Where:** `apps/multiplexer-desktop/src/main.rs` `center` (1371-1399), `handle_key` (644-768), `draft_display` (1958-1974). Desktop crate has zero `TextInput`, `EditorElement`, `InputHandler`, `focus_handle`, or `track_focus`.
- **Spec:** plan/34 §4.5 (a real draft that grows, wraps, and accepts Enter / Shift+Enter). GPUI 0.2.2 exposes `Keystroke.key_char` as the typed character (`gpui-0.2.2/src/platform/keystroke.rs`).
- **Evidence:** The well is a `div` with `.id("composer")`. Empty draft paints a placeholder `SharedString`. Non-empty draft paints `draft_display`, which splices a `|` glyph at a char index. Keys are taken on the window root (`on_key_down` at 846-848) and applied by hand: `type_char` for `key.len() == 1`, special cases for space / backspace / arrows. `handle_key` reads `event.keystroke.key` only. It never reads `key_char`. `Workspace::type_char` also drops control chars (`workspace.rs` 418-421).
- **Impact:** No OS caret, no IME, no selection, no undo. Shift+letter types the unshifted key (`key` is `"a"`, not `"A"`). Shift+1 types `1`, not `!`. Non-US layouts and CJK composition cannot enter the draft. The painted `|` is content, not a caret.
- **Fix:** Project a GPUI text input (or `InputHandler` + `focus_handle`) bound to `workspace.draft` / `cursor`. Until that lands, insert `key_char` when present so Shift and IME at least reach the string.

### C-02 Send is a rectangular ghost label, not a circular accent arrow

- **Severity:** P1
- **Where:** `center` (1401-1404), `ghost_btn` (1924-1956), `theme.rs` `send_bg` (56-58).
- **Spec:** plan/34 §4.5 Send: circular, 36px, `Theme::accent()` fill, right-pointing arrow. Accessible name stays `"Send"`. Muted (`Theme::send_bg()`, no accent) when draft is empty or `workspace.busy`.
- **Evidence:** Send is `ghost_btn("Send", "↵", ...)`. That helper is a 32px-tall rounded rectangle with a hairline, the word `"Send"`, and a muted hint. When `label == "Send"` the fill is `Theme::send_bg()` (accent_muted) on every paint, empty or not. There is no 36px circle, no arrow glyph, no muted-vs-accent switch, no `send_circle()` helper.
- **Impact:** The primary affordance still looks like the placeholder "short well + ghost Send" that plan/34 §1 called out as unfinished. Click still no-ops when empty or busy, but the control never looks inert.
- **Fix:** Replace the Send `ghost_btn` with a 36px circle (arrow, catalog id `send`). Accent when `!draft.trim().is_empty() && !workspace.busy`, else `send_bg()`.

### C-03 Composer chips are not `empty_state_tiles()`, and the catalog fourth tile is missing

- **Severity:** P1
- **Where:** `center` chip row (1338-1365), `empty_center` (1706-1726), `chip` (1728-1747), `crates/multiplexer-shell/src/widgets.rs` `empty_state_tiles` (179-186). `crates/multiplexer-shell/src/empty_state.rs` does not exist. Desktop never calls `empty_state_tiles`.
- **Spec:** plan/34 §4.4, §5.1, D91: empty thread shows a large mark + four icon tiles from `empty_state_tiles()` (`chip_what`, `chip_summarize`, `chip_git_status`, `chip_test`). Composer chip row goes away. `chip_test` sends `"Run the tests"`. "Copy last" is per-bubble hover, not a chip. "dir" / file-list chip must not return.
- **Evidence:** Empty center is still one muted sentence (`"A control surface for your agents"`). Five tiny `chip()` pills sit on the composer for every thread, empty or not: "What can you do?", "Summarize this repo", "git status", "List project files", "Copy last". The fourth chip opens the Files inspector. It is not `chip_test` / `"Run the tests"` (catalog label in `controls.rs` 141). `widgets::empty_state_tiles()` returns those first four strings (including `"List project files"`) and is unused. Plan/34's `[SuggestionTile; 4]` API, icons, and actions are not implemented.
- **Impact:** Suggestions stay mixed into the composer. Catalog `chip_test` is dead chrome. A fifth uncatalogued chip ("List project files") plus "Copy last" as a chip violate plan/10 (no dead unlabeled chrome) and D91.
- **Fix:** Land `empty_state.rs` as specified. Empty transcript projects the four tiles. Delete the composer chip row and `empty_center()`. Keep catalog ids `chip_*` on the tiles.

### C-04 Focus is a three-way enum, not pane focus, and overlays do not take keys

- **Severity:** P1
- **Where:** `Focus` (47-52), `handle_key` (644-768), Esc (647-664), Tab (757-758, 822-823), palette restore (183-193, 786-791), help overlay (1626-1657), approval hints (1460-1465).
- **Spec:** plan/10 §2.2 (center is the default focus target), plan/10 §3 (exactly one focused pane, `focus_path`). plan/34 §4.5: composer focused hairline is `Theme::accent()`. Catalog `allow` / `deny` / `help_close` are live controls.
- **Evidence:**
  1. Focus is `Composer | Terminal | Palette`. Clicking the left rail, inspector, title bar, or transcript does not change it, so the composer accent ring stays on while the user clicks elsewhere.
  2. There is no GPUI `FocusHandle`. Window keys always hit `handle_key`. The composer click handler only sets the enum (1386-1391).
  3. Esc with no overlay forces `Focus::Composer`, even when the user was in the terminal.
  4. Help overlay is visual only. `handle_key` does not check `help_open` except to dismiss on Esc, so typing with help open mutates the draft.
  5. Approval paints `A` / `D` hints but those keys are inserted into the draft. `ClientAction::Approve` / `Deny` are mouse-only.
  6. Closing the palette always returns `Focus::Composer`, even if focus was Terminal before Ctrl+K.
- **Impact:** Keyboard routing is guessable only if the user watches the composer/term border color. Overlay chords lie. Terminal Esc is a trap. Approval during a turn can type `a`/`d` into the next prompt.
- **Fix:** Give composer and terminal real focus handles. Help and palette must own keys while open. Bind `a`/`d` to approve/deny when a card is pending and focus is not an input. Esc should dismiss overlays first, then stay on the current input.

### C-05 Shift+Enter inserts a newline the well cannot host

- **Severity:** P1
- **Where:** `handle_key` (717-722), `insert_text` (771-783), `center` well (1366-1404), `draft_display` (1958-1974). Catalog: `controls.rs` 149, 197 (`newline` / `shift-enter`).
- **Spec:** plan/34 §4.5: Enter sends, Shift+Enter newline (unchanged). Well min height 56px, grows with wrapped draft, cap 8 lines (~168px), then clip. Multi-line layout: draft full width above a 36px footer (paperclip, model pill, send).
- **Evidence:** Shift+Enter does call `insert_text("\n")` when `focus == Composer`. That path works. The well is a single `items_center` row with `py_2`, no `min_h(56)`, no line cap, no footer split. `draft_display` embeds `\n` and a `|` into one `SharedString` inside that row. `type_char` will not accept `'\n'` (`is_control`), so this special case is the only newline path.
- **Impact:** Newline is logically stored, then painted as an unbounded multi-line string that stretches the chip+send cluster. No 8-line clip. Users who hit Shift+Enter get a broken layout, not a taller composer.
- **Fix:** Keep the Shift+Enter mapping. Grow the well to min 56 / max ~168, wrap the draft, move chrome into the 36px footer when `draft` contains `\n`.

### C-06 Unknown slash eats the draft and only whispers in the terminal

- **Severity:** P1
- **Where:** `send` (441-449), `handle_slash` `Unknown` (499-503), `center` hint (1406-1412), `parse_slash` / `slash_hint` (`slash.rs` 25-62).
- **Spec:** plan/34 §4.5 slash hint bar: if `parse_slash(&draft)` is `Some(cmd)`, paint `/{token}  {slash_hint(cmd)}` in accent, 22px *above* the well. `slash_hint(Unknown(_))` is `"unknown command"`. Sending a leading `/` is a command, never a user bubble.
- **Evidence:** Any leading `/` parses, including `"/"` (`Unknown("")`) and `"/foo"`. The hint under the well is `format!("/  {}", slash_hint(&cmd))`, so unknown paints `/  unknown command` with no token and no suggestions. On Enter, `send` clears `draft` and `cursor`, then `handle_slash` writes `term_meta("unknown /{name}  try /help /new /stop /cp /cores /mcp /git /term /skills")`. No `flash`, no composer error, no keep-draft. `/palette` is handled but omitted from that try-list. A real prompt that starts with `/` (or a path like `/Users/...`) can never be sent.
- **Impact:** A typo `/hew` destroys the line. Feedback is a muted terminal meta line, easy to miss. The on-well hint does not name the token or offer completions.
- **Fix:** On `Unknown`, do not clear the draft. Flash `unknown /{name}` (or keep the hint bar on the token). Paint `/{token}  unknown command`. Add a documented escape (e.g. only treat a single `/token` line as a command) so a leading-slash prompt can still send.

### C-07 `send()` refuses slash (including `/stop`) while busy

- **Severity:** P1
- **Where:** `send` (437-449), `handle_slash` `Stop` (474), `handle_key` Enter (717-721).
- **Spec:** plan/34 §4.3: composer send is inert while busy. Interrupt stays available (header Stop, title-bar Stop, catalog `stop`). `/stop` is `slash_hint` "stop the running turn" (`slash.rs` 50).
- **Evidence:** `send` returns immediately when `pending_turn.is_some() || workspace.busy`, before `parse_slash`. Enter and the Send button share that gate. `/stop` therefore cannot run from the composer during a turn. Ctrl+. and the title-bar Stop still call `interrupt()` directly.
- **Impact:** The slash the hint advertises for stopping a turn is dead exactly when it is needed. User who types `/stop` and hits Enter sees nothing (draft stays, send no-ops).
- **Fix:** Parse slash before the busy gate. Allow `SlashCommand::Stop` (and other non-turn commands) while busy. Keep the busy gate only for `send_draft` / `spawn_grok_turn`.

### C-08 Slash hint bar is idle-empty, below the well, and drops the token

- **Severity:** P2
- **Where:** `center` (1330-1412).
- **Spec:** plan/34 §4.5, §6.2: 22px row *above* the well. Known command: `/{token}  {slash_hint}`. Else (including empty draft): muted `idle_composer_hint()` = `Enter send · Shift+Enter newline · / for commands`. Keyboard hints do not live in the placeholder.
- **Evidence:** The hint is the *third* child under the composer, after chips and the well. Non-slash paints `div().child("")`. There is no `idle_composer_hint` symbol. Known commands paint `/  start a new chat` (no `new`). Placeholder still holds the old hints (see C-09).
- **Impact:** Idle users never see the Shift+Enter / slash contract in the specified slot. Partial `/ne` looks like an unknown command with no token, not a filter.
- **Fix:** Move a 22px bar above the well. Branch `parse_slash` vs `idle_composer_hint()`. Include the parsed token in the accent line.

### C-09 Placeholder still embeds keyboard hints

- **Severity:** P2
- **Where:** `center` (1393-1396).
- **Spec:** plan/34 §4.5 / §5.1: `composer_placeholder()` is exactly `Message Grok…` (U+2026). Must not contain `Enter` or `/help`. Hints move to the idle hint bar.
- **Evidence:** Empty well paints `"Message Grok…  Enter send  Shift+Enter newline  /help"`. `composer_placeholder()` does not exist. The prefix matches; the rest is the old contract.
- **Impact:** The well is noisy. `/help` in the placeholder disagrees with the idle hint (`/ for commands`) and with the help overlay slash list.
- **Fix:** Paint only `Message Grok…` when `draft.is_empty()`. Put hints in the bar from C-08.

### C-10 Composer well is still the short surface (no attach, no in-well model pill, no min 56)

- **Severity:** P1
- **Where:** `center` (1366-1412), `controls.rs` Composer ids (148-150, test 606-610: `send`, `newline`, `paste` only).
- **Spec:** plan/34 §4.5, §6.4: min height 56, glass fill `hsla(0.0, 0.0, 1.0, 0.06)`, pad 12/8/8, row `[paperclip] [draft] [model pill] [send circle]`. Paperclip visible, disabled, tooltip `coming`, catalog id `attach` (Composer count 4, `REQUIRED_IDS.len() == 40`). Model pill inside the well, `ClientAction::CycleModel`, not a new catalog id. `COMPOSER_MIN_HEIGHT = 56.0`.
- **Evidence:** Well uses `Theme::surface()`, `px_3` / `py_2`, no min height. No paperclip. Model pill lives in the title bar (928-941), not in the well. `attach` is absent. Controls tests still pin Composer == 3 and required ids == 39.
- **Impact:** plan/34 acceptance items for the well, attach catalog, and in-composer model pill are all red. The title-bar pill is a second projection that already exists. The well does not.
- **Fix:** Rewrite the well per §4.5 / §6.2. Add `attach` + flash `coming` in the same desktop commit as the paperclip. Reuse `CycleModel` for an in-well pill.

---

## Counts

| Severity | Count | Ids |
|---|---|---|
| P0 | 1 | C-01 |
| P1 | 7 | C-02, C-03, C-04, C-05, C-06, C-07, C-10 |
| P2 | 2 | C-08, C-09 |
| **FINDINGS** | **10** | |

Requested checks vs findings:

| Requested | Finding |
|---|---|
| Fake text field (no real GPUI input) | C-01 |
| Missing circular send | C-02 |
| Chips vs `empty_state_tiles` | C-03 |
| Focus bugs | C-04 |
| Shift+Enter | C-05 |
| Slash unknown UX | C-06 (C-07 / C-08 adjacent) |

---

## What already matches

- Enter (no Shift) calls `send()` when focus is Composer (`handle_key` 717-721).
- `parse_slash` / `slash_hint` in `multiplexer-shell` cover the locked command set, aliases, case, and trim. Tests exist.
- Known slash on Enter clears the draft and dispatches (`send` 445-449). `/term` sets `Focus::Terminal`. `/new` clears `session_id`.
- Ctrl+V paste goes through `insert_at` at the cursor (`handle_key` 700-707).
- Word-left backspace, arrows, Home/End, Delete are wired to `multiplexer-shell` composer helpers.
- Default `focus` is `Focus::Composer` on launch.

---

## Suggested first-code order (from plan/34 §10)

1. Add `empty_state.rs` (`empty_state_tiles`, `composer_placeholder`, `idle_composer_hint`, `COMPOSER_MIN_HEIGHT`, `ATTACH_TOOLTIP`) and stop using `widgets::empty_state_tiles` as the suggestion source.
2. Rewrite `ShellView::center`: tiles in the empty transcript, delete `chip` row / `empty_center`, circular send, hint bar above a min-56 well.
3. Host `attach` in `controls.rs` (40 ids) and a real GPUI input (C-01). Without C-01 the rest is chrome on a fake field.
4. Split slash parse out of the busy send gate so `/stop` works mid-turn.
