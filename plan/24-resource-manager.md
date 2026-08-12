# 24: Resource Manager (Process & Resource Control)

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Core runtime / Performance
**Depends on:** `02-architecture.md`, `06-orchestration-engine.md`, `16-performance.md`, `21-mcp-lifecycle-supervisor.md`, `22-*.md`, `23-*.md`
**Feeds:** `15-testing-strategy.md`, `16-performance.md`, `19-roadmap-and-milestones.md`, `20-risks-and-open-questions.md`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context) and with `docs/DECISIONS.md` (the locked decisions). Where a decision is not yet
settled, it is listed under **Open questions** and is **not** decided unilaterally here. New
decisions proposed here are numbered **D57+** in the style of `docs/DECISIONS.md`; they are
proposals for the decision log, not locked decisions.

**Locked decisions applied (D1, D11, D13, D18, D21, D22, D23, D33):** This doc reflects the
locked decisions from `docs/DECISIONS.md`:
- **D1** : Rust + GPUI, single native server binary; the resource manager is a component of that
  binary, not a sidecar.
- **D11** : Multiplexer owns subagent scheduling; the resource manager owns the *resources* those
  subagents run on (cores, RAM, process containment).
- **D13** : consolidated `multiplexer-*` crate layout; the resource manager lives in
  `multiplexer-core` (policy, allocator, data model) with OS plumbing in `multiplexer-server`.
- **D18** : bounded channels with backpressure; telemetry ingestion follows the same rule.
- **D21 / D33** : mutation testing applies to all core logic, including the resource allocator;
  70% mutation score is the merge floor.
- **D22** : a dedicated performance stage in CI enforces the hard perf gates; the resource
  manager is what makes the memory and fan-out gates enforceable.
- **D23** : secrets session-cache model; child-process env/headers reference secrets via the same
  mechanism, never raw values.

This document is **separate from** `21-mcp-lifecycle-supervisor.md`. The two are complementary
and both are required: plan/21 owns *which* MCP servers exist, their reuse, reference counting,
and restart policy; this doc owns *how* every process tree (MCP servers included) is contained,
limited, pinned, and reaped, and how work is allocated across a fleet of machines. The MCP
supervisor is one *consumer* of the resource manager's containment and limit primitives.

---

## 1. Problem statement

Multiplexer's core promise is "blazing-fast, no orphans, real insight." Today, on the very
machine this product is being built on, the Grok CLI fails that promise in a way that is
directly observable:

- **8 grok.exe sessions** hold **101 node processes / ~10.4 GB RAM**.
- With more accumulated sessions the user has observed **562 processes / ~27.9 GB RAM**.
- Each **npx-based** server costs **2 node processes** (the `npx-cli.js` wrapper plus the actual
  server), doubling the count.
- Sessions restart and spawn a fresh copy of every configured MCP server rather than reusing
  running ones, and never tear the fleet down on exit.

This is the orphaned-process pile-up. It is not a cosmetic annoyance: it is a **resource
exhaustion failure** that makes the machine unusable and directly contradicts the performance
targets in `16-performance.md` (memory under budget, dozens of concurrent subagents). The root
cause is that no competitor treats child processes as a *managed, bounded, contained resource*.
They spawn, leak, and forget.

There is a second, deeper problem this doc addresses: **no competitor gives the user any control
over *where* work runs.** CPU affinity, memory caps, and cross-machine delegation are absent from
Grok CLI, Claude, Cursor, Orca, T3 Code, and Codex Desktop. A user with a 32-core machine cannot
say "pin this agent session to cores 8-15 and cap it at 4 GB," and a user with a fleet of
machines cannot spread work across them at all. Multiplexer's "control surface for your agents"
positioning demands exactly this: the user should see every machine, every core, every gigabyte,
and decide how their agents use them.

This doc is the plan for the **resource manager**: the component that (a) contains every process
tree so nothing is ever orphaned, (b) pins sessions to dedicated cores and caps their memory, and
(c) allocates work across 1-100 machines, all behind a beautiful live visual.

---

## 2. Why this is the killer feature

