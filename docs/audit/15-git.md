# 15: Git / Worktree UI

**Scope:** Desktop Git tab, worktree list/create/remove, git status, pre-existing reminder.
**Sources:** `apps/multiplexer-desktop/src/{inspector.rs,main.rs}`, `crates/multiplexer-shell/src/{inspector_model.rs,actions.rs,bindings.rs,workspace.rs}`, `crates/multiplexer-server/src/{git.rs,worktree_create.rs,server.rs}`, `crates/multiplexer-worktree/src/{git.rs,reminder.rs}`, `crates/multiplexer-wire/src/methods.rs`, `plan/25-worktree-hooks.md`, `plan/36-feature-gap-ui.md` row E / §4.3.
**Date:** 2026-08-12
**Method:** Read-only. No cargo.

The Git inspector lists worktree **paths** and offers Reload / Status / New WT. The server already implements `git.worktrees` and `git.worktree.create`. The worktree crate already implements list, add, remove (dirty-safe), and `reminder_from_list`. The desktop does not use create, remove, or the reminder parser. Create is a composer paste. Status is a one-shot `git status` dump. The reminder is a dismiss-only bar over the second listed path.

---

## Findings

### GIT-01: Create is a composer hint only

**Severity:** High
**Where:** `apps/multiplexer-desktop/src/inspector.rs` (`NewWorktreeHint`), `apps/multiplexer-desktop/src/main.rs` (`inspector_click`), `crates/multiplexer-shell/src/bindings.rs` (`host_call`)
**Plan:** `plan/36` row E and §4.3. Server already implements `git.worktree.create`.

**Evidence:**

- Git tab button **New WT** is labeled "Hint a worktree path" and fires `InspectorAction::NewWorktreeHint`.
- Handler only does:

```
set_draft("git worktree add ../mux-feat -b feat")
focus = Composer
term_meta("edit the worktree command, then Enter to send or run it in Term")
```

- Composer Enter sends the line as a **chat turn** (`grok -p`), not as git. The user must copy the hint into the Term strip themselves.
- `ClientAction` has no `CreateWorktree`. `host_call` maps `RefreshGit` to `git.worktrees` and the unit test asserts that call is **not** `git.worktree.create`.
- Desktop `host_action` never mentions create. `Server::dispatch` already routes `GIT_WORKTREE_CREATE` to `worktree_create::create`.

**Gap:** plan/36: "The Git tab's **New WT** button pastes a shell string. That is a hint, not a UI." Still true. Create never hits the live RPC.

---

### GIT-02: No create form

**Severity:** High
**Where:** `crates/multiplexer-shell/src/workspace.rs`, `crates/multiplexer-shell/src/inspector_model.rs` (`git_rows`), `apps/multiplexer-desktop/src/inspector.rs` (`tab_buttons`)
**Plan:** `plan/36` §4.3 user-visible contract.

**Evidence:**

- No `WorktreeDraft { path, branch, create_branch }`. No path field, no branch field, no create-branch toggle.
- Git toolbar is Reload / Status / New WT. There is no **Create** control. `controls.rs` has `refresh_git` and `run_git_status` only.
- `git_rows` renders `ws.worktrees: Vec<String>` as path titles. No draft row.
- Server `parse_create` requires nonempty `cwd`, `path`, `branch`, plus optional `create_branch` (default false). Nothing in the shell can supply those params.
- plan/36 names test `worktree_create_draft_dispatches_rpc`. That test does not exist.

**Gap:** Prefill was supposed to be path `../mux-<branch>` and branch `feat`, with Create as the primary (control id `create_worktree`). New WT was allowed only as a secondary copy-command. Primary is still the hint.

---

### GIT-03: Remove is unused

**Severity:** High
**Where:** `crates/multiplexer-worktree/src/git.rs` (`WorktreeService::remove`), `crates/multiplexer-server/src/git.rs` (`GitCatalog`), `crates/multiplexer-wire/src/methods.rs`, Git tab UI
**Plan:** `plan/25` §1 / §4.8 / D66 and D67. `plan/07` §2.2 remove after merge or discard.

**Evidence:**

- `WorktreeService::remove` is implemented and tested: dirty without `force` returns `WorktreeError::Dirty` and does not call `git worktree remove`; `force` uses `-f`.
- `GitCatalog` only has `list_worktrees` and `create_worktree`. No remove method.
- Wire constants stop at `git.worktrees` and `git.worktree.create`. There is no `git.worktree.remove`.
- Git tab has no Remove / Discard / Prune button. Row click only `toggle_right_row` (accordion). `selected_worktree` is never set from the UI, so there is no selected target even if a button existed.
- No `ClientAction` for remove. Palette has Refresh git and Dismiss reminder, not remove.

**Gap:** Lifecycle is list-only. Dirty-safe remove lives in the crate and is unreachable. plan/25's "never `-f` by default" policy cannot be offered because remove is not on the wire or the rail.

---

### GIT-04: Status is one-shot

