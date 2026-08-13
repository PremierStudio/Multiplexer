# 40: Exhaustive full-featured UI program

Parent synthesis of **18 exclusive Grok reviews** (2026-08-12). Live tree: `4da32e7` plus this plan. Children do not run cargo. Parent owns fmt, clippy, test, mutants.

This file does not replace `docs/PLAN-CONTEXT.md` or `docs/DECISIONS.md`. Where those still say "replace the Grok pager with GPUI" or "editor is the center pane," **this file plus the user's 2026-08-12 direction win** until DECISIONS is amended. `plan/39` is the wave skeleton. **This file wins on depth, control lists, keyboard lock, and engine order.**

Reviewers (exclusive scopes): title chrome, left rail, center chat, Grok TUI host, inspector chrome/session, cores/points, MCP/skills/hooks, git/diffs/files, term/browser/HAR/activity, overlays/commands, keyboard/a11y, visual system, competitor bar, engine honesty, pop-out/persist, remote/fleet/Tailscale, editor review loop, agents/orchestration.

---

## 0. How to read this

Every promised control is tagged:

| Tag | Meaning |
|---|---|
| **NOW** | Ship on current engines (`grok -p`, porcelain, `cmd.exe /C`, detect, RAM pointers). TDD first. |
| **ENGINE** | Needs a new native backend. Do not paint as ready. |
| **HOST** | Real `grok` already does it. Multiplexer launches or observes. Never reimplement. |
| **WONT** | Competitor has it. We refuse. Reason required. |

A click either mutates chrome **or** pushes a `Notice`. Silent no-ops are bugs.

---

## 1. Product locks (do not reopen)

1. **Outlook workbench.** Left list, center host, right inspector, bottom drawer. Pop-out is part of the metaphor.
2. **Do not rebuild the Grok pager in GPUI.** Chat log is `grok -p`. Grok TUI hosts the real interactive `grok` binary (console / Windows Terminal now, ConPTY later).
3. **Grok edits. Multiplexer reviews.** Native editor is a pane or pop-out, never the center.
4. **UI on a real engine, or a labeled stub.** No toast that a missing method is ready.
5. **Hide is occupied 0.** IconRail (44 now, 48 when the kit lands) is a third state.
6. **Native Windows caption stays.** `appears_transparent: false`. Custom glass caption only with `WindowControlArea`.
7. **Left rail stays four destinations:** Threads (Chats), Agents, Files (Projects), Activity. Git, Search, and Settings are inspector / overlay, not extra icons.
8. **One thread list, two projections.** Left Threads is the inbox. Left and right Agents are the work scan of the same `Thread` vec. Not `session_ids`. Not a fake graph.

---

## 2. What the window is today (honest)

Outlook chrome over:

- Chat log: `grok --always-approve --cwd <project> -p <prompt>` off the UI thread. Not `turn.send`. Not in-process.
- Grok TUI: `wt.exe -d cwd grok` or a new console. No `-p`. Host card. Composer hidden.
- Terminal: `cmd.exe /C` one-shot. No kill. No PTY.
- Diffs: porcelain parse + LastTurn / FileName sort. No `git diff` text. Click is last-turn wash.
- Browser: `start "" url`.
- MCP: inventory + `McpLife` flag. Reload resets Ready. Start/Stop silent if nothing expanded.
- Points: RAM pointer. Revert does not touch files. Create error invents `local-N`.
- Cores: `sample_cores`. Copy still says Job Object is armed.
- Remotes: `where.exe tailscale`. Pill says `local+ts`. Detect only.
- Connection: `workspace.connect(Vec::new())` at boot, so Session says **connected** with no hello/ping.
- Palette: substring. Empty query is commands only (good). Hints `g c` … `g b` do not bind.
- Settings: F2 overlay. Esc does not close it. No disk persist.
- Hide: `left_open` / `right_open` bool. Closed rail still occupies **44**. Bottom closed occupies **36**. Drag math is `win_h - y` (status and handle not subtracted).
- Catalog: 51 ids. Painted but missing: pills, hide X, Files/Activity/Agents tabs, `stop_tui`, settings fields.
- `shortcut_map` is asserted at boot and **never read** by `handle_key`.
- `LayoutForest` is unused. One HWND.
- No editor crate.

**Confirmed bugs (parent re-read):**

| Bug | Evidence |
|---|---|
| Empty Send silent | `main.rs` `send()` returns on empty trim |
| `chip_test` shells cargo | `main.rs` 1815-1817 `run_shell("cargo test --workspace --offline")` |
| Boot `connected` | `main.rs` 85 `connect(Vec::new())` |
| Job Object lie | `workspace.rs` 690 |
| Agents `term_meta` | `main.rs` 1497, 1516 |
| Esc skips settings | `handle_key` 968-985 |
| WT stub can mark TUI exited | `pump` `try_wait` on `wt.exe` Child |

---

## 3. Honesty table (brochure vs window)

