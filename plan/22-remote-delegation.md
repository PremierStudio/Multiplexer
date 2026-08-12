# 22: Remote Session Delegation

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Remote / Core runtime
**Depends on:** `02-architecture.md`, `04-wire-contract.md`, `14-remote-and-relay.md`, `17-security-and-secrets.md`, `06-orchestration-engine.md`
**Feeds:** `15-testing-strategy.md`, `19-roadmap-and-milestones.md`, `20-risks-and-open-questions.md`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context). Where a decision is not yet settled, it is listed under **Open questions** and is
**not** decided unilaterally here. New decisions proposed here are numbered **D46+** in the
style of `docs/DECISIONS.md`; they are proposals for the decision log, not locked decisions.

**Locked decisions applied (D1, D13, D18, D19, D24, D25, D37, D38):** This doc reflects the
locked decisions from `docs/DECISIONS.md`:
- **D1** : Rust + GPUI, single native server binary; delegation is a mode of that binary, not a
  separate product.
- **D13** : consolidated `multiplexer-*` crate layout; the delegation control plane lives in
  `multiplexer-wire` (contract) and `multiplexer-server` (composition), with the remote agent
  reusing the same binary in `--remote` mode.
- **D18** : bounded channels with backpressure; delegated session event flow follows the same
  rule end to end.
- **D19** : unified `session.start` params `{provider, model, workspace, initial_prompt, resume,
  config}`; delegation reuses this shape with a `target` field added.
- **D24** : relay is a TLS-terminating pipe, not E2EE; delegation over the relay inherits this
  honest claim.
- **D25** : the remote agent independently enforces permission modes, worktree confinement, and
  approval gating on the remote host; delegation is the strongest expression of this boundary.
- **D37** : pairing issues a long-lived device credential minted into short-lived tickets;
  delegation uses the same credential model.
- **D38** : local tickets are keychain-only, never a plaintext token file.

---

## 1. Problem statement

Today, the user's agent session runs on the same machine as the UI. The user works on system A
(the control surface: editor, panes, chat, browser, HAR) and the agent session executes on the
same system A. This couples two things that do not need to be coupled: the *interactive surface*
and the *compute/execution plane*.

Real workflows break that coupling. A user with a powerful desktop or a beefy remote box wants
the agent to run where the resources, the filesystem, the git state, and the long-lived
processes live, while they steer it from a thin laptop, a second desktop, or a phone. The agent
session is a long-lived, stateful thing: it holds a workspace, terminals, git refs, checkpoints,
and a running process tree. That state should live on the machine that is best suited to hold
it, and the UI should be able to attach, detach, resume, and fork it from anywhere.

The current remote story (plan/14) covers *how a client reaches a core*: local, bearer-paired,
relay tunnel, and SSH. But it does not fully cover *delegation*, where the agent session itself
executes on a different machine than the user's primary control surface, and where the session
persists on that remote machine so it can be resumed or forked independently of any particular
client connection.

This is a first-class differentiator. No major competitor does true delegation well: the closest
precedents either keep execution local and only mirror the UI, or run on vendor-owned cloud VMs
rather than the user's own machines. Multiplexer's server-centric architecture is uniquely
positioned to make delegation a first-class, secure, first-party feature.

---

## 2. Why this is a product feature

Delegation is not a niche transport detail; it is a product capability with durable competitive
value. The precedents show both the demand and the gap:

1. **Codex App Server is the closest precedent.** OpenAI's Codex harness runs a long-lived
   server that hosts threads, with a bidirectional JSON-RPC 2.0 contract (stdio/JSONL or
   WebSocket) and an Item/Turn/Thread lifecycle. Clients are thin shells over that server. This
   is exactly the "server owns the session, clients are thin" shape Multiplexer already has, and
   it validates that a long-lived, delegatable session server is a real, shipped pattern.
   https://openai.com/index/unlocking-the-codex-harness/
2. **Claude Remote Control is NOT delegation.** It keeps execution local and only mirrors the UI
   to a remote viewer. The agent still runs on the user's machine; the remote is a screen, not a
   compute plane. This is the key distinction: delegation moves *execution*, not just *rendering*.
   https://code.claude.com/docs/en/remote-control
3. **Cursor Cloud Agents run on vendor VMs, not user machines.** They delegate to isolated cloud
   VMs that the user does not own or control. That is a different value proposition: it is
   convenient but it is not "run on my hardware, my network, my filesystem." Multiplexer's
   delegation targets the user's own machines (a home server, a beefy desktop, a lab box), which
   is a distinct and complementary story.
   https://cursor.com/blog/self-hosted-cloud-agents
4. **VS Code Remote proves the on-demand FS model.** VS Code Remote does not bulk-sync the
   filesystem; it exposes a remote filesystem through an on-demand FileSystemProvider RPC and
   does file watching on the remote side. This is the correct model for delegation: no full
   mirror, just on-demand access to the machine that owns the state.
   https://code.visualstudio.com/docs/remote/ssh

The strategic point: delegation is the natural consequence of Multiplexer's server-centric
runtime. The server already owns agent processes, terminals, git, fs, checkpoints, and HAR. If
that server can run on a different machine than the UI, then delegation is not a new subsystem;
it is the same runtime exposed across a machine boundary. Nobody else has this combination of
(a) a single native binary that owns everything, and (b) a clean wire contract that lets a thin
client steer it from anywhere.

---

## 3. Design goals

1. **Execution on B, control on A.** The agent session (workspace, terminals, git, checkpoints,
   process tree) executes on system B. System A is a thin control surface over the wire
   contract. A is not a dumb terminal; it is the full Multiplexer UI, but it holds no
   authoritative session state.
2. **Sessions persist on B.** A delegated session lives on B independent of any client
   connection. A can disconnect, and the session keeps running. A (or another client) can
   reconnect, resume, observe, or fork it. This is the "long-lived server hosting threads" model
   from Codex, applied to the user's own machine.
3. **On-demand FS, no full mirror.** Following VS Code Remote, B's filesystem is accessed
   on-demand through the wire contract's `fs.*` methods and file watching happens on B. There is
   no bulk sync and no full mirror of the workspace to A.
4. **Latency that feels local.** Direct P2P (Tailscale/WireGuard) gives tens of milliseconds;
   relay paths feel laggy. Delegation must prefer direct paths and degrade gracefully.
5. **Secure by default.** WireGuard/WireGuard-class tunnel plus app-layer tokens plus WSS.
   Outbound-only workers preferred (no inbound ports to open on B).
6. **Reconnect, resume, and fork.** A dropped connection never loses the session. The wire
   contract's resume cursor and idempotency keys (plan/04) let a client catch up; the session
   itself is never tied to a connection.

---

## 4. Proposed architecture

Delegation splits the runtime into two cooperating planes, both running the same Multiplexer
binary:

- **Control plane (system A):** the user's primary UI. Runs the full Multiplexer desktop
  (editor, panes, chat, browser, HAR) but holds no authoritative session state. It is a thin
  client over the wire contract, exactly like the mobile app, but with the full desktop UI.
- **Execution plane (system B):** a Multiplexer core in `--remote` mode. It owns the agent
  session: the workspace, terminals, git refs, checkpoints, and the running process tree. It is
  the single source of truth for that session.

