# 13 — Server RPC vs desktop

**Date:** 2026-08-12
**Scope:** Wire method constants vs `Server::dispatch` vs desktop `rpc()` / inspector actions. Which unused methods can get a thin honest UI this wave (plan/36).
**Sources:** `crates/multiplexer-wire/src/methods.rs`, `crates/multiplexer-server/src/server.rs` (`dispatch`), `apps/multiplexer-desktop/src/main.rs` (`rpc()`, `handle_frame`, `host_action`, `inspector_click`), `crates/multiplexer-shell/src/bindings.rs`, `apps/multiplexer-desktop/src/inspector.rs`, `plan/36-feature-gap-ui.md`, `plan/04-wire-contract.md` §4.
**Method:** Read-only. No cargo.

FINDINGS: 10

## Verdict

The wire file names **71** constants. The server router implements **18**. The desktop `rpc()` helper is called for **6** of those 18. The other 12 wired methods have tests and handlers and no GPUI caller. The remaining **53** constants (including the `event` notification name) return `method not found`.

The desktop is not a JSON-RPC client in the plan/04 sense. It constructs `Server::with_local()` in-process and posts frames through `handle_frame`. `host_call` already maps several actions to `HostCall::Rpc`, then `dispatch` throws the method string away and reimplements the same calls in `host_action`. Send starts a session over RPC, then runs `grok -p` on a worker thread. That is the largest honesty gap: the UI looks like a live agent session and never calls `turn.send`.

Plan/36 already said this. It is still true.

---

## Inventory

### Counts

| Bucket | N | Meaning |
|---|---|---|
| Wire constants in `methods.rs` | 71 | Includes `event` plus 70 client/control names |
| Server `dispatch` arms | 18 | Everything else is `method not found` |
| Desktop unique `rpc()` methods | 6 | Seven call sites; `git.worktrees` is used twice |
| Wired and unused by desktop | 12 | Handler exists; UI never posts the frame |
| Unwired constants | 53 | No router arm (52 requests + `event`) |

`event` is a server-push method name, not a client request. It is listed under unwired because `dispatch` never accepts it as a request. The desktop also never decodes notification frames from `handle_frame` except the specific result parsers (`session_id_from`, `worktree_paths`, `checkpoint_from`, `first_error`).

### Wired and used

Desktop `rpc()` + `handle_frame` only. Shell `host_call` strings that the desktop discards do not count as used.

| Method | Desktop site | What the UI actually does |
|---|---|---|
| `session.start` | `ensure_session` | First Send. Stores `session_id`, marks `Workspace` connected. Turns still go to `spawn_grok_turn`. |
| `session.interrupt` | `interrupt` | Stop / `/stop` / Ctrl+.. Also sets `ignore_turn` so the `grok -p` worker is dropped, not killed through the server. |
| `approval.respond` | `respond_approval` | Allow / Deny. Only fires if `pending_approval` is set. Desktop never ingest events, so this path is dead in the running app. |
| `git.worktrees` | `refresh_worktrees`, `refresh_reminder` | Git Reload, boot, and after a turn. Paths only. Second path becomes the reminder bar. |
| `checkpoint.create` | `create_checkpoint` | Points New, `/cp`, Ctrl+S. Label always `"manual"`. On RPC error, fabricates a local row. |
| `checkpoint.revert` | `revert_checkpoint` | Points Revert. Pointer move only (see audit 18). |

### Wired and unused

Router arm exists. No desktop `rpc()` call.

| Method | Server truth | Why the UI looks related anyway |
|---|---|---|
| `session.list` | Returns `{ sessions }` | Left **Agents** paints `session_id` from `session.start`, or "No live session / Send a turn to start". Never lists. |
| `session.get` | Snapshot | Session tab shows a locally cached id. |
| `session.stop` | Stops the backend session | New chat / Delete set `session_id = None` and leave the server session alive. |
| `turn.send` | `backend.send_turn`, drains events | Composer Send, chips, palette Send. All use `spawn_grok_turn` (`grok -p`). |
| `git.worktree.create` | Real catalog create (`cwd`, `path`, `branch`, `create_branch`) | Git **New WT** pastes `git worktree add ../mux-feat -b feat` into the composer. |
| `checkpoint.list` | Lists the in-memory catalog | Points tab is a local `Vec<CheckpointRow>` seeded at boot, never refreshed from list. |
| `terminal.create` | `TerminalHub` id + spec | Term strip is `spawn_command` / `cmd.exe /C`. Hub is an in-memory stub (no PTY). |
| `terminal.list` | Hub ids + alive flags | Term tab dumps `terminal_log`. |
| `terminal.input` | Bytes into the hub buffer | Term Enter runs a one-shot command. |
| `terminal.kill` | Hub kill | No kill control. A second command is refused while `pending_cmd` is set. |
| `system.ping` | `{ pong: true }` | Session **Connection** is `Workspace.connect` after `session.start`, not a hello/ping. |
| `system.hello` | `server_info` + `protocol_version` | Same. No handshake. No `protocol_version` check from the client. |