| Surface | Engine today | UI may say | Forbidden |
|---|---|---|---|
| Chat | `grok -p` | Headless log | Streaming tokens, tool cards, in-process harness |
| Grok TUI | `wt.exe` / new console | Host for the real pager | Embedded / in-pane (until Attached ConPTY) |
| Terminal | `cmd.exe /C` | Command log | Ghostty, vim, PTY ready |
| Diffs | porcelain + sort | Working-tree index | Apply, comments, fake hunks |
| Browser | `start ""` | System browser, CDP later | Viewport, HAR waterfall, Design Mode |
| MCP | inventory + flag | Supervised table, no child | PID, crash from a child, marketplace |
| Points | RAM store | Pointer only, files unchanged | Restored workspace |
| Cores | `sample_cores` | Sampled. Reservation is a flag | Job Object armed |
| Remotes | local + `where tailscale` | Detect only, Serve later | Connect, tickets, 1-100 machines |
| Agents | local threads | Local threads only | Fan-out graph, spawn |
| Editor | none | Absent or Open external | Fake buffer, Vim, LSP |
| Pop-out | forest unused | Absent until second window | Help that says every pane pops out |
| Usage | local counters + char/4 | Local snapshot only | Dollars, account quota |
| Session | unused hello/ping | `disconnected` / `no hello` until handshake | `connected` without hello+id |
| Approvals | chrome, never pending on `-p` | Hidden until a real event | Live A/D on the daily path |
| HAR | no crate | Absent | Fake waterfall |
| Fleet | single node sample | This machine | 100 ghost nodes |

### 3.1 Lying strings to delete in Wave 0

| Copy | Where | Replacement |
|---|---|---|
| `Job Object kill-on-close is armed.` | `workspace.rs` 690 | `CPU samples only. Reservation is a local flag. Job Object is not attached.` |
| `connected` at boot | `main.rs` 85 | Do not `connect([])`. Stay `Disconnected` until hello+ping, then `ready` / `connected` only with a session id |
| Left Agents subtitle `connected` | `main.rs` 1510 | `local session` or drop; click selects the thread |
| `local+ts` | title remotes pill | `local` or `ts detected` |
| `reverted to {id}` | `main.rs` 616 | `pointer set to {id}; files unchanged` |
| Palette `Restore checkpoint` | `palette.rs` | `Select checkpoint pointer` |
| MCP Ready as Good/live | rows + status `mcp ready/N` | `flag` / `mcp listed N` |
| `Reuse/teardown still applies` | `mcp_detail` | `Inventory only. No child.` |
| Palette hints `g c` … `g b` | `palette.rs` | Empty until prefix mode exists |
| `chip_test` cargo shell | `main.rs` 1815 | SendPrompt `Run the tests` |
| Empty center `Start a session` | widgets | `What should we build?` + honest `-p` subtitle |
| Composer `Message Grok…` | `main.rs` 1852 | `Headless grok -p…` until Engine C |
| README `Grok Build-embedded` | README | `Grok Build-vendored` / `CLI grok -p` |

Honesty tests must `assert!(!text.contains(lie))`. Existing tests that lock `connected` after `connect([])` must be rewritten.

---

## 4. Full destination by surface (nothing left out)

### 4.1 Title, hide, resize, status

**Pills (NOW, 20px, clickable):**

| Pill | Click | Shift+click | Honesty |
|---|---|---|---|
| Project | Open left Files + right Files | Copy full path | cwd, not a picker this wave |
| Branch | Open Git | Copy branch | From `git_status`, never reminder `"existing"` |
| Model | Open Session models / Settings list | Cycle | Catalog from `[model.*]`, not cycle-only |
| Turns | Open Session usage | Copy `usage_lines()` | `local snapshot only` |
| Remotes | Open Settings remotes | Copy detect list | `Serve later`. Never Connect |

**Three-state rails:**

| Gesture | Left / right | Bottom |
|---|---|---|
| Title chats / inspector | Open ↔ IconRail | n/a |
| Title terminal | n/a | Peek ↔ Open (restore `last_open`) |
| In-pane X | **Hidden (0)** | **Hidden (0)** |
| Icon click / `Ctrl+1..4` / `SelectTab` | Open | n/a |
| Reset | both Open, bottom Peek | |
| Focus layout | both Hidden, bottom Peek | Shift+click Reset + palette |

Occupied: Open = width (left 180-420, right 220-480), IconRail = 44 (48 in Wave 4), Hidden = 0. Bottom: Hidden 0, Peek 36, Open last height default 240, min 80, max 480.

**Drag (NOW):**

```
left  = (mouse_x - pad).clamp(180, 420)
right = (win_w - mouse_x - pad).clamp(220, 480)
bottom = (win_h - mouse_y - status_h - handle_h).clamp(80, 480)
```

`PAD=8`, `STATUS_H=28`, `HANDLE_H=8`. Viewport is client area: do **not** subtract native caption. Double-click handle resets 240. `Ctrl+Alt+Up/Down` nudge 16px.

**Overflow (narrow title):** keep chats toggle, project, model, Run/Stop, palette, inspector. Drop remotes, then turns, then branch, then Settings/Help/Terminal/Reset into `⋯`. Never clip a live control without the overflow menu.