**Severity:** Medium
**Where:** `apps/multiplexer-desktop/src/main.rs` (`RunGitStatus`, `run_shell`, pending_cmd), `crates/multiplexer-shell/src/inspector_model.rs` (`git:status` row)
**Plan:** `plan/04` §4.8 `git.status` / `git.diff`. `plan/36` notes those methods still return method not found.

**Evidence:**

- **Status** and the composer chip `git status` both call `run_shell("git status")`: spawn `cmd.exe` in the project cwd, dump up to 40 lines into the Term strip.
- When the command finishes, if the body contains `"git"` or the inspector is on Git, `set_git_status` stores the first **800 characters** of that blob. Any other command whose stdout happens to contain "git" while the Git tab is open overwrites the same field.
- `git_rows` then adds a single `git:status` row whose subtitle is **the first line** of that string. No porcelain parse, no file list, no dirty badge per worktree, no live refresh.
- `methods::GIT_STATUS` and `GIT_DIFF` exist. `Server::dispatch` does not implement them (fall through to `method not found`). Desktop never calls them.
- Status is not refreshed on turn complete (only `refresh_worktrees` is). There is no watch.

**Gap:** Status is a truncated shell transcript, not `git.status`. It is not scoped to `selected_worktree`. It is not structured.

---

### GIT-05: Reminder is crude

**Severity:** High
**Where:** `apps/multiplexer-desktop/src/main.rs` (`refresh_reminder`, `reminder_bar`, title-bar git pill), `crates/multiplexer-worktree/src/reminder.rs`
**Plan:** `plan/25` §3.3, §4.7, D68. Reminder must warn and offer resume/reuse before creating another worktree.

**Evidence:**

- `refresh_reminder` runs once at window init. It lists worktrees, then:

```
if let Some(path) = paths.into_iter().nth(1) {
    self.workspace.set_reminder("existing", path);
}
```

- Branch is the literal `"existing"`. Path is **whatever is second** in the list (usually the first linked worktree, or a sibling checkout), not a match on the current branch.
- `reminder_from_list` (and `WorktreeService::reminder`) already implement branch matching, main-path skip, and "no reminder when fleet length is 1". The desktop does not call them.
- Reminder is not recomputed after Reload, after a turn, or after a (nonexistent) create. Dismiss is the only action (`ClientAction::DismissReminder`). No Resume, no Reuse, no Create-anyway.
- Bar copy: `Existing worktree on {branch}: {path}` so the user sees **Existing worktree on existing: &lt;path&gt;**.
- Title-bar git pill shows `reminder.0` or else `"main"`. When the crude reminder is set, the pill reads `existing`.

**Gap:** plan/25: parse porcelain, warn if the repo already has worktrees, offer resume/reuse, only create after confirm. Today: if there are two paths, show a dismissible bar labeled "existing".

---

### GIT-06: Inspector rows drop porcelain

**Severity:** Medium
**Where:** `apps/multiplexer-desktop/src/main.rs` (`worktree_paths`, `refresh_worktrees`), `crates/multiplexer-shell/src/inspector_model.rs` (`git_rows`), `crates/multiplexer-server/src/git.rs` (`WorktreeInfo`)
**Plan:** `plan/32` §5.6 (title = short path, badge = branch). `plan/25` §4.6 porcelain as source of truth.

**Evidence:**

- `git.worktrees` returns `WorktreeInfo { path, head, branch, detached, locked, prunable }`.
- `worktree_paths` keeps only `row.path`. `Workspace.worktrees` is `Vec<String>`.
- `git_rows` sets title = full path, icon = Git glyph, `selected = ws.selected_worktree == Some(i)`. No subtitle, no branch badge, no locked/prunable/detached mark.
- Row click does not call a `select_worktree`. It only toggles `right_expanded_id`. `selected_worktree` stays `None` unless a test writes it.
- Integrations tiles also project paths only (`git:{i}`, subtitle `"worktree"`).

**Gap:** The fleet view cannot show which tree is dirty, locked, or on which branch. Selection is a dead field.

---

## What already works (not findings)

- `git.worktrees` dispatch + `WorktreeService::list` porcelain parse.
- `git.worktree.create` parse/reply (`cwd`, `path`, `branch`, `create_branch`) and catalog `add`.
- `WorktreeService::remove` dirty-refuse + optional `-f` (headless only).
- `reminder_from_list` unit tests (unused by the desktop).
- Reload refreshes the path list. `/git` and the Term builtin `git` only switch the inspector tab.

---

## Plan delta (25 + 36 E)

| Contract | Now |
|---|---|
| Three-field draft + Create → `git.worktree.create` | Composer string `git worktree add ../mux-feat -b feat` |
| Refresh list and select new path on success | Never called |
| Safe remove, never `-f` by default, persist dirty | `remove` exists, no UI / no RPC |
| Reminder: porcelain, resume/reuse, no silent pile-up | Second path, branch `"existing"`, Dismiss only |
| Status as structured `git.status` | One-shot `cmd git status`, 800-char blob, first line as subtitle |

---

FINDINGS: 6