### Unwired constants

Calling any of these today returns `method not found` (`server.rs` `_` arm).

| Namespace | Constants | Plan/36 this wave? |
|---|---|---|
| `event` | `event` | No. Notification name. Desktop does not subscribe. |
| `turn.*` leftover | `turn.cancel`, `turn.history` | No. Turns are `grok -p`. |
| `approval.list` | `approval.list` | No. Approval bar exists; inbound events do not. |
| `userInput.*` | `userInput.respond`, `userInput.cancel` | No. No user-input chrome. |
| `checkpoint.*` leftover | `checkpoint.diff`, `checkpoint.apply` | No. Engine has no refs (audit 18). |
| `terminal.*` leftover | `terminal.resize`, `terminal.attach` | No. No PTY. |
| `fs.*` | `fs.read`, `fs.write`, `fs.list`, `fs.watch`, `fs.unwatch`, `fs.stat` | No. Files uses local `list_project_tree`. |
| `git.*` leftover | `git.status`, `git.diff`, `git.commit`, `git.branches`, `git.checkout` | No. Status is `cmd.exe /C git status`. |
| `browser.*` | `list`, `launch`, `navigate`, `cdp`, `close`, `screenshot` | No. No tab. Later (CDP). |
| `har.*` | `start`, `stop`, `replay`, `list` | No. No tab. Later. |
| `orchestration.*` | `spawn`, `subscribe`, `unsubscribe`, `list` | `list` stub only. Spawn is later. |
| `model.*` | `list`, `select`, `get` | `list` / `select` stub. Cycle is local today. |
| `remote.*` | `list`, `connect`, `disconnect` | `list` stub. Connect/Serve later. |
| `auth.*` | `providers`, `login`, `status`, `logout` | No. No account. |
| `telemetry.*` | `usage`, `resources`, `subscribe` | `usage` echo stub. Cores is `sample_cores`. |
| `system.capabilities` | `system.capabilities` | No. Hello already unused. |
| subscription | `subscribe`, `unsubscribe`, `attach_stream`, `stream.ack` | No. In-process frames, no WS client. |

Plan/36 §1 named a shorter unwired list (`model.*`, `telemetry.usage/resources`, `orchestration.*`, `remote.*`, `fs.list/read`, `browser.*`, `har.*`, `auth.*`, `git.status/diff`). The table above is the full leftover. The extra names are real constants, not implied.

---

## This wave: unused that can get a thin honest UI

Rule from plan/36: UI on an existing engine or a labeled stub. No PTY. No CDP. No in-process grok. No toast that says a missing method is ready.

### Do this wave (honest, cheap)

| Method | Why it is honest now | UI |
|---|---|---|
| `git.worktree.create` | Already dispatched and implemented | Git **Create** with path / branch / create-branch. Plan/36 §4.3. Replace New WT as primary. |
| `checkpoint.list` | Already dispatched | After create/revert, reload Points from the catalog instead of only appending a local row. Do not add a fake diff. |
| `session.stop` | Already dispatched | New / Delete should stop the server session they abandon. |
| `session.list` / `session.get` | Already dispatched | Session tab and left Agents can show the real list, labeled as server sessions, not subagents. |
| `system.hello` / `system.ping` | Already dispatched | Session connection line: hello once, ping on refresh. Failures stay "disconnected". |
| `model.list` / `model.select` | Unwired; plan/36 allows a stub | Session picker. Catalog from static + optional `[model.*]`. Select by id, not only cycle. |
| `telemetry.usage` | Unwired; plan/36 allows an echo | Session Usage block from local `record_turn`. Account: `local / not signed in`. No dollars. |
| `orchestration.list` | Unwired; plan/36 prefers a labeled empty | Agents tab already says "Local threads only." Stub `{ subagents: [] }` so the client path exists. Do not animate fan-out. |
| `remote.list` | Unwired; plan/36 allows detect | Settings / Session: `local` + `tailscale` detected/not found. No Serve, no tickets. |

### Do not this wave (would be a lie)

| Method | Why not |
|---|---|
| `turn.send` | Wired, but Send is `grok -p`. Routing through `turn.send` is a product path change, not a thin stub. Plan/36 keeps `grok -p`. |
| `terminal.create/list/input/kill` | Hub records input in RAM. Plan/08 stub, no PTY. Term strip is honest as a one-shot `cmd.exe`. Do not relabel it as `terminal.*`. |
| `browser.*` / `har.*` | No CDP. No tab. |
| `fs.*` | Files tree is local. Do not invent `fs.list`. |
| `git.status` / `git.diff` | Status button already shells out. A stub RPC that shells again is not a new surface. |
| `auth.*` | No account. |
| `orchestration.spawn` / `remote.connect` | Engines do not exist. |
| `checkpoint.diff` / `checkpoint.apply` | No git refs. |
| `telemetry.resources` / `telemetry.subscribe` | Cores already sample locally. A subscribe wall would overclaim. |
| `subscribe` / `attach_stream` / `stream.ack` | No listening client. |

---

## UI that pretends an RPC exists

These controls look like wire methods. They do not call them.

| UI | Looks like | Actually |
|---|---|---|
| Composer Send, chips, palette Send | `turn.send` on the session from `session.start` | `spawn_grok_turn` / `grok -p`. Session id is unused for the turn. |
| Stop / `/stop` | `session.interrupt` stops the agent | RPC plus `ignore_turn`. The CLI child is not the session backend. |
| Term strip Run / Term tab | `terminal.*` | `windows_cmd` one-shot. Builtins are tab switches. |
| Git **Status** | `git.status` | `run_shell("git status")`. Dump lands in `git_status` and a `git:status` row. |
| Git **New WT** | `git.worktree.create` | Composer paste. Enter sends the line as a chat turn unless the user retypes it in Term. |
| Session **Model** / `/model` | `model.select` | `workspace.cycle_model()` over three hardcoded ids. |
| Cores **Reload** | `telemetry.resources` | `sample_cores` in-process. |
| Files tree | `fs.list` | `list_project_tree` at boot. Honest if labeled local. |
| Left **Agents** | `session.list` / `orchestration.list` | `connection.session_ids` from the last `session.start`. Copy: "Send a turn to start." |
| Inspector **Agents** | orchestration dashboard | Local threads. Body copy is honest. |
| Allow / Deny | live `approval.respond` | RPC is coded. Nothing in desktop ever `set_pending_approval`. |
| Session **Connection: connected** | `system.hello` succeeded | `Workspace.connect(vec![id])` after parsing `session.start`. |
| MCP Start / Stop | a supervisor RPC | Local `McpLife` projection. Honest if the tab still says supervised in-process. There is no `mcp.*` wire method. |

`host_call` maps Interrupt, CreateCheckpoint, RestoreCheckpoint, RefreshGit, Approve, Deny to `HostCall::Rpc`. Desktop:

```
HostCall::NeedsHost | HostCall::Rpc { .. } => { self.host_action(action, cx); }
```

The method string and `params_json` are never sent. The six live RPCs are handwritten in `host_action` / helpers. `Send` is `NeedsHost` and never becomes `turn.send`.

---

## Findings

### F1. Send never calls `turn.send`

- **Severity:** Critical
- **Where:** `apps/multiplexer-desktop/src/main.rs` (`send`, `ensure_session`, `spawn_grok_turn`), `crates/multiplexer-server/src/server.rs` (`TURN_SEND`)
- **Plan:** plan/04 §4.2; plan/36 "Agent path is `spawn_grok_turn` (`grok -p`)"
- **Evidence:** `send()` calls `ensure_session()` (`session.start`), then `spawn_grok_turn(TurnRequest { program: "grok", ... })`. Grep of `apps/multiplexer-desktop` finds no `TURN_SEND` and no `"turn.send"`. Server tests exercise `turn.send` and event drain. The session id is stored and shown. It is not an argument to the turn.
- **Why it matters:** The product story is server-centric turns. The running desktop is a session handle plus a side-channel CLI. Interrupt can only ignore the worker. Approvals and turn events have nothing to attach to.
- **This wave:** Do not add a fake `turn.send` toast. Keep `grok -p` or route Send through the existing handler. Those are different jobs. The UI should not imply the session owns the turn until one of them is true.

### F2. `host_call` Rpc is discarded