**Status (28px, clickable chips):** connection (honest), cwd, model, `{turns} · {tok} tok` local, git dirty from last porcelain, `mcp N cfg`, CPU average %, process working set, layout name (`Outlook` / `Focus` / `Custom`). Drop duplicate `cpu {count}` and `mcp ready/total`.

**Window:** extract `window_options()`. Native caption. Min 920×620. First-run 1360×860. Persist bounds in AppData. Snap is OS. No tray (recommend never in Waves 0-4).

**Focus layout:** `Ctrl+Shift+H` + Shift+click Reset + palette. Second invoke restores snapshot.

### 4.2 Left rail

**Threads (Chats):** 56px cards. Title, preview (`thread_preview`), model badge, pulse from `status`, unread dot, pin mark. Hover delete. New / Del. Filter. Context: Open, Pin, Rename, Mark unread, Copy id, Archive, Delete, Stop if selected+running. No drag-reorder (pin + recency). Export ENGINE.

**Agents:** same `AgentRow` as right Agents. Banner `Local threads only`. Click = `SelectThread` + composer. No New/Del. No `term_meta`. No `session_ids` as rows. Session id is a badge on the selected thread.

**Files / Projects:** current cwd tree, not worktrees, not remotes. Leaf titles, folder/file glyph, indent, selected wash. Folder click expands. File click selects + opens right Files. Header Reload. Context: Mention, Reveal (copy abs), Open external, Copy relative. Cap 80 + truncated hint. Worktree switcher stays on Git.

**Activity:** projector (see 4.9). Same ids as right Activity. Never a blank pane. Click jumps source.

**Icon rail:** ids `left-rail-threads|agents|files|activity`, 32 hit in 44 (48 later), tooltip = `rail_label()`. `Ctrl+1..4`. `Focus::Left` + Up/Down/Enter/Delete/`n`.

### 4.3 Center Chat log (`CenterMode::Gui`)

Not a pager clone. Mode bar caption stays honest.

**NOW:**

- Thread header: title, model, `headless · grok -p` / `working · {elapsed}`.
- Context strip: `Grok does not see earlier bubbles. This send is a new process.`
- Working row: `Grok is working · {n}s` + `One-shot headless turn. No live tokens.`
- Transcript: overflow-y scroll, wrap, You / **Grok** (not Agent), timestamps, hover copy, collapse long, error tone, interrupted as system.
- Empty: 2×2 tiles from `empty_state_tiles()`. Hide chips after first message. `chip_test` SendPrompt.
- Composer: min 56 (72 when kit lands), placeholder `Headless grok -p…`, hint **above** well, circular send, disabled attach (`coming`).
- `/stop` works while busy (slash before busy gate).
- Empty send: Notice.
- Unknown slash: keep draft + Notice.
- Layer G slashes (`/compact`, `/rewind`, …): list as `Grok TUI · not in Chat log`. Do not send as `-p`.
- Per-thread draft + queue while busy. Interrupt does not drain queue.
- Retry last / edit-and-resend as new `-p`.
- `@` picker from `workspace.files`.
- Drop `ensure_session` as a hard fail on the `-p` path (session RPC is unused by the turn).
- Stop tooltip: `Ignore this turn. The grok -p process is not killed.` until killable Child.

**NOW+:** killable `Child` for `-p`. Selectable bubble text. Optional stitch-last-K (default off, labeled local). `--model` only after argv tests prove grok honors it.

**ENGINE C:** `VendoredGrokFactory` or ACP `TextDelta` into the last bubble. Real interrupt. Attachments. Session context. Then hide the stitch banner. Still not a pager.

**WONT:** voice mic (dead mic is a lie). Tool cards before Engine C.

### 4.4 Grok TUI host

Host the real pager. Never clone scrollback, slash, dashboard, plan, memory, imagine.

**Wave 0:** track **grok** pid, not `wt.exe`. `TuiLife::Failed` ≠ idle. Footer stays `In-pane ConPTY is later`. Catalog `stop_tui`. Reload porcelain on grok exit. Honesty string table.

**Later NOW:** `--cwd`, `-s <uuid>`, optional `--model`, YOLO checkbox **off** by default. WT argv `--window new nt --title --`. Fallback console. Focus console (`SetForegroundWindow`). Detach / Reopen `-r`. One TUI per Mux thread (lazy) with same-cwd notice. Quit prompts; default detach.

**ENGINE:** ConPTY + cell grid **of real grok** in center. Share `multiplexer-terminal` with the drawer. HostReserved only: `Ctrl+Shift+G`, `Ctrl+Shift+L`, `Ctrl+[` `]`, `Ctrl+Shift+P` palette, `` Ctrl+` ``, F2. Never steal Ctrl+K/P/N/S/./Tab/Esc/? from grok.

**WONT:** GPUI clone of `xai-grok-pager`. Left rail listing grok's dashboard sessions (clone risk).

### 4.5 Right chrome + Session + Settings

**12 destinations stay.** Session cannot be hidden. Overflow ▾ when the icon column is short. Customize order/hide in Settings. Palette unhides.

**Icon-only toolbar.** Hide empty toolbar containers (Term/Skills/Activity today leak a row).

**Session rows (NOW):** project, model, models (`*` current), connection, handshake, session id, threads, usage, usage note `local snapshot only`, last error, palette, help.

Connection labels: `disconnected` → `connecting` → `ready` (hello+ping, no session) → `connected` (non-empty id) → `error`.

**Model catalog:** parse `~/.grok/config.toml` `[model.*]` keys (no `op://` resolve). Merge stub `model.list`. `/model <id>` selects.