The orphan pile-up is a bug fix; resource control is a product. Both live in the same component,
and together they are a durable differentiator:

1. **No competitor contains process trees.** The 562-process / 27.9 GB failure mode is
   universal across the category. A client that guarantees "when I close a session, its entire
   process tree dies with it" is measurably better on the axis users feel most: a machine that
   does not slowly rot. This is the same ownership discipline as plan/21, applied at the
   operating-system level and made unconditional (kill-on-close), not dependent on a server
   remembering to clean up.
2. **No competitor pins or caps.** CPU affinity and memory limits are the difference between
   "dozens of subagents thrashing all cores" and "each agent session runs on its own dedicated
   cores, isolated from the UI and from each other." Deterministic performance is a feature:
   the UI never stutters because an agent saturated the machine, and one runaway agent cannot
   starve the rest.
3. **No competitor delegates across machines.** The fleet view (1-100 machines, each with its
   cores and RAM, click to enable/disable) turns Multiplexer from a single-machine tool into a
   control surface for a whole compute fleet. This is the "control surface for your agents"
   promise taken literally.
4. **It is the enforcement mechanism for the perf gates.** `16-performance.md` sets hard memory
   and fan-out gates. Those gates are only enforceable if the runtime can actually bound and
   contain the processes it spawns. The resource manager is what makes "memory under budget" a
   guarantee rather than a hope.

This maps directly onto Multiplexer's server-centric architecture: a single native binary owns
agent processes, terminals, git, fs, checkpoints, and HAR. The resource manager is the natural
extension of that "runtime owns child processes" model, adding *containment, limits, affinity,
and fleet allocation* on top of the ownership plan/06 and plan/21 already establish.

---

## 3. Design goals

1. **No orphans, unconditionally.** Every process tree spawned by the runtime is contained in an
   OS primitive (Job Object on Windows, cgroup v2 on Linux) with **kill-on-close**: when the
   owning handle drops, the entire tree dies. This is a kernel guarantee, not a cleanup routine,
   so it cannot be skipped. It directly fixes the 562-process / 27.9 GB pile-up.
2. **Reserve cores for the app.** The core app reserves cores 0,1 for itself (UI, render thread,
   orchestration). Agent sessions are pinned to the remaining cores, so the UI never contends
   with agent work.
3. **Pin sessions to dedicated cores.** Each agent session is pinned to a set of cores, giving
   deterministic, isolated performance. Sessions do not thrash each other or the UI.
4. **Cap memory per session and per tree.** Each session tree has a hard memory limit (Job Object
   `ProcessMemoryLimit`/`JobMemoryLimit` on Windows, cgroup v2 `memory.max` on Linux). A runaway
   agent is killed by the kernel, not left to exhaust the machine.
5. **Beautiful live visual.** A node view: each machine, its cores (click to enable/disable for
   agent use), and RAM usage. The user sees and controls exactly where work runs.
6. **Delegate across 1-100 machines.** A fleet scheduler allocates each session to a machine with
   enough free cores and RAM, tracked via a per-machine core bitmap and free-RAM figure.
7. **Windows-first.** Job Objects are the primary containment mechanism; cgroup v2 is the Linux
   path. Windows is the first-class citizen per D9/D35.

---

## 4. Proposed architecture

The resource manager is a component of the server runtime. It owns three things: (a) the
**containment + limits** primitive every process tree lives in, (b) the **affinity + memory
policy** that decides where a session runs, and (c) the **fleet scheduler** that allocates work
across machines. It sits alongside the orchestration engine (plan/06) and the MCP lifecycle
supervisor (plan/21): the scheduler owns subagents, the MCP supervisor owns MCP servers, and the
resource manager owns the *resources* both of them run on.

### 4.1 Placement in the runtime

