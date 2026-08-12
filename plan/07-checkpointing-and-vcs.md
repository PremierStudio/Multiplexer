# 07 — Checkpointing & VCS

**Status:** Draft (planning phase)
**Scope:** Checkpoint capture/revert, parallel git worktrees, diff model, source-control integrations, inline diff comments → agent, conflict handling, testing.
**Consistency:** This doc follows `docs/PLAN-CONTEXT.md` exactly. Where a decision is still open upstream, it is listed in [Open questions](#9-open-questions) and not decided unilaterally.

---

## 1. Checkpointing

### 1.1 Model

Every agent **turn** is bracketed by two hidden Git refs. The runtime captures the workspace state *before* a turn begins (the pre-turn checkpoint) and *after* the turn's tool activity settles (the post-turn checkpoint). This gives us:

- **Per-turn diff** = `post_turn_N..pre_turn_{N+1}` (the delta produced by turn N).
- **Full-thread diff** = `pre_turn_0..post_turn_M` (everything the thread has changed since it started).
- **Revert** = reset the working tree to any captured ref.

Checkpoints are **cheap** because they are Git refs, not snapshots: capturing is `git commit` (tree objects are content-addressed and deduplicated), so a turn that touches 3 files costs ~3 new blobs. We never copy the working tree.

### 1.2 Ref namespace

All checkpoint refs live under a reserved, hidden namespace so they never collide with user branches and are invisible to normal `git log`/`git branch`:

```
refs/multiplexer/threads/<thread_id>/checkpoints/pre/<seq>
refs/multiplexer/threads/<thread_id>/checkpoints/post/<seq>
refs/multiplexer/threads/<thread_id>/head            # current tip
refs/multiplexer/threads/<thread_id>/base            # thread start (pre_turn_0)
```

- `<thread_id>` is the orchestration thread UUID (see `plan/06-orchestration-engine.md`).
- `<seq>` is a monotonically increasing per-thread turn counter.
- Refs are created with `git update-ref` (not `git branch`) so they are lightweight and never appear in user-facing branch listings.

### 1.3 Capture flow

The orchestration engine (event-sourced, see `plan/06`) drives checkpointing through the **Provider Adapter** contract — `checkpoint_revert` is already a first-class adapter method, and capture is symmetric:

```
turn N begins
  ├─ pre_turn_N  = commit current index+worktree  → refs/.../pre/N
  ├─ agent runs (tools mutate the worktree)
  ├─ post_turn_N = commit current index+worktree  → refs/.../post/N
  └─ refs/.../head = post_turn_N
```

**Capture algorithm (per checkpoint):**

1. `git add -A` (stage everything, including deletions and untracked files).
2. `git commit-tree` against the current `HEAD` with a machine-readable message:
   ```
   multiplexer: checkpoint <thread_id> turn <seq> phase=<pre|post>
   ```
3. `git update-ref refs/multiplexer/... <new-tree-commit>`.
4. Record the commit SHA in the SQLite read model (see §3) alongside the turn event.

**Empty-checkpoint optimization:** if `git diff --cached --quiet` reports no changes, we skip the commit and point the ref at the previous ref's target (no-op). This keeps the ref graph sparse for read-only turns.

**Untracked files:** `git add -A` includes untracked files, so checkpoints capture new files created by the agent (e.g. a scaffolded project). This is intentional and required for correct revert.

### 1.4 Revert

Revert is a two-step, crash-safe operation:

1. **Ref-level:** `git reset --hard <target-ref>` (or `git checkout <target-ref> -- .` for a non-destructive variant).
2. **Read-model-level:** the orchestration projector replays/truncates the read model to the state at that checkpoint so the UI, diff pane, and agent activity reflect the reverted reality.

Because checkpoints are immutable commits, revert is always possible regardless of what the agent did — including deleting files, creating files, or rewriting history inside the worktree. See [§6 Checkpoint revert flow](#6-checkpoint-revert-flow) for the full user-facing flow.

### 1.5 Garbage collection

Checkpoint refs are retained for the lifetime of the thread (they are the thread's history). When a thread is archived/deleted, its refs are removed with `git update-ref -d` and the commits become unreachable and are reclaimed by normal `git gc`. We never run `git gc` ourselves on the hot path; we rely on Git's automatic maintenance.

---

## 2. Git worktrees

### 2.1 The parallel-worktree model

Each **task** (and each **subagent** within a task) runs in its **own isolated Git worktree** on its **own branch**. This is the baseline bar (Orca has it) and it is the foundation of our conflict-free parallelism.

```
main repo (bare-ish, owns refs + objects)
  └─ worktree A  branch feature/agent-a   (task A)
  └─ worktree B  branch feature/agent-b   (task B)
  └─ worktree C  branch feature/agent-c   (subagent of A)
```

Key properties:

- **No stashing, no branch juggling.** Each worktree has a fixed branch; switching context is a UI action, not a `git checkout` that mutates a shared working directory.
- **Isolation.** A subagent that deletes a file, rewrites history, or leaves the tree dirty cannot affect a sibling worktree.
- **Concurrency.** Dozens of subagents (our performance target) each get a worktree; the OS/Git handles the filesystem fan-out, and the orchestration scheduler (see `plan/06`) dispatches work across them.

### 2.2 Worktree lifecycle

```
create:  git worktree add <path> -b <branch> <base>
  ├─ path:  <data_dir>/worktrees/<task_id>/
  ├─ branch: feature/<task_id> (or user-provided name)
  └─ base:  thread base, or a parent task's branch for subagents
run:     agent operates inside the worktree path (its cwd)
checkpoint: refs under refs/multiplexer/threads/<thread_id>/...
remove:  git worktree remove <path>  (after merge or discard)
```

**Worktree registry:** the runtime keeps a SQLite table `worktrees(task_id, path, branch, base_sha, status, created_at)` so the UI can render the fleet and the scheduler can allocate work.

### 2.3 Branch topology

- **Thread base branch:** the branch the user started the thread from (e.g. `main`).
- **Task branches:** `feature/<task_id>` forked from the base (or from a parent task branch for subagents → a **stack**).
- **Stacked branches** are the natural representation of subagent fan-out: `main → task A → subagent A1 → subagent A2`. This maps directly onto stacked-PR tooling (§4.5).

### 2.4 SSH remote worktrees

Per the baseline bar, worktrees can live on a **remote host** (SSH). The runtime treats a remote worktree as a transport-abstracted filesystem + git endpoint: the same checkpoint/diff/revert code paths run against it via the remote/relay layer (`plan/14-remote-and-relay.md`). This is a stretch item for MVP and is tracked in [Open questions](#9-open-questions).

---

## 3. Diff model

### 3.1 Computation

Diffs are computed **on demand** from the checkpoint refs — we never store full diff text for every turn (that would be redundant with Git's object store). The runtime computes:

| Query | Ref range | Purpose |
|-------|-----------|---------|
| Per-turn diff | `post_turn_{N-1}..post_turn_N` (equivalently `pre_turn_N..post_turn_N`) | Inline diff-apply, diff comments, "what did this turn change" |
| Full-thread diff | `base..head` | "everything this thread changed", PR body, review |
| Task diff | `task_base..task_head` | per-worktree review |

Computation uses `git diff` with a **unified, zero-context** format plus a machine-readable **hunk/line index** so the UI can map diff lines back to file/line coordinates for inline comments and apply.

### 3.2 Storage

- **Authoritative:** Git object store (the refs themselves).
- **Read model:** the SQLite read model stores, per turn, the list of changed files + per-file stat (additions/deletions) + the diff **metadata** (hunk headers, line maps). Full diff *text* is fetched from Git on demand and cached in memory with an LRU.
- **Why not store full text:** diffs can be large (a turn that reformats a whole file); Git already has the content; storing text duplicates it and bloats the DB.

### 3.3 Serving to the UI

The diff pane (right bar, see `plan/10-ui-pane-system.md`) requests diffs over the JSON-RPC/WebSocket contract (`plan/04-wire-contract.md`):

```
rpc: diff.get { thread_id, range: {type: turn|thread|task, ...} }
resp: { files: [{ path, status, hunks: [{ header, lines: [{type, old_line, new_line, text}] }] }] }
```

The response is a **structured** diff (not raw text) so the GPUI editor can:
- render inline diff-apply (accept/reject individual hunks or lines),
- place inline comments on specific `new_line` coordinates,
- jump to the file/line in the editor via LSP coordinates.

### 3.4 Diff-apply

Applying a diff back into the working tree is done with `git apply` (with `--3way` fallback to `git apply --3way` when context has drifted). Because the diff is derived from a real Git commit, `git apply --3way` can use the blob SHAs for reliable three-way merge. The UI's "accept hunk" maps to applying the corresponding patch fragment.

---

## 4. Source-control integrations

Reference model: **T3 Code's source-control design** (server-centric, provider-abstracted). We follow the same shape but implement natively in Rust (no Electron/Effect). Each integration is a **`ScmProvider` trait** behind the runtime, so GitHub/GitLab/Bitbucket/Azure DevOps are pluggable.

```rust
trait ScmProvider {
    fn clone_repo(&self, url: &str, dest: &Path) -> Result<()>;
    fn publish_branch(&self, branch: &str) -> Result<PublishInfo>;
    fn create_pr(&self, req: CreatePrRequest) -> Result<PrRef>;
    fn review_threads(&self, pr: &PrRef) -> Result<Vec<ReviewThread>>;
    fn post_review_comment(&self, pr: &PrRef, thread: &CommentTarget, body: &str) -> Result<()>;
    fn stacked_actions(&self, stack: &[BranchRef]) -> Result<Vec<StackedAction>>;
}
```

### 4.1 Clone / publish

- **Clone:** `git clone` via the provider's authenticated transport (HTTPS with a stored token from the OS keychain, or SSH). The clone becomes the thread base.
- **Publish:** push the task branch to the remote and record the remote ref. Publishing is explicit (user action), never implicit — the agent's local worktrees stay private until the user chooses to publish.

### 4.2 Create PRs / MRs

- **GitHub:** Pull Requests. **GitLab:** Merge Requests. **Bitbucket:** Pull Requests. **Azure DevOps:** Pull Requests.
- The runtime builds the PR from the **task diff** (§3.1) — title from the task summary, body from the full-thread diff summary + agent's final message.
- Draft vs ready, base branch, reviewers, and labels are all configurable per integration.

### 4.3 Review threads

Review threads (comments on PR/MR lines) are fetched and **projected into the read model** so they appear in the diff pane alongside local inline comments. This unifies local and remote review in one surface.

### 4.4 Worktrees

Remote worktrees (GitHub Codespaces-style, or plain SSH) reuse the same `ScmProvider` transport for clone/push. See §2.4.

### 4.5 Stacked actions

Because subagent fan-out produces **stacked branches** (§2.3), the runtime supports stacked-PR workflows: create/update a stack of PRs where each PR targets its parent branch, and reorder/rebuild the stack when the base changes. This is a differentiator over single-branch tools and matches how our orchestration actually works.

### 4.6 Auth

All provider tokens live in the **OS keychain** (per `docs/PLAN-CONTEXT.md` — never in plaintext/config). Configs reference secrets only via `op://Vault/Item/field` placeholders. See `plan/17-security-and-secrets.md`.

---

## 5. Inline diff comments → agent

This is a **baseline bar** (Orca has it): a user drops a comment on a diff line and it is sent back to the agent.

### 5.1 Flow

```
1. User selects a line in the diff pane → "Comment"
2. UI sends rpc: diff.comment { thread_id, file, new_line, body }
3. Runtime persists the comment in the read model (threaded, replyable)
4. Runtime converts it to a user-input event on the agent's turn
   (Provider Adapter: user_input_respond)
5. Agent receives the comment as context and acts on it
6. Agent's next turn produces a new checkpoint; the comment thread is
   linked to the turn that addressed it
```

### 5.2 Comment → agent payload

The comment is delivered to the agent as a structured user message with the exact file/line context so the agent can locate the code without ambiguity:

```
user_input_respond {
  kind: "diff_comment",
  file: "src/lib.rs",
  line: 42,
  body: "This borrow could be avoided — see the lifetime here.",
  thread_id: "...",
}
```

### 5.3 Threading & persistence

Comments form reply threads stored in the read model (`comments(id, thread_id, file, line, author, body, parent_id, turn_seq, created_at)`). They survive thread revert (they are metadata, not workspace state) and can be exported to a remote PR review thread (§4.3).

---

## 6. Checkpoint revert flow

### 6.1 User-facing flow

```
User: "Revert to checkpoint before turn 3" (or "revert this turn")
  ├─ UI confirms scope: turn-only | thread-to-point | whole-thread
  ├─ runtime: git reset --hard refs/multiplexer/threads/<id>/pre/3
  ├─ runtime: truncate/replay read model to turn 2 state
  ├─ UI: diff pane + editor + agent activity refresh
  └─ (optional) agent is notified the workspace was reverted
```

### 6.2 Scope options

| Scope | Ref target | Effect |
|-------|-----------|--------|
| Revert last turn | `pre_turn_N` | Undo turn N's changes |
| Revert to point | `pre_turn_K` | Undo turns K..N |
| Whole-thread reset | `base` | Back to thread start |

### 6.3 Safety

- Revert is **non-destructive to checkpoints**: the reverted-to commits remain, so the user can re-apply forward if they change their mind (the refs are immutable).
- Revert is **destructive to the working tree**: we confirm with the user and surface any uncommitted local edits that would be lost (we diff the worktree against the target ref first).
- Revert is atomic at the ref level (`git reset --hard` is a single ref update) and the read-model replay runs in the same transaction as the orchestration projector (see `plan/06`), so the two never diverge.

### 6.4 Adapter contract

`checkpoint_revert` is a first-class Provider Adapter method (per `docs/PLAN-CONTEXT.md`), so a revert can also be triggered programmatically (e.g. by a subagent that detects it went down a wrong path, or by the user from the mobile app).

---

## 7. Conflict handling

### 7.1 Why parallel worktrees avoid conflicts

Because each task/subagent has its **own worktree and branch**, there is no shared working directory to corrupt and no branch juggling. Two agents editing the same file in different worktrees do not conflict at the filesystem level — they conflict only at **merge** time, and only if they touched overlapping lines.

### 7.2 Merge management

Merges happen at explicit, user-visible boundaries:

1. **Subagent → parent task:** when a subagent finishes, its branch is merged into the parent task branch. If the subagent only touched files the parent didn't, this is a fast-forward or clean merge.
2. **Task → base:** when the user accepts a task, its branch merges into the thread base.
3. **Thread → upstream:** when the user publishes, the branch merges into the remote base (via PR/MR, §4).

### 7.3 Conflict resolution

- **Detection:** `git merge` reports conflicts; the runtime surfaces them in the diff pane with both sides (`ours`/`theirs`) and a merge editor.
- **Policy:** conflicts are **never auto-resolved** by the runtime. The user resolves them in the editor (or delegates resolution to an agent as a new subagent task).
- **`--3way` apply:** for diff-apply (§3.4), three-way merge with blob SHAs minimizes spurious conflicts when context has drifted.

### 7.4 Shared-file contention

Two agents editing the same file concurrently is a real risk in fan-out. Mitigations:

- The scheduler can **serialize** writes to a contended file (see `plan/06` for the scheduler's cross-thread awareness).
- The diff model makes the overlap visible early so the user can intervene before a messy merge.

---

## 8. Testing

TDD at inception (per `docs/PLAN-CONTEXT.md`): unit → property → mutation → integration → component → e2e, with CI coverage gates (≥85% line, ≥80% branch, ≥70% mutation score).

### 8.1 Unit tests (co-located `#[cfg(test)]`)

- **Checkpoint capture:** given a temp repo + a set of file mutations, assert the pre/post refs point at the expected commits and the per-turn diff is correct.
- **Empty-checkpoint optimization:** a read-only turn produces no new commit (ref aliases previous target).
- **Revert:** `reset --hard` to a pre-turn ref restores the exact file contents (including deletions and untracked files created by the agent).
- **Ref namespace:** refs land under `refs/multiplexer/...` and never appear in user branch listings.
- **Diff queries:** per-turn vs full-thread ranges return the correct file sets and hunk metadata.
- **Worktree lifecycle:** create/remove registry entries, branch naming, base selection.
- **Comment→agent payload:** a diff comment serializes to the correct `user_input_respond` shape.

### 8.2 Property-based tests (proptest)

- **Checkpoint/revert round-trip:** for arbitrary sequences of file mutations (create/edit/delete/rename), `capture → revert → capture` yields identical trees (invariant: revert is the inverse of capture).
- **Diff range algebra:** `diff(base..head) == diff(base..mid) ⊕ diff(mid..head)` for any mid checkpoint.
- **Ref monotonicity:** turn sequence numbers strictly increase; refs never point backward.

### 8.3 Mutation tests (cargo-mutants)

Target the checkpoint capture/revert logic and the diff-range projector — these are the highest-risk, highest-value code paths. Gate: ≥70% mutation score killed.

### 8.4 Integration tests (real git repo)

- Spin up a **real temp git repo** (via `git2`/`gix` or the `git` CLI) and drive the full lifecycle: create worktree → run a fake agent (mock ACP, per `docs/PLAN-CONTEXT.md`) that mutates files → capture checkpoints → query diffs → revert → assert read model + filesystem consistency.
- **Parallel worktrees:** create N worktrees, run N fake agents concurrently, assert isolation (no cross-contamination) and correct merges.
- **Conflict scenario:** two agents edit overlapping lines in the same file; assert conflict detection and that resolution requires explicit user action.
- **ScmProvider mocks:** a fake GitHub/GitLab provider (in-memory HTTP server) to test clone/publish/PR/review-thread flows without network.

### 8.5 Component & e2e

- **Component (GPUI):** diff pane rendering, inline comment placement, revert confirmation dialog.
- **E2E:** drive the real app headless — create a thread, run an agent, comment on a diff line, verify the comment reaches the agent and the next checkpoint reflects the response.

---

## 9. Open questions

Referenced from `docs/PLAN-CONTEXT.md` (not decided here):

1. **MVP scope of VCS integrations:** which of GitHub/GitLab/Bitbucket/Azure DevOps ship in MVP vs later? (Relates to open question 7 — Orca baseline scope.)
2. **SSH remote worktrees in MVP:** baseline bar includes them, but they add transport complexity; defer to roadmap (`plan/19`).
3. **Stacked-PR depth:** how aggressively to build stacked-PR automation in MVP vs single-PR first.
4. **Checkpoint retention policy:** per-thread lifetime vs configurable retention; interaction with `git gc`.
5. **Conflict auto-resolution delegation:** whether to allow an agent to resolve merge conflicts as a subagent task, or keep resolution strictly manual in MVP.
6. **Diff storage tradeoff:** structured diff metadata in the read model (chosen here) vs full-text storage — confirm the read-model size budget in `plan/16-performance.md`.
7. **Windows git availability:** whether to bundle a Git for Windows distribution or require a system Git (affects worktree/checkpoint reliability on our Windows-first target).