**Usage:** `UsageSnapshot` on Workspace. Bump turns on **finished** turn, not `send_draft`. Every surface prints the note.

**Settings PAGE:** exclusive full-window (center dimmed, not `CenterMode::Settings`). Nav: Appearance, Models, Bindings, Inspector customize, Session/usage, Remotes/About. Persist `%APPDATA%\Multiplexer\settings.json`. No secrets. Esc pops. `Ctrl+,` + F2 + gear. Remove New WT leak from Settings.

**Pop-out (Wave 4):** `LayoutForest::detach(3)` + second `open_window` + Dock. Ghost strip on primary. `Ctrl+Shift+D` / `E`. Until then: do not advertise pop-out.

### 4.6 Cores + Points

**Cores NOW:** delete Job sentence. Long-lived `sysinfo::System`. Merge reserved by index (never pass `0..8` as reserved). Click toggles **flag**. RAM line. Caption: reservation is a flag, not pinned. Process list absent or `No contained processes`. `CoreCell` 4-wide bars in Wave 4. Power-adaptive 1/5/15s when selected.

**Cores ENGINE:** `ResourceManager` + `JobContainment` on live children. Pin + caps. `telemetry.resources`. Job badge only when assigned. Fleet 1-100 is Phase 4, separate from this tab.

**Points NOW:** banner `Pointer only. Files unchanged.` List from `checkpoint.list` after create/revert/tab enter. No `local-N`. Click selects. Revert requires selection (Notice, not last-row). Success: `pointer set to {id}`. `seq` badge. Palette renamed.

**Points ENGINE:** hidden-git refs, revert that resets the tree, `restored: true` only then, `checkpoint.diff`, confirm+scope, pre/post on turns.

### 4.7 MCP + Skills + hooks

**MCP NOW:** `selected_mcp` (survives tab switch). Merge inventory by name (Reload must not reset Ready). Disable Start when Ready. Notice if none selected. Shared helper for inspector + `ClientAction`. Restart = table-only. Subtitle `inventory flag (no child)`. Plus form writes `config.toml` (env **keys** only, no values). Optional `@mcp:name` text mention with toast `text only`. Marketplace **absent** (no empty tab).

**Skills NOW:** `SkillItem { name, source, enabled, preview }`. Refresh off UI thread. Enable is a local flag (`not loaded into grok`). Preview first 4 KiB of SKILL.md. Create writes SKILL.md.

**Hooks:** section only if `hooks.toml` parses non-empty. List, do not run. Worktree hooks: paths only if mux hooks file exists. Reminder uses `reminder_from_list`, not `nth(1)` + `"existing"`.

**ENGINE:** live `Supervisor` + Job, PID, crash, log tail, `mcp.*` RPC, marketplace, skill disable persisted to grok, hooks trust, worktree runner.

### 4.8 Git + Diffs + Files (review workbench)

**Git NOW:** header path / branch / dirty / ahead-behind. Create form on the tab (`wt_path`, `wt_branch`, `wt_create_branch` + existing RPC). Relabel New WT → Create. Store `WorktreeCard`, not `Vec<String>` paths only. Reminder from `reminder_from_list`. Resume / Reuse / Dismiss. Switch cwd = set `ws.project` + reload files/diffs. Remove + confirm (never `-f` first; refuse dirty unless confirm force; refuse primary). Status porcelain in selected cwd. Off-thread git.

**Diffs NOW:** `selected_diff`. Click selects (not last-turn wash). Last-turn is a badge. Sort LastTurn / FileName (leaf) / Status / Path. Expand = host `git diff -- path` text (unstaged, then cached, then untracked note). Cap ~64KiB. Mention `@path`. Open external. Reload after TUI exit **and** after `-p`. Honesty: text only, no apply.

**Files NOW:** Reload, Reveal, Mention, Open. Leaf titles. Selected wash on left and right. Stop `CycleFile` rotation (Next file). Filter box. Empty: muted line + real Reload, not a fake row.

**ENGINE:** structured `git.diff` hunks, stage/discard, comments → agent, commit/push/PR, `multiplexer-editor` pane.

**Open external NOW:** `$VISUAL` else `$EDITOR` else `start ""`. Fire-and-forget. Stays after the native pane ships.

### 4.9 Terminal, Browser, HAR, Activity

**Strip NOW:** `ProcessCapture` (already in `multiplexer-terminal`, unused). Stream `try_read`. Kill button + `Ctrl+.` kills capture first if live. `term_cwd` + `cd`/`pwd`. History Up/Down. Hidden 0 / Peek 36 / Open last. Builtins via `SelectTab` (open a closed rail). `help` does not toggle F1 overlay. Empty Run Notice.

**Term inspector:** Play / Clear / Kill. Stable `term:{seq}`. Same log.

