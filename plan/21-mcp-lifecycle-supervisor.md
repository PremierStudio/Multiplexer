# 21: MCP Lifecycle Supervisor

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Orchestration / Core runtime
**Depends on:** `02-architecture.md`, `03-vendored-grok-build.md`, `06-orchestration-engine.md`, `17-security-and-secrets.md`
**Feeds:** `15-testing-strategy.md`, `16-performance.md`, `20-risks-and-open-questions.md`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context). Where a decision is not yet settled, it is listed under **Open questions** and is
**not** decided unilaterally here. New decisions proposed here are numbered **D41+** in the
style of `docs/DECISIONS.md`; they are proposals for the decision log, not locked decisions.

**Locked decisions applied (D1, D11, D13, D18, D23):** This doc reflects the locked decisions
from `docs/DECISIONS.md`:
- **D1** : Rust + GPUI, single native server binary; the supervisor is a component of that
  binary, not a sidecar.
- **D11** : Multiplexer owns child-process scheduling; the supervisor extends that ownership to
  MCP server processes.
- **D13** : consolidated `multiplexer-*` crate layout; the supervisor lives in
  `multiplexer-core` (state machine) with process plumbing in `multiplexer-server`.
- **D18** : bounded channels with backpressure; MCP tool-result ingestion follows the same rule.
- **D23** : secrets session-cache model; MCP server env/headers reference secrets via the same
  mechanism, never raw values in configs.

---

## 1. Problem statement

Grok CLI sessions spawn their full MCP server fleet at startup and never tear it down on exit.
Each session restart spawns a fresh copy of every configured MCP server rather than reusing
running ones. The result is unbounded process and memory accumulation: **N sessions = N copies
of each server.**

A live diagnosis on this machine (2026-08-12) found:

- **8 grok.exe sessions** holding **101 node processes / ~10.4 GB RAM**.
- With more accumulated sessions the user has observed **562 processes / ~27.9 GB RAM**.
- Each **npx-based** server costs **2 node processes** (the `npx-cli.js` wrapper plus the actual
  server), doubling the count.
- The observed servers: `mcp-linear`, `context7`, `shadcn`, `chrome-devtools-mcp`,
  `mcp-remote` (Atlassian), `mcp-mailtrap`. Some are npx-based; some are remote URL servers that
  materialize as local `mcp-remote` node proxies.