- **Severity:** Major
- **Where:** `apps/multiplexer-desktop/src/main.rs` (`dispatch`), `crates/multiplexer-shell/src/bindings.rs`
- **Evidence:** `dispatch` matches `HostCall::Rpc { .. }` and calls `host_action` with the `ClientAction` only. Bindings already format `session.interrupt`, `checkpoint.create`, `checkpoint.revert`, `git.worktrees`, `approval.respond`. Desktop rebuilds the same JSON with `rpc()`. `host_call` unit tests are the only consumers of those strings. Bindings also assert RefreshGit is **not** `git.worktree.create`.
- **Why it matters:** The shell claims to be "the single map" so the desktop must not inline method strings. The desktop inlines every live method. New this-wave RPCs (`git.worktree.create`, `model.select`) will drift the same way unless `dispatch` posts `call.method` / `call.params_json`.
- **This wave:** When Create / SelectModel land, send the `HostCall::Rpc` payload. Do not add a third copy of the JSON.

### F3. Twelve wired methods have no desktop caller

- **Severity:** Major
- **Where:** `crates/multiplexer-server/src/server.rs` 129–147 vs `apps/multiplexer-desktop/src/main.rs` `handle_frame` sites
- **Evidence:** Dispatch: `session.{start,list,get,stop,interrupt}`, `turn.send`, `approval.respond`, `git.worktrees`, `git.worktree.create`, `checkpoint.{list,create,revert}`, `terminal.{create,list,input,kill}`, `system.{ping,hello}`. Desktop unique methods: `session.interrupt`, `git.worktrees`, `checkpoint.create`, `checkpoint.revert`, `approval.respond`, `session.start`. Unused: `session.list`, `session.get`, `session.stop`, `turn.send`, `git.worktree.create`, `checkpoint.list`, `terminal.create`, `terminal.list`, `terminal.input`, `terminal.kill`, `system.ping`, `system.hello`.
- **Why it matters:** The server surface the UI can tell the truth about is larger than the six calls. Points never lists. New chat never stops. Git never creates. Term never talks to the hub. Connection never hellos.
- **This wave:** Wire the "Do this wave" table. Leave `turn.send` and `terminal.*` unlabeled as RPC.

### F4. `git.worktree.create` is live and the button is a hint

- **Severity:** High
- **Where:** `crates/multiplexer-server/src/worktree_create.rs`, `apps/multiplexer-desktop/src/inspector.rs` (`NewWorktreeHint`), `apps/multiplexer-desktop/src/main.rs` (`inspector_click`)
- **Plan:** plan/36 row E / §4.3
- **Evidence:** Server `parse_create` requires `cwd`, `path`, `branch`, optional `create_branch`. Desktop New WT sets the composer draft to a `git worktree add` string and focuses the composer. Composer Enter is a chat turn. `ClientAction` has no `CreateWorktree`. Bindings tests refuse to map RefreshGit to create.
- **Why it matters:** This is the cheapest unused wired method. The catalog and the worktree crate already create. The UI is the only missing piece.
- **This wave:** Yes. Primary **Create**. Keep New WT as a secondary copy-command if tests need it.

### F5. Term chrome is not `terminal.*`

- **Severity:** High
- **Where:** `apps/multiplexer-desktop/src/main.rs` (`run_shell`, `run_terminal_draft`), `crates/multiplexer-server/src/terms.rs`, `crates/multiplexer-terminal/src/lib.rs`
- **Plan:** plan/36: no PTY; keep the cmd strip. plan/08: Ghostty later.
- **Evidence:** Term Enter and Git Status call `spawn_command(windows_cmd(...))`. Crate docs: "In-memory terminal hub (plan/08 stub) plus piped process capture. PTY spawn is later." Server `terminal.create` still needs `cols` / `rows` and writes into that hub. Desktop never calls it. The Term tab is a log of the cmd strip.
- **Why it matters:** A Term tab plus `terminal.create` on the router looks finished. The hub cannot host `vim`. Painting a PTY chrome on it would be the HAR-toast class of lie.
- **This wave:** Keep the one-shot strip. Do not bind Run to `terminal.input`.

### F6. Session lifecycle is start-only

- **Severity:** High
- **Where:** `apps/multiplexer-desktop/src/main.rs` (`ensure_session`, `handle_slash` New, `dispatch` NewThread), left Agents rail
- **Evidence:** `session.start` is the only lifecycle call that creates state. New thread sets `session_id = None` and does not `session.stop`. Delete is local. `session.list` / `session.get` are unused. Left Agents shows the connected ids or "Send a turn to start." Inspector Agents projects local threads and says spawn is not wired (honest). Session tab prints `connected` via `Workspace.connect`.
- **Why it matters:** Abandoned backend sessions accumulate. The Agents rail reads like a live session list and is a cache of one start. Hello/ping never run, so protocol mismatch cannot be seen.
- **This wave:** `session.stop` on New/Delete. Optional `session.list` + `system.hello` on the Session tab. Do not rename left Agents to an orchestration dashboard.