**Browser NOW:** detect Chrome/Edge/Firefox/Brave/Arc via registry + well-known paths. Default = StartMenuInternet else Edge else Chrome. Open that exe. Detect button. Still `CDP later`. No viewport.

**HAR:** no tab until crate + import or live capture. Import-only tab only if labeled `imported file, no live CDP`.

**Activity NOW:** `activity_items` order: reminder, approval, busy, notices, log. Stable ids (`act:log:{seq}`). Filter. Jump: reminder→Git, approval→card, busy→composer, notice→dismiss, log→Term. Left Activity is the same projector (cap 20). Empty: one `act:empty` that opens right Activity.

**Approvals:** hide card when `pending` is None. Daily `-p` never sets pending. When pending exists (tests / Engine C): D12 four-way Allow / Deny / Allow once / Always. Esc does not deny unless approval is the Esc target (parent lock: Esc = Deny only when approval is top and no overlay).

**ENGINE:** Ghostty/ConPTY drawer, CDP, Design Mode, HAR waterfall, replay.

### 4.10 Overlays and commands

**Overlay policy (parent lock):**

- Palette and Help may stack (max 2). Esc pops top.
- Settings and Search are **exclusive** (replace, do not sit under a hidden palette).
- Approval, reminder, toasts, context menu are not on the modal stack.

**Palette NOW:** `PaletteHit` + fuzzy subsequence. Namespaces Commands / Panes / Files / Threads. Empty query = commands + panes + recent (cap 8). Never dump the file tree. Group headers. Scroll so selected is visible. Real query caret. Preview column if the card stays ≤720px.

**Search:** `Ctrl+Shift+F`. Names only. Empty = hint, no hits. Content search ENGINE.

**Toasts:** cap 8, paint 3, top-right. Info/Good auto 4s. Warn/Danger stay. Delete `flash`. Esc dismisses newest when no modal.

**Context menus:** thread, file, MCP, checkpoint, diff, worktree. Same `dispatch` path.

**One registry:** `ControlSpec.action` == `CommandId` == palette id. Collapse `InspectorAction` into `ClientAction`. Slash is another input to the same registry (`/files` `/agents` `/search` `/settings` `/diff` `/browser` `/tui`).

### 4.11 Keyboard, focus, a11y

Replace `Focus { Composer, Terminal, Palette }` with regions: Left, Center, Composer, Right, Bottom, Overlay.

`handle_key` reads `BindingTable`. Overlay context wins. `CenterTui` is hatch-only when in-pane (and ignores printable keys when TUI is external).

**Tab:** Ctrl+Tab cycles regions (skip hidden). In-region Tab cycles controls. Composer Tab accepts slash/@ menu else leaves region. Live composer↔terminal Tab swap goes away.

**Lists:** Up/Down/Home/End/Enter. Delete on Threads only.

**A11y NOW:** names on every icon (not glyph-as-name), contrast tests, `reduce_motion`, `ui_scale` 100-200, overlay max width `min(560, viewport-48)`, high-contrast token table (Settings toggle).

**Screen reader:** ENGINE / unknown. GPUI 0.2.2 has no AccessKit in this tree. Do not advertise NVDA. Keep name table for the day it lands.

### 4.12 Visual system

Wave 4 (can start V0 fills in parallel with honesty):

- Ban `hsla(` in `main.rs`. Extract `apps/multiplexer-desktop/src/widgets.rs` + `rows.rs`.
- Glass bands: pane ≤0.55. Kill approval/toast 0.70/0.75.
- Compact default when kit lands: title 48, rail 48, pills 20, icons 32, status 28, fleet row 56, inspector line 36.
- Light and dark both paint. No leftover dark fills.
- Every row: clip, nowrap, leaf names, no raw `thr-N`.
- Empty pattern: mark + title + body + Primary. Not fake rows.
- Unicode glyphs now; path icons + brand PNGs in V3.
- Toast stack replaces full-width illegal bars.

Native caption stays. plan/35 transparent caption loses to plan/28.

### 4.13 Windowing, persist, first-run, crash

**NOW:** `window_options`. Settings JSON (theme/density/model/bindings). Layout JSON in AppData keyed by project (not in-repo). Per-thread draft (required before crash journal).

**Wave 4:** inspector pop-out, then left/bottom/center. Close pop-out = dock. Refuse detach of last live primary.

**First-run:** project, detect grok, theme, keychain notice (no paste). Skip writes the flag.

**Crash journal:** AppData, threads+drafts, marker. Notice: `Restored chats and drafts. Files and checkpoints were not replayed.` Running `-p` does not resume.

**Single-instance:** named mutex + pipe. Second launch hands off argv.

**Deep links:** `multiplexer://pair|session|open` parse now; pair is honest stub until relay.

**About:** version, SHA, Apache-2.0, grok path. Updates: `not shipped` until plan/18 R4. Never fake "up to date."

**Tray:** default off. Not in Waves 0-4.

### 4.14 Remote, Tailscale, usage, pairing

**NOW:** remotes pill → Settings Serve later. Refresh detect. Copy local URL (or `in-process, no listen` notice). Copy MagicDNS **only** if `tailscale status --json` has `DNSName`. Three-state: absent / binary / running. Usage: no `$`. Account: `not signed in`. Pairing QR preview labeled `Device exchange later` (no secret persist).