This is not a one-off. It is the direct consequence of a client that treats MCP servers as
per-session, per-process, unmanaged children. Every session restart leaks a full fleet. The
problem compounds with the "dozens of concurrent subagents" workload Multiplexer targets: each
subagent that inherits MCP servers (grok-build's default `mcpInheritance: all`) multiplies the
fleet further.

---

## 2. Why this is a product feature, not just a bug fix

MCP 2.0 introduces statelessness, and the natural reaction is "the process pile-up is a legacy
problem that will disappear." That reaction is wrong, and it is the core insight behind making
this a first-class feature:

1. **Most MCP servers will NOT adopt statelessness.** The MCP ecosystem is dominated by
   stateful, process-bound servers (npx stdio servers, `mcp-remote` proxies, local tool
   daemons). Statelessness is a spec capability, not a migration that the installed base will
   make. A client that assumes statelessness will leak processes for years.
2. **Even stateless servers benefit from lifecycle management.** A stateless server still costs a
   process and memory while running. Reuse, teardown, supervision, and resource limits apply
   regardless of whether the server holds state.
3. **The competitive value is real and durable.** No major client (Grok CLI, Claude, Cursor,
   Orca, T3 Code) manages MCP server lifecycle well. A client that (a) reuses running servers
   across sessions, (b) tears down child processes on session exit, and (c) supervises and
   restarts crashed servers is measurably better on the two axes users feel most: memory and
   reliability. This is a differentiator that survives MCP 2.0 adoption because it is about
   *process ownership*, not protocol version.

This maps directly onto Multiplexer's core architecture: a server-centric runtime where a single
native binary owns agent processes, terminals, git, fs, checkpoints, and HAR. The MCP supervisor
is the natural extension of that "runtime owns child processes" model. It is not a new concept;
it is the same ownership discipline applied to MCP servers.

---

## 3. Design goals

1. **Reuse running servers across sessions.** A server configured once is spawned once and shared
   by every session that needs it. No per-session duplicate fleet.
2. **Tear down on session exit.** When the last session using a server ends, the server is
   stopped and its process tree is reaped. No orphans.
3. **Supervise and restart crashed servers.** A crashed server is detected and restarted with
   backoff, so a transient failure does not permanently disable a tool.
4. **Resource limits.** Max server processes, max aggregate RAM, and an idle timeout bound the
   fleet even when many servers are configured.
5. **No orphaned processes.** Every MCP child process is tracked and reaped on exit, crash, or
   idle timeout. On Windows this uses Job Objects so a runaway server tree is terminated with the
   server.

---

## 4. Proposed architecture

The supervisor is a component of the server runtime. It owns every MCP child process, tracks it
in the read model, reuses it across sessions, and reaps it. It sits alongside the orchestration
engine and the parallel scheduler (plan/06): the scheduler owns subagents, the supervisor owns
MCP servers. Both are "runtime owns child processes" surfaces.

### 4.1 Placement in the runtime

```
┌───────────────────────────────────────────────────────────────┐
│                     MULTIPLEXER SERVER                        │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  ORCHESTRATION ENGINE (event-sourced, plan/06)          │  │
│  │  command queue → decider → projector → SQLite read model│  │
│  └───────────────┬─────────────────────────────────────────┘  │
│                  │  MCP tool calls / results (bounded, D18)   │
│  ┌───────────────▼─────────────────────────────────────────┐  │
│  │  MCP LIFECYCLE SUPERVISOR                               │  │
│  │  ┌────────────┐ ┌────────────┐ ┌──────────────────────┐ │  │
│  │  │ Server     │ │ Process    │ │ Reuse / Reap /       │ │  │
│  │  │ Registry   │ │ Table      │ │ Restart (backoff)    │ │  │
│  │  └────────────┘ └────────────┘ └──────────────────────┘ │  │
│  └───────────────┬─────────────────────────────────────────┘  │
│                  │  spawn / signal / reap (Job Objects on Win)│
│  ┌───────────────▼─────────────────────────────────────────┐  │
│  │  MCP SERVER PROCESSES (npx, mcp-remote, http proxies)   │  │
│  └─────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

The supervisor is not a sidecar. It is in-process with the server, so it shares the read model,
the secrets session cache (D23), and the resource monitor. It is the single owner of MCP child
processes; no other component spawns or kills them.

### 4.2 Server registry

The registry is the set of *configured* servers, resolved from the same sources grok-build reads:
`~/.grok/config.toml` `[mcp_servers.<name>]`, project `.grok/config.toml`, `.mcp.json`, and
plugin/Claude/Cursor compat sources (per `third_party/grok-build` docs). Each entry carries:

| Field | Meaning |
|---|---|
| `name` | unique server id |
| `identity` | the reuse key (see §4.4) |
| `transport` | `stdio` (npx/command) or `http`/`sse` (remote URL) |
| `command` / `url` | how to start or reach it |
| `env` / `headers` | resolved via the secrets session cache (D23), never raw values |
| `scope` | user / project / plugin / managed |
| `enabled` | on/off, plus the disabled list grok-build persists |

The registry is a projection of config, not a separate store. Config changes (add/remove/enable/
disable) are events that the supervisor reacts to, matching grok-build's hot-reload behavior.

### 4.3 Process table and lifecycle states

The supervisor keeps a process table in the read model. Each live server instance has a state:

```
spawned ──▶ ready ──▶ stopped
   │           │
   │           └──▶ crashed ──▶ (backoff) ──▶ spawned
   └──▶ crashed (startup failure) ──▶ (backoff) ──▶ spawned
```

| State | Meaning |
|---|---|
| `spawned` | process launched, MCP handshake not yet complete |
| `ready` | handshake done, tools listed, usable by sessions |
| `crashed` | process exited unexpectedly, or handshake failed |
| `stopped` | torn down deliberately (session exit, idle timeout, limit) |

Transitions are events (`McpServerSpawned`, `McpServerReady`, `McpServerCrashed`,
`McpServerStopped`) projected into the read model, consistent with plan/06's event-sourced
model. The dashboard shows the fleet as a projection, for free.

### 4.4 Reuse policy

Reuse is keyed by **server identity**, not by session. The identity is a stable hash of the
server's config: `name`, `transport`, `command`/`url`, and the resolved `env`/`headers` (the
config hash). Two sessions that configure the same server with the same identity share one
process.

- **Reference counting.** Each session holds a reference to the server instances it uses. The
  supervisor increments on attach, decrements on detach. When the count reaches zero, the server
  is eligible for teardown (or idle timeout, whichever comes first).
- **Config-hash mismatch.** If a server's config changes (e.g. a different `url`), the identity
  changes, so a new instance is spawned and the old one is drained and stopped. This prevents
  silently reusing a server with stale configuration.
- **Stateless reuse.** For MCP 2.0 stateless servers, reuse is trivial and safe. For legacy
  stateful servers, reuse is still correct because the server is shared the way a daemon is
  shared: sessions multiplex over one process. This is the same model a database connection pool
  uses, and it is the entire point of the feature (see §6).

### 4.5 Teardown on session exit

When a session stops (plan/06 `SessionStop`), the supervisor decrements its references. A server
whose reference count reaches zero is stopped: the process is signaled, its process tree is
reaped (Job Object on Windows, process group on Unix), and its state becomes `stopped`. This is
the direct fix for the orphaned-fleet problem: no session exit leaves a server behind.

### 4.6 Crash detection and restart with backoff

The supervisor monitors each `ready` server's process. On unexpected exit, or on a handshake
failure at startup, the server transitions to `crashed` and is restarted with **exponential
backoff** (e.g. 1s, 2s, 4s, capped at 30s, with jitter). A server that crashes repeatedly past a
threshold (e.g. 5 consecutive failures) is marked `stopped` and surfaced to the user as
permanently failed, rather than restart-looping forever. This mirrors grok-build's own
auto-recovery for HTTP MCP servers but generalizes it to stdio servers and adds backoff.

### 4.7 Resource limits

| Limit | Default (proposal) | Behavior |
|---|---|---|
| Max server processes | 32 | new spawns beyond the cap are queued, not dropped |
| Max aggregate RAM | configurable (e.g. 4 GB) | the supervisor stops the least-recently-used idle server to stay under budget |
| Idle timeout | 10 min | a `ready` server with zero references is stopped after the timeout |

The resource monitor (plan/16) feeds the RAM figure; the supervisor enforces the limit. These
numbers are proposals to be tuned with real data, not assumed (see §9).

---

## 5. Key design decisions (proposed D41+)

These are proposals for `docs/DECISIONS.md`, in its style. They are **not** locked; they are
offered for the decision log.

### D41. MCP lifecycle ownership: the supervisor owns all MCP child processes (PROPOSED)
- **Decision:** A single in-process MCP lifecycle supervisor in the server owns every MCP child
  process: spawn, reuse, supervise, reap. No other component spawns or kills MCP servers.
- **Rationale:** Extends the server-centric "runtime owns child processes" model (D1) to MCP.
  One owner prevents the orphaned-fleet failure mode by construction.

### D42. Reuse key: server identity / config hash (PROPOSED)
- **Decision:** Reuse is keyed by a stable hash of server config (`name`, `transport`,
  `command`/`url`, resolved `env`/`headers`), not by session. Sessions reference-count shared
  instances.
- **Rationale:** Two sessions with the same server config share one process; a config change
  yields a new identity and a fresh instance, never stale reuse.

### D43. Teardown on zero references (PROPOSED)
- **Decision:** A server is stopped when its reference count reaches zero (session exit) or its
  idle timeout elapses, whichever comes first. Its process tree is reaped (Job Object on
  Windows).
- **Rationale:** This is the direct fix for orphaned processes. No session exit leaves a server
  behind.

### D44. Crash restart with backoff (PROPOSED)
- **Decision:** Crashed servers restart with exponential backoff (1s to 30s, jittered), capped at
  5 consecutive failures before being marked permanently failed and surfaced to the user.
- **Rationale:** Transient failures recover automatically; a permanently broken server does not
  restart-loop.

### D45. Resource limits (PROPOSED)
- **Decision:** Enforce max server processes, max aggregate RAM, and an idle timeout, fed by the
  resource monitor. Over-budget, stop the least-recently-used idle server.
- **Rationale:** Bounds the fleet even with many configured servers; prevents the 562-process /
  27.9 GB failure mode from ever recurring.

---

## 6. MCP 2.0 consideration

The design degrades gracefully across MCP versions because it is built on **process ownership**,
not protocol statefulness:

- **Legacy stateful servers** (the installed base): reuse is safe because sessions multiplex over
  one shared process, exactly like a daemon or a connection pool. Teardown, supervision, and
  limits all apply.
- **MCP 2.0 stateless servers**: reuse is trivially safe, and the same lifecycle applies. The
  only difference is that a stateless server could in principle be torn down more aggressively
  (e.g. shorter idle timeout), which is a per-server policy knob, not a different architecture.
- **No protocol dependency.** The supervisor never inspects whether a server is stateful. It
  treats every server as a process to own. If MCP 2.0 adoption is slow (the expectation), the
  feature is still fully valuable; if adoption is fast, the feature still pays for itself on
  memory and reliability.

The one MCP-2.0-specific consideration is **session multiplexing semantics**: a shared server
must tolerate concurrent sessions issuing tool calls. MCP already supports concurrent requests
over one transport, so this is not a new requirement; the supervisor only needs to ensure it
does not serialize sessions onto a shared server in a way that breaks per-session isolation (see
§7).

---

## 7. Security considerations

The supervisor concentrates process ownership, which is powerful and therefore security-
sensitive. It follows plan/17's principles: least privilege, fail closed, auditability.

1. **Process isolation.** Each MCP server runs as its own process with the environment resolved
   from the secrets session cache (D23). No raw secrets in configs; env/headers reference the
   session cache. A server sees only its own env, never other servers' or the server's own
   secrets.
2. **Avoid killing unrelated processes.** Teardown targets only the tracked process tree. On
   Windows this uses a Job Object created at spawn, so we kill exactly the server's descendants,
   never a process we did not spawn. We never signal by PID guesswork or by pattern-matching
   process names.
3. **Safe teardown.** Stop is graceful-first (signal, allow cleanup), then forced (Job Object
   terminate) after a short grace period. A server that ignores the signal is terminated, not
   left running.
4. **Not nuking shared servers.** Because reuse is reference-counted, a server shared by several
   sessions is never torn down while any session still references it. Teardown only fires at zero
   references, so one session's exit cannot kill another session's tools.
5. **Trust boundary.** MCP servers are untrusted code (they run arbitrary commands via npx).
   They are confined to their own process and their own env; they do not inherit the server's
   ambient authority. Permission modes (plan/17 §7) still gate MCP tool calls; the supervisor
   does not bypass approval gating.
6. **Auditability.** Every spawn, reuse, crash, restart, and teardown is an event in the read
   model, replayable for review, consistent with plan/17's auditability principle.

---

## 8. Testing strategy

The supervisor is tested under the project's TDD-at-inception gate chain (fmt → clippy → unit+
property → mutation → integration → component → e2e → coverage), per plan/15.

### 8.1 Unit tests (state machine)

Co-located `#[cfg(test)]` modules over the lifecycle state machine. For each transition:
- **Happy path:** `spawned → ready → stopped`; `spawned → crashed → (backoff) → spawned`.
- **Invalid transitions:** the supervisor rejects illegal transitions (e.g. `stopped → ready`
  without a respawn) with a typed error.
- **Reference counting:** attach/detach increments and decrements correctly; teardown fires only
  at zero references.
- **Backoff:** the backoff schedule is deterministic and testable (inject a clock).

### 8.2 Property tests (proptest)

- **Lifecycle invariants:** under arbitrary sequences of attach/detach/spawn/crash/stop commands,
  no server is ever `ready` with zero references and not scheduled for teardown; no server is
  ever torn down while referenced; the process table never disagrees with the event log.
- **Reuse identity:** proptest over config variations asserts that identical configs hash to the
  same identity and differing configs hash differently.
- **Resource limits:** under arbitrary spawn/stop sequences, max processes and max RAM are never
  exceeded.

### 8.3 Integration tests (real core + mock MCP server)

- **Spawn/reuse/reap:** start two sessions against the same mock MCP server; assert one process
  is spawned, both attach; stop one session, assert the server stays; stop the second, assert the
  server is reaped.
- **Crash/restart:** kill the mock server process; assert it is detected, restarted with backoff,
  and returns to `ready`.
- **Teardown on exit:** stop a session; assert its server's process tree is gone (no orphans).
- **Resource limit:** configure a low RAM cap; assert the supervisor stops an idle server to stay
  under budget.
- **Real-binary smoke tests** with a real npx MCP server when available (CI-optional).

### 8.4 Mutation testing

cargo-mutants over the state machine, reference counter, and backoff logic. CI gates: ≥85% line,
≥80% branch, ≥70% mutation score killed (D21, D33). The state machine is a prime mutation target.

### 8.5 E2E

Drive the real app headless; assert that after opening and closing several sessions, the process
count returns to baseline (no accumulated fleet). This is the direct regression test for the
original problem.

---

## 9. Open questions / risks

These are flagged, not decided here:

1. **Default limits.** The max-process (32), max-RAM (4 GB), and idle-timeout (10 min) defaults
   are proposals. The right numbers depend on real resource-monitor data (plan/16) and should be
   tuned, not assumed.
2. **Reuse vs isolation tension.** Sharing a stateful server across sessions is the point, but a
   user may want per-session isolation for a specific server (e.g. one that holds a session-
   scoped credential). Whether to expose a per-server "isolate" override is a product decision.
3. **Session multiplexing semantics.** A shared server must tolerate concurrent tool calls from
   multiple sessions. MCP supports this, but per-session isolation of results (which session's
   call produced which result) must be verified, not assumed.
4. **Interaction with grok-build's own MCP management.** grok-build is centralizing MCP
   management server-side (gateway catalog, per `docs/UPSTREAM-TRAJECTORY.md`). How our
   supervisor reconciles with managed/gateway servers, and with grok-build's own auto-recovery,
   needs a decision as upstream evolves (track via D31).
5. **Windows Job Object vs Unix process groups.** Teardown semantics differ by platform; the
   exact signal/terminate sequence and grace period need platform-specific tests.
6. **MVP scope.** Whether the full supervisor (reuse + supervision + limits) ships in MVP or is
   phased (e.g. teardown first, reuse second, supervision third) is a roadmap decision for
   plan/19.

**Flagged consistency note:** this doc is consistent with PLAN-CONTEXT (server-centric runtime,
event-sourced orchestration, bounded channels, secrets session-cache model). If any locked
decision flips (e.g. stack, crate layout), the affected sections (§4, §5) must be revisited.