The two planes talk over the **same JSON-RPC-over-WebSocket wire contract** (plan/04). Delegation
is therefore not a new protocol; it is the existing contract carried over a remote transport
(plan/14's four kinds), with the session living on B.

### 4.1 What lives where

| Concern | System A (control) | System B (execution) |
|---|---|---|
| Agent session runtime (in-process grok-build) | No | **Yes** |
| Workspace filesystem | On-demand view via `fs.*` | **Yes** (owns the files) |
| Git state / checkpoints | Read via `git.*` / `checkpoint.*` | **Yes** (owns the refs) |
| Terminals (PTY) | Render via `terminal.*` streams | **Yes** (owns the PTYs) |
| Process tree / agent tool exec | No | **Yes** |
| HAR capture | Render via `har.*` | **Yes** (CDP on B) |
| Editor buffers | **Yes** (local, on A) | No |
| UI panes / windows | **Yes** | No |
| Session identity / persistence | No | **Yes** |

The split is clean: everything *mutable and long-lived* lives on B; everything *interactive and
ephemeral* lives on A. The editor buffers are the one deliberate exception: they live on A so
typing is local and instant, and they are flushed to B on save via `fs.write` (matching the
on-demand FS model). Unsaved buffers are never discarded on disconnect; they are marked "pending
sync" and flushed on reconnect (consistent with plan/14 §3.3).

### 4.2 Session lifecycle on B

A delegated session is a first-class object on B, independent of any connection:

```
created ──▶ running ──▶ paused ──▶ running
   │           │
   │           └──▶ completed / failed
   └──▶ (fork) ──▶ new session (child of this one)
```

- **created:** `session.start` with a `target` field naming B (see §4.3). B materializes the
  session from the unified params (D19).
- **running:** the session executes turns, owns terminals/git/checkpoints. It streams events to
  any subscribed client.
- **paused:** a client can pause the session (stop consuming turns) without killing it; the
  process tree and state remain on B.
- **fork:** a client can fork a session, creating a child session on B that starts from the
  parent's checkpoint. This is how a user branches an agent's work without duplicating state.
- **completed / failed:** terminal states; the session record and its checkpoints remain on B for
  later inspection or fork.

Because the session persists on B, a client that disconnects does not end the session. The
session's event log is in B's read model (plan/06); a reconnecting client replays missed events
from a resume cursor (plan/04 §8.2) and re-subscribes to streams.

### 4.3 Wire contract additions

Delegation reuses the existing contract and adds a small, explicit surface. It does not invent
methods that contradict plan/04; it extends the `session.*` and `remote.*` namespaces that
already exist.

- **`session.start`** gains an optional `target` field: `{provider, model, workspace,
  initial_prompt, resume, config, target?}`. When `target` is present, the session is created on
  the named remote (B) rather than locally. This is a strict superset of D19's unified shape.
- **`session.list` / `session.get`** already return sessions; when a session is delegated, its
  record includes `target` and `execution_host` so the UI can show where it runs.
- **`remote.*`** (plan/04 §4.13) already covers connect/disconnect/list. Delegation adds the
  notion that a connected remote can *host* sessions, not just be a transport hop. `remote.list`
  reports whether each remote is a full execution host or a thin relay.
- **`session.fork`** (new, in the `session.*` namespace): `{session_id, checkpoint_id?}` returns
  a new `session_id` on the same execution host. This is the fork primitive.
- **`session.pause` / `session.resume`** (new, in the `session.*` namespace): control the
  running/paused state on B without killing the session.

All additions are additive (minor protocol version bump per plan/04 §9.2); no existing method or
event shape changes. The event set is unchanged: delegated sessions emit the same canonical
events (`agent_message_chunk`, `tool_call`, `terminal_output`, `fs_change`, `checkpoint`, etc.)
on the same stream names. A client cannot tell, from the event stream alone, whether the session
is local or delegated; that is the point.

### 4.4 Control plane on A

System A runs the full desktop UI but treats the delegated session as a remote resource. It
subscribes to the session's streams, renders terminal output, shows diffs, and forwards intents
(`turn.send`, `approval.respond`, `terminal.input`, `fs.write`). It holds editor buffers locally
for typing latency, but every authoritative read/write goes through the wire contract to B.

A can also run *local* sessions (the normal case) and *delegated* sessions side by side. The UI
shows a session's execution host so the user always knows where a given agent is running.

### 4.5 Transport and latency

Delegation prefers a direct P2P path between A and B. Tailscale/WireGuard give tens of
milliseconds of latency, which feels local for terminal and editor interaction. The relay
(plan/14 §4) is the fallback for when no direct path exists (e.g. cellular), accepting 20-50+ ms
of added latency. The wire contract's window-based flow control (plan/04 §8) keeps high-volume
streams (terminal output, diffs) from flooding a slow path.

The transport is plan/14's four kinds, unchanged. Delegation does not add a fifth transport; it
adds the *semantic* that a connected remote hosts sessions. The latency guidance is a design
constraint, not a new transport.

---

## 5. Key design decisions (proposed D46+)

These are proposals for `docs/DECISIONS.md`, in its style. They are **not** locked; they are
offered for the decision log.

### D46. Delegation = same binary in `--remote` mode (PROPOSED)
- **Decision:** A delegated execution host is the same Multiplexer binary running in `--remote`
  mode (the mode plan/14 §3 already defines for SSH remote worktrees). There is no separate
  "delegation server" product or crate.
- **Rationale:** Reuses the server-centric runtime, the wire contract, and the remote-agent
  security boundary (D25) already designed. Delegation is a mode, not a new subsystem.

### D47. Sessions persist on the execution host, independent of any connection (PROPOSED)
- **Decision:** A delegated session lives on B and is not tied to any client connection. Clients
  attach, detach, resume, observe, and fork it; disconnecting never ends the session.
- **Rationale:** This is the Codex App Server model (long-lived server hosting threads) applied
  to the user's own machine, and it is the core of the differentiator. It also makes reconnect
  and fork trivial because the session state is never in a client.

### D48. On-demand FS, no full mirror (PROPOSED)
- **Decision:** B's filesystem is accessed on-demand through the wire contract's `fs.*` methods;
  file watching happens on B. There is no bulk sync and no full mirror of the workspace to A.
- **Rationale:** Matches VS Code Remote's proven model and avoids the cost and staleness of a
  full mirror. Editor buffers are the one local exception (typing latency), flushed on save.

### D49. Delegation is a superset of the existing wire contract (PROPOSED)
- **Decision:** Delegation reuses the JSON-RPC-over-WebSocket contract and adds only a `target`
  field to `session.start` plus `session.fork` / `session.pause` / `session.resume`. No existing
  method or event shape changes; the event set is identical for local and delegated sessions.
- **Rationale:** Keeps the contract a single source of truth (D13, D20) and makes delegation
  invisible to the event stream, so the UI and mobile client work unchanged.

### D50. Direct P2P preferred, relay fallback (PROPOSED)
- **Decision:** Delegation prefers a direct P2P path (Tailscale/WireGuard, tens of ms) and uses
  the relay (plan/14 §4) only when no direct path exists. The relay remains a TLS-terminating
  pipe (D24), not E2EE.
- **Rationale:** Latency is the difference between "feels local" and "feels laggy." Direct paths
  are the default; the relay is the anywhere fallback.

---

## 6. Security

Delegation moves execution to a machine the user may not be sitting at, which raises the stakes
on every security control. It follows plan/17's principles and, critically, D25: the remote
agent is **not a dumb executor**. It independently enforces the same controls on B that the
local core enforces locally.

1. **Independent enforcement on B (D25).** The execution host re-validates every operation
   against the 4-way approval model (`allow`/`deny`/`allow_once`/`allow_always`, D12) on B,
   confines all fs/git/process operations to the authorized worktree(s), and gates approvals on
   B. A compromised or buggy control plane on A cannot bypass B's gating. A's requests are
   *proposals* that B validates against its own policy.
2. **Transport security.** Delegation uses the plan/14 transport stack: TLS 1.3, ticket + DPoP
   at the application layer, and a WireGuard/WireGuard-class tunnel for direct paths. Over the
   relay, the honest D24 claim applies (TLS-terminating pipe, not E2EE); the optional per-tunnel
   E2EE layer from plan/14 §4.1 is available for users who want the relay to be a dumb pipe.
3. **Outbound-only workers preferred.** The execution host establishes an outbound connection to
   the control plane (or to the relay), so no inbound firewall port needs to be opened on B. This
   is the same model as plan/14's relay tunnel and is the default for delegation.
4. **Credentials never leave their machine.** Provider tokens, git credentials, and keychain
   material stay on B (the machine that owns the session). A never receives B's secrets; it
   receives only short-lived tickets and DPoP proofs (D37, D38). On the wire, only tickets,
   bearer tokens, and DPoP proofs travel.
5. **Least privilege per connection.** Each control-plane connection binds to a scope (which
   sessions, worktrees, and capabilities it may touch). A phone observing a delegated session
   gets read-mostly scope; the desktop gets control scope. This is plan/14 §1.1 invariant 3.
6. **Auditability.** Every delegated-session event (spawn, turn, tool call, approval, fork,
   pause, resume) is in B's read model, replayable for review, consistent with plan/17's
   auditability principle and plan/14 §6.
7. **No raw secrets in configs.** B's config references secrets only as `op://Vault/Item/field`
   (resolved via the session-cache model, D23), never raw values. This is unchanged from plan/17.

---

## 7. Testing strategy

Delegation is tested under the project's TDD-at-inception gate chain (fmt → clippy → unit+
property → mutation → integration → component → e2e → coverage), per plan/15.