```
┌───────────────────────────────────────────────────────────────┐
│                     MULTIPLEXER SERVER                        │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  ORCHESTRATION ENGINE (event-sourced, plan/06)          │  │
│  │  command queue → decider → projector → SQLite read model│  │
│  └───────────────┬─────────────────────────────────────────┘  │
│                  │  spawn / stop / telemetry                  │
│  ┌───────────────▼─────────────────────────────────────────┐  │
│  │  RESOURCE MANAGER                                       │  │
│  │  ┌────────────┐ ┌────────────┐ ┌──────────────────────┐ │  │
│  │  │ Containment│ │ Affinity & │ │ Fleet scheduler      │ │  │
│  │  │ (Job/cgroup│ │ Memory     │ │ (core bitmap + RAM,  │ │  │
│  │  │  per tree) │ │ policy     │ │  1-100 nodes)        │ │  │
│  │  └────────────┘ └────────────┘ └──────────────────────┘ │  │
│  │  ┌────────────────────────────────────────────────────┐ │  │
│  │  │ Telemetry data model (NodeState / CoreState)       │ │  │
│  │  └────────────────────────────────────────────────────┘ │  │
│  └───────────────┬─────────────────────────────────────────┘  │
│                  │  spawn / signal / reap / pin / limit       │
│  ┌───────────────▼─────────────────────────────────────────┐  │
│  │  PROCESS TREES (agent sessions, subagents, MCP servers, │  │
│  │  terminals) - each in its own Job Object / cgroup       │  │
│  └─────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

The resource manager is not a sidecar. It is in-process with the server, so it shares the read
model, the secrets session cache (D23), and the resource monitor (plan/16 §6). It is the single
owner of process containment, affinity, and limits; no other component spawns a process outside
a managed container.

### 4.2 Containment: Job Objects (Windows) / cgroup v2 (Linux)

Every process tree the runtime spawns (an agent session, a subagent, an MCP server, a terminal
PTY) is created inside a containment primitive. The primitive is created *before* the first
child is spawned, and the child is assigned to it before it can fork further, closing the
spawn/assign race.

**Windows: Job Objects.** We use the `win32job` crate (https://github.com/ohadravid/win32job-rs,
https://crates.io/crates/win32job) for a safe wrapper over `CreateJobObjectW`,
`AssignProcessToJobObject`, and `SetInformationJobObject`. The critical flag is
**`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`** (`ExtendedLimitInfo::limit_kill_on_job_close()`): when
the last handle to the job closes (including on `Drop` of the `Job` wrapper), every process
associated with the job is terminated by the kernel. This is the orphan fix: it is a kernel
guarantee, not a cleanup routine, so a crashed or forgotten session cannot leak its tree.

- **Nested jobs (Windows 8+).** A session may contain subagents, and each subagent may itself
  spawn a tree. Windows 8+ supports nested jobs: a process can belong to a hierarchy of jobs,
  and kill-on-close on a parent job terminates all processes in that job *and its child jobs*.
  This gives us a clean hierarchy: one job per session tree, nested jobs for sub-trees, and
  closing the session job reaps everything below it. On Windows 7 (unsupported target) nesting
  is unavailable; we do not target it.
- **Memory limits.** The same job carries `ProcessMemoryLimit` (per-process) and `JobMemoryLimit`
  (aggregate for the whole tree). A runaway agent that exceeds its cap is terminated by the
  kernel.
- **Process-count limits.** `ActiveProcessLimit` caps the number of live processes in the tree,
  so a fork bomb inside a session is bounded.

**Linux: cgroup v2.** The `processkit` crate (https://crates.io/crates/processkit,
https://docs.rs/processkit) provides a cross-platform abstraction over exactly this containment
model. On Linux it creates a child cgroup v2 and uses `memory.max` for memory, `pids.max` for
process count, and `cpu.max` for CPU quota; teardown uses `cgroup.kill`. On Windows it uses a
Job Object with kill-on-close. On macOS/BSD it falls back to a POSIX process group (no resource
limits, but still kill-on-drop). The active mechanism is queryable via `group.mechanism()`.

**Crate strategy.** We use `processkit` as the primary abstraction because it unifies
containment + limits + kill-on-drop across platforms and is tokio-native (matching our async
runtime, plan/16 §5.2). We use `win32job` directly where we need Windows-specific control that
`processkit` does not expose (e.g. fine-grained nested-job affinity or precise
`JobMemoryLimit` semantics). Both are thin over the same OS primitives; the resource manager
hides the choice behind a `Containment` trait.

```rust
// processkit: whole-tree kill-on-drop + limits in one primitive
use processkit::{Command, ProcessGroup, ProcessGroupOptions};

