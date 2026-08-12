# 14 — Remote Access & Relay

> **Status:** Authoritative plan doc. Consistent with `docs/PLAN-CONTEXT.md` (the shared plan context). If any statement here conflicts with PLAN-CONTEXT, PLAN-CONTEXT wins and the conflict should be flagged in `plan/20-risks-and-open-questions.md`.
>
> **Purpose:** Define how Multiplexer's server-centric runtime is reached from anywhere — the four remote-access kinds (local, bearer-paired, relay tunnel, SSH), WebSocket ticket authentication, the relay service, pairing, security posture, multi-machine account/usage tracking, and the testing strategy. This doc is the remote/relay counterpart to `plan/04-wire-contract.md` (the JSON-RPC-over-WebSocket contract), `plan/13-mobile-app.md` (the primary remote client), and `plan/17-security-and-secrets.md` (secrets policy).

> **Locked decisions applied:** This doc has been reconciled against `docs/DECISIONS.md`. Applied: **D24** (relay is a TLS-terminating pipe — no E2EE claim), **D25** (SSH `--remote` agent independently enforces permissions/worktree confinement/approval gating), **D37** (pairing issues a long-lived device credential minted into short-lived tickets). These supersede any earlier "open question" or conflicting wording in this doc.

---

## 1. Remote access model — the four target kinds

PLAN-CONTEXT fixes the remote/relay architecture: **local + paired + relay tunnel + SSH; WebSocket ticket auth (5-min TTL); Tailscale serve.** The core is a single native Rust binary that owns agent processes, terminals, git, filesystem, checkpoints, and HAR capture. Every client — desktop, mobile, web — is a thin shell over one authenticated **JSON-RPC over WebSocket** contract. Remote access is therefore *not* a separate product: it is the same server runtime exposed over four transport kinds, differing only in **how the client reaches the server** and **how it authenticates**.

| Kind | Reach | Transport | Auth | Typical client | Use case |
|------|-------|-----------|------|----------------|----------|
| **Local** | Same machine | `ws://127.0.0.1:<port>` (loopback) | OS-keychain session token | Desktop app | Primary; the desktop client talks to the local core |
| **Bearer-paired** | Same LAN / trusted network | `ws://<host>:<port>` | Short-lived bearer token (pairing) | Mobile app, second desktop | Phone/second machine on the same network |
| **Relay tunnel** | Anywhere (internet) | `wss://<relay>/<tunnel-id>` | Ticket + DPoP over TLS | Mobile app, web | Connect from anywhere (e.g. cellular) without port-forwarding |
| **SSH** | Remote host | SSH transport (not raw WS) | SSH keys / agent | Desktop app, CLI | Run agents on a remote box with full file/git/terminal access |

**Tailscale serve** is an *endpoint provider*, not a fifth kind: it can back the bearer-paired and relay-tunnel kinds by exposing the core on a private Tailscale network (`tailscale serve` / `tailscale funnel`), giving the client a stable, authenticated endpoint without us operating a relay. We treat Tailscale as an optional, first-class provider rather than a requirement.

### 1.1 Design invariants

1. **One wire contract, four transports.** The JSON-RPC-over-WebSocket message schema (`plan/04`) is identical across all four kinds. Transport only changes *how* a socket is established and *what* credential is presented on connect. This keeps the client and server code paths uniform and testable.
2. **The core never trusts the transport.** Authentication is enforced at the WebSocket handshake (ticket/bearer/DPoP), independent of whether the socket is loopback, LAN, relay, or SSH-forwarded. A local socket is *not* implicitly trusted — it still presents a session token.
3. **Least privilege per connection.** Each connection binds to a *scope* (which threads, worktrees, and capabilities it may touch). A phone observing a session gets read-mostly scope; the desktop gets full scope.
4. **No raw secrets on the wire.** Only short-lived tickets, bearer tokens, and DPoP proofs travel over the wire. Long-lived secrets (SSH keys, provider tokens, keychain material) never leave the machine that owns them.

### 1.2 Relationship to the wire contract

The remote layer is the *transport + auth* half of the contract; `plan/04` defines the *message* half. Concretely:

- **Connect:** client opens a WebSocket, presents a credential in the opening handshake (subprotocol header or first message), server validates, returns a session-scoped connection id.
- **Messages:** JSON-RPC requests/notifications/responses exactly as in `plan/04`, multiplexed over the socket.
- **Reconnect:** on drop, client reconnects with a fresh ticket and a `resume` cursor so the event-sourced read model (`plan/06`) can replay missed events. Idempotency keys on commands prevent double-apply.

---

## 2. WebSocket ticket authentication (5-min TTL)

Tickets are the universal credential for remote connections. They are **short-lived (5-minute TTL), single-purpose, and bound to a scope**. A ticket is *not* a password — it is a capability that can be minted, revoked, and expired, and it is always presented alongside a DPoP proof (see §6).

### 2.1 Ticket lifecycle

```
[Client]                          [Core]                          [Relay]
   | 1. request ticket (scope)      |                                |
   |------------------------------->|                                |
   |                                | 2. mint ticket (JWT, 5-min)    |
   | 3. ticket + DPoP proof         |<-------------------------------|
   |<-------------------------------|                                |
   | 4. open WS, present ticket     |                                |
   |------------------------------->|                                |
   |                                | 5. validate: sig, exp, scope,  |
   |                                |    DPoP nonce, single-use       |
   | 6. connection accepted         |                                |
   |<-------------------------------|                                |
```

1. **Mint:** the client requests a ticket for a specific scope (e.g. `observe:thread:<id>`, `control:worktree:<id>`, `full`). The core signs a compact JWT containing `sub` (client id), `scope`, `iat`, `exp = iat + 300s`, `jti` (unique id), and a `cnf` (confirmation) claim carrying the client's DPoP public-key thumbprint.
2. **Present:** the client opens the WebSocket and presents the ticket plus a DPoP proof bound to the current request.
3. **Validate:** the core checks signature, expiry (5-min TTL), scope match, DPoP nonce freshness, and single-use (`jti` consumed). On success it upgrades to a long-lived *connection session*; the ticket itself is then dead.
4. **Renew:** long-lived connections renew the *connection session* (not the ticket) via a refresh flow, so a 5-minute ticket does not force a reconnect every 5 minutes. The ticket TTL governs *establishment*, not the lifetime of an established connection.

### 2.2 Why 5 minutes

- **Short blast radius:** a leaked ticket is useless within 5 minutes and is single-use, so replay is impossible.
- **Scope-bound:** even a valid ticket cannot exceed its minted scope.
- **Revocable:** the core can blacklist a `jti` or a client id instantly, cutting off a compromised client at the next renewal.
- **Stateless-ish validation:** the core can validate expiry/signature without per-ticket state; only the single-use `jti` set and the connection-session table need to be held in memory.

### 2.3 Ticket auth in the wire contract

The ticket is carried in the WebSocket **subprotocol** header (`Sec-WebSocket-Protocol: multiplexer.v1, ticket.<jwt>`) or, if the transport (e.g. some relay hops) strips headers, as the first JSON-RPC message `auth.ticket`. The server MUST reject the socket if no valid credential is presented within a short window (e.g. 10s). The exact placement is finalized in `plan/04`; this doc requires only that the credential is *always* presented at handshake and *never* sent as a plain query-string parameter (which would leak into logs).

---

## 3. SSH remote worktrees

SSH remote worktrees are a **baseline-bar requirement** (Orca parity, PLAN-CONTEXT §"Baseline bar"). The user runs agents against worktrees on a remote host over SSH, with full file editing, git, and terminals — and the experience must feel local. This is the most complex remote kind because it is not a thin client over the wire contract; it is the **core itself running against a remote filesystem/process namespace**.

### 3.1 Architecture

Two cooperating halves:

- **Local core (Multiplexer desktop):** owns the UI, the editor, the agent runtime (in-process grok-build), and the wire contract. It presents the remote worktree as if it were local.
- **Remote host:** runs a small **remote agent** (the same Multiplexer binary in `--remote` mode, or a lightweight companion) that exposes filesystem, git, and process/terminal operations over an SSH channel.

The local core talks to the remote agent over an **SSH connection** (not a raw WebSocket). File edits, git commands, and terminal PTYs are proxied over SSH channels; the agent runtime runs *locally* but operates on the remote workspace through the proxied filesystem/git/execution primitives. This mirrors how `xai-grok-workspace` (fs/VCS/execution) is designed to be backend-agnostic — we supply an SSH-backed implementation of those primitives.

**Trust boundary (D25):** the remote agent is **not a dumb executor** that trusts the local core implicitly. It **independently enforces** the same security controls on the remote host that the local core enforces locally:

- **Permission modes** — the remote agent re-validates every operation against the 4-way approval decision model (`allow`/`deny`/`allow_once`/`allow_always`, `plan/04`/`plan/17`) on the remote side, rather than trusting the local core's assertion that an operation was approved. A compromised or buggy local core cannot bypass remote-side gating.
- **Worktree confinement** — the remote agent confines all fs/git/process operations to the authorized remote worktree(s); it refuses paths, git refs, or process spawns that escape the granted scope, independent of what the local core requests.
- **Approval gating** — approval prompts for remote-side operations are evaluated and gated on the remote host, so the remote agent is the enforcement point for what actually executes there.

The remote agent's policy (allowed scopes, worktree roots, permission defaults) is configured on the remote host and enforced there; the local core's requests are treated as *proposals* that the remote agent validates against that policy. This is consistent with `plan/17`'s threat model (D25).

```
[Multiplexer desktop]                 [Remote host]
  UI / editor / agent runtime            remote-agent (multiplexer --remote)
        |                                        |
        |  SSH (auth: keys/agent)                |
        |  - fs ops (read/write/stat/watch)      |
        |  - git ops (status/diff/commit/checkout)|
        |  - PTY channels (terminals)            |
        |  - process spawn (agent tool exec)     |
        +----------------------------------------+
```

### 3.2 Required capabilities (baseline bar)

| Capability | Requirement |
|---|---|
| **Full file editing** | Open remote files in the native editor; save writes through the SSH fs backend; watch for remote changes (inotify/FSEvents/ReadDirectoryChangesW proxied) |
| **Git** | status, diff, commit, checkout, branch, merge-back from remote worktree — all through the SSH git backend |
| **Terminals** | Ghostty terminal attached to a remote PTY over an SSH channel, with splits |
| **Auto-reconnect** | On SSH drop, transparently reconnect and resume the PTY/fs session; buffer and replay missed events; never lose unsaved edits |
| **Port forwarding** | Local↔remote port forwarding so the agent's dev servers and the browser/CDP integration reach the right host |
| **Passphrase caching** | Cache SSH key passphrases in the OS keychain (never plaintext), so reconnects don't re-prompt; respect `ssh-agent` when available |

### 3.3 Auto-reconnect design

- The SSH layer maintains a **session state machine**: `connected → reconnecting → connected`. On drop it backs off exponentially (e.g. 0.5s → 2s → 8s, capped), re-authenticates from the keychain/agent, and resumes.
- **Idempotent fs/git ops** carry operation ids; on reconnect the client replays any op whose acknowledgement was lost, and the server dedupes by id. This is the same idempotency mechanism as the wire contract (§1.2).
- **Terminal resume:** PTY state (scrollback, cursor, env) is snapshotted and restored on reconnect, so a dropped SSH link does not kill a running agent terminal.
- **Unsaved-edit safety:** local editor buffers are never discarded on disconnect; they are marked "pending sync" and flushed on reconnect.

### 3.4 Passphrase caching & secrets

- SSH key passphrases are stored in the **OS keychain** (Windows Credential Manager / macOS Keychain / libsecret) — never in config files or plaintext.
- When `ssh-agent` is running, we prefer it and never touch the passphrase ourselves.
- Remote-side secrets (provider tokens, git credentials on the remote) follow the same policy: OS keychain on the remote, `op://Vault/Item/field` references only in configs, never raw values (see `plan/17`).

### 3.5 Windows-first note

SSH on Windows uses the bundled OpenSSH client (Windows 10+ ships it) and the native `ssh-agent` service. We must handle Windows path translation (`C:\...` ↔ `/mnt/c/...` on the remote), CRLF handling in proxied files, and Windows OpenSSH quirks. This is our responsibility (PLAN-CONTEXT: Windows build support is ours).

---

## 4. Relay tunnel

The relay tunnel lets a client connect **from anywhere** (e.g. a phone on cellular) without the user configuring port-forwarding, a VPN, or a public IP. It is modeled on T3 Connect (T3 Code's relay) and is the transport behind the "connect from anywhere" mobile story.

### 4.1 How it works

1. The **core** (on the user's desktop) opens a persistent outbound WebSocket to the relay service, registering a **tunnel id** (a random, unguessable slug). Because the connection is *outbound*, no inbound firewall rule or public IP is needed.
2. The **client** (phone) connects to the relay with the tunnel id and is multiplexed onto the core's outbound connection.
3. The relay is a **TLS-terminating pipe** (D24): it terminates TLS and forwards bytes between the client socket and the core socket. It is *not* end-to-end encrypted — the relay operator (Cloudflare or self-hosted) **can see plaintext application data**. We do **not** claim E2EE for the default relay. The confidentiality story is honest TLS: each leg (client↔relay, relay↔core) is TLS-protected, and the relay operator is a trusted intermediary for that hop.

```
[Phone] --wss--> [Relay] <--wss-- [Core (desktop)]
   |                 |                 |
   |  tunnel id      |  outbound conn  |
   +-----------------+-----------------+
        TLS-terminating pipe (relay sees plaintext)
        auth: ticket + DPoP at the application layer
```

**Optional E2EE layer (D24):** because the relay operator can see plaintext, a user who wants the relay to be a *dumb pipe* (unable to read traffic) can opt into a **per-tunnel session-key E2EE layer** established via the pairing handshake (§5). In that mode the client and core derive a shared session key during pairing and encrypt application frames before they reach the relay, so the relay forwards only ciphertext. This is **optional and opt-in**; the default claim is honest TLS-terminating, not E2EE. The default must never be described as "end-to-end encrypted."

### 4.2 Relay implementation options

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| **Cloudflare Worker + Durable Objects** | Global edge, cheap, no server to operate, WebSocket support, Durable Objects give sticky state for the pipe | Vendor coupling; per-connection state in DOs | **Recommended** (matches PLAN-CONTEXT "Cloudflare Worker or similar") |
| Self-hosted relay (Rust) | Full control, no vendor | We operate infra, TLS/certs, scaling, abuse | Fallback / for self-hosters |
| Tailscale funnel | No relay to operate; Tailscale handles auth + NAT traversal | Requires Tailscale on both ends; not "anywhere" without the client installing Tailscale | Complementary provider, not the default relay |

The recommended default is a **Cloudflare Worker + Durable Objects** relay: each tunnel is a Durable Object that holds the two WebSocket connections and forwards frames. This gives global reach, TLS termination, and near-zero operational cost. The relay is deliberately stateless about *content* — it only routes.

### 4.3 Tunnel lifecycle & security

- **Tunnel id** is a high-entropy random slug (e.g. 128-bit) — it is a capability, not a name. Guessing it is infeasible.
- **Ticket + DPoP still apply.** The tunnel id gets the client to the core; the ticket + DPoP proof authenticate the client to the core. A leaked tunnel id alone grants nothing.
- **Revocation:** the core can close its outbound connection to the relay, instantly killing all tunneled sessions.
- **Rate/abuse limits** on the relay (per-IP, per-tunnel) to prevent relay abuse; the relay never stores payloads.

---

## 5. Pairing

Pairing is how a **phone or second machine** is authorized to talk to the core. It is the bearer-paired kind's bootstrap and the trust root for the relay kind.

### 5.1 Pairing flow (QR code → device credential → short-lived tickets)

Pairing reconciles with `plan/13` and `plan/17` (D37): the QR encodes a **one-time code**; the exchange issues a **long-lived device credential** (device id + stored secret in the OS keychain); that credential is then **minted into short-lived tickets** for actual use. **No long-lived bearer secret is ever used directly on the wire.**

1. **Display:** the desktop core shows a pairing screen with a QR code encoding a **pairing URL** — `multiplexer://pair?host=<host>&port=<port>&code=<one-time-code>`.
2. **Scan:** the phone's Multiplexer app scans the QR (or the user types the code on a second desktop).
3. **Exchange:** the phone presents the one-time code to the core over the local network (or via the relay). The core validates it (single-use, short TTL, e.g. 2 minutes), then issues a **long-lived device credential** — a **device id** plus a **stored secret** — and a **device profile** (name, capabilities, default scope). The device also generates its DPoP keypair here and registers the public key with the core (the secret is bound to that key).
4. **Store:** the phone stores the device credential (device id + secret) in its **OS keychain** (D38 — never a plaintext token file). The core stores the device's public identity, DPoP public key, and scope in its own keychain-backed store.
5. **Connect:** thereafter the phone **never sends the long-lived secret on the wire**. Instead it uses the device credential to **mint short-lived tickets** (5-min TTL, single-use, scope-bound, `cnf`-bound to its DPoP key — §2) and presents those tickets + DPoP proofs to connect. The long-lived secret is used only to authenticate the ticket-minting request locally; it is never a bearer credential on the wire. No re-pairing is needed until the device is revoked.

### 5.2 Pairing security properties

- **One-time code:** single-use, short TTL, high entropy — replay and brute-force are infeasible.
- **Human-in-the-loop:** pairing requires physical access to the desktop (scanning the QR) — a strong out-of-band trust signal.
- **Scoped by default:** a newly paired device gets a **read/observe** scope; the user can elevate it to control scope per device in the desktop UI.
- **Revocable:** the desktop lists paired devices and can revoke any of them instantly (kills their connection sessions and invalidates their credential).

### 5.3 Multi-device

The core supports many paired devices simultaneously (phone + second desktop + web). Each device has its own credential, scope, and connection sessions. This is the foundation of the multi-machine story (§7).

---

## 6. Security

Security is a first-class, non-negotiable property of the remote layer. PLAN-CONTEXT fixes the posture: **OS keychain for local secrets; OAuth for providers; passkeys/DPoP for remote.**

### 6.1 DPoP (Demonstrating Proof of Possession)

DPoP binds an access token to the client's public key, so a stolen token cannot be replayed by an attacker who lacks the private key.

- The client generates an asymmetric keypair (e.g. Ed25519) at pairing/registration time; the public key is registered with the core (and embedded in the ticket's `cnf` claim).
- Every authenticated request carries a **DPoP proof**: a JWT signed with the client's private key, bound to the HTTP method, URL, and a fresh nonce from the server.
- The core verifies the proof against the registered public key and the nonce. A token without a valid proof is rejected even if the token itself is valid.

### 6.2 Layered security model

| Layer | Mechanism |
|---|---|
| **Transport** | TLS 1.3 everywhere except loopback-local (which is still authenticated at the app layer); the relay terminates TLS and **can see plaintext** (D24) — it is a TLS-terminating pipe, not E2EE. Optional per-tunnel E2EE via the pairing handshake (§4.1) if the user opts in |
| **Handshake** | Ticket (5-min TTL, single-use, scope-bound) + DPoP proof |
| **Session** | Connection session with refresh; idempotency keys; per-connection scope |
| **Secrets** | OS keychain only; `op://Vault/Item/field` references in configs; never raw secrets in files, logs, or the wire |
| **Revocation** | Per-device, per-tunnel, per-connection-session revocation; blacklist `jti`/client ids |

### 6.3 Secrets policy (aligned with `plan/17`)

- **Local secrets** (SSH passphrases, device credentials, provider tokens) live in the **OS keychain** — never plaintext files.
- **Configs** reference secrets only as `op://Vault/Item/field` (1Password references), never raw values.
- **On the wire:** only short-lived tickets, bearer tokens, and DPoP proofs. Long-lived secrets never leave their owning machine.
- **Logging:** redact all credentials; never log query strings, tickets, or proofs. The relay logs only routing metadata (tunnel id, timestamps, byte counts), never payloads.

### 6.4 Threat model (summary)

| Threat | Mitigation |
|---|---|
| Ticket replay | Single-use `jti`, 5-min TTL, DPoP binding |
| Token theft | DPoP proof requires the private key; short TTLs |
| Tunnel-id guessing | 128-bit random slug; tunnel id alone grants nothing |
| Relay compromise | Relay is a TLS-terminating pipe; the operator can see plaintext (D24). Mitigations: ticket+DPoP auth, short-lived scoped sessions, and optional per-tunnel E2EE via pairing for users who want the relay to be a dumb pipe |
| Rogue paired device | Scoped by default; instant revocation |
| Local socket abuse | Local connections still authenticate at the app layer |
| Secret exfiltration | OS keychain; `op://` references; redacted logs |

---

## 7. Multi-machine

Multiple desktops and devices must be able to observe/control the same agent fleet, with **account/usage tracking across machines** (baseline bar).

### 7.1 Model

- **One account, many devices.** The user's account is the unit of identity and usage metering. Devices (desktop cores, phones, web) are registered to the account.
- **A device is a core or a client.** A *core* hosts worktrees/agents; a *client* (phone, second desktop, web) connects to a core. A second desktop can be both: a client of the primary core *and* a core hosting its own worktrees.
- **Fleet view.** The UI shows all cores and their worktrees/agents across machines, so the user can see and steer the whole fleet from any device.

### 7.2 Usage & account tracking

- **Local-first metering:** each core records usage events (agent turns, tokens, subagent fan-out, storage) into its local SQLite read model (`plan/06`), then syncs aggregates to the account service.
- **Cross-machine rollup:** usage is attributed to the account, not the device, so the user sees total usage across all machines.
- **Privacy:** usage data is aggregate and does not include file contents or prompts. Sync is encrypted and authenticated (OAuth + DPoP).
- **Billing/entitlements** are out of MVP scope (enterprise SSO/admin is a non-goal, `plan/00` §6), but the metering substrate is built now so it can be surfaced later.

### 7.3 Multi-core coordination

- Cores discover each other via the account service (or Tailscale). A client can connect to any core it is authorized for.
- Worktree/agent state is per-core (each core owns its processes), but the *read model* can be aggregated for a fleet dashboard. Cross-core orchestration (one agent on machine A depending on machine B) is deferred — see Open questions.

---

## 8. Testing

TDD at inception is non-negotiable (PLAN-CONTEXT §Testing). The remote layer gets unit, integration, and security tests, all gated in CI (fmt → clippy → unit+property → mutation → integration → component → e2e → coverage).

### 8.1 Unit tests

- **Ticket auth:** mint/validate/expire/revoke; single-use `jti`; scope enforcement; DPoP proof verification (valid, wrong key, stale nonce, replayed proof). Property-based (proptest) over ticket fields and DPoP nonce sequences.
- **Pairing:** one-time-code single-use, TTL expiry, scope defaulting, revocation.
- **Tunnel id:** entropy/uniqueness, no collision under proptest.
- **Reconnect state machine:** `connected → reconnecting → connected` transitions, backoff, idempotent-op replay.

### 8.2 Integration tests

- **SSH:** spin up a real SSH server (or a mock `multiplexer --remote` over a loopback SSH channel); exercise fs read/write/watch, git status/diff/commit, PTY spawn, port forwarding, and auto-reconnect (kill the SSH process mid-operation, assert clean resume).
- **Relay:** run a local relay (Cloudflare Worker via `wrangler dev`, or the Rust fallback); connect a core and a client through it; assert end-to-end message flow. For the **default TLS-terminating mode**, assert the relay can observe plaintext (honest D24 claim); for the **optional E2EE mode**, assert the relay sees only ciphertext.
- **Wire contract:** schema-verified JSON-RPC over each of the four transports (contract tests from `plan/04`).

### 8.3 Security tests

- **Replay:** capture a ticket/proof and replay it — must be rejected.
- **Expiry:** a ticket aged past 5 minutes must be rejected.
- **Scope escalation:** a ticket minted for `observe` must be rejected for `control`.
- **Tunnel-id brute force:** attempt many guesses — all must fail; rate limiting engages.
- **Secret hygiene:** assert no raw secrets in logs, configs, or wire captures (grep-based + property tests on the serializer).
- **Mutation testing:** the auth/DPoP/ticket modules must meet the ≥85% line / ≥80% branch / ≥70% mutation-score-killed gates.

### 8.4 E2E

- Drive the real app headless: pair a simulated phone, connect over relay, run an agent on a remote SSH worktree, edit a file, and assert the read model and UI reflect it. This beats T3 Code (which has no e2e).

---

## 9. Open questions (referenced, not decided)

Per PLAN-CONTEXT, these are pending user decisions and must not be decided unilaterally. This doc references them where they affect the remote/relay design; they are tracked in `plan/20-risks-and-open-questions.md`.

1. **Relay provider:** Cloudflare Worker + Durable Objects (recommended) vs self-hosted Rust relay vs Tailscale-funnel-as-default. This doc recommends Cloudflare but does not decide.
2. **Tailscale posture:** is Tailscale serve a supported first-class endpoint provider, or an optional add-on? (PLAN-CONTEXT lists it as an endpoint provider; scope is open.)
3. **Multi-core cross-machine orchestration:** is a fleet dashboard (aggregate read model) in MVP, or is per-core control only? Cross-core agent dependencies are deferred.
4. **Account service scope:** is account/usage tracking local-first with optional sync (recommended), or does it require a hosted account service in MVP? (Enterprise SSO/admin is a confirmed non-goal.)
5. **Web client:** the wire contract supports web, but the web client is a non-goal for MVP (`plan/00` §6). The relay tunnel is built to serve it later.
6. **Mobile stack** (native vs Expo/React Native, PLAN-CONTEXT Q2) affects the pairing/QR client implementation — see `plan/13`.
7. **Orca baseline scope** (Q7): SSH remote worktrees are baseline-bar; whether the full remote matrix (relay + Tailscale + multi-machine) ships in MVP or is staged is decided in `plan/19`.

---

## References

- `docs/PLAN-CONTEXT.md` — authoritative shared plan context (remote/relay line, baseline bar, security posture, testing).
- `plan/04-wire-contract.md` — JSON-RPC-over-WebSocket contract this layer transports and authenticates.
- `plan/06-orchestration-engine.md` — event-sourced read model used for reconnect replay and usage metering.
- `plan/13-mobile-app.md` — the primary remote client (pairing, observe/control).
- `plan/17-security-and-secrets.md` — OS keychain, `op://` references, DPoP/passkeys.
- `plan/19-roadmap-and-milestones.md` — staging of the remote matrix.
- `plan/20-risks-and-open-questions.md` — consolidated open decisions.