**ENGINE:** `multiplexer-remote`, tickets, DPoP, Serve (not Funnel), A-controls-B, fleet `NodeState`, SSH worktrees, live mobile pairing, keychain `SecretStore`.

### 4.15 Editor + review loop

**NOW:** Open external, Reveal, Mention, diff text preview, optional `@path:12-18` from preview lines (labeled preview lines, not file lines).

**PANE:** `crates/multiplexer-editor` on `PaneId(3)` or Diffs-hosted body. **Reject Editor on `PaneId(2)` in tests.** Read-only first, then edit. Pop-out via detach.

**D4:** rope, tree-sitter, Vim, LSP (no bundled servers), hunk apply (`git apply --3way`), anchored comments → `userInput.respond` when the adapter has it; prose fallback until then.

Comment send is explicit. No fake apply toast.

### 4.16 Agents + orchestration

**NOW:** typed `AgentRow` + `ThreadStatus { Idle, Running, Error }`. Model copied on new. Elapsed while running. Banner on screen (not only dead `agents_detail`). `/agents`, `Ctrl+2`, catalog ids. Optional `orchestration.list` stub `{ threads, subagents: [], note }`. `orchestration.spawn` stays method not found.

**ENGINE:** fork `spawn_subagent` (upstream default is **32**, not 16; amend DECISIONS). Our scheduler, 1-100 with resman. Right tree + budgets + cancel child. Left stays flat. Workflows UI after spawn is observable. No Rhai editor this year.

---

## 5. Competitor bar (compressed)

130+ rows live in the reviewer report. P0/P1/P2 for Multiplexer:

**P0 (Waves 0-3, no new engines):** honesty + click contract; live pills; hide/drag math; fuzzy palette + name search; Git create form; diff text + mention + open external; Files Reload/Reveal/Open; Agents list rows; MCP merge + disable; model pick by id; Settings page + persist; toast stack; TUI copy never says embedded.

**P1:** BindingTable; hello/ping; Points list+banner; Cores flags; Skills/hooks; Activity projector; inspector pop-out; light/dark; leaf names; remote detect; kill pending cmd; context menus.

**P2 engines (do not fake):** ProcessCapture; hidden-git; hunks+comments; ConPTY host grok; Engine C stream; CDP/HAR/Design Mode; editor crate; Ghostty splits; MCP children + marketplace; Tailscale Serve; mobile; scheduler; GitHub/Linear; CLI; signing/updates.

**Never clone:** pager, center IDE, bundled Chromium, Electron, Warp-the-product, 75-provider identity, fake engines, macOS-only glass, live `op`, "relay is E2EE."

**How we win:** host the best pager; Outlook review workbench they do not have; engines only when true; one server, thin clients.

---

## 6. Keyboard lock (conflicts resolved)