let group = ProcessGroup::with_options(
    ProcessGroupOptions::default()
        .max_memory(4 * 1024 * 1024 * 1024) // 4 GiB for the whole session tree
        .max_processes(256)
)?;
let _job = group.start(&Command::new("grok-agent")).await?;
// Dropping `_job` (or the owning handle) kills the entire tree, unconditionally.
```

**Why kill-on-close is the right fix.** The orphan pile-up exists because cleanup is *best
effort*: a server "remembers" to reap its children, and when it forgets or crashes, the children
survive. Kill-on-close moves the guarantee into the kernel: the tree dies with its owner, no
matter what the owner does. This is the difference between "we try not to leak" and "leaking is
structurally impossible."

### 4.3 Affinity policy

**Reserve cores 0,1 for the app.** On startup, the resource manager sets the app process's
affinity mask to exclude cores 0 and 1 (or, equivalently, sets the app's affinity to the
remaining cores). The UI render thread, the orchestration engine, and the WebSocket listener run
on the reserved cores, so agent work can never starve the UI. This directly protects the
`< 16 ms` input-latency gate (plan/16 §4): the frame path never contends with agent CPU.

**Pin each session to dedicated cores.** Each agent session is pinned to a set of cores drawn
from the *enabled, unreserved* pool. Pinning gives deterministic, isolated performance: one
session's CPU-bound work cannot degrade another session or the UI. We use two crates:

- **`core_affinity`** (https://crates.io/crates/core_affinity) for thread-level pinning
  (`set_for_current`), used to pin the app's own worker threads and to pin a session's main
  thread.
- **`affinity`** (https://crates.io/crates/affinity, https://github.com/elast0ny/affinity) for
  process-level affinity on Windows (`set_process_affinity` →
  `SetProcessAffinityMask`), which child processes inherit. On Linux it uses
  `sched_setaffinity`.

**Windows processor groups (>64 cores).** `SetProcessAffinityMask`/`SetThreadAffinityMask`
operate on a 64-bit mask *within a single processor group*. On machines with more than 64 logical
processors (high-core Threadrippers, dual-socket servers), the `affinity` and `core_affinity`
crates only affect the process's primary group and cannot address cores ≥ 64. For those machines
we must use group-aware APIs: `GetLogicalProcessorInformationEx` to discover topology, and
`SetThreadGroupAffinity`/`GetThreadGroupAffinity` with a `GROUP_AFFINITY` (group number + 64-bit
mask), or the newer CPU Sets APIs. The resource manager abstracts this behind an `Affinity`
trait: the simple path (≤ 64 cores, single group) uses `affinity`/`core_affinity`; the group-aware
path uses direct `windows` crate calls. This is a real requirement for the "1-100 machines"
fleet story, where high-core machines are common.

**Affinity is a policy, not a hard rule for the OS.** We never steal cores from OS interactivity
or from the user's other applications. Cores are *enabled* for agent use by default, but the user
can disable any core in the visual (see §4.5); a disabled core is removed from the allocator's
pool and never assigned to a session. The app's reserved cores (0,1) are never offered to
sessions at all.

### 4.4 Memory policy

Each session tree carries a hard memory cap, enforced by the same containment primitive used for
kill-on-close (Job Object `ProcessMemoryLimit`/`JobMemoryLimit` on Windows, cgroup v2
`memory.max` on Linux). Because the limit lives on the containment object, a memory-limited
runaway tree is still fully reaped on drop: containment and limits are one object, not two.

| Limit | Default (proposal) | Enforced by |
|---|---|---|
| Per-session tree memory | 4 GiB | Job `JobMemoryLimit` / cgroup `memory.max` |
| Per-process memory | 2 GiB | Job `ProcessMemoryLimit` |
| Max processes per tree | 256 | Job `ActiveProcessLimit` / cgroup `pids.max` |
| CPU quota (optional, per session) | none by default | cgroup `cpu.max` / Job scheduling class |

The defaults are proposals to be tuned with real resource-monitor data (plan/16 §6), not assumed
(see §9). The resource manager enforces the caps; the resource monitor reports the usage that
feeds the visual and the fleet scheduler's free-RAM figure.

### 4.5 Live telemetry data model

The visual is a projection of a live data model, consistent with plan/06's event-sourced read
model. The core shapes:

```rust
pub struct NodeState {
    pub id: NodeId,
    pub cores: Vec<CoreState>,   // one per logical core
    pub ram_total: u64,          // bytes
    pub ram_used: u64,           // bytes
    pub sessions: Vec<SessionRef>,
    pub enabled: bool,           // whole node can be disabled
}

