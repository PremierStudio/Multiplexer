# 04 — Wire Contract: JSON-RPC over WebSocket

**Status:** Draft for adversarial review
**Owner:** Wire-contract subagent
**Consistency:** Must not contradict `docs/PLAN-CONTEXT.md`. Conflicts are flagged in §10 Open questions, never silently diverged.

**Locked decisions applied:** D12 (4-way approval enum, carried by the ProviderAdapter trait), D13 (`multiplexer-wire` shared schema crate), D15 (explicit wire↔ProviderEvent mapping table, §5), D18 (bounded backpressure, incl. the adapter channel), D19 (unified `session.start` params).

---

## 1. Purpose & scope

Multiplexer is a **server-centric runtime**: one native Rust binary owns agent processes, terminals, git, filesystem, checkpoints, and HAR capture. Every client — desktop (Rust + GPUI), paired mobile app, and web — is a **thin shell** over a single authenticated **JSON-RPC 2.0 over WebSocket** contract. This document defines that contract in depth.

The contract is the **only** boundary between core and clients. It must therefore be:

- **Explicit** — every method, event, and error is schema-verified on both sides (no ad-hoc fields).
- **Versioned** — clients and server can drift safely; the server is the source of truth.
- **Subscription-based** — clients subscribe to the streams they need; there is **no broadcast bus**.
- **Backpressured** — large streams (terminal output, diffs, HAR) are chunked and flow-controlled.
- **Testable** — contract conformance tests run on both sides against the same schema.

The contract deliberately mirrors the **Provider Adapter trait** (`start_session`, `send_turn`, `interrupt_turn`, `approval_respond`, `user_input_respond`, `checkpoint_revert`, `session_stop` + canonical `ProviderEvent` stream). The wire event set is a **superset** of `ProviderEvent`: a real transformation layer maps wire events onto adapter events (and vice versa), and terminal/HAR/fs/telemetry events have **no `ProviderEvent` counterpart**. The explicit mapping is in §5 (D15).

---

## 2. Transport

### 2.1 JSON-RPC 2.0 over WebSocket

- **Protocol:** JSON-RPC 2.0 (spec-compliant: `jsonrpc: "2.0"`, `id`, `method`, `params`, `result`, `error`).
- **Transport:** a single WebSocket connection per client. TLS (wss) everywhere except loopback local dev, where ws is permitted on `127.0.0.1` only.
- **Framing:** one JSON-RPC message per WebSocket text frame. No length-prefixing needed (WebSocket already frames); large payloads are split into chunks at the application layer (§8).
- **Binary:** binary frames are reserved for future use (e.g., terminal PTY bytes, HAR blobs). For MVP all payloads are UTF-8 JSON text frames.
- **Endpoints:**
  - Local: `ws://127.0.0.1:<port>/rpc` (server binds loopback; token-gated).
  - Remote/relay: `wss://<relay>/rpc` (ticketed, §6).
  - SSH: tunneled WebSocket over the SSH connection.
- **Connection lifecycle:** one logical session per connection. A client may open multiple connections (e.g., a control connection + a high-volume terminal connection) but must use the same ticket; the server correlates them by `client_id`.

### 2.2 Authentication & ticketing

- Local and remote connections are **ticketed**: a short-lived ticket (5-min TTL, from PLAN-CONTEXT) is exchanged for a longer-lived authenticated session.
- Ticket issuance and the full handshake are specified in §6.

### 2.3 Streaming / subscription model

The core **does not broadcast**. Every event stream is opt-in:

- A client sends a `subscribe` request naming a **stream** (e.g., `turn:<thread_id>`, `terminal:<pty_id>`, `har:<session_id>`, `subagent:<thread_id>`).
- The server pushes matching **notifications** (§4) only to subscribers of that stream.
- A client may `unsubscribe` at any time; the server stops pushing and reclaims buffers.
- Streams are **per-connection** by default; a client can request a stream be attached to a different connection via `attach_stream` (used by the pop-out pane / multi-window model).

This gives the "Outlook-style" pane model its data flow: each pane subscribes to exactly the streams it renders.

---

## 3. Message framing

### 3.1 Request

```json
{
  "jsonrpc": "2.0",
  "id": "req_01HZ...",
  "method": "turn.send",
  "params": {
    "thread_id": "thr_01HZ...",
    "text": "refactor the auth module",
    "model": "grok-4"
  }
}
```

- `id`: client-generated, unique per connection. Opaque string (recommended) or integer. Used for correlation of the response.
- `params`: a **named object** (never positional arrays) — required by our schema and by JSON-RPC best practice.

### 3.2 Response

```json
{
  "jsonrpc": "2.0",
  "id": "req_01HZ...",
  "result": { "turn_id": "trn_01HZ...", "accepted": true }
}
```

- `result` echoes the request `id`. A response is **always** paired to a request; there is exactly one response per request id.

### 3.3 Error response

```json
{
  "jsonrpc": "2.0",
  "id": "req_01HZ...",
  "error": {
    "code": -32602,
    "message": "invalid params: thread_id is required",
    "data": {
      "kind": "invalid_params",
      "details": { "field": "thread_id", "reason": "required" }
    }
  }
}
```

- `error.code` is a JSON-RPC integer; `error.data.kind` is our stable machine-readable error identifier (§7).

### 3.4 Notification (client → server)

A notification has **no `id`** and therefore **no response**. Used for fire-and-forget control (e.g., `terminal.input`, `userInput.respond`).

```json
{ "jsonrpc": "2.0", "method": "terminal.input", "params": { "pty_id": "pty_01", "data": "ls\r" } }
```

### 3.5 Event (server → client)

The server pushes events as **notifications** (no `id`). Events carry a `stream` field so the client can route them to the right pane/subscription.

```json
{
  "jsonrpc": "2.0",
  "method": "event",
  "params": {
    "stream": "turn:thr_01HZ...",
    "event": "agent_message_chunk",
    "seq": 42,
    "data": { "text": "Refactoring the auth module..." }
  }
}
```

- `seq`: monotonically increasing per stream, enabling gap detection and ordered replay.
- All server-pushed events use the single method `event` with a discriminated `event` field. This keeps the client's dispatch table small and the schema uniform.

### 3.6 Correlation

- **Request/response:** by `id`.
- **Event → originating request:** events carry an optional `in_response_to` (the request `id`) when they are a direct consequence of a request (e.g., `turn.send` → a stream of `agent_message_chunk` events). This lets a client correlate a command with its resulting stream without guessing.
- **Stream identity:** `stream` + `seq` uniquely identify an event.

---

## 4. Method namespaces

The RPC surface is grouped by namespace. Each method is either **request/response** (returns a result) or **notification** (fire-and-forget). Streaming methods return an initial result and then emit events on a named stream.

### 4.1 `session.*` — agent session lifecycle

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `session.start` | req | `{provider, model, workspace, initial_prompt, resume, config}` | `{session_id, thread_id}` | Unified shape (D19); mirrors `start_session` adapter |
| `session.stop` | req | `{session_id, force?}` | `{}` | Mirrors `session_stop` |
| `session.interrupt` | req | `{session_id}` | `{}` | Mirrors `interrupt_turn` |
| `session.list` | req | `{}` | `{sessions:[...]}` | |
| `session.get` | req | `{session_id}` | `{session}` | Full snapshot |

### 4.2 `turn.*` — send a turn and stream its execution

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `turn.send` | req | `{thread_id, text, model?, attachments?}` | `{turn_id}` | Emits `turn:<thread_id>` events |
| `turn.cancel` | req | `{turn_id}` | `{}` | |
| `turn.history` | req | `{thread_id, cursor?, limit?}` | `{turns:[...], next_cursor?}` | Paginated |

### 4.3 `approval.*` — respond to permission requests

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `approval.respond` | req | `{approval_id, decision, reason?}` | `{}` | `decision` is the **4-way enum**: `allow`/`deny`/`allow_once`/`allow_always` (D12) |
| `approval.list` | req | `{session_id, pending?}` | `{approvals:[...]}` | |