### 7.1 Unit tests (routing and session identity)

Co-located `#[cfg(test)]` modules over the delegation logic:
- **Routing:** a `session.start` with a `target` routes to the named execution host; without a
  `target`, it stays local. Invalid `target` values are rejected with a typed error.
- **Session state machine:** `created → running → paused → running → completed/failed` and the
  `fork` transition; illegal transitions (e.g. `fork` on a completed session) are rejected.
- **Fork:** forking from a checkpoint produces a child session with the correct parent link and
  a fresh session id.
- **Resume cursor:** a reconnecting client resumes from a `seq` and replays only missed events.

### 7.2 Property tests (proptest)

- **Session identity:** under arbitrary session-start params (with and without `target`), the
  session id is unique and stable; identical params on the same host produce distinct ids (each
  start is a new session), and the `target` field never leaks into the local session id space.
- **State machine invariants:** under arbitrary sequences of start/pause/resume/fork/stop, a
  session is never `running` and `paused` simultaneously; a fork always has a live parent or a
  completed parent with a checkpoint; the event log never disagrees with the session state.
- **Routing:** proptest over `target` values asserts that only known hosts are accepted and
  unknown hosts fail closed.

### 7.3 Integration tests (A controls B, mock)

- **A-controls-B:** run a real control-plane core against a mock execution host (or a real
  `multiplexer --remote` over loopback); assert `session.start` with `target` creates a session
  on B, `turn.send` streams events back, and `fs.write` lands on B.
- **Reconnect/resume:** kill the connection mid-turn; assert the session on B keeps running, the
  client reconnects, and missed events are replayed from the resume cursor.
- **Fork:** fork a running session; assert a child session is created on B from the parent's
  checkpoint and both can be observed independently.
- **Independent enforcement (D25):** from a mock control plane, attempt an operation outside the
  granted scope (e.g. a path outside the worktree, or an unapproved tool call); assert B rejects
  it even though A requested it.
- **Wire contract:** schema-verified JSON-RPC over each transport (contract tests from plan/04),
  including the new `target` field and `session.fork` / `session.pause` / `session.resume`.

### 7.4 Mutation testing

cargo-mutants over the routing, session state machine, fork, and resume-cursor logic. CI gates:
≥85% line, ≥80% branch, ≥70% mutation score killed (D21, D33). The session state machine and the
routing logic are prime mutation targets.

### 7.5 E2E

Drive the real app headless with a real execution host on a second (loopback or local-network)
instance: start a delegated session, edit a file on A, run an agent turn on B, assert the read
model on B and the UI on A both reflect it, then disconnect and reconnect and assert the session
resumed. This is the direct regression test for the delegation story.

---

## 8. Open questions / risks

These are flagged, not decided here:

1. **Execution-host discovery.** How does A learn about available execution hosts (B machines)?
   Options include the account service (plan/14 §7), Tailscale, or manual configuration. The
   discovery mechanism is a product decision, not settled here.