pub struct CoreState {
    pub index: usize,            // logical core index
    pub enabled: bool,           // click to enable/disable for agent use
    pub usage: f32,              // 0..100, from sysinfo per-core Cpu
    pub pinned_session: Option<SessionId>, // which session (if any) owns this core
}
```

- **Per-core usage** comes from the `sysinfo` crate (https://crates.io/crates/sysinfo,
  https://docs.rs/sysinfo): `System::cpus()` returns a `&[Cpu]` with per-core `cpu_usage()`.
  CPU usage is a *delta* between samples, so we keep one long-lived `System` instance and
  refresh no faster than `sysinfo::MINIMUM_CPU_UPDATE_INTERVAL` (200 ms on Linux; ~200 ms is the
  practical floor). We use `refresh_cpu_usage()` (or a minimal `CpuRefreshKind`) rather than full
  refreshes to keep sampling cheap. This aligns with plan/16 §6's power-adaptive sampling: the
  resource manager samples at the monitor's cadence (1 s AC, 5 s battery, 15 s constrained), never
  faster than the sysinfo floor.
- **RAM usage** comes from `sysinfo` system memory plus the per-tree Job/cgroup accounting.
- **Cores are clickable.** Toggling a core's `enabled` bit is an event (`CoreEnabled` /
  `CoreDisabled`) projected into the read model; the allocator reacts by removing/adding it to
  the pool. Disabling a core that a session is pinned to triggers a re-pin of that session to a
  remaining enabled core (or a pause, if none remain).
- **The visual is a projection.** Because the data model is event-sourced, the node view is free:
  it is the read model rendered by the GPUI UI and streamed to mobile over the wire contract,
  exactly like the orchestration dashboard (plan/06 §4.3).

### 4.6 Fleet scheduler (1-100 machines)

The fleet scheduler allocates each new session to a machine with enough free cores and RAM. The
control node (the machine running the Multiplexer server) tracks, for every known node:

- a **core bitmap** of enabled, unreserved, unpinned cores (which cores are available right now),
- a **free-RAM figure** (total minus used minus reserved), and
- a **heartbeat** (last-seen timestamp, liveness).

**Allocation algorithm.** When a session requests N cores and M bytes of RAM, the scheduler picks
a node that (a) is alive (heartbeat fresh), (b) has ≥ N free cores, and (c) has ≥ M free RAM. It
then pins the session to N of those cores and reserves M bytes. Allocation is a pure function of
the node states, so it is unit- and property-testable (see §7). The default is to prefer the
local node (lowest latency, no network), then remote nodes by free capacity.

**Assignment transport.** The control node assigns work to a remote node over the existing
JSON-RPC-over-WebSocket contract (plan/04) or gRPC for the fleet path. The remote node runs a
thin Multiplexer server instance that owns the actual process trees (containment, affinity,
limits) on that machine and reports telemetry back. This reuses the server-centric model: a
remote node is just another server instance the control node talks to. Heartbeats and telemetry
flow over the same channel.

**Scale.** The design targets 1-100 machines. The control node holds one `NodeState` per machine;
the allocator is O(nodes) per allocation, trivially fine at 100 nodes. The bottleneck is the
assignment channel, not the allocator, so the fleet path must use a connection pool with
backpressure (D18) rather than one channel per session.

### 4.7 Relationship to the MCP lifecycle supervisor (plan/21)

plan/21 and this doc are complementary and both required:

| Concern | Owned by |
|---|---|
| Which MCP servers exist, reuse key, reference counting | plan/21 |
| Restart policy (backoff), crash detection | plan/21 |
| **Containment** (every MCP server tree in a Job Object / cgroup, kill-on-close) | **this doc (24)** |
| **Limits** (max memory, max processes per MCP tree) | **this doc (24)** |
| **Affinity** (which cores an MCP server runs on) | **this doc (24)** |
| **Fleet allocation** (which machine an MCP server runs on) | **this doc (24)** |

Concretely: when plan/21 decides to spawn an MCP server, it asks the resource manager for a
contained, limited, pinned process group. When plan/21 decides to tear a server down (zero
references), it drops the group handle and the resource manager's kill-on-close reaps the whole
tree. plan/21's resource limits (§4.7 of that doc) are *enforced* by this doc's containment
primitives. The two must be designed together: plan/21 owns the lifecycle state machine, this
doc owns the OS-level enforcement.

---

## 5. Proposed design decisions (D57+)

These are proposals for `docs/DECISIONS.md`, in its style. They are **not** locked; they are
offered for the decision log.

### D57. Containment ownership: the resource manager owns all process containment (PROPOSED)
- **Decision:** A single in-process resource manager owns process containment, affinity, and
  limits for every process tree the runtime spawns (agent sessions, subagents, MCP servers,
  terminals). No other component spawns a process outside a managed container.
- **Rationale:** Extends the server-centric "runtime owns child processes" model (D1) to the OS
  level. One owner makes orphaned processes structurally impossible.

### D58. Kill-on-close containment (PROPOSED)
- **Decision:** Every process tree is created inside a Job Object (Windows) / cgroup v2 (Linux)
  with kill-on-close. When the owning handle drops, the kernel terminates the entire tree.
- **Rationale:** This is the direct, unconditional fix for the 562-process / 27.9 GB pile-up.
  Cleanup is a kernel guarantee, not a best-effort routine.

### D59. Reserve cores 0,1 for the app (PROPOSED)
- **Decision:** The core app reserves cores 0,1 for itself (UI, render thread, orchestration).
  Agent sessions are pinned to the remaining enabled cores.
- **Rationale:** Protects the `< 16 ms` input-latency gate (plan/16 §4): the frame path never
  contends with agent CPU.

### D60. Per-session affinity and memory caps (PROPOSED)
- **Decision:** Each session is pinned to dedicated cores and carries a hard memory cap (default
  4 GiB per tree, 2 GiB per process, 256 processes per tree), enforced by the containment
  primitive.
- **Rationale:** Deterministic, isolated performance; a runaway agent is killed by the kernel,
  not left to exhaust the machine.

### D61. Fleet scheduler with core bitmap + free RAM (PROPOSED)
- **Decision:** A fleet scheduler allocates each session to a node (1-100 machines) with enough
  free cores and RAM, tracked via a per-node core bitmap and free-RAM figure, over the existing
  JSON-RPC/WS contract (or gRPC for the fleet path).
- **Rationale:** Turns Multiplexer into a control surface for a whole compute fleet, a capability
  no competitor has.

### D62. Group-aware affinity for >64 cores (PROPOSED)
- **Decision:** On machines with more than 64 logical processors, affinity uses group-aware APIs
  (`GetLogicalProcessorInformationEx` + `SetThreadGroupAffinity`/`GetThreadGroupAffinity`, or CPU
  Sets), not the 64-bit single-group `SetProcessAffinityMask`. The simple path (≤ 64 cores) uses
  the `affinity`/`core_affinity` crates.
- **Rationale:** High-core machines are common in the fleet story; the classic mask APIs cannot
  address cores ≥ 64. A resource manager that silently ignores half the cores would be broken.

### D63. Crate choice: `processkit` + `win32job` (PROPOSED)
- **Decision:** Use `processkit` (https://crates.io/crates/processkit) as the primary
  containment + limits abstraction (Job Object / cgroup v2, kill-on-drop, max_memory,
  max_processes, cpu_quota), with `win32job`
  (https://github.com/ohadravid/win32job-rs) for Windows-specific control, behind a
  `Containment` trait.
- **Rationale:** `processkit` unifies containment + limits + kill-on-drop across platforms and is
  tokio-native; `win32job` gives precise Windows control where needed. Both are thin over the
  same OS primitives.

### D64. Telemetry via `sysinfo` at the monitor cadence (PROPOSED)
- **Decision:** Per-core CPU usage and RAM come from a single long-lived `sysinfo` `System`
  instance, refreshed no faster than `sysinfo::MINIMUM_CPU_UPDATE_INTERVAL` (200 ms) and at the
  resource monitor's power-adaptive cadence (plan/16 §6).
- **Rationale:** CPU usage is a delta between samples; refreshing too fast yields garbage. One
  long-lived instance keeps diffs valid and sampling cheap.

---

## 6. Security considerations

The resource manager concentrates OS-level power (kill trees, pin cores, cap memory, allocate
across machines), which is powerful and therefore security-sensitive. It follows plan/17's
principles: least privilege, fail closed, auditability.

1. **Never kill unrelated processes.** Teardown targets only the tracked process tree, via the
   Job Object / cgroup created at spawn. We never signal by PID guesswork or by pattern-matching
   process names. Kill-on-close terminates exactly the processes assigned to that job, never a
   process we did not spawn. This is the same discipline as plan/21 §7.2, made a kernel
   guarantee.
2. **Never steal cores from OS interactivity.** Cores are enabled for agent use by default, but
   the user can disable any core in the visual. A disabled core is removed from the allocator's
   pool and never assigned. The app's reserved cores (0,1) are never offered to sessions. We do
   not pin the OS or the user's other applications; we only constrain our own children.
3. **Containment before spawn.** The Job Object / cgroup is created *before* the first child is
   spawned, and the child is assigned to it before it can fork further. This closes the
   spawn/assign race so a fast-forking child cannot escape containment.
4. **Fail closed on limits.** A session that exceeds its memory or process cap is terminated by
   the kernel, not allowed to keep running. A node whose heartbeat is stale is treated as
   unavailable and never allocated work.
5. **Trust boundary.** Remote fleet nodes are semi-trusted (they run our server instance and
   enforce the same containment locally). Per D25, a remote node independently enforces its own
   permission modes and containment; the control node does not assume a remote node will police
   itself. Assignment and telemetry flow over the authenticated contract (plan/17 §4).
6. **Auditability.** Every spawn, pin, limit, kill, core toggle, and fleet allocation is an event
   in the read model, replayable for review, consistent with plan/17's auditability principle.

---

## 7. Testing strategy

The resource manager is tested under the project's TDD-at-inception gate chain (fmt → clippy →
unit+property → mutation → integration → component → e2e → coverage), per plan/15. The pure
allocator and the containment wrapper are prime mutation targets (D21, D33).

### 7.1 Unit tests

- **Bitmap allocator.** Co-located `#[cfg(test)]` modules over the core-bitmap allocator: given a
  set of enabled, unreserved cores, allocating N cores returns a valid, disjoint set; freeing
  returns them to the pool; disabling a core removes it; allocating more than available fails
  with a typed error.