The `decision` field is the **4-way decision enum** (`allow` / `deny` / `allow_once` / `allow_always`), not a boolean. This is a locked decision (D12): `allow_once`/`allow_always` are real product features (permission modes). The **ProviderAdapter trait (plan/05) must carry the same 4-way enum** in its `approval_respond` — the wire `decision` maps 1:1 onto it, never a boolean.

### 4.4 `userInput.*` — respond to user-input requests

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `userInput.respond` | notif | `{request_id, text}` | — | Mirrors `user_input_respond` |
| `userInput.cancel` | notif | `{request_id}` | — | |

### 4.5 `checkpoint.*` — VCS checkpoints & diffs

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `checkpoint.list` | req | `{thread_id, cursor?, limit?}` | `{checkpoints:[...]}` | Hidden git refs per turn |
| `checkpoint.diff` | req | `{checkpoint_id, base?}` | `{diff}` | Structured diff (§8) |
| `checkpoint.revert` | req | `{checkpoint_id}` | `{result}` | Mirrors `checkpoint_revert` |
| `checkpoint.apply` | req | `{checkpoint_id, paths?}` | `{result}` | Selective apply |

### 4.6 `terminal.*` — Ghostty-class terminals

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `terminal.create` | req | `{session_id?, cwd?, cols?, rows?, shell?}` | `{pty_id}` | |
| `terminal.resize` | notif | `{pty_id, cols, rows}` | — | |
| `terminal.input` | notif | `{pty_id, data}` | — | Raw bytes (base64) |
| `terminal.kill` | req | `{pty_id}` | `{}` | |
| `terminal.list` | req | `{}` | `{ptys:[...]}` | |
| `terminal.attach` | req | `{pty_id, connection?}` | `{}` | Route output to a connection |

Terminal output is pushed on the `terminal:<pty_id>` stream (§8.1).

### 4.7 `fs.*` — filesystem

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `fs.read` | req | `{path, offset?, limit?}` | `{content, encoding, truncated?}` | |
| `fs.write` | req | `{path, content, encoding?}` | `{bytes}` | |
| `fs.list` | req | `{path, recursive?}` | `{entries:[...]}` | |
| `fs.watch` | req | `{path, recursive?}` | `{watch_id}` | Emits `fs:<watch_id>` events |
| `fs.unwatch` | req | `{watch_id}` | `{}` | |
| `fs.stat` | req | `{path}` | `{stat}` | |

All `fs.*` paths are **server-resolved** against the active worktree root; clients never send absolute host paths they invented. Path traversal is rejected (§7).

### 4.8 `git.*` — VCS operations

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `git.status` | req | `{worktree?}` | `{status}` | |
| `git.diff` | req | `{worktree?, ref?, paths?}` | `{diff}` | Structured diff |
| `git.commit` | req | `{message, paths?, amend?}` | `{commit}` | |
| `git.branches` | req | `{worktree?}` | `{branches:[...]}` | |
| `git.checkout` | req | `{worktree?, ref}` | `{}` | |
| `git.worktrees` | req | `{}` | `{worktrees:[...]}` | Parallel worktrees |
| `git.worktree.create` | req | `{path?, base?}` | `{worktree}` | |

### 4.9 `browser.*` — system-browser integration (CDP)

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `browser.list` | req | `{}` | `{browsers:[...]}` | Detected installed browsers |
| `browser.launch` | req | `{browser_id, profile?, headless?}` | `{browser_session_id, cdp_url}` | |
| `browser.navigate` | req | `{browser_session_id, url}` | `{}` | |
| `browser.cdp` | req | `{browser_session_id, method, params?}` | `{result}` | Raw CDP passthrough |
| `browser.close` | req | `{browser_session_id}` | `{}` | |
| `browser.screenshot` | req | `{browser_session_id, format?}` | `{image}` | base64 |

### 4.10 `har.*` — HAR capture & replay

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `har.start` | req | `{browser_session_id, filter?}` | `{har_session_id}` | Emits `har:<id>` events |
| `har.stop` | req | `{har_session_id}` | `{har}` | Full HAR document |
| `har.replay` | req | `{har_session_id, speed?, loop?}` | `{replay_id}` | |
| `har.list` | req | `{}` | `{har_sessions:[...]}` | |

### 4.11 `orchestration.*` — subagent fan-out

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `orchestration.spawn` | req | `{parent_thread_id, prompt, agent_budget?, model?}` | `{subagent_id}` | Mirrors `spawn_subagent` |
| `orchestration.subscribe` | req | `{thread_id}` | `{}` | Subscribe to a thread's events |
| `orchestration.unsubscribe` | req | `{thread_id}` | `{}` | |
| `orchestration.list` | req | `{parent_thread_id?}` | `{subagents:[...]}` | Live dashboard data |

### 4.12 `model.*` — model registry

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `model.list` | req | `{}` | `{models:[...]}` | From `[model.*]` config |
| `model.select` | req | `{thread_id, model}` | `{}` | Per-thread model |
| `model.get` | req | `{model}` | `{model}` | |

### 4.13 `remote.*` — remote/relay/SSH

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `remote.list` | req | `{}` | `{remotes:[...]}` | |
| `remote.connect` | req | `{remote_id, ticket?}` | `{session_id}` | |
| `remote.disconnect` | req | `{remote_id}` | `{}` | |

### 4.14 `auth.*` — provider & remote auth

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `auth.providers` | req | `{}` | `{providers:[...]}` | |
| `auth.login` | req | `{provider}` | `{auth_url?, status}` | OAuth flow |
| `auth.status` | req | `{provider?}` | `{status}` | |
| `auth.logout` | req | `{provider}` | `{}` | |

### 4.15 `telemetry.*` — usage & resource monitoring

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `telemetry.usage` | req | `{period?}` | `{usage}` | Account/usage tracking |
| `telemetry.resources` | req | `{}` | `{resources}` | From Rust sidecar (NDJSON) |
| `telemetry.subscribe` | req | `{interval_ms?}` | `{}` | Emits `telemetry:resources` events |

### 4.16 `system.*` — connection & contract

| Method | Type | Params | Result | Notes |
|---|---|---|---|---|
| `system.hello` | req | `{client_info, protocol_version}` | `{server_info, protocol_version, capabilities}` | Handshake after auth |
| `system.ping` | req | `{}` | `{pong}` | Liveness |
| `system.capabilities` | req | `{}` | `{capabilities}` | Feature negotiation |

---

## 5. Event / notification types

All server-pushed events use the single `event` method with a discriminated `event` field (§3.5). The canonical set:

| Event | Stream | Payload (key fields) | Notes |
|---|---|---|---|
| `agent_message_chunk` | `turn:<thread>` | `{text}` | Incremental assistant text |
| `agent_thought_chunk` | `turn:<thread>` | `{text}` | Reasoning/thought stream |
| `tool_call` | `turn:<thread>` | `{tool, args, call_id}` | Tool invocation started |
| `tool_call_update` | `turn:<thread>` | `{call_id, status, output?}` | Progress/completion |
| `plan` | `turn:<thread>` | `{plan}` | Structured plan object |
| `permission_request` | `turn:<thread>` | `{approval_id, tool, summary}` | Needs approval |
| `user_input_request` | `turn:<thread>` | `{request_id, prompt}` | Needs user input |
| `checkpoint` | `turn:<thread>` | `{checkpoint_id, ref, summary}` | New checkpoint created |
| `terminal_output` | `terminal:<pty>` | `{data, encoding}` | PTY bytes (base64) |
| `terminal_exit` | `terminal:<pty>` | `{exit_code}` | |
| `har_event` | `har:<session>` | `{entry}` | Network entry captured |
| `subagent_status` | `orchestration:<parent>` | `{subagent_id, status, progress?}` | Fan-out dashboard |
| `fs_change` | `fs:<watch>` | `{path, kind}` | File watcher events |
| `turn_status` | `turn:<thread>` | `{turn_id, status}` | running/completed/failed |
| `session_status` | `session:<id>` | `{session_id, status}` | |
| `telemetry_resources` | `telemetry:resources` | `{cpu, mem, ...}` | Periodic resource sample |
| `error` | any | `{code, message, data}` | Async error on a stream |

### 5.1 Wire ↔ ProviderEvent mapping (D15)

The wire event set is a **superset** of the canonical `ProviderEvent` stream (plan/05). A real transformation layer exists in the server's projector: it does **not** forward adapter events to the wire with "no transformation beyond adding `stream`/`seq`". The mapping is explicit:

| Wire event | ProviderEvent counterpart | Transformation |
|---|---|---|
| `agent_message_chunk` | `TextDelta` | 1:1 (add `stream`/`seq`) |
| `agent_thought_chunk` | `TextDelta` (thought variant) | 1:1, tagged as thought |
| `tool_call` | `ToolCallStarted` | 1:1 (add `stream`/`seq`) |
| `tool_call_update` | `ToolCallFinished` | 1:1 (add `stream`/`seq`) |
| `plan` | `Plan` | 1:1 |
| `permission_request` | `PermissionRequested` | 1:1 (carries the 4-way decision enum, D12) |
| `user_input_request` | `UserInputRequested` | 1:1 |
| `checkpoint` | `CheckpointCreated` | 1:1 |
| `turn_status` | `TurnFinished` | 1:1 (add `stream`/`seq`) |
| `session_status` | `SessionStatus` | 1:1 |
| `terminal_output` | — (no counterpart) | Wire-only; no `ProviderEvent` |
| `terminal_exit` | — (no counterpart) | Wire-only; no `ProviderEvent` |
| `har_event` | — (no counterpart) | Wire-only; no `ProviderEvent` |
| `subagent_status` | — (no counterpart) | Wire-only; no `ProviderEvent` |
| `fs_change` | — (no counterpart) | Wire-only; no `ProviderEvent` |
| `telemetry_resources` | — (no counterpart) | Wire-only; no `ProviderEvent` |
| `error` | `Error` | 1:1 (add `stream`/`seq`) |

The wire set is a **superset**: terminal, HAR, fs, and telemetry events have **no `ProviderEvent` counterpart** and are produced by the server's own subsystems, not the adapter. plan/05 references this table.

---

## 6. Auth handshake

### 6.1 Ticket issuance

- A **ticket** is a short-lived (5-min TTL) single-use credential used to bootstrap an authenticated WebSocket session.
- Tickets are issued out-of-band:
  - **Local:** the server writes a ticket to the OS keychain / a local token file on startup; the desktop client reads it directly. No network round-trip.
  - **Remote/relay:** the user authenticates via OAuth/passkey on the relay's web page, which returns a ticket to the client.
  - **SSH:** the SSH session itself authenticates; a ticket is minted inside the tunnel.
- Ticket payload: `{client_id, scope, exp, nonce}` signed by the server (HMAC or Ed25519).

### 6.2 Connection handshake

1. Client opens WebSocket to `/rpc`.
2. Client sends `system.hello` with `client_info` + `protocol_version` and the ticket in a header or first-message field.
3. Server validates the ticket (signature, TTL, scope, single-use), binds a session, and replies with `server_info`, negotiated `protocol_version`, and `capabilities`.
4. All subsequent messages are authenticated by the established session.

### 6.3 DPoP (Demonstration of Proof-of-Possession)

- For remote/relay, the client proves possession of its private key by binding each request to a DPoP proof (JWT with `htm`/`htu`/`jti`), preventing ticket replay by a third party.
- Local loopback uses the ticket alone (trusted transport); DPoP is mandatory for any non-loopback connection.
- Passkeys are supported as the primary remote auth factor (per PLAN-CONTEXT).

### 6.4 Re-auth & expiry

- Sessions are long-lived but re-validated on a sliding window; a client that loses its session receives an `auth_expired` error and must re-handshake with a fresh ticket.
- The 5-min ticket TTL bounds the window in which a stolen ticket is usable; the established session is the long-lived credential.

---

## 7. Error model

Structured, machine-readable errors. Every error has a JSON-RPC `code` (integer) and a stable `data.kind` (string) plus optional `data.details`.

### 7.1 JSON-RPC standard codes

| Code | Meaning |
|---|---|
| `-32700` | Parse error |
| `-32600` | Invalid request |
| `-32601` | Method not found |
| `-32602` | Invalid params (schema violation) |
| `-32603` | Internal error |

### 7.2 Multiplexer application codes (`data.kind`)

| Code | `kind` | Meaning |
|---|---|---|
| `-32000` | `auth_required` | No/invalid session |
| `-32001` | `auth_expired` | Session expired, re-handshake |
| `-32002` | `ticket_invalid` | Bad/expired/replayed ticket |
| `-32003` | `permission_denied` | Scope violation |
| `-32004` | `not_found` | Resource (thread/pty/checkpoint) missing |
| `-32005` | `conflict` | State conflict (e.g., turn already running) |
| `-32006` | `invalid_state` | Operation invalid in current state |
| `-32007` | `path_invalid` | Path traversal / outside worktree |
| `-32008` | `provider_error` | Upstream provider/adapter failure |
| `-32009` | `rate_limited` | Backpressure / quota exceeded |
| `-32010` | `unsupported` | Capability not negotiated |
| `-32011` | `stream_closed` | Subscribed stream no longer exists |
| `-32012` | `protocol_version_mismatch` | Client/server version drift |

### 7.3 Error response shape

```json
{
  "jsonrpc": "2.0",
  "id": "req_01HZ...",
  "error": {
    "code": -32005,
    "message": "a turn is already running on this thread",
    "data": {
      "kind": "conflict",
      "details": { "running_turn_id": "trn_01HZ..." }
    }
  }
}
```

Clients switch on `data.kind`, never on `message` text. `message` is human-readable and localized client-side.

---

## 8. Backpressure & streaming

Large streams (terminal output, diffs, HAR) must not flood a slow client or the WebSocket buffer.

### 8.1 Chunking

- **Terminal output:** PTY bytes are batched and emitted as `terminal_output` events with a `data` base64 payload, coalesced on a short timer (e.g., 16–50 ms) so a burst becomes a few frames, not thousands. Clients render coalesced frames.
- **Diffs:** `checkpoint.diff` / `git.diff` return a **structured diff** (list of hunks with per-line metadata) rather than one giant string. Large diffs are paginated via `{cursor, limit}` or returned as a `diff:<id>` stream of `diff_chunk` events.
- **HAR:** `har_event` entries are pushed incrementally; the full document is only materialized on `har.stop`. Replay is a separate stream.

### 8.2 Flow control

- **Window-based:** each subscription has a server-side send window (e.g., 1024 events or 4 MiB). When the window is full, the server stops emitting and the client must send `stream.ack` (with the last `seq` it consumed) to reopen it.
- **Slow-consumer policy:** if a client never acks, the server drops the subscription (or coalesces) rather than buffering unboundedly. The client re-subscribes and catches up from a checkpoint `seq`.
- **Resume:** subscriptions carry an optional `from_seq` so a reconnecting client resumes without replaying everything.

The wire uses **window-based flow control and bounded buffers** (D18). This is consistent with the **ProviderAdapter event channel (plan/05), which must also be bounded** with backpressure — NOT `mpsc::UnboundedReceiver`. The provider-ingestion worker (plan/06) is the bounding point, so backpressure propagates end-to-end from the adapter through the projector to the wire window.

### 8.3 Backpressure on client → server

- Large uploads (file writes, HAR replay) are chunked with `fs.write`/`har.replay` accepting `{offset, chunk}` sequences; the server acks each chunk before the next is sent.

---

## 9. Contract testing

The contract is **schema-verified on both sides** so clients cannot drift. This is a hard requirement from PLAN-CONTEXT ("Contract: JSON-RPC wire contract schema-verified on both sides").

### 9.1 Schema definition

- The contract is defined in a **single source of truth**: the shared schema crate **`multiplexer-wire`** (Rust) — the consolidated `multiplexer-*` name per D13 (no `mx-` prefix) — that:
  - Defines every method, params, result, event, and error as typed Rust structs (serde).
  - Derives **JSON Schema** (`schemars`) for every type.
  - Is consumed by the server (Rust) directly and by clients via **codegen**.