2. **Latency budget.** The "feels local" threshold (tens of ms direct, 20-50+ ms relay) is a
   design target. The exact budget for terminal and editor interaction needs real measurement
   (plan/16) and may require adaptive chunking on slow paths.
3. **Editor buffer conflict.** Editor buffers live on A (typing latency) but the authoritative
   files live on B. Concurrent edits (A's buffer vs B's agent tool writes) need a defined
   conflict policy. This is the same class of problem as plan/14 §3.3's unsaved-edit safety, but
   for delegated sessions it is more likely to occur.
4. **Fork semantics.** Whether a fork copies the full workspace or shares it (e.g. via a
   worktree) is a product decision. plan/07 (checkpointing) and plan/14 (worktrees) inform it,
   but the exact fork model is open.
5. **Multi-core coordination.** plan/14 §7 defers cross-core orchestration (one agent on machine
   A depending on machine B). Delegation is a step toward that, but full cross-core dependency
   is still out of scope.
6. **MVP scope.** Whether delegation ships in MVP (Phase 4, mobile/remote) or is staged later is
   a roadmap decision for plan/19. The wire-contract additions are small, but the execution-host
   discovery and conflict policy may gate it.
7. **Windows-first.** The execution host runs the same Windows-first binary (D9, D35). Remote
   process/PTY semantics on Windows (Job Objects, path translation) need the same platform
   attention as plan/14 §3.5.

**Flagged consistency note:** this doc is consistent with PLAN-CONTEXT (server-centric runtime,
single wire contract, event-sourced read model, bounded channels, secrets session-cache model)
and extends plan/14 (remote/relay) rather than contradicting it. It is the delegation counterpart
to plan/14's transport story: plan/14 defines *how a client reaches a core*; this doc defines
*how a session executes on a different core than the user's primary UI*. It relates to the
forthcoming **plan/23 (Tailscale)** as the direct-path transport provider for delegation, and to
the forthcoming **plan/24 (resource manager)** as the owner of execution-host resource accounting
and limits on B. If any locked decision flips (e.g. stack, crate layout, relay posture), the
affected sections (§4, §5, §6) must be revisited.

---

## References

- `docs/PLAN-CONTEXT.md`: authoritative shared plan context (server-centric runtime, remote/relay
  line, security posture, testing).
- `docs/DECISIONS.md`: locked decisions applied (D1, D13, D18, D19, D24, D25, D37, D38).
- `plan/02-architecture.md`: server-centric runtime, single native binary, wire contract.
- `plan/04-wire-contract.md`: JSON-RPC-over-WebSocket contract this doc extends.
- `plan/06-orchestration-engine.md`: event-sourced read model used for resume and fork.
- `plan/14-remote-and-relay.md`: the transport/relay story this doc extends.
- `plan/17-security-and-secrets.md`: OS keychain, `op://` references, DPoP/passkeys, D25.
- `plan/19-roadmap-and-milestones.md`: staging of delegation in the roadmap.
- `plan/20-risks-and-open-questions.md`: consolidated open decisions.
- Codex App Server (closest precedent): https://openai.com/index/unlocking-the-codex-harness/
- Claude Remote Control (UI mirror, not delegation): https://code.claude.com/docs/en/remote-control
- Cursor Cloud Agents (vendor VMs, not user machines): https://cursor.com/blog/self-hosted-cloud-agents
- VS Code Remote (on-demand FS, no bulk sync): https://code.visualstudio.com/docs/remote/ssh