- **Memory reservation.** Reserving M bytes against a node's free-RAM figure succeeds when
  available and fails when not; freeing returns the bytes.
- **Affinity policy.** Given the reserved cores (0,1) and a set of enabled cores, the session
  core set never includes a reserved or disabled core.
- **Containment wrapper.** The `Containment` trait's platform backends are exercised with a dummy
  child that we pin and limit (see §7.3); the wrapper's error paths (limit unsupported on a
  platform) return typed errors, not panics.

### 7.2 Property tests (proptest)

- **Allocator invariant:** under arbitrary sequences of allocate/free/enable/disable commands,
  the sum of cores pinned to sessions is always ≤ the number of enabled, unreserved cores. No
  core is ever assigned to two sessions at once.
- **Memory invariant:** under arbitrary reserve/free sequences, total reserved memory never
  exceeds a node's free-RAM figure.
- **No-orphan invariant:** under arbitrary spawn/stop sequences, every process tree is either
  alive inside a live containment primitive or has been reaped; no tree is left running after its
  owner drops.
- **Fleet invariant:** under arbitrary node-join/leave and allocation sequences, work is never
  assigned to a dead (stale-heartbeat) node or a node without enough free cores/RAM.

### 7.3 Integration tests (real core + dummy child)

- **Pin and limit a dummy child:** spawn a CPU-bound dummy child, pin it to a specific core, and
  assert (via `sysinfo`) that it runs on that core and is capped by its memory limit.
