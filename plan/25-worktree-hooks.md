# 25: Worktree Hooks & Lifecycle

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Orchestration / Core runtime
**Depends on:** `02-architecture.md`, `03-vendored-grok-build.md`, `06-orchestration-engine.md`, `07-checkpointing-and-vcs.md`, `17-security-and-secrets.md`
**Feeds:** `15-testing-strategy.md`, `19-roadmap-and-milestones.md`, `20-risks-and-open-questions.md`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context). Where a decision is not yet settled, it is listed under **Open questions** and is
**not** decided unilaterally here. New decisions proposed here are numbered **D65+** in the
style of `docs/DECISIONS.md`; they are proposals for the decision log, not locked decisions.

**Locked decisions applied (D1, D7, D11, D13, D23, D25):** This doc reflects the locked decisions
from `docs/DECISIONS.md`:
- **D1** : Rust + GPUI, single native server binary; worktree lifecycle is a component of that
  binary, not a sidecar.
- **D7** : Orca baseline scope, match all; parallel isolated worktrees are a baseline bar this
  doc makes actually usable.
- **D11** : Multiplexer owns subagent scheduling; worktree lifecycle for subagents is owned
  here, not inherited from grok-build for free.
- **D13** : consolidated `multiplexer-*` crate layout; the worktree manager lives in
  `multiplexer-core` (state machine) with git plumbing in `multiplexer-server`.
- **D23** : secrets session-cache model; worktree hooks that need secrets reference the same
  mechanism, never raw values.
- **D25** : remote-agent trust boundary; the remote agent independently enforces worktree
  confinement (see §6).

This doc **extends** `07-checkpointing-and-vcs.md` (§2 Git worktrees). It does not contradict
it: plan/07 defines the parallel-worktree model and the `worktrees(task_id, path, branch,
base_sha, status, created_at)` registry; this doc adds the **lifecycle automation and hooks**
that make that model usable and safe in practice.

---

## 1. Problem statement

Plan/07 gives every task and subagent its own isolated git worktree. That is the right model,
and it is the baseline bar (Orca has it). But a worktree model with no lifecycle automation is
**tedious and unsafe** in daily use:

1. **Creation is manual and error-prone.** The user must remember the exact `git worktree add
   -b <branch> <path> <base>` incantation, pick a collision-free path, and decide the base ref.
   The failure modes are real: a branch already checked out in another worktree (git refuses,
   unless `--force`), a path that collides with an existing worktree, or a base ref that is not
   where the user thinks it is. See https://git-scm.com/docs/git-worktree.
2. **Removal is dangerous.** `git worktree remove` refuses a dirty worktree by default, which
   is good, but the common reactions are bad: `git worktree remove -f` to force it (silently
   deleting work), or manually deleting the directory and leaving stale administrative metadata
   behind (which then needs `git worktree prune`). A client that auto-removes worktrees must
   never do so destructively by default.