- **Codegen:** from the JSON Schema, we generate typed clients for:
  - **Rust desktop client:** reuses the crate directly (no codegen needed).
  - **Mobile (Swift/Kotlin):** codegen'd stubs (e.g., via `quicktype` or a custom generator).
  - **Web:** TypeScript types + a thin RPC client.
- The schema crate is the **versioned artifact**; `protocol_version` in `system.hello` is derived from it.

### 9.2 Versioning

- `protocol_version` is a semver string (e.g., `1.2.0`).
- **Major** = breaking change (method removed, param/event shape changed). Server and client must match major.
- **Minor** = additive (new method/event/field). Server and client negotiate; unknown fields are ignored, unknown methods/events are rejected with `unsupported`.
- On mismatch: server rejects with `protocol_version_mismatch` and includes the server's version so the client can prompt an upgrade.

### 9.3 Conformance tests (both sides)

- **Server side:** a test harness replays a corpus of canned requests/events and asserts the server's responses/emissions are schema-valid and semantically correct. Property-based tests (proptest) generate arbitrary valid/invalid messages and assert the server never emits a malformed frame.
- **Client side:** a **mock server** (in-process, Rust) implements the contract from the same schema crate and serves canned fixtures. Mobile/web/desktop clients run their full logic against the mock for deterministic offline tests (PLAN-CONTEXT: "mock server for offline determinism").
- **Schema conformance:** both sides validate every inbound/outbound message against the schema at the boundary (a `validate` step in the RPC layer). Any violation is a bug and fails CI.

### 9.4 CI gates

- Contract tests run in the standard gate order: fmt → clippy → unit+property → mutation → integration → component → e2e → coverage. Schema conformance is part of unit+property; the mock-server client tests are part of integration.

---

## 10. Open questions

These reference pending decisions from PLAN-CONTEXT; we do not decide them unilaterally.

1. **Schema language / codegen toolchain** — JSON Schema + `schemars` + custom codegen is proposed, but the exact mobile codegen path (Swift/Kotlin) is unconfirmed. Could use `quicktype` or a bespoke generator.
2. **Binary frames** — reserved for terminal/HAR bytes; whether MVP uses them or stays all-JSON is open.
3. **Multi-connection model** — whether the pop-out pane model needs multiple WebSocket connections or one connection with `attach_stream` suffices. Proposed: one connection + stream routing; confirm against the UI plan (plan/10).
4. **DPoP strictness on local** — whether local loopback requires DPoP or ticket-only is acceptable. Proposed: ticket-only on loopback, DPoP mandatory remote.
5. **Stream ack granularity** — window-based ack vs per-event ack; proposed window-based. Confirm against performance targets (plan/16).
6. **`fs.*` path model** — server-resolved against worktree root is proposed; whether clients may pass absolute host paths is open (security review, plan/17).
7. **Event `seq` semantics** — per-stream monotonic is proposed; whether a global ordering is needed for cross-stream correlation is open.
8. **`protocol_version` negotiation** — semver with major-lock is proposed; whether to support multiple concurrent majors server-side is open.

---

## Appendix A — Minimal handshake example

Client → server:
```json
{ "jsonrpc": "2.0", "id": "h1", "method": "system.hello",
  "params": { "client_info": { "name": "multiplexer-desktop", "version": "0.1.0" },
              "protocol_version": "1.2.0", "ticket": "tk_..." } }
```
Server → client:
```json
{ "jsonrpc": "2.0", "id": "h1",
  "result": { "server_info": { "name": "multiplexer-core", "version": "0.1.0" },
              "protocol_version": "1.2.0",
              "capabilities": ["terminal", "har", "browser", "orchestration"] } }
```

## Appendix B — Subscription example

Client:
```json
{ "jsonrpc": "2.0", "id": "s1", "method": "orchestration.subscribe",
  "params": { "thread_id": "thr_01HZ..." } }
```
Server:
```json
{ "jsonrpc": "2.0", "id": "s1", "result": { "stream": "orchestration:thr_01HZ..." } }
```
Then events flow on `orchestration:<thread>` until `unsubscribe` or connection close.