- **Kill-on-close reaps the tree:** spawn a dummy child that forks grandchildren, drop the
  containment handle, and assert the entire tree is gone (no orphans). This is the direct
  regression test for the original problem.
- **Memory cap fires:** spawn a dummy child that allocates past its cap and assert it is
  terminated by the kernel.
- **Core toggle:** disable a core in the data model and assert the allocator stops assigning it
  and re-pins any session that was on it.
- **Fleet allocation:** simulate several nodes with different free cores/RAM and assert the
  scheduler picks the correct node for a given request.

### 7.4 Mutation testing

cargo-mutants over the bitmap allocator, memory reservation, affinity policy, and containment
wrapper. CI gates: ≥85% line, ≥80% branch, ≥70% mutation score killed (D21, D33). The allocator
is a prime mutation target: a killed mutant here means a real correctness regression (double-
assigning a core, exceeding free RAM) is caught.

### 7.5 E2E

Drive the real app headless; assert that after opening and closing several sessions, the process
count returns to baseline (no accumulated fleet). This is the direct regression test for the
562-process / 27.9 GB problem, and it complements plan/21 §8.5 (which asserts the same for MCP
servers specifically).

---

## 8. Open questions / risks

These are flagged, not decided here:

1. **Default limits.** The per-session memory (4 GiB), per-process (2 GiB), and process-count
   (256) defaults are proposals. The right numbers depend on real resource-monitor data
   (plan/16 §6) and should be tuned, not assumed.
