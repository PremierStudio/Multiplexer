# 17 — Security & Secrets

**Status:** Draft for adversarial review
**Owner:** Security & secrets subagent
**Consistency:** Must not contradict `docs/PLAN-CONTEXT.md`. Conflicts are flagged in §10 Open questions, never silently diverged.
**Scope:** Local secrets, provider auth, remote auth, browser security, HAR privacy, agent sandboxing, supply chain, threat model, security testing, open questions.

> **Locked decisions applied:** This doc has been updated to conform to the locked decisions in `docs/DECISIONS.md`:
> **D23** (secrets session-cache model — no runtime `op://` resolution), **D24** (relay is a TLS-terminating pipe, not E2EE), **D25** (remote agent independently enforces trust), **D38** (keychain-only local tickets), **S4** (browser.cdp opt-in is a human approval), **S5** (HAR redaction from a curated allowlist + user patterns, not keychain scanning).

---

## 1. Purpose & principles

Multiplexer is a **server-centric runtime**: a single native Rust binary owns agent processes, terminals, git, filesystem, checkpoints, and HAR capture, and exposes them to thin clients over one authenticated JSON-RPC-over-WebSocket contract. That concentration of power — one process that can run arbitrary shell commands, drive the user's real browsers, and hold their provider credentials — makes security a **first-class architectural concern**, not a bolt-on.

This document defines how we keep secrets safe, authenticate clients and providers, sandbox the embedded harness, protect the browser/CDP surface, respect HAR privacy, and satisfy supply-chain obligations. It follows the machine's global secrets policy (see `C:\Users\gollum\Tools\agent-policy\SECRETS.md`) and the approved architecture in PLAN-CONTEXT.

### 1.1 Guiding principles

1. **Least privilege.** Every component gets the minimum credential, scope, and permission it needs. Permission modes (§6) default to the least permissive safe option.
2. **Defense in depth.** No single control is trusted alone: ticket auth is backed by DPoP, DPoP by passkeys; CDP is bound to loopback *and* token-gated; secrets live in the OS keychain *and* never in plaintext files.
3. **Fail closed.** On any ambiguity (missing ticket, unknown permission, unverified origin), deny. Never silently downgrade to a permissive path.
4. **No raw secrets in plaintext.** Secrets live in the OS keychain; configs carry only `op://Vault/Item/field` references (never raw values). This is a hard rule from the global secrets policy.
5. **Auditability.** Every sensitive action (permission grant, secret access, remote connection, browser launch) is recorded in the event-sourced read model so it can be reviewed and replayed.
6. **Privacy by default.** HAR and agent transcripts do not store sensitive request/response bodies unless the user opts in (§5).

---

## 2. Local secrets — OS keychain

### 2.1 Storage model

Local secrets (provider API keys, OAuth tokens, relay credentials, browser CDP tokens) are stored in the **OS keychain**, never in plaintext files or configs.

| Platform | Backend | Rust crate |
|---|---|---|
| Windows | Windows Credential Manager (DPAPI-backed) | `keyring` (or `windows-sys` CredWrite/CredRead) |
| macOS | Keychain Services | `keyring` / `security-framework` |
| Linux | Secret Service (libsecret) / kwallet | `keyring` |

We use the `keyring` crate behind a thin `SecretStore` trait in `multiplexer-auth`, so the backend is swappable and testable with an in-memory fake.

### 2.2 What goes in the keychain vs what stays out

| Stored in OS keychain | Never stored in plaintext |
|---|---|
| Provider API keys (Grok, OpenRouter, Claude, Codex, OpenCode) | Raw keys/tokens in `config.toml`, `.env`, `settings.json`, or committed files |
| OAuth access + refresh tokens | `op://` references resolved to raw values |
| Relay / remote credentials | Session tickets (short-lived, in-memory only) |
| Browser CDP tokens (see §4) | HAR bodies containing secrets (see §5) |
| SSH key passphrases (delegated to OS SSH agent) | Any secret in logs, telemetry, or crash dumps |

### 2.3 `op://` references in configs — references only, resolved via session cache

Per the global secrets policy, configs and catalogs may contain **only** `op://Vault/Item/field` references — never raw secret values, and never runtime live `op` reads. Multiplexer adopts this model for its own config surface:

- `config.toml` `[auth_provider.*]` entries reference secrets as `op://Vault/Item/field` (or a keychain service/account pair), **not** inline values.
- **No runtime `op://` resolution.** Multiplexer does **not** shell out to live `op` reads (`op read` / `op item get` / `op inject` / `op run`) and does **not** embed an unspecified 1Password SDK to resolve references at runtime. Those paths are banned by the global secrets policy.
- Instead, `SecretStore` reads from the **OS keychain** and the **session cache** (the `%LOCALAPPDATA%\mcp-session\*.env` model — user-only ACL, this-boot-only). `op://` references in configs are resolved through the **session-cache / refresh mechanism**: a cold-path refresh (user-initiated, once per boot) populates the session cache from 1Password; the hot path reads the cached values. Multiplexer never performs a live `op` read in its own runtime.
- **No secret value is ever written to disk by Multiplexer.** Resolution is in-memory for the lifetime of the process.

**Design rule:** the only place a raw secret may exist is (a) the OS keychain, (b) the session cache (user-only ACL, this-boot-only), or (c) transiently in process memory while in use. It is never serialized to a plaintext config, logged, or sent over the wire (except to the provider that owns it, over TLS).

### 2.4 Keychain + session-cache access control

- Keychain reads are **scoped to the Multiplexer process** where the platform allows (Windows Credential Manager entries are per-user; macOS keychain items can be access-controlled to the app).
- Multiplexer follows the machine's **session-cache model** for `op://`-referenced secrets: a cold-path refresh (user-initiated, once per boot) writes a user-only-ACL, this-boot-only session cache (mirroring `%LOCALAPPDATA%\mcp-session\*.env`); the hot path reads from that cache and the OS keychain. The session cache is **not** a plaintext store of long-lived secrets — it is a short-lived, boot-scoped convenience cache with user-only ACL, and it is never committed or shared.
- We do **not** cache decrypted secrets in plaintext files for "convenience" beyond the session-cache model above, and we never write `.env`-style caches for secrets that live in the OS keychain.
- On first use of a secret, if it is missing from the keychain and the session cache, we surface a clear onboarding prompt (never a raw paste into chat/logs) and direct the user to run the cold-path refresh.

---

## 3. Provider auth — OAuth

### 3.1 Providers and their auth models

| Provider | Auth model | Token storage | Refresh |
|---|---|---|---|
| Grok (in-process) | API key / OAuth (per `[auth_provider.*]` config) | OS keychain | n/a or OAuth refresh |
| DeepSeek V4 Flash (OpenRouter) | OpenRouter API key | OS keychain | n/a (static key) |
| Claude | OAuth 2.1 + PKCE (per Superset precedent) | OS keychain | Refresh token rotation |
| Codex | OAuth (GitHub-backed) | OS keychain | Refresh token |
| OpenCode | Provider-agnostic (Models.dev) | OS keychain | Per-provider |

### 3.2 OAuth flow (desktop + mobile)

Because Multiplexer is server-centric, the **server** owns the OAuth flow; clients are thin shells that render the flow and forward intents.

1. Client calls `auth.login { provider }`.
2. Server generates a PKCE `code_verifier`/`code_challenge`, opens the provider's authorization URL in the **system browser** (reusing `multiplexer-browser`), and starts a local callback listener on a **random loopback port**.
3. Provider redirects to `http://127.0.0.1:<random_port>/callback?code=...&state=...`.
4. Server validates `state` (CSRF) and exchanges `code` + `code_verifier` for tokens.
5. Access token + refresh token are stored in the OS keychain (§2). The refresh token is stored **encrypted at rest** by the keychain backend.
6. Server emits `auth.status` so the client updates its UI.

**Security requirements for the OAuth listener:**
- Bind to `127.0.0.1` only — never `0.0.0.0` or a LAN interface.
- Use a **random per-flow port** (not a fixed well-known port) to reduce squatting/race risk.
- Validate `state` strictly; reject mismatches.
- Use PKCE (RFC 7636) for all public clients (mobile/web) and where the provider supports it for confidential clients.
- The callback listener is **ephemeral** — it exists only for the duration of the flow and is torn down afterward.

### 3.3 Token refresh & rotation

- Refresh tokens are rotated on use where the provider supports it; a rotated token invalidates the previous one, bounding the window of a leaked token.
- Refresh is triggered lazily on a 401/`auth_expired` and proactively on a sliding window (e.g., refresh when < 20% of lifetime remains).
- On refresh failure, the server clears the stored token, emits `auth_expired`, and prompts re-login. It never silently retries in a loop (rate-limit / lockout protection).
- Access tokens are held **in memory only** and never written to disk.

### 3.4 Logout & revocation

- `auth.logout` revokes the refresh token at the provider (where supported), removes the keychain entry, and clears in-memory tokens.
- Logout is recorded in the read model for audit.

---

## 4. Remote auth — tickets, DPoP, passkeys

Remote/relay is specified in `plan/14-remote-and-relay.md`; this section covers the **security** of that surface. The transport model: local + paired + relay tunnel + SSH, WebSocket ticket auth (5-min TTL), Tailscale serve.

**Relay honesty (D24):** the relay is a **TLS-terminating pipe** — the relay operator (Cloudflare or self-hosted) **can see plaintext** application data. Multiplexer does **not** claim end-to-end encryption for the relay. Mitigations that make this acceptable: ticket/DPoP auth (short-lived, scoped, single-use), short-lived scoped sessions, and an **optional** per-tunnel session-key E2EE layer established via the pairing handshake. The default claim is honest TLS-terminating, not E2EE.

### 4.1 Ticket auth (5-min TTL)

- A **ticket** is a short-lived (5-min TTL), single-use credential used to bootstrap an authenticated WebSocket session (per PLAN-CONTEXT and the wire contract §6).
- Ticket payload: `{client_id, scope, exp, nonce}` signed by the server (HMAC-SHA256 with a server-held key, or Ed25519).
- Issuance is out-of-band:
  - **Local:** server writes the ticket to the **OS keychain only** on startup; the desktop client reads it directly. No plaintext local token file is used, and no network round-trip.
  - **Remote/relay:** the user authenticates via OAuth/passkey on the relay's web page, which returns a ticket to the client.
  - **SSH:** the SSH session itself authenticates; a ticket is minted inside the tunnel.
- **Single-use:** a ticket is consumed on first successful handshake; replay of the same ticket is rejected (`ticket_invalid`).
- **TTL enforcement:** `exp` is checked on every handshake; expired tickets are rejected. The 5-min TTL bounds the window a stolen ticket is usable; the established session is the long-lived credential.

### 4.2 DPoP (Demonstration of Proof-of-Possession)

- For **any non-loopback** connection, the client proves possession of its private key by binding each request to a DPoP proof (JWT with `htm`/`htu`/`jti`), preventing ticket replay by a third party (per wire contract §6.3).
- The server verifies the DPoP proof's `htm` (HTTP method) and `htu` (HTTP URI) match the actual request, and that the `jti` is not replayed.
- **Local loopback** uses the ticket alone (trusted transport); DPoP is mandatory for remote/relay. (Strictness on local is an open question — §10.)

### 4.3 Passkeys

- Passkeys (WebAuthn) are the **primary remote auth factor** (per PLAN-CONTEXT).
- The relay's web page offers passkey sign-in; a successful assertion yields a ticket (§4.1) bound to the authenticated identity.
- Passkey credentials are stored by the OS/platform authenticator (Windows Hello, iCloud Keychain, etc.) — Multiplexer never stores passkey private keys.
- Discovery: `publicKeyCredential` / conditional UI on the relay page; the mobile app uses the platform authenticator for the same flow.

### 4.4 Session lifecycle & re-auth

- Sessions are long-lived but re-validated on a sliding window; a client that loses its session receives `auth_expired` and must re-handshake with a fresh ticket.
- On disconnect, the server invalidates the session's ticket and any outstanding DPoP nonces.
- **Scope enforcement:** a session carries a `scope` (e.g., `observe` vs `control`); the server rejects any method outside the session's scope with `permission_denied`. The mobile app may be granted observe-only by default.

---

## 5. Browser security (CDP surface)

Launching the user's **real installed browsers** with remote debugging is a significant security surface (differentiator #3; full detail in `plan/11-system-browser-integration.md`). We must never expose the debugging port to the network or to other processes.

### 5.1 CDP launch hardening

| Control | Requirement |
|---|---|
| **Loopback-only binding** | Launch the browser with `--remote-debugging-address=127.0.0.1` (or equivalent per browser). Never bind to `0.0.0.0`/LAN. |
| **Random port** | Use a **random, ephemeral** debugging port per launch (e.g., `--remote-debugging-port=0` and read the actual port from `DevToolsActivePort`), never a fixed well-known port. |
| **Token / auth** | Where supported, pass a per-launch token (`--remote-debugging-token` or DevTools `Host: header` auth) so a random process on the same host cannot connect. |
| **No remote exposure** | The CDP endpoint is reachable only from the Multiplexer server process on loopback. It is never forwarded, tunneled, or exposed via the relay. |
| **Profile isolation** | Launch with a dedicated, disposable user-data-dir (or a user-chosen profile) so CDP does not silently attach to the user's everyday browsing session. |
| **Teardown** | On close/crash, terminate the browser process tree and remove the temp profile. |

### 5.2 CDP access control

- The `browser.cdp` raw passthrough (wire contract §4.9) is **privileged**: only sessions with `control` scope may invoke it.
- The server validates CDP method/params against an allowlist where feasible and rejects dangerous methods (e.g., `Browser.setDownloadBehavior` to arbitrary paths, `Page.navigate` to `file://` unless explicitly allowed) unless the user opts in.
- The `browser.cdp` escape hatch ("user opts in") is gated on a **human approval**, not an agent action. The agent cannot self-grant the opt-in; only a human user can approve it through the approval flow (§7.2), and the grant is scoped, audited, and recorded in the read model.
- CDP traffic stays **in-process** (server ↔ browser over loopback); it is never streamed to thin clients. Clients see only the sanitized `browser.*` RPC surface.

### 5.3 Browser import & detection

- Detecting installed browsers reads registry/known paths only — no execution of untrusted code.
- Launching uses the browser's own binary path; we do not download or bundle a browser (no bundled Chromium).

---

## 6. HAR privacy

