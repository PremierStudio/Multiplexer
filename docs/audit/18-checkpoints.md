# 18. Checkpoints UI audit

**Date:** 2026-08-12
**Scope:** Points inspector rows, `create_checkpoint` / `revert_checkpoint`, server `checkpoint.*`, `plan/07-checkpointing-and-vcs.md`.
**Method:** Read-only. No `cargo`. Compared live code to plan/07, plan/04 §4.5, plan/19 §1.8, plan/32 §5.5.
**Verdict:** The Points tab is a labeled pointer list over an in-memory stub. It is not hidden-git checkpointing. Create does not snapshot the tree. Revert does not restore files. There is no diff.

FINDINGS: 8

Honesty first: most of the product gap is **engine**. The UI already has a tab, New/Revert buttons, palette/slash/Ctrl+S, and rows. Those controls are wired to RPCs that only move RAM pointers. Painting a fuller inspector will not make revert real. The engine must grow git refs (or another durable snapshot) before the UI can tell the truth.

---

## Layer split

| Layer | What exists | What plan/07 requires |
|---|---|---|
| Engine (`multiplexer-checkpoint`) | In-memory `Vec` + `current` map. Ids `cp-N`. Label + seq only. Crate says it does not spawn git or touch the filesystem. | Hidden refs under `refs/multiplexer/threads/<id>/checkpoints/{pre,post}/<seq>`. `git add -A` + `commit-tree` + `update-ref`. Per-turn pre/post. |
| Engine (server) | Dispatches `checkpoint.list` / `create` / `revert` onto that store. `checkpoint.diff` and `checkpoint.apply` are wire constants with no handlers. | Structured diff from ref ranges. `git reset --hard` (or checkout) plus read-model truncate. Selective apply. |
| Engine (provider / core) | `ProviderAdapter` has no `checkpoint_revert`. Desktop uses `Server::with_local` (provider bridge + separate catalog), not `RuntimeBackend`. Session start does not bracket turns. | Adapter method + orchestration `CheckpointCreated` / `CheckpointRevert`. Every turn captured. |
| UI (shell + desktop) | Points tab, New/Revert, rows titled by label. Local `Vec<CheckpointRow>` (id + label only). Click expands. Always labels `"manual"`. | Click selects. `#seq` badge. Confirm + scope. Diff pane. Do not claim restore until the engine restores files. |

The crate description is explicit: "In-memory hidden-ref checkpoint store (Phase 1.8 stub; no real git)." Plan/19 §1.8 still lists that milestone as "hidden git refs per turn, diff query, revert." The stub shipped. The git backend did not.

Vendored grok-build has a different rewind model (`xai-grok-workspace` hunks + optional `.grok/rewind-checkpoints/*.json`). Multiplexer did not embed that either. Plan/07 chose hidden git refs, not grok-build rewind. Neither path is live.

---

## F1. Store is in-memory only, not hidden-git

- **Severity:** Critical
- **Layer:** Engine (UI only displays the stub)
- **Plan:** 07 §1.1-1.3, 19 §1.8, 06 checkpoints as hidden git refs
- **Evidence:**
  - `crates/multiplexer-checkpoint/src/lib.rs` lines 1-4: "Ids stand in for `refs/multiplexer/...` until a real git backend lands. This crate does not spawn git or touch the filesystem."
  - `crates/multiplexer-checkpoint/src/store.rs` line 1: "In-memory checkpoint table. No git refs are written."
  - `Checkpoint` is `{ id, session_id, label, seq }`. No SHA, no ref name, no phase, no path list.
  - `create` allocates `cp-{next}` and pushes a struct. Grep of first-party crates finds no `git add`, `commit-tree`, `update-ref`, or `refs/multiplexer`.
  - `Cargo.toml` description names the stub.
- **Why it matters:** Capture is supposed to be cheap because Git trees are content-addressed. A RAM row is not a workspace pointer. Restart loses history. Two processes cannot share it. Revert and diff have nothing to aim at.

---

## F2. Revert does nothing to files