2. **Affinity vs oversubscription.** Pinning sessions to dedicated cores gives isolation but can
   underutilize a machine (a pinned session idles its cores while another session is CPU-bound).
   Whether to offer an "oversubscribe" mode (allow sharing) is a product decision.
3. **Fleet transport.** Whether the fleet path uses the existing JSON-RPC/WS contract or gRPC is
   open; the choice affects latency, connection pooling, and backpressure (D18) for 100-node
   scale.
4. **Remote-node trust and auth.** How a remote node authenticates to the control node, and how
   much authority it is granted, needs a decision consistent with plan/17 §4 and D25.
5. **Windows processor-group coverage.** The group-aware affinity path (>64 cores) is real but
   adds platform complexity; whether it ships in MVP or is phased is a roadmap decision
   (plan/19).
6. **Interaction with plan/21 limits.** plan/21 proposes its own max-process/max-RAM defaults for
   MCP servers. These must reconcile with this doc's per-tree caps so the two do not conflict
   (the tighter cap wins, but the defaults should be coherent).
7. **cgroup v2 prerequisites on Linux.** `processkit` limits require running at the real cgroup
   v2 hierarchy root (not under systemd session/scope/service). How we handle machines where that
   prerequisite is not met (fall back to process-group containment with no limits) needs a
   decision.

**Flagged consistency note:** this doc is consistent with PLAN-CONTEXT (server-centric runtime,
event-sourced orchestration, bounded channels, secrets session-cache model, Windows-first) and
with plan/16 (perf gates, resource monitor, power-adaptive sampling) and plan/21 (MCP lifecycle).
If any locked decision flips (e.g. stack, crate layout), the affected sections (§4, §5) must be
revisited.