The built-in HAR profiler/replayer (differentiator #4; full detail in `plan/12-har-profiler-replayer.md`) captures network traffic via CDP. Network bodies routinely contain **secrets** (auth headers, cookies, tokens, form data). We must not store them by default.

### 6.1 Default: no sensitive bodies

- **By default, HAR capture stores metadata only** — URLs (with query strings redacted by default), status, timing, sizes, MIME type — and **not** request/response bodies.
- **Bodies are opt-in.** The user must explicitly enable body capture (per session or globally) before any request/response body is retained.
- Even when bodies are enabled, **sensitive headers are redacted by default**: `Authorization`, `Cookie`, `Set-Cookie`, `Proxy-Authorization`, `X-Api-Key`, `X-Auth-Token`, and any header matching a configured secret pattern.

### 6.2 Redaction options

| Option | Behavior |
|---|---|
| `metadata_only` (default) | No bodies; query strings redacted; sensitive headers redacted |
| `bodies_redacted` | Bodies captured but secrets scrubbed (regex + curated allowlist + user-configured patterns) |
| `bodies_full` | Full bodies captured (explicit user opt-in, per session) |

- Redaction is applied **at capture time** in the server, before anything is written to the read model or disk — not as a post-hoc scrub that could leak in the gap.
- Redaction patterns are seeded from a **curated allowlist** (a built-in header/pattern list) plus **user-configured patterns** — **not** by scanning all OS keychain items. Multiplexer does not enumerate the keychain to seed redaction; only explicitly curated and user-supplied patterns are used.

### 6.3 Storage & sharing

- HAR documents are stored locally in the read model / on disk with the same ACL as other local data; they are **not** uploaded anywhere by default.
- HAR **replay** runs locally against the recorded session; it does not re-send captured credentials unless the user explicitly opts in (and even then, redaction applies).
- If HAR is ever shared (export), the export is redacted per §6.2 and the user is warned before any body-bearing export.

---

## 7. Agent sandboxing & permission modes

The embedded harness (`xai-grok-shell` + `xai-grok-tools` + `xai-grok-workspace`) runs shell commands and mutates the filesystem/git on the user's behalf. This is the highest-risk surface: a malicious or buggy agent prompt can execute arbitrary commands. We scope permissions through **permission modes** and the approval flow.

### 7.1 Permission modes

| Mode | Behavior | Default for |
|---|---|---|
| **Supervised** | Every sensitive tool call (shell, fs write, git push, network) requires explicit user approval via `approval.respond` | New/untrusted sessions, remote/mobile control |
| **Auto-accept edits** | File edits within the active worktree are auto-accepted; shell/network/git-push still require approval | Trusted local sessions |
| **Auto** | All worktree-scoped operations auto-accepted; out-of-scope operations (outside worktree, network, destructive) still prompt | Power users, explicit opt-in |
| **Full access** | Everything auto-accepted, including out-of-worktree and destructive operations | Explicit, per-session opt-in only; never default |

- The mode is a **per-session** setting, chosen at `session.start` and changeable only by the user (never by the agent).
- **Scope is enforced server-side**, not by the agent. The agent cannot escalate its own mode; the server rejects any tool call outside the current mode's scope with `permission_denied`.

### 7.2 Approval flow

- A tool call requiring approval emits a `permission_request` event (wire contract §4.3) with `{approval_id, tool, summary}`.
- The user responds `allow / deny / allow_once / allow_always` via `approval.respond`.
- `allow_always` records a **scoped rule** (tool + path/pattern) in the read model; it never means "allow everything."
- Approvals are **audited**: every grant/denial is an event in the read model, replayable for review.
- **Timeout:** pending approvals expire (configurable, default e.g. 5 min); on timeout the call is denied, not silently allowed.

### 7.3 Worktree confinement

- Agent fs/git operations are confined to the **active worktree root** by default (mirrors the wire contract's server-resolved `fs.*` path model).
- Path traversal outside the worktree is rejected (`path_invalid`).
- Out-of-worktree access requires an explicit mode/approval step.

### 7.4 Sandbox crate disposition

- `xai-grok-sandbox` may rely on Unix isolation (namespaces, seccomp, `chroot`) that is unavailable on Windows. Per `plan/03`, we **evaluate** it: gate behind `cfg(unix)` and provide a no-op/limited Windows sandbox or defer in MVP (flagged in §10 and Plan 20).
- On Windows, process-tree cleanup uses Job Objects so a runaway agent process tree is terminated on session stop/crash.

---

## 8. Supply chain

### 8.1 Vendored grok-build (Apache 2.0) obligations

Per PLAN-CONTEXT and `plan/03`, we vendor a fork of `xai-org/grok-build` (Apache 2.0) under `third_party/`. Obligations:

- **Retain** upstream license and copyright notices in the vendored source.
- Ship a **`THIRD-PARTY-NOTICES`** file in our distribution listing grok-build (and all other third-party deps) and their licenses.
- **Document provenance:** a `SOURCE_REV` file records the exact upstream commit our fork is based on; our fork's README states it is derived from `xai-org/grok-build`.
- Do not misrepresent the fork as upstream.

### 8.2 Dependency auditing

- **`cargo audit`** runs in CI on every change; a vulnerability in any direct or transitive dependency fails the gate.
- **`cargo deny`** enforces license allowlist and duplicate-crate policies.
- **`cargo outdated`** / Dependabot-style updates are reviewed on a cadence; security-relevant updates are prioritized.
- Vendored crates are audited as part of the fork sync (each upstream merge re-runs the audit against the merged tree).
- **Supply-chain integrity:** pin exact versions / `Cargo.lock` is committed; CI builds from the lockfile. Vendored fork is committed (self-contained), so builds have no network dependency on GitHub.

### 8.3 Secrets in the repo

- `.gitignore` excludes `.env`, keychain dumps, HAR bodies, and any local secret files.
- A **secret-scanning** step (e.g., `gitleaks` or `trufflehog`) runs in CI and pre-push to catch accidental commits of keys/tokens.
- Configs contain only `op://` references, never raw values (§2.3).

---

## 9. Threat model

### 9.1 Threat enumeration & mitigations

| # | Threat | Vector | Mitigation |
|---|---|---|---|
| T1 | **Malicious agent commands** | A prompt (or prompt-injection from fetched content) causes the agent to run destructive shell/fs/git commands | Permission modes (§7), worktree confinement, approval flow, server-side scope enforcement, audit trail |
| T2 | **Browser CDP exposure** | Debugging port bound to LAN / fixed port / no token → another process or host drives the user's browser | Loopback-only binding, random port, per-launch token, no remote forwarding (§5) |
| T3 | **Relay MITM / ticket replay** | Attacker intercepts or replays a ticket over the relay | TLS (wss) everywhere non-loopback, 5-min single-use tickets, DPoP proof-of-possession, passkeys (§4) |
| T4 | **Secret exfiltration** | Agent reads a secret file and sends it to a provider; or a secret is logged/committed | Keychain-only storage, no raw secrets in configs, HAR redaction, secret-scanning in CI, permission modes gate fs reads of sensitive paths |
| T5 | **OAuth callback hijack** | Attacker races the loopback callback port or forges `state` | Random per-flow port, `127.0.0.1` only, strict `state` validation, PKCE (§3) |
| T6 | **Path traversal** | Client/agent sends `../` or absolute paths to escape the worktree | Server-resolved paths, traversal rejection (`path_invalid`), worktree confinement (§7.3) |
| T7 | **HAR body leak** | Captured network bodies contain tokens that get stored/shared | Metadata-only default, redaction at capture, opt-in bodies, warned export (§6) |
| T8 | **Supply-chain compromise** | Malicious/compromised dependency or vendored crate | `cargo audit`/`deny` in CI, committed lockfile, vendored fork provenance, secret-scanning (§8) |
| T9 | **Local privilege / keychain theft** | Another local process reads our keychain entries or our in-memory tokens | Keychain access control scoped to process, tokens in memory only, no plaintext caches (§2) |
| T10 | **Remote session takeover** | Stolen long-lived session credential | Sliding re-validation, scope enforcement, DPoP binding, session invalidation on disconnect (§4) |
| T11 | **Prompt injection via fetched content / Design Mode** | Web content or browser DOM steers the agent into harmful actions | Permission modes gate tool calls; fetched content is treated as untrusted input; approvals for sensitive ops |
| T12 | **Denial of service** | A client floods the server / a runaway agent spawns unbounded subagents | Backpressure (wire contract §8), subagent budget caps (grok-build), rate limiting, process-tree cleanup |
| T13 | **Compromised local core driving a remote host** | A malicious/buggy local core issues commands to an SSH `--remote` agent, or a malicious remote host abuses the tunnel | The remote agent **independently enforces** permission modes, worktree confinement, and approval gating on the remote host (D25). It is **not** a dumb executor that trusts the local core implicitly — it re-validates scope/approvals locally before acting |

### 9.2 Trust boundaries

```
[ OS keychain ] ──(SecretStore)── [ multiplexer-auth ]
        ▲                              │
        │                              ▼
[ Provider APIs ] ◄── TLS ── [ multiplexer-provider ] ◄── [ multiplexer-server ]
                                                      │  ▲
        [ thin clients: desktop / mobile / web ] ─────┘  │ (JSON-RPC/WS, ticket+DPoP)
                                                         ▼
[ System browser ] ◄── loopback CDP (token) ── [ multiplexer-browser / multiplexer-har ]
[ Agent processes ] ◄── permission modes ── [ embedded harness ]
[ Remote host ] ◄── SSH --remote ── [ remote agent: independently enforces
                                      permission modes / worktree confinement /
                                      approval gating (D25) ]
```

- **Trusted:** the server process, the OS keychain, the local desktop client on loopback.
- **Semi-trusted:** remote/mobile clients (scoped, DPoP-bound, observe-by-default).
- **Independently enforcing:** the SSH `--remote` agent (D25) — it does **not** trust the local core implicitly; it re-enforces permission modes, worktree confinement, and approval gating on the remote host before acting.
- **Untrusted:** provider responses, fetched web content, browser DOM, any network peer.

---

## 10. Security testing

Security is tested at every layer of the TDD-at-inception gate chain (PLAN-CONTEXT §Testing). CI gates: fmt → clippy → unit+property → mutation → integration → component → e2e → coverage.

### 10.1 Unit & property tests

- **Auth:** ticket sign/verify, TTL enforcement, single-use (replay rejected), scope checks, DPoP proof verification (valid/invalid `htm`/`htu`/`jti`), passkey assertion handling.
- **Secrets:** `SecretStore` fake round-trips; `op://` reference resolution; redaction of sensitive headers/patterns (proptest over header/body corpora).
- **Permission modes:** the mode state machine (supervised → auto-accept → auto → full) transitions; scope enforcement rejects out-of-scope calls; `allow_always` rule matching.
- **Path handling:** proptest over path inputs asserting traversal is rejected and paths resolve inside the worktree.
- **Mutation:** auth, secrets, and permission crates must hit the CI mutation gates (≥70% killed).

### 10.2 Integration tests

- **Ticket validation:** a mock server rejects expired, replayed, and wrong-scope tickets; a valid ticket establishes a session.
- **Browser port security:** launch a real (or mocked) browser and assert the CDP endpoint binds to `127.0.0.1` on a random port with a token, and that a second process cannot connect without the token.
- **Secret handling:** assert no secret value is written to disk, logged, or present in a HAR metadata-only capture; assert redaction strips configured secrets from bodies.
- **OAuth flow:** mock provider; assert `state` validation, PKCE exchange, keychain storage, and refresh/rotation.

### 10.3 E2E / component

- **E2E:** drive the real app headless; a scripted malicious-agent scenario must be blocked by the permission mode and produce an audited denial.
- **Component (GPUI):** the approval dialog renders correctly and only the permitted decision is forwarded.

### 10.4 Continuous security

- `cargo audit` + `cargo deny` + secret-scanning run in CI on every change (§8).
- A dedicated security test job runs the auth/browser/secrets suites on the Windows CI runner.

---

## 11. Open questions (flag, don't decide unilaterally)

1. **DPoP strictness on local loopback** — ticket-only (proposed) vs DPoP also on localhost. Wire contract §6.3 proposes ticket-only on loopback; confirm.
2. **Sandbox disposition on Windows** — gate `xai-grok-sandbox` behind `cfg(unix)` with a no-op/limited Windows sandbox, or defer entirely in MVP (ties to Plan 03 / Plan 20).
3. **Permission mode defaults** — whether new sessions default to `supervised` or `auto-accept edits`; and whether remote/mobile default to observe-only scope.
4. **HAR body capture default** — confirm `metadata_only` as the shipped default and the exact redaction header list.
5. **Keychain backend** — confirm `keyring` crate choice and the Windows Credential Manager backend.
6. **`op://` vs keychain service/account** — whether configs reference secrets via `op://Vault/Item/field` (global policy style) or native keychain service/account pairs, or both.
7. **Secret-scanning tool** — `gitleaks` vs `trufflehog` vs another; confirm CI/pre-push integration.
8. **THIRD-PARTY-NOTICES / legal review** — confirm the notices approach and whether legal review is required (ties to Plan 03 §10.1).

**Flagged consistency note:** this doc is consistent with PLAN-CONTEXT (OS keychain, OAuth, passkeys/DPoP, 5-min ticket TTL, permission modes, vendored Apache 2.0 fork). If any PLAN-CONTEXT decision flips (e.g., stack, remote model), the affected sections (§3, §4, §7) must be revisited.