| Chord | Action | Notes |
|---|---|---|
| `Ctrl+K` | Palette | Keep. Not a hatch in TUI (Grok uses Ctrl+K to scroll). |
| `Ctrl+Shift+P` | Palette (advertised) | VS Code / Cursor muscle. Hatch-safe. |
| `Ctrl+P` | Search / files | **Breaking.** Today this is palette. |
| `Ctrl+Shift+F` | Search overlay | Names only. |
| `Ctrl+N` | New thread | Not in TUI hatch. |
| `Ctrl+[` `]` | Rails Open ↔ IconRail | Also `Ctrl+B` alias for left. |
| `` Ctrl+` `` | Bottom Peek ↔ Open | Hatch. |
| `Ctrl+.` | Kill capture if live, else grok interrupt | Not in TUI hatch. |
| `Ctrl+S` | Checkpoint pointer | Help must say pointer only. Not in TUI. |
| `Ctrl+Shift+G` | Toggle center mode | Hatch. |
| `Ctrl+Shift+L` | Reset Outlook | |
| `Ctrl+Shift+H` | Focus layout | |
| `Ctrl+Shift+D` | Pop-out inspector | **Not** Diffs tab. Diffs = icon / `/diff`. |
| `Ctrl+Shift+E` | Dock | |
| `Ctrl+W` | Close pop-out | Refuse last primary. |
| `Ctrl+Tab` | Next region | |
| `Ctrl+1..4` | Left sections | |
| `Ctrl+,` / `F2` | Settings | Hatch F2. |
| `F1` | Help | Not `?`. |
| `Ctrl+Alt+Up/Down` | Bottom height | |
| `A` / `D` / `O` / `L` | Approval 4-way | Only when pending and no overlay. |
| `Esc` | Pop overlay → toast → reminder → (approval Deny if top) → composer | Never first Esc in Attached TUI. |

**Do not bind globally:** `Ctrl+Shift+W` (Git vs Stop TUI vs close). Stop TUI = button + palette. Git = `/git` + icon.

Prefix `g c` is out until a real prefix state machine exists.

---

## 7. Wave program

TDD each slice. Children write tests+code. Parent cargo. Do not mix Wave 5 engines into chrome PRs.

### Wave 0. Honesty + click contract (first implementation)

1. Delete lying copy (Job Object, boot connected, `local+ts`, revert, Ready-as-process, `g *`, `local-N`).
2. Every dead click: Notice or mutate. Empty send, last-thread delete, MCP no row, mention no file, Revert no selection.
3. Title pills click.
4. Left Agents = `agent_rows` / `SelectThread`. Kill `term_meta`.
5. `row_detail` non-empty or drop `cursor_pointer`.
6. Help Close. Settings Esc. Approval A/D only when pending.
7. Catalog = painted set (pills, hide ids, `tab_files` / `tab_activity` / `tab_agents`, `stop_tui`).
8. `chip_test` SendPrompt. `/stop` before busy gate.
9. TUI: grok pid not `wt.exe`; porcelain on exit; `stop_tui` catalog.
10. Amend PLAN-CONTEXT / plan/03 / D-text: host the pager, editor is a pane. README badge honesty.
11. Extract `window_options`.

### Wave 1. Chrome that hides and resizes

Three-state rails + bottom Hidden 0. Drag math. Overflow `⋯`. Focus layout. Status fields. Disabled empty Run. Danger Stop.

### Wave 2. Command system

Overlay policy. BindingTable drives `handle_key`. Fuzzy palette + namespaces. Search overlay. Toast stack. Settings page + persist. `Ctrl+1..4`, `Ctrl+,`, `Ctrl+Shift+P` / `Ctrl+P` split. Recent commands. Collapse `InspectorAction`.

### Wave 3. Workbench that reviews

Git form + `reminder_from_list` + switch/remove. Diff text + mention + open external + TUI-exit reload. Files toolbar + filter + no rotate. Session handshake + models from config. Points list+banner. Cores merge+flag+RAM. MCP merge+disable. Skills/hooks. Activity projector + left sync. Agents full rows. ProcessCapture kill. Browser detect. Remotes Settings Serve later. Per-thread draft + chat working row + context strip.

### Wave 4. Outlook metaphor + density + pop-out

Widgets extract. Ban raw hsla. Compact 48/48. Light+dark. Leaf names. Inspector pop-out. Path glyphs. Toast top-right. Context menus.

### Wave 5. Engines (own milestones, this order)

1. Hidden-git + revert that touches files + `checkpoint.diff` (stops the Points lie for real).
2. Structured `git.diff` hunks + comments → thread.
3. ConPTY cell host of **real grok** (shared with drawer).
4. Engine C: in-process/ACP streaming Chat (product change).
5. CDP detect/launch → HAR import → waterfall → Design Mode. Then `InspectorTab::Har`.
6. Tailscale Serve + tickets + attach.
7. `multiplexer-editor` pane/pop-out (D4 list, not center).
8. Ghostty-class PTY splits.
9. Live MCP children + marketplace + hooks trust.
10. Fork scheduler, fleet 1-100, mobile pairing live.

Killable ProcessCapture is **Wave 3**, not 5 (crate already exists).

---

## 8. TDD contract (named tests that must go red first)

Highest-value reds (full lists live in the 18 reviews):

**Honesty:** `resource_detail_does_not_claim_job_object`, `new_workspace_is_disconnected`, `connect_empty_sessions_is_not_connected`, `revert_copy_is_pointer_set_not_restored`, `create_error_does_not_push_local_n`, `mcp_badge_is_not_ready_good`, `palette_hints_have_no_unbound_prefix`, `chip_test_is_send_prompt`, `empty_send_pushes_notice`.

**Chrome:** `rail_vis_hidden_occupies_zero`, `toggle_left_cycles_open_and_icon_only`, `hide_left_is_hidden_not_icon`, `bottom_hidden_occupies_zero`, `toggle_bottom_restores_last_open_height`, `bottom_drag_subtracts_status_and_handle`, `title_overflow_drops_remotes_then_turns_then_branch`, `focus_layout_hides_both_rails`.

**Chat:** `plan_send` (busy+`/stop` = Slash; busy+text = Queue; idle+unknown slash keeps draft), `draft_persists_per_thread_on_select`, `working_copy_formats_seconds_and_minutes`, `classify_slash` Layer G vs Mux.

**TUI:** `grok_launch_has_no_prompt_flag` (keep), `summary_names_surface_not_embedded`, `after_exit_requests_porcelain`, `mark_failed_is_failed_not_stopped`.

**Workbench:** `reminder_from_list_used_not_nth`, `diff_click_selects_not_last_turn`, `cycle_file_selects_next_without_rotating`, `merge_mcp_inventory_preserves_ready_by_name`, `checkpoint_rows_lead_with_pointer_banner`, `agent_rows_are_threads_not_session_ids`.

**Commands:** `overlay_esc_closes_topmost_only`, `settings_esc_pops_stack`, `binding_table_drives_resolve`, `palette_fuzzy_subsequence_ranks`, `search_hits_rank_files_threads_commands`, `every_control_is_a_command`.

**Theme:** `no_chrome_alpha_above_pane_cap`, `dark_text_contrasts_bg`, `window_options_keep_native_caption`. CI grep: no `hsla(` in `main.rs`.

Mutation floor 70% (D33) on new modules: chrome occupancy, slash classifier, inventory merge, BindingTable, honesty copy helpers, porcelain/status parsers.

---

## 9. Catalog delta (minimum)

Add Surfaces: `StatusBar`, `Settings`, `Search`, `ToastStack`, `ContextMenu`, `Resize`.

Add ids (non-exhaustive; catalog must equal paint): `project_pill`, `branch_pill`, `model_pill`, `turns_pill`, `remotes_pill`, `title_overflow`, `hide_left`, `hide_right`, `hide_bottom`, `focus_layout`, `left_section_*` (4), `tab_files`, `tab_activity`, `tab_agents`, `stop_tui`, `focus_tui`, `refresh_files`, `reveal_file`, `open_external`, `start_mcp`, `stop_mcp`, `restart_mcp`, `refresh_skills`, `term_kill`, `detect_browsers`, `toggle_search`, `settings_close`, `settings_theme`, `settings_density`, `pop_out_inspector`, `dock_inspector`, `allow_once`, `allow_always`, `toast_dismiss`.

`REQUIRED_IDS.len()` is no longer frozen at 51. Update every length pin in the same commit as the paint.

---

## 10. Docs to amend (Wave 0 chore)

| Doc | Change |
|---|---|
| `docs/PLAN-CONTEXT.md` | Host pager, do not replace. Editor is pane/pop-out. Header: differentiators are targets, not the running desktop. |
| `docs/DECISIONS.md` | New D: host pager + review editor as pane. Amend D4 note ("center pane"). Amend D11 "16-child" → upstream default 32, we still fork. |
| `plan/03`, `plan/09`, `plan/10`, `plan/19` | Same pager/center wording. |
| `plan/08` | Status: ENGINE after command-log strip. |
| `plan/33` | Tab count 12, not 7. |
| `README.md` | Badge and comparison table honesty. |
| Audits `01-11`, `14-16`, `19-20` | Stamp obsolete when the matching wave lands. |

---

## 11. Open questions for Justin

Each has a recommended answer so work is not blocked.

1. **Hide split.** Title cycles IconRail. X is Hidden 0. **Recommend yes.**
2. **Chat forever `-p` vs Engine C.** **Recommend:** `-p` is the honest face until a named Engine C milestone. Do not drip fake deltas.
3. **External editor now.** **Recommend yes** (`$VISUAL` / `start`). Native pane later. Both stay.
4. **Persist settings this month.** **Recommend yes:** theme/density/model/bindings. No secrets. AppData, not in-repo.
5. **Focus layout.** **Recommend** `Ctrl+Shift+H` + Shift+click Reset + palette.
6. **Ctrl+P break.** Today palette. Destination search/files. **Recommend break it** and advertise `Ctrl+Shift+P` for palette.
7. **Tray.** **Recommend no** (or default off forever until asked).
8. **Settings page vs overlay.** **Recommend** exclusive full-window page (dimmed center), not `CenterMode::Settings`.
9. **One TUI per thread vs workspace.** **Recommend** per thread, lazy, same-cwd warning. Wave 0 can stay one Child.
10. **TUI default permission.** **Recommend Ask.** YOLO checkbox off.
11. **Detach last primary pane.** **Recommend refuse.**
12. **Single-instance.** **Recommend yes** + project switch.
13. **Points Revert button.** **Recommend keep**, rename to Set pointer, until hidden-git.
14. **Left Agents vs merge into Threads.** **Recommend keep both** (inbox vs work scan). Duplicate paint is the failure mode.
15. **Screen reader promise.** **Recommend none** until AccessKit spike.

---

## 12. First implementation slice

After this doc: **Wave 0 + Wave 1** (honesty, dead clicks, hide-to-zero, drag math, pill clicks, Agents `SelectThread`, TUI pid honesty, catalog bump). That is the screenshot class of bugs ("90% of buttons", overflow, cannot hide, bottom does not resize).

Do not start ConPTY, CDP, editor crate, Serve, or spawn in that PR.

Parent sequence inside the slice:

1. Headless tests in `multiplexer-shell` (occupancy, honesty copy, Agents click, empty send Notice).
2. Desktop wiring.
3. Parent: fmt → clippy `-D warnings` → test → mutants on touched modules.

---

## 13. Critical files

- `apps/multiplexer-desktop/src/main.rs` — title, rails, center, drag, `handle_key`, send, TUI pump, boot `connect([])`
- `apps/multiplexer-desktop/src/controls.rs` — catalog and `shortcut_map`
- `apps/multiplexer-desktop/src/inspector.rs` — toolbars
- `crates/multiplexer-shell/src/workspace.rs` — chrome, threads, MCP, checkpoints, lying `resource_detail`
- `crates/multiplexer-shell/src/actions.rs` / `bindings.rs` / `palette.rs` / `inspector_model.rs`
- `crates/multiplexer-client/src/tui.rs` / `turn.rs`
- `crates/multiplexer-theme` — bands and light table
- `crates/multiplexer-layout` — detach/redock (Wave 4)
- `docs/PLAN-CONTEXT.md` / `docs/DECISIONS.md` — Wave 0 amend