### F7. Approval RPC has no inbound

- **Severity:** High
- **Where:** `apps/multiplexer-desktop/src/main.rs` (`respond_approval`, `approval_bar`), `crates/multiplexer-shell/src/approval_ui.rs`
- **Evidence:** Allow/Deny post `approval.respond` when `pending_approval` is set. Desktop never calls `set_pending_approval`. `handle_frame` results are not scanned for `event` / `permission_request`. `approval.list` is unwired. Turns do not go through the backend, so the FakeProvider / bridge cannot raise a permission on the path the UI uses.
- **Why it matters:** The approval card is real chrome over a queue that the host never fills. That is a control that pretends a protocol exists.
- **This wave:** Leave the card. Do not add `approval.list` until Send uses a path that can emit permission events. Four-way decisions stay later (plan/36).

### F8. Points never calls `checkpoint.list`

- **Severity:** Medium
- **Where:** `apps/multiplexer-desktop/src/main.rs` (`create_checkpoint`, boot seed), `crates/multiplexer-server/src/server.rs` (`checkpoint_list`)
- **Evidence:** Boot creates a local `CheckpointStore`, inserts `"start"`, then `install_checkpoints` that store on the server. Create/Revert RPC against it. List is never requested. Create on error invents another local row (`local-N`) that the server does not have. Revert then talks to a missing id.
- **Why it matters:** The tab can diverge from the catalog. List is already implemented. This is not a new engine.
- **This wave:** Yes, refresh from `checkpoint.list` after create/revert and on tab focus. Still no diff.

### F9. Model, cores, and git status pretend registry / telemetry / `git.status`

- **Severity:** Medium
- **Where:** `apps/multiplexer-desktop/src/inspector.rs`, `apps/multiplexer-desktop/src/main.rs` (`cycle_model`, `refresh_cores`, `RunGitStatus`)
- **Plan:** plan/36 H / I; plan/04 `model.*`, `telemetry.*`, `git.status`
- **Evidence:** Model button hint is "Cycle the session model." Catalog is `grok`, `grok-4.6`, `fake`. No `model.list` / `model.select`. Cores Reload resamples `sample_cores`. Git Status runs the shell, not `git.status` (unwired). All three are operable. None of them are the named RPC.
- **Why it matters:** Cycling a hardcoded list is fine if the Session tab does not look like a registry. A Status button is fine if it stays "run git status." Relabel or stub. Do not add a Browser tab in the same spirit.
- **This wave:** Model picker + `model.select` stub. Usage snapshot, not `telemetry.resources`. Leave Status as a shell unless a thin `git.status` wrapper is added and labeled.

### F10. Fifty-three unwired names, including every plan/36 stub

- **Severity:** Medium (inventory). High if a stub UI is painted first.
- **Where:** `crates/multiplexer-wire/src/methods.rs` vs `dispatch` `_` arm
- **Evidence:** 71 constants, 18 arms. Unwired includes every name plan/36 offered as a this-wave stub: `model.list/select/get`, `telemetry.usage/resources`, `orchestration.*`, `remote.*`. Also the later engines: `browser.*`, `har.*`, `auth.*`, `fs.*`, leftover git/checkpoint/terminal/turn/userInput, and stream control. Desktop has no `rpc()` to any of them. Inspector has no Browser/HAR tab (good).
- **Why it matters:** The wire file is the product brochure. Clients and tests can already spell methods the router rejects. Stubs are allowed only when the UI body says so (`{ subagents: [] }`, tailscale `not found`, usage `n/a`).
- **This wave:** Add only the stubs in the "Do this wave" table. Do not add `browser.list` so a future tab can compile.

---

## Related (not counted)

- Desktop RPC is in-process `handle_frame`, not JSON-RPC-over-WebSocket. `apps/multiplexer-server` listen path is unused by the GPUI app. That is architecture, not a missing inspector button.
- `checkpoint.create` / `revert` are used and still do not snapshot or restore files (audit 18). Usage is not honesty.
- Files Mention / MCP Start-Stop are local host actions. They are not RPC gaps.
- `EVENT` is the push method. Until the desktop reads notification frames, even a live `turn.send` would drop chunks.

## FINDINGS: 10