- **Severity:** Critical
- **Layer:** Engine. The UI then reports success as if restore happened.
- **Plan:** 07 §1.4, §6.1-6.3 (`git reset --hard` or checkout, then truncate/replay the read model)
- **Evidence:**
  - Engine: `CheckpointStore::revert` (`store.rs` 82-89) looks up the id, writes `current[session_id] = id`, returns the same struct. Comment: "Existing checkpoints are kept (revert is not a truncate)." Tests (`crates/multiplexer-checkpoint/tests/store.rs`) only assert the pointer and list length.
  - Server: `checkpoint_revert` (`crates/multiplexer-server/src/server.rs` 285-307) calls `catalog.revert` and encodes the row. No cwd, no git, no fs.
  - Adapter: `ProviderAdapter` (`crates/multiplexer-provider/src/adapter.rs`) has no `checkpoint_revert` (required by PLAN-CONTEXT and plan/19 §1.3).
  - UI: `ShellView::revert_checkpoint` (`apps/multiplexer-desktop/src/main.rs` 337-358) RPCs `checkpoint.revert`, then `select_checkpoint` and `term_meta("reverted to {id}")`. No confirmation, no editor refresh, no worktree reset.
- **Why it matters:** The Points tab Revert button, palette "Restore checkpoint", and the terminal line all tell the user the workspace went back. The files on disk are unchanged. That is a lying control, not a missing pane.

---

## F3. Labels are weak

- **Severity:** Major
- **Layer:** Both. Engine stores a free string and nothing else useful. UI always sends `"manual"`.
- **Plan:** 07 §1.3 machine-readable message `multiplexer: checkpoint <thread_id> turn <seq> phase=<pre|post>`. 32 §5.5 title = label, badge = `#seq`, subtitle = id.
- **Evidence:**
  - Bindings hardcode the create label: `{"session_id":"...","label":"manual"}` (`crates/multiplexer-shell/src/bindings.rs` 60-64). Desktop `create_checkpoint` sends the same (`main.rs` 317-320). Fallback local rows also use `"manual"` (`main.rs` 323-326).
  - Seeded boot row is `"start"` (`main.rs` 125-130). `SessionRuntime::start` also hardcodes `"start"` (`crates/multiplexer-core/src/runtime.rs` 81), on a store the desktop catalog does not use.
  - `CheckpointRow` is only `{ id, label }` (`crates/multiplexer-shell/src/workspace.rs` 147-151). No `seq`, though the server already returns `seq` on `CheckpointInfo`.
  - `checkpoint_rows` (`crates/multiplexer-shell/src/inspector_model.rs` 106-122) sets title = label, subtitle = id, no badge, no meta, no file stats, no pre/post.
- **Why it matters:** After two Ctrl+S presses the list is `start`, `manual`, `manual`. The user cannot tell turn, phase, or what changed. Even when the engine later writes real refs, the UI has nowhere to show SHA, phase, or `#seq`.

---

## F4. No diff

- **Severity:** Critical (engine has no range to diff). Major (UI has no pane).
- **Plan:** 07 §3 (`diff.get` / per-turn / full-thread / task). 04 §4.5 `checkpoint.diff` + `checkpoint.apply`, structured hunks. 32/10 right rail as the place those hunks would land.
- **Evidence:**
  - Wire names exist: `CHECKPOINT_DIFF`, `CHECKPOINT_APPLY` (`crates/multiplexer-wire/src/methods.rs` 30-35). Server `dispatch` handles list/create/revert only (`server.rs` 139-141). Anything else is `method not found` (`server.rs` 149-154).
  - No first-party caller of `checkpoint.diff` or `checkpoint.apply`. No structured hunk type in the checkpoint crate. `EventKind::Checkpoint` exists on the wire (`crates/multiplexer-wire/src/event.rs` 32-33) and is never emitted by `RuntimeBackend`.
  - Inspector rows have empty `meta` / no badge, so expand shows nothing beyond the same label + id. There is no per-checkpoint file list, no hunk view, no accept/reject, no comment-on-line.
- **Why it matters:** Diff is the reason checkpoints are git refs. Without refs, a diff RPC would be fiction. The UI should not grow a fake diff until the engine can answer `pre_N..post_N`.

---

## F5. Points rows expand; they do not select

- **Severity:** Major
- **Layer:** UI
- **Plan:** 32 §5.5: click calls `select_checkpoint(Some(id))`. Selected row shows a Revert chip. Tab Revert uses `selected_checkpoint`.
- **Evidence:**
  - `inspector_row_el` (`apps/multiplexer-desktop/src/main.rs` 1881-1886) only `toggle_right_row(id)`. That is the accordion key (`point:cp-1`), not the checkpoint id.
  - `checkpoint_rows` does set `row.selected` from `selected_checkpoint`, but nothing in the click path writes that field. Create selects the new id. Revert re-selects the id it already targeted.
  - `revert_checkpoint` (`main.rs` 338-342) falls back to `checkpoints.last()` when nothing is selected. After boot, the seed `"start"` is not selected, so Revert hits the last row (often the newest manual), not the row the user expanded.
  - No per-row Revert chip. Tab buttons are only New / Revert (`apps/multiplexer-desktop/src/inspector.rs` 56-66).