3. **Worktrees accumulate.** grok-build persists worktrees until explicit `grok worktree rm` or
   `gc --max-age` (https://docs.x.ai/build/features/worktrees). Claude Code auto-removes only
   *clean* worktrees and persists dirty ones (https://code.claude.com/docs/en/worktrees). A
   client with no cleanup policy leaks worktrees, branches, and disk until the user manually
   sweeps them.
4. **Pre-existing worktrees are invisible.** When a user starts a new session in a repo that
   already has worktrees (from a previous session, a crashed run, or another tool), the client
   silently creates yet another worktree instead of offering to resume or reuse the existing
   one. The user loses track of what is where.
5. **No lifecycle hooks.** Git hooks do **not** fire on worktree create/remove (they fire on
   git operations like commit/checkout, not on `git worktree add/remove`). So there is no
   natural place to run setup (copy `.env`, install deps, start services) or teardown (kill
   processes, drop DBs, archive) around a worktree's life. Claude Code solves this with
   `WorktreeCreate`/`WorktreeRemove` hooks (https://code.claude.com/docs/en/hooks); grok-build
   has no equivalent first-class hook surface.

The result: the parallel-worktree model that should be a differentiator is, without automation,
a source of tedium and data-loss risk. This doc makes worktrees **actually useful** by owning
their lifecycle end to end.

---

## 2. Why hooks + UX matter

The competitive landscape shows that worktree *isolation* is table stakes, but worktree
*lifecycle* is where clients differentiate:

- **grok-build** (our embedded harness): worktrees live under `~/.grok/worktrees/<repo>/<name>`,
  start from HEAD **including uncommitted changes** (dirty state is copied in), are **detached
  at the base commit**, and **persist until explicit `grok worktree rm` or `gc --max-age`**.
  Subagents can request isolation. There is no auto-remove and no create/remove hook surface
  (verified in the vendored `xai-grok-workspace` worktree module: `create_worktree_*`,
  `remove_worktree`, `gc_worktrees_mgmt`, `run_auto_gc_best_effort`). https://docs.x.ai/build/features/worktrees
- **Claude Code**: worktrees under `.claude/worktrees/<name>/`, **locks** a worktree while an
  agent is running (so cleanup cannot remove it), **auto-removes clean** worktrees on exit,
  **persists dirty** ones with a prompt, and exposes **`WorktreeCreate`/`WorktreeRemove` hooks**
  that fully replace or customize the default git logic. https://code.claude.com/docs/en/worktrees, https://code.claude.com/docs/en/hooks
- **Codex**: no first-class `--worktree` flag; the user creates worktrees manually and starts
  Codex inside them (open feature request: https://github.com/openai/codex/issues/12862). This
  is the "tedious" baseline we must beat.

The insight: **isolation is the feature; lifecycle is the product.** A client that (a) creates
the worktree for you, (b) runs setup hooks, (c) safely cleans up clean worktrees, (d) persists
and reminds about dirty ones, and (e) surfaces pre-existing worktrees on session start is
measurably better than one that hands you `git worktree add` and walks away. This maps directly
onto Multiplexer's server-centric runtime: a single native binary already owns git, fs, and
checkpoints (plan/07), so owning worktree lifecycle is the natural extension.

**Why Multiplexer should own hooks, not git hooks:** git hooks do not fire on worktree
create/remove, and they are per-repo shell scripts that are hard to version, hard to secure, and
invisible to the read model. Multiplexer's own lifecycle hooks (`on-session-start/finish`,
`on-worktree-create/remove`) are declared in its own config, run in-process or as sandboxed
commands, and are **events in the read model** (auditable, per plan/17). This is the same
"runtime owns child processes" discipline as plan/21's MCP supervisor, applied to worktrees.

---

## 3. Design goals

1. **Auto-create.** Starting a session in a worktree is one action; the runtime resolves the
   path, branch, and base ref, and creates the worktree. No manual `git worktree add`.
2. **Safe auto-remove on finish.** A worktree that is **clean** (no uncommitted changes, no
   untracked files, no new commits of its own) is removed automatically on session finish. A
   **dirty** worktree is never removed by default: it is persisted and surfaced to the user.
3. **Pre-existing worktree reminder.** On session start, the runtime parses
   `git worktree list --porcelain` and, if the repo already has worktrees, warns and offers to
   resume or reuse one instead of silently creating another.
4. **Lifecycle hooks.** `on-session-start/finish` and `on-worktree-create/remove` hooks in
   Multiplexer's own config, for setup and teardown around worktree life.
5. **No data loss.** Auto-remove refuses dirty worktrees, never uses `-f` by default, and prunes
   stale metadata safely.
6. **Consistency with grok-build.** Default persistence matches grok-build (persist until
   explicit remove or gc); auto-remove is an **opt-in** enhancement on top, not a change to the
   vendored default.

---

## 4. Proposed architecture

The worktree lifecycle manager is a component of the server runtime, alongside the orchestration
engine (plan/06) and the MCP supervisor (plan/21). It owns every worktree create/remove/list/
gc operation and every lifecycle hook. It is event-sourced like the rest of the engine: worktree
lifecycle transitions are events projected into the read model.

### 4.1 Placement in the runtime

```
┌───────────────────────────────────────────────────────────────┐
│                     MULTIPLEXER SERVER                        │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  ORCHESTRATION ENGINE (event-sourced, plan/06)          │  │
│  │  command queue → decider → projector → SQLite read model│  │
│  └───────────────┬─────────────────────────────────────────┘  │
│                  │  worktree commands / events                │
│  ┌───────────────▼─────────────────────────────────────────┐  │
│  │  WORKTREE LIFECYCLE MANAGER                             │  │
│  │  ┌────────────┐ ┌────────────┐ ┌──────────────────────┐ │  │
│  │  │ Porcelain  │ │ Create/    │ │ Hook runner          │ │  │
│  │  │ parser     │ │ Remove/GC  │ │ (on-session/worktree)│ │  │
│  │  └────────────┘ └────────────┘ └──────────────────────┘ │  │
│  └───────────────┬─────────────────────────────────────────┘  │
│                  │  git worktree add/remove/list --porcelain  │
│  ┌───────────────▼─────────────────────────────────────────┐  │
│  │  GIT WORKTREES (under <data_dir>/worktrees/, plan/07)   │  │
│  └─────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

The manager is in-process with the server, so it shares the read model, the secrets session
cache (D23), and the resource monitor. It is the single owner of worktree lifecycle; no other
component creates or removes worktrees.

### 4.2 Worktree registry (extends plan/07)

Plan/07 defines the `worktrees(task_id, path, branch, base_sha, status, created_at)` table. This
doc extends it with lifecycle fields:

| Field | Meaning |
|---|---|
| `task_id` | owning task/session (plan/07) |
| `path` | absolute worktree path |
| `branch` | branch checked out in this worktree |
| `base_sha` | base commit the worktree was created from |
| `status` | `creating` / `active` / `locked` / `dirty` / `clean` / `removing` / `removed` |
| `created_at` | creation time |
| `last_accessed_at` | last use, for gc expiry (mirrors grok-build's `touch_worktree_for_cwd`) |
| `dirty` | whether the worktree holds uncommitted/untracked work (computed at finish) |
| `lock_reason` | why the worktree is locked while an agent runs (see §4.5) |
| `hook_state` | which lifecycle hooks have run for this worktree |

The registry is a projection of the event log, consistent with plan/06. The UI renders the
worktree fleet as a projection, for free.

### 4.3 Lifecycle state machine

```
                ┌──────────────────────────────────────────────┐
                ▼                                              │
   creating ──▶ active ──▶ locked ──▶ (agent runs) ──▶ active  │
      │           │                                            │
      │           └──▶ (session finish)                        │
      │                    │                                   │
      │                    ├──▶ clean ──▶ removing ──▶ removed │
      │                    └──▶ dirty ──▶ (persist, remind)    │
      └──▶ (create failed) ──▶ error                           │
                                                               │
   removed ◀── gc (expire stale / missing dir) ────────────────┘
```

Transitions are events (`WorktreeCreating`, `WorktreeCreated`, `WorktreeLocked`,
`WorktreeUnlocked`, `WorktreeClean`, `WorktreeDirty`, `WorktreeRemoving`, `WorktreeRemoved`,
`WorktreeGc`), projected into the read model, consistent with plan/06's event-sourced model.

### 4.4 Auto-create

On `StartSession` with a worktree request (plan/06), the manager:

1. **Parses the existing fleet** with `git worktree list --porcelain` (see §4.6). If the repo
   already has worktrees, it does **not** silently create another: it surfaces the reminder
   (§4.7) and offers resume/reuse.
2. **Resolves the path** under `<data_dir>/worktrees/<repo_slug>/<label>` (plan/07 §2.2), with
   collision resolution (mirrors grok-build's `resolve_label_collision`).
3. **Resolves the branch.** A branch can be checked out in only one worktree at a time
   (https://git-scm.com/docs/git-worktree). The manager picks a fresh `feature/<task_id>` branch
   (plan/07 §2.3), or, for a resume, reuses the existing worktree's branch. It never uses
   `--force` to steal a branch checked out elsewhere.
4. **Resolves the base ref.** Default is the thread base (plan/07 §2.2); for a subagent, the
   parent task's branch. grok-build starts from HEAD **including uncommitted changes**; this doc
   keeps that as the default (D65), with a `clean` mode that starts from a clean ref.
5. **Runs `git worktree add`** (or the vendored fast-worktree path, see §4.8), then runs the
   `on-worktree-create` hook (§4.9).
6. **Locks the worktree** while the agent runs (§4.5).

### 4.5 Locks while running

While an agent/session is active in a worktree, the manager runs `git worktree lock` so
concurrent cleanup, gc, or another tool cannot remove it. This mirrors Claude Code's behavior
(https://code.claude.com/docs/en/worktrees). The lock is released when the agent finishes. On
crash recovery (plan/06 §8.2), the manager releases locks left by killed processes, but never
unlocks a lock the user set manually.

### 4.6 Porcelain parser

The manager parses `git worktree list --porcelain` (with `-z` for NUL-terminated, newline-safe
parsing) to enumerate the fleet. The porcelain format is stable and machine-readable
(https://git-scm.com/docs/git-worktree):

```
worktree /path/to/linked-worktree
HEAD abcd1234abcd1234abcd1234abcd1234abcd1234
branch refs/heads/master

worktree /path/to/other-linked-worktree
HEAD 1234abc1234abc1234abc1234abc1234abc1234a
detached

worktree /path/to/locked-worktree
HEAD ...
branch refs/heads/some-branch
locked
```

The parser is a pure function over the porcelain text: given the output, it returns a list of
`{path, head_sha, branch|detached, locked, prunable}` records. It is unit- and property-tested
(§7). This is the foundation of both the pre-existing reminder (§4.7) and the gc sweep (§4.10).

### 4.7 Pre-existing worktree reminder

On session start, before creating a worktree, the manager parses the fleet. If the repo already
has worktrees (from a previous session, a crashed run, or another tool), it:

1. **Warns** the user that worktrees already exist for this repo.
2. **Offers to resume or reuse** one of them (matching by branch, label, or last access) instead
   of creating a new one.
3. **Only creates a new worktree** if the user confirms, or if none of the existing ones is
   reusable.

This prevents the "silent worktree pile-up" failure mode and makes pre-existing work visible.
The reminder is a read-model projection, so it also shows in the UI fleet view.

### 4.8 Auto-remove on finish (safe)

On session finish (plan/06 `SessionStop`), the manager inspects the worktree for "work that
removal would delete": uncommitted changes, untracked files, and new commits of its own. This
mirrors Claude Code's clean/dirty inspection (https://code.claude.com/docs/en/worktrees).

- **Clean** (no uncommitted/untracked changes and no new commits of its own): the worktree is a
  candidate for auto-remove. For an **unnamed/auto-generated** worktree, remove automatically.
  For a **named** worktree, prompt first (so the user can keep it).
- **Dirty** (has changes or new commits): **never auto-remove**. Persist the worktree and branch,
  and surface it to the user with a prompt: keep (persist for later resume) or remove (deletes
  everything, including the work, with explicit confirmation).

**Auto-remove safety rules (non-negotiable):**

1. **Refuse dirty.** Never remove a worktree with uncommitted/untracked changes or unpushed
   commits, unless the user explicitly confirms a destructive remove.
2. **Never `-f` by default.** `git worktree remove -f` silently deletes work. The manager only
   uses `--force` after explicit user confirmation, and logs it as an auditable event.
3. **Stash or prompt, not force.** If a worktree is dirty and the user wants it gone, offer to
   stash the changes (or commit them to a branch) before removal, rather than deleting them.
4. **Prune stale metadata.** If a worktree directory is missing (e.g. manually deleted), the
   manager runs `git worktree prune` to clean up administrative metadata, rather than leaving
   stale entries. It never recreates a missing directory.

### 4.9 Lifecycle hooks (Multiplexer's own, not git hooks)

Git hooks do not fire on worktree create/remove. Multiplexer owns lifecycle hooks in its own
config, run in-process or as sandboxed commands, and recorded as events in the read model:

| Hook | Fires | Purpose |
|---|---|---|
| `on-session-start` | session begins | pre-worktree setup, env, reminder decisions |
| `on-session-finish` | session ends | teardown, clean/dirty decision, auto-remove |
| `on-worktree-create` | after `git worktree add` | copy `.env`, install deps, start services, unique ports/DBs |
| `on-worktree-remove` | before/after removal | kill processes, drop DBs, archive, cleanup |

Config shape (in Multiplexer's config, consistent with grok-build's hook config in
`xai-grok-hooks`):

```toml
[worktree.hooks]
on_worktree_create = "bash .multiplexer/hooks/worktree-create.sh"
on_worktree_remove = "bash .multiplexer/hooks/worktree-remove.sh"
on_session_start   = "bash .multiplexer/hooks/session-start.sh"
on_session_finish  = "bash .multiplexer/hooks/session-finish.sh"
```

Hooks receive structured input (worktree path, branch, base sha, session id) on stdin and can
return a path or a decision, mirroring Claude Code's `WorktreeCreate`/`WorktreeRemove` hook
contract (https://code.claude.com/docs/en/hooks). Hooks that need secrets reference the session
cache (D23), never raw values. Hooks are sandboxed and gated by permission modes (plan/17), the
same as any tool call.

### 4.10 Garbage collection

The manager provides `gc` that:

1. **Prunes missing directories** (removes tracking entries whose directories no longer exist),
   mirroring `grok worktree gc`.
2. **Expires idle worktrees** by `last_accessed_at` beyond a configurable `--max-age`, mirroring
   `grok worktree gc --max-age` (https://docs.x.ai/build/features/worktrees).
3. **Never gc's a locked worktree** (an agent is running) or a dirty one (it holds work).

Auto-gc is best-effort and off the hot path, mirroring grok-build's `run_auto_gc_best_effort`
(verified in the vendored source). It is dry-run by default until the user opts in.

### 4.11 Interaction with the vendored grok-build worktree code

The vendored `xai-grok-workspace` already implements create/remove/gc with a fast CoW copy path
(`xai-fast-worktree`), dirty/clean copy modes, and a worktree DB with `last_accessed_at` and
auto-gc. This doc **reuses** that machinery (D65) rather than reimplementing `git worktree`
from scratch. Multiplexer adds the **lifecycle automation and hooks** on top: the pre-existing
reminder, the clean/dirty auto-remove policy, the lock discipline, and the `on-worktree-*` hook
runner. Where grok-build's default is "persist until explicit remove or gc", Multiplexer keeps
that default and layers opt-in auto-remove on top (D66).

---

## 5. Key design decisions (proposed D65+)

These are proposals for `docs/DECISIONS.md`, in its style. They are **not** locked; they are
offered for the decision log.

### D65. Reuse vendored worktree machinery, own the lifecycle (PROPOSED)
- **Decision:** Reuse grok-build's vendored worktree create/remove/gc and fast-worktree copy
  path (`xai-grok-workspace` + `xai-fast-worktree`), and add Multiplexer's own lifecycle
  automation and hooks on top. Default persistence matches grok-build (persist until explicit
  remove or gc).
- **Rationale:** Reusing the vendored machinery avoids reimplementing git worktree semantics and
  keeps the fork syncable (D5, D31). The lifecycle automation is where the product value is, and
  it is Multiplexer-owned (D11), not inherited.

### D66. Auto-remove only if clean, opt-in (PROPOSED)
- **Decision:** Auto-remove on session finish applies **only to clean worktrees** (no
  uncommitted/untracked changes, no new commits of its own). Dirty worktrees persist and are
  surfaced to the user. Auto-remove is opt-in, layered on top of grok-build's persist-by-default.
- **Rationale:** Matches Claude Code's safe behavior (https://code.claude.com/docs/en/worktrees)
  and never risks data loss. Persist-by-default is the grok-build baseline; auto-remove is a
  convenience the user opts into.

### D67. Never force-remove by default (PROPOSED)
- **Decision:** The manager never runs `git worktree remove -f` unless the user explicitly
  confirms a destructive remove, which is logged as an auditable event. Dirty worktrees are
  stashed or prompted, not force-deleted.
- **Rationale:** `-f` silently deletes work. Safety is non-negotiable (plan/17); force is an
  explicit, audited user decision.

### D68. Pre-existing worktree reminder on session start (PROPOSED)
- **Decision:** On session start, parse `git worktree list --porcelain`; if the repo already has
  worktrees, warn and offer resume/reuse before creating a new one.
- **Rationale:** Prevents silent worktree pile-up and makes pre-existing work visible. The
  porcelain parser is the foundation.

### D69. Lock while running, release on finish and on crash recovery (PROPOSED)
- **Decision:** Lock a worktree while an agent runs (`git worktree lock`); release on finish.
  On crash recovery, release locks left by killed processes, but never unlock a user-set lock.
- **Rationale:** Mirrors Claude Code (https://code.claude.com/docs/en/worktrees) and prevents
  concurrent cleanup from removing a running worktree.

### D70. Multiplexer-owned lifecycle hooks, not git hooks (PROPOSED)
- **Decision:** Lifecycle hooks (`on-session-start/finish`, `on-worktree-create/remove`) are
  declared in Multiplexer's own config, run in-process or as sandboxed commands, and are events
  in the read model. Git hooks are not used for worktree lifecycle.
- **Rationale:** Git hooks do not fire on worktree create/remove, are per-repo shell scripts that
  are hard to secure and version, and are invisible to the read model. Multiplexer-owned hooks
  are auditable (plan/17) and consistent with the server-centric runtime.

### D71. Porcelain parser as a pure, tested component (PROPOSED)
- **Decision:** The `git worktree list --porcelain` parser is a pure function, unit- and
  property-tested, and is the single source of truth for the fleet view, the reminder, and gc.
- **Rationale:** The parser is the foundation of the reminder and gc; a wrong parse means wrong
  cleanup decisions. Pure + tested is the plan/06 discipline.

---

## 6. Safety

Worktree lifecycle concentrates destructive operations (remove, gc, force), so it is
security-sensitive. It follows plan/17's principles: least privilege, fail closed, auditability.

1. **No data loss by default.** Auto-remove refuses dirty worktrees; `-f` is never used without
   explicit, audited user confirmation; dirty work is stashed or prompted, not deleted.
2. **Never remove a running worktree.** Locks (D69) prevent cleanup of an active worktree; gc
   skips locked and dirty worktrees.
3. **Branch discipline.** A branch is checked out in only one worktree (git restriction,
   https://git-scm.com/docs/git-worktree). The manager never uses `--force` to steal a branch
   checked out elsewhere.
4. **Hook sandboxing.** Lifecycle hooks run as sandboxed commands gated by permission modes
   (plan/17), the same as any tool call. Hooks that need secrets reference the session cache
   (D23), never raw values.
5. **Remote trust boundary (D25).** The remote agent independently enforces worktree confinement
   on the remote host; it is not a dumb executor that trusts the local core. Worktree lifecycle
   on a remote host follows the same rules locally.
6. **Auditability.** Every create, lock, unlock, clean/dirty decision, remove, force, and gc is
   an event in the read model, replayable for review (plan/17).
7. **Prune, don't recreate.** Missing worktree directories are pruned (metadata cleanup), never
   silently recreated.

---

## 7. Testing strategy

The worktree lifecycle manager is tested under the project's TDD-at-inception gate chain (fmt →
clippy → unit+property → mutation → integration → component → e2e → coverage), per plan/15.

### 7.1 Unit tests (porcelain parser + state machine)

Co-located `#[cfg(test)]` modules.

- **Porcelain parser:** given representative `git worktree list --porcelain` output (main,
  linked, detached, locked, prunable, `-z` NUL-terminated, paths with spaces/newlines), assert
  the parsed `{path, head_sha, branch|detached, locked, prunable}` records are correct.
- **Lifecycle state machine:** happy path `creating → active → locked → active → clean →
  removing → removed`; dirty path `active → dirty → (persist)`; invalid transitions rejected
  with a typed error.
- **Clean/dirty detection:** given a worktree with uncommitted changes, untracked files, or new
  commits, assert it is classified dirty; given a pristine worktree, assert clean.
- **Branch resolution:** assert a fresh `feature/<task_id>` branch is chosen, and that a branch
  already checked out elsewhere is never reused with `--force`.

### 7.2 Property tests (proptest)

- **Dirty-refuse invariant:** under arbitrary sequences of file mutations (create/edit/delete/
  rename, staged/untracked), a worktree classified dirty is never auto-removed; a worktree
  classified clean has no uncommitted/untracked changes and no new commits of its own.
- **Porcelain round-trip:** arbitrary worktree fleets serialize to porcelain and parse back to
  the same records (invariant: parse ∘ emit is identity).
- **Lifecycle invariants:** no worktree is ever removed while locked; no worktree is ever
  removed while dirty without an explicit destructive-confirm event; the registry never
  disagrees with the event log.

### 7.3 Integration tests (real temp git repo)

- Spin up a **real temp git repo** (via `git2`/`gix` or the `git` CLI) and drive the full
  lifecycle: create worktree → run a fake agent that mutates files → classify clean/dirty →
  auto-remove clean / persist dirty → assert read model + filesystem consistency.
- **Pre-existing reminder:** create a worktree externally, start a session, assert the reminder
  fires and offers resume/reuse instead of silently creating another.
- **Lock discipline:** lock a worktree, assert gc and auto-remove skip it; release, assert it is
  eligible.
- **Crash recovery:** simulate a crash with a locked worktree, restart, assert the lock is
  released and the worktree is not removed.
- **Hooks:** register `on-worktree-create`/`on-worktree-remove` hooks, assert they fire with the
  correct structured input and their side effects run.

### 7.4 Mutation testing

cargo-mutants over the porcelain parser, the clean/dirty classifier, and the lifecycle state
machine. CI gates: ≥85% line, ≥80% branch, ≥70% mutation score killed (D21, D33). The
dirty-refuse logic and the parser are prime mutation targets.

### 7.5 Component & e2e

- **Component (GPUI):** worktree fleet view, pre-existing reminder dialog, clean/dirty prompt,
  destructive-remove confirmation.
- **E2E:** drive the real app headless; create a session in a worktree, run an agent, finish,
  assert a clean worktree is auto-removed and a dirty one persists and is surfaced. This is the
  direct regression test for the original problem.

---

## 8. Open questions / risks

These are flagged, not decided here:

1. **Auto-remove default.** Whether auto-remove of clean worktrees is on by default or opt-in
   (D66 proposes opt-in) is a product decision; the default affects how much the user must
   configure.
2. **Named vs unnamed worktrees.** Whether a user-named worktree always prompts on removal (like
   Claude Code) or is treated like an auto-generated one is a UX decision.
3. **Dirty-worktree disposition.** When a dirty worktree persists, whether to auto-stash, auto-
   commit to a branch, or only prompt is a product decision; auto-commit could surprise users.
4. **gc defaults.** The `--max-age` default and whether auto-gc is on by default (grok-build
   ships it best-effort) need tuning with real data (plan/16).
5. **Hook security.** Lifecycle hooks run arbitrary commands; the exact sandbox and permission
   gating (plan/17) needs a decision, especially for hooks that need secrets or network.
6. **Interaction with grok-build's own worktree DB.** grok-build keeps a separate worktree DB
   (`WorktreeDb`) with `last_accessed_at` and auto-gc. How Multiplexer's registry reconciles with
   that DB (share it, or keep its own projection) needs a decision as upstream evolves (track via
   D31).
7. **Fast-worktree copy vs plain `git worktree add`.** grok-build's fast CoW copy path
   (`xai-fast-worktree`) is Linux/btrfs-oriented; on Windows-first (D9) the plain `git worktree
   add` path may be the reliable default. Platform-specific behavior needs verification.

**Flagged consistency note:** this doc is consistent with PLAN-CONTEXT (server-centric runtime,
event-sourced orchestration, parallel worktrees as baseline bar, secrets session-cache model) and
extends plan/07 without contradicting it. If any locked decision flips (e.g. stack, crate
layout), the affected sections (§4, §5) must be revisited.
