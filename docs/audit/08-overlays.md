# 08: Overlays (help, approval, reminder, flash)

**Scope:** Desktop HELP overlay, approval card, worktree reminder bar, flash/toasts.
**Against:** `apps/multiplexer-desktop/src/main.rs`, `crates/multiplexer-client/src/turn.rs`, `crates/multiplexer-shell` (approval, notices, settings, actions), `plan/36-feature-gap-ui.md` §4.10 / §4.12, `plan/30` help copy, `plan/35` overlay bar.
**Not in scope:** Palette richness (audit 11 / plan/36 §4.11). In-process grok embed (D10).
**Date:** 2026-08-12

Painted overlays today: palette (`Ctrl+K`) and help (`F1`). Approval and reminder are document-flow bars under the title, not cards. Flash is one status-bar string. Settings and toast stack exist as headless fields (`settings_open`, `Workspace.notices`) and are never projected.

---

### OV-HELP-WALL
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:1626
- problem: `help_overlay` is a 560px glass sheet whose body is one muted `div` holding a five-line concatenated blob (Enter / Shift+Enter / Ctrl+K / F1, rails, Tab, slash list, term builtins). There is no two-column shortcut table, no Composer / Chrome / Terminal grouping, no search, and no scroll. `plan/30` §14.8 and §11.2 told this blob to be replaced with chord lines that include `` Ctrl+` `` and the new left-section keys. The overlay still says "Ctrl+[ / ] rails" and never mentions `` Ctrl+` ``, Ctrl+1..4, or Ctrl+,. A second help surface (`help_text()` in `crates/multiplexer-shell/src/terminal_ui.rs:87`) prints a different, shorter line ("Builtins: clear, help, cores, mcp") when the term `help` builtin runs, so F1 and `mux> help` disagree.
- fix: `apps/multiplexer-desktop/src/main.rs` (`help_overlay`) + `crates/multiplexer-shell/src/terminal_ui.rs` (`help_text`). TDD: `help_overlay_lists_grouped_chords` (assert grouped lines include `` Ctrl+` `` and Ctrl+,; `help_text()` contains the same term builtins the strip actually handles).

### OV-HELP-CLICK
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:1637
- problem: The help backdrop toggles help closed on any left click. The inner card does not swallow the event (palette does, at `palette_overlay` line 1585). Clicking the "Keyboard" heading or the wall of text dismisses help. Catalog id `help_close` (`apps/multiplexer-desktop/src/controls.rs:180`) is never painted. There is no Close control, only an accidental full-card hit target.
- fix: `apps/multiplexer-desktop/src/main.rs` (`help_overlay` card `on_mouse_down` no-op + a Close ghost). TDD: `help_card_click_does_not_dismiss` (toggle open, apply card click vs backdrop click; only backdrop / Esc / `help_close` clears `help_open`).

### OV-SETTINGS-NONE
- severity: P1
- evidence: apps/multiplexer-desktop/src/main.rs:976
- problem: `plan/36` §4.10 (this wave, gap L) requires a Settings overlay on `Ctrl+,` and a palette **Settings** row: theme, default model, project path, Remote, keybindings. Headless already has `UiSettings`, `Workspace.settings` / `settings_open`, and `ClientAction::ToggleSettings` (`crates/multiplexer-shell/src/actions.rs:36`, `:78`). Desktop never paints a settings overlay, never binds `Ctrl+,`, and never dispatches `ToggleSettings`. The title-bar Settings glyph is the inspector toggle ("Hide inspector" / "Show inspector"), so the one control that looks like Settings opens the right rail. `controls.rs` has no `Surface::Settings` and no `settings_theme` / `settings_model` / `settings_close` ids. Help copy has no `Ctrl+,` line.
- fix: `apps/multiplexer-desktop/src/main.rs` (settings overlay + `Ctrl+,`) + `apps/multiplexer-desktop/src/controls.rs` (`Surface::Settings`) + `crates/multiplexer-shell/src/palette.rs` (Settings row). TDD: `settings_overlay_applies_default_model` (plan/36 §7 #10: open, `default_model = "fake"` sets `ws.model`, close clears `settings_open`).

### OV-FLASH-ONE
- severity: P1
- evidence: apps/multiplexer-desktop/src/main.rs:69
- problem: `plan/36` §1 and §4.12 (gap N) require a toast stack (max 3 visible, kinds info/ok/warn/error, auto-dismiss info/ok, Esc dismisses newest). Live chrome is `ShellView.flash: Option<String>`. It is set only by copy-last-message and copy-session-id (`main.rs:386`, `:396`), concatenated into the status bar (`status_bar` at `:1537`), never given a kind, never stacked, and never cleared except by overwrite. Turn finish / fail (`pump` `:562`), checkpoint create, MCP reload, and worktree refresh all scream into `term_meta` instead. Headless `Workspace.notices` + `push_notice` (`crates/multiplexer-shell/src/notices.rs:20`, cap 5 vs plan cap 8) are compiled and unused: desktop never calls `push_notice` and never paints a stack. There is no `Surface::ToastStack`, no `ClientAction::DismissToast`, no 4s timer.
- fix: `crates/multiplexer-shell/src/notices.rs` (cap 8, `dismiss_newest`) + `apps/multiplexer-desktop/src/main.rs` (top-right stack, delete `flash`). TDD: `toast_queue_caps_and_dismisses` (plan/36 §7 #12: nine pushes -> len 8; dismiss newest removes last).

### OV-APPROVAL-GROKP
- severity: P0
- evidence: crates/multiplexer-client/src/turn.rs:13
- problem: The daily agent path is `spawn_grok_turn` from `send()` (`main.rs:458`). Every child is `grok --always-approve --cwd <cwd> -p <prompt>`. `pump()` only maps stdout into an assistant bubble or `mark_error`. Nothing calls `Workspace::set_pending_approval` outside unit tests (`approval_ui.rs` tests only). `GrokAdapter::approval_respond` returns `InvalidState("approvals not wired")` (`crates/multiplexer-provider/src/grok.rs:296`). Server `approval.respond` exists and the bar can render a `PendingApproval`, but the grok -p loop never emits one, so the catalog `ApprovalCard` (Allow / Deny) is dead chrome. Tools run with a silent blanket approve. This is an engine gap (one-shot CLI cannot pause) and a UI honesty gap (the card pretends a gate exists).
- fix: This wave (honest UI): `apps/multiplexer-desktop/src/main.rs` + `crates/multiplexer-client/src/turn.rs` (status / help must say grok -p auto-approves; do not paint an empty gate as live). Next engine slice: drop `--always-approve` only when the in-process adapter can emit `ProviderEvent::ApprovalRequested` and desktop polls it into `set_pending_approval`. TDD: `grok_p_path_never_sets_pending_approval` (after a fake successful turn, `pending_approval()` is None and argv still contains `--always-approve`); later `approval_event_sets_pending_card`.

### OV-APPROVAL-KEYS
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:1460
- problem: The approval bar labels Allow with shortcut "A" and Deny with "D". `handle_key` never maps `a` / `d` to `ClientAction::Approve` / `Deny`. Esc closes palette, then help, then reminder, then resets focus (`main.rs:647`). It never denies or dismisses a pending approval. Palette rows "Approve" / "Deny" exist (`palette.rs:148`) but only work if a pending request was injected. Keyboard users cannot operate the card that the buttons advertise.
- fix: `apps/multiplexer-desktop/src/main.rs` (`handle_key` global A/D when `pending` is Some and no text field owns the key). TDD: `approval_a_d_keys_dispatch` (set pending, key `a` -> `approval.respond` allow and pending cleared; `d` -> deny).

### OV-APPROVAL-BAR
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:1441
- problem: `Surface::ApprovalCard` is painted as a 1-line chrome bar (`approval-card`) that concatenates `card_title()` and `card_body()` into one string plus two `ghost_btn`s. It is not a card: no stacked title/body, no path or command preview, no Warn tone from `plan/31` §3.2, no queue when a second request arrives (`set_pending_approval` replaces). Reminder and approval both sit in document flow under the title and push the workspace down rather than floating over the center pane.
- fix: `apps/multiplexer-desktop/src/main.rs` (`approval_bar` -> floating card over center) + `crates/multiplexer-shell/src/approval_ui.rs` (keep one pending this wave, format title/body separately). TDD: `approval_card_separates_title_and_body` (title is `Allow {tool}?`, body is summary or fallback, allow/deny labels stay distinct).

### OV-REMINDER-CRUDE
- severity: P2
- evidence: apps/multiplexer-desktop/src/main.rs:290
- problem: `refresh_reminder` takes `worktree_paths(frames).nth(1)` and calls `set_reminder("existing", path)`. It ignores `multiplexer_worktree::reminder_from_list`, which already returns a real branch + linked path. The bar then prints `Existing worktree on existing: {path}` (`reminder_bar` `:1416`). The title-bar git pill shows that same `"existing"` string (`:920`). Dismiss is the only action. Esc is advertised on the Dismiss button even though Esc first closes palette/help. No "use that worktree" or "open path" control.
- fix: `apps/multiplexer-desktop/src/main.rs` (`refresh_reminder` must call `reminder_from_list` with the current branch). TDD: `reminder_uses_branch_from_list` (two worktrees on `feat` -> reminder branch `feat` and the linked path, not the literal `existing`).

### OV-STACK-ZORDER
- severity: P1
- evidence: apps/multiplexer-desktop/src/main.rs:873
- problem: Render order is title, reminder bar, approval bar, rails, terminal, status, then palette overlay, then help overlay. Neither overlay sets a z-index. Help is painted last, so it covers the palette when both are open (F1 and Ctrl+K are both handled before the palette-focus branch, so both can be true at once). Esc order is the opposite: palette first, then help, then reminder (`handle_key` `:647`). The user sees help and Esc closes the hidden palette. Help and palette are full-window dimmers, so they cover the reminder and approval bars: Allow / Deny / Dismiss are unclickable while an overlay is up. `settings_open` can be true in the model with nothing painted, so it cannot join the Esc stack. Notices cannot join it either. Flash lives in the status bar under the dimmer. There is no single "topmost overlay" rule.
- fix: `crates/multiplexer-shell` (exclusive overlay enum or a `topmost_overlay()` helper) + `apps/multiplexer-desktop/src/main.rs` (paint one modal at a time; Esc pops topmost; keep approval/reminder above the dimmer or disable dimmer over the gate). TDD: `overlay_esc_closes_topmost_only` (open palette then help -> Esc clears `help_open` and leaves palette; next Esc closes palette; pending approval is not dismissed by Esc).

### OV-APPROVAL-FOURWAY
- severity: P3
- evidence: crates/multiplexer-wire/src/approval.rs:17
- problem: Wire D12 is four decisions (`allow` / `deny` / `allow_once` / `allow_always`). The card exposes two buttons and `respond_approval` only sends `"allow"` or `"deny"` (`main.rs:221`). `plan/36` §5 lists 4-way chrome as later (with provider embed), so this is not a this-wave blocker. It is still a contract lie if the card is ever shown on a provider that needs Allow once / Always.
- fix: Later, with in-process events: `crates/multiplexer-shell/src/approval_ui.rs` + desktop card. TDD: `approval_four_way_labels` (do not implement this wave unless OV-APPROVAL-GROKP is actually gated).

---

FINDINGS: 10  P0:1 P1:3 P2:5 P3:1