- **Why it matters:** This is a real UI bug on top of the stub. Even a pointer-only revert needs a selected target. Today expand look-alike selection and last-row fallback fight each other.

---

## F6. Session buckets, two stores, no `checkpoint.list`

- **Severity:** Major
- **Layer:** Both (engine split + UI local cache)
- **Plan:** 04 §4.5 list is `{thread_id}` (implemented as `session_id`). 07 §1.2 refs are per-thread. 06 read model is the UI source of truth.
- **Evidence:**
  - Desktop `Server::with_local` installs an empty `CheckpointStore`, then `ShellView::new` replaces it with a store seeded as session `"local"` / label `"start"` (`main.rs` 125-132). That catalog is independent of `SessionRuntime`'s store.
  - `create_checkpoint` uses `session_id.unwrap_or("local")` (`main.rs` 313-320). After `ensure_session`, later creates go under the real provider session id. The seed stays in `"local"`. The UI `Vec` appends both.
  - Desktop never calls `checkpoint.list`. The rail is an append-only client copy. RPC error path invents `local-N` rows (`main.rs` 322-328) that `checkpoint.revert` cannot find.
  - `RuntimeBackend::start` creates `"start"` in `SessionRuntime.checkpoints` (`runtime.rs` 81), but `checkpoint.*` RPCs do not read that store. Desktop does not use `Server::with_runtime`.
- **Why it matters:** List and create can disagree by session. The inspector can show rows the catalog would not list together. Ghost `local-N` rows make revert fail after a create error.

---

## F7. No confirm, no scope, no dirty-tree warning

- **Severity:** Major
- **Layer:** UI (blocked on engine F2 for a real restore)
- **Plan:** 07 §6.1-6.3. Confirm scope: last turn / to-point / whole-thread. Surface uncommitted local edits that `reset --hard` would destroy. Checkpoints stay; working tree does not.
- **Evidence:** Revert is one click (inspector), one palette item, no dialog. Grep of the desktop app finds no confirm/dialog/scope for checkpoints. No pre-revert `git diff` against the target. No agent notify. No read-model replay (there is no event log to replay).
- **Why it matters:** When F2 is fixed, the current one-shot Revert becomes a data-loss button. The confirm/scope UI should land with the git backend, not after users have already learned that Revert is harmless.

---

## F8. Turns are not checkpointed

- **Severity:** Major
- **Layer:** Engine. UI has no turn timeline to bind to.
- **Plan:** 07 §1.1: every turn is bracketed by pre and post refs. 06 `CheckpointCreated` on `SendTurn`. 19 §1.8 "hidden git refs per turn".
- **Evidence:** `send` (`main.rs` 437-466) starts a session and spawns `grok -p`. It does not call `checkpoint.create` before or after the turn. Server `turn.send` / `RuntimeBackend::send_turn` do not create checkpoints. Manual create is the only capture path (Ctrl+S, /cp, New, palette).
- **Why it matters:** Even if create wrote a git ref, the product model is per-turn, not "user remembered to press Ctrl+S". A Points list of manuals is not a rewind timeline.

---

## What the UI does get right

- Points is a first-class inspector tab with New and Revert (`inspector.rs`).
- Host map is centralized: `CreateCheckpoint` -> `checkpoint.create`, `RestoreCheckpoint` -> `checkpoint.revert` (`bindings.rs`). Desktop should keep using that map.
- Palette, slash `/cp` `/points`, Ctrl+S, and term builtin `points` all reach the same host actions.
- Server RPC validation for list/create/revert is real (empty ids rejected, unknown revert is `not_found`). That is catalog hygiene, not VCS.
- `CheckpointInfo` already has `seq`. The UI just drops it.

None of that substitutes for refs, file restore, or a diff.

---

## Suggested order (not in scope for this audit)

1. Engine: hidden-git capture (or an explicit decision to embed grok-build rewind instead). Until then, label the tab as a stub in the UI so Revert cannot be mistaken for restore.
2. Engine: `checkpoint.revert` resets the worktree; `checkpoint.diff` returns structured hunks; adapter grows `checkpoint_revert`.
3. Engine: pre/post on every turn; one catalog used by session start and RPC.
4. UI: click selects; show `#seq`; stop hardcoding `"manual"`; confirm + scope; render the engine diff. Do not invent a client-side diff.
