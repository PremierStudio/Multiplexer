# 13 — Mobile App

> **Status:** Authoritative plan doc. Consistent with `docs/PLAN-CONTEXT.md` (the shared plan context). If any statement here conflicts with PLAN-CONTEXT, PLAN-CONTEXT wins and the conflict should be flagged in `plan/20-risks-and-open-questions.md`.
>
> **Purpose:** Define the paired mobile companion — the thin client that lets a user control and observe agents from their phone. This doc covers the requirement, the stack decision, the shared wire contract, core features, remote access, mobile UI, and testing.
>
> **Dependencies:** `plan/04-wire-contract.md` (the JSON-RPC-over-WebSocket contract), `plan/14-remote-and-relay.md` (tunnel/SSH/Tailscale + WebSocket ticket auth), `plan/10-ui-pane-system.md` (design system), `plan/17-security-and-secrets.md` (keychain, passkeys/DPoP), `plan/15-testing-strategy.md` (contract testing).

> **Locked decisions applied:** This doc has been reconciled against `docs/DECISIONS.md`. Applied: **D2** (mobile stack locked to Expo / React Native, not native SwiftUI/Kotlin), **D8** (mobile app is part of the MVP — MVP = Phases 1–4 — and cannot slip past it), **D37** (pairing issues a long-lived device credential minted into short-lived tickets). These supersede any earlier "open question" or conflicting wording in this doc.

---

## 1. The requirement

**A paired mobile app is required.** This is an explicit user ask and a hard product commitment — it appears in PLAN-CONTEXT as a core differentiator (#8), in the baseline bar ("mobile companion"), and in the North Star metric ("steer them from their phone"). It is not optional and not a stretch goal.

The mobile app is the **thin client** for the same server-centric runtime that powers the desktop. The desktop's single native Rust binary owns agent processes, terminals, git, filesystem, checkpoints, and HAR capture; the phone is a remote control surface over the same authenticated **JSON-RPC-over-WebSocket** contract. The phone does **not** run agents, does not hold the runtime, and does not duplicate state — it observes and steers the desktop's core.

### 1.1 Why the phone matters

| Scenario | What the user does from the phone |
|---|---|
| **Approvals** | A long-running agent hits a tool-approval gate; the user taps **Accept** / **Reject** from anywhere instead of racing back to the desk |
| **Notifications** | Agent finished, errored, or needs input — a push notification arrives with one-tap deep-link into the session |
| **Live activity** | Watch a fan-out of subagents work in real time (turn progress, tool calls, diffs) while away from the desk |
| **Remote access** | Connect to the desktop's core from outside the LAN via relay tunnel / SSH / Tailscale |
| **Terminal work** | Read terminal output and send keystrokes for quick interventions on a running session |
| **Account / usage** | Check usage metering, active sessions, and account status on the go |

### 1.2 Relationship to the desktop

The mobile app is a **peer client**, not a mirror. It connects to the same core as the desktop and can be used **concurrently** with it (e.g., the desktop runs the editor while the phone approves a gate). It is deliberately read-mostly with a small, high-value write surface (approvals, interrupts, user input, terminal keystrokes, session start/stop). This keeps the mobile surface calm and safe while still being genuinely useful.

---

## 2. Mobile stack decision (LOCKED — D2)

**The mobile stack is locked to Expo / React Native (iOS + Android), NOT native SwiftUI/Kotlin.** This resolves PLAN-CONTEXT open question **#2** (native vs Expo/RN). The decision is final (D2 in `docs/DECISIONS.md`) and supersedes any "open question" framing in this doc or in `plan/20-risks-and-open-questions.md`.

### 2.1 Rationale

- **The mobile app is a thin client.** It renders a small, well-defined surface (status, approvals, notifications, terminal, usage) over the stable JSON-RPC-over-WebSocket contract. Neither stack is stressed by this workload — performance is a non-issue for either, so native's runtime edge buys us nothing here.
- **The contract is the real asset, not the native UI.** The durable investment is the shared JSON-RPC client and the contract-test suite, not platform-specific rendering. Expo/RN gives us **one codebase for both platforms** (iOS + Android), **faster shipping**, and the shared contract + mock-server testing gives offline determinism.
- **Native would only matter if we needed heavy platform-specific rendering**, which we don't — the terminal is rendered server-side / via the contract, and the mobile surface is read-mostly with a small write surface (approvals, interrupts, user input, terminal keystrokes).
- **Team fit:** the Rust core team has no Swift/Kotlin today; JS/TS overlaps with any future web-client work and lowers ramp-up.

### 2.2 What this means concretely

| Aspect | Decision (D2) |
|---|---|
| **Stack** | Expo / React Native (iOS + Android) |
| **Codebases** | One shared JS/TS codebase |
| **Contract reuse** | One shared JS JSON-RPC client library reused across iOS/Android (and web) |
| **Push notifications** | Expo Notifications (wraps APNs/FCM) |
| **Build/release** | One Expo toolchain; EAS build; OTA updates |
| **Performance** | More than adequate for a thin client |
| **Native escape hatch** | Expo modules / config plugins for any native gaps (occasional, not the default) |

> **Note (D2):** The desktop UI stays **Rust + GPUI**. Only the mobile thin client is React Native.

The mobile app **must** be a thin client over the same JSON-RPC-over-WebSocket contract as desktop/web (§3), and it **must** be tested against the shared contract with a mock server for offline determinism (§7).

---

## 3. Shared contract

The mobile app uses **the same wire contract as desktop and web** — the authenticated **JSON-RPC-over-WebSocket** contract defined in `plan/04-wire-contract.md`. There is **no mobile-specific protocol**. This is the architectural linchpin of the whole product: one server, many thin clients, one contract.

### 3.1 What the mobile client consumes

The mobile app implements a **client-side subset** of the contract — it does not need every method, but it must speak the same framing, auth, and event model:

| Contract surface | Mobile use |
|---|---|
| **Session list / status** | Enumerate active sessions, per-session state (running / awaiting-approval / awaiting-input / done / error) |
| **Live event stream** | Subscribe to `ProviderEvent` / orchestration events for live activity (turn progress, tool calls, diffs) |
| **Approval methods** | `approval_respond` (accept / reject) — the core mobile write |
| **User-input methods** | `user_input_respond` for sessions blocked on the user |
| **Interrupt** | `interrupt_turn` / `session_stop` for steering |
| **Terminal** | Read terminal output stream; send keystrokes |
| **Usage / account** | Read usage metering and account status |
| **Checkpoint** | `checkpoint_revert` for a quick "undo to last good turn" from the phone |

### 3.2 Contract testing with a mock server

Because the mobile app is a thin client, its correctness is dominated by **contract conformance**, not business logic. The plan is:

- **Schema-verified contract on both sides** (per PLAN-CONTEXT testing: "JSON-RPC wire contract schema-verified on both sides"). The mobile client is tested against the **same schema** the desktop core is tested against.
- **A mock server** implements the contract with deterministic, scripted responses (no real agents, no real filesystem). The mobile client runs its full integration suite against the mock server **offline** — no network, no desktop, no flakiness.
- **The same mock server** is shared with the web client and used in desktop contract tests, so all three clients are proven against one canonical implementation of the contract.

This gives **offline determinism**: mobile tests are repeatable in CI and on a developer laptop with no infrastructure.

---

## 4. Core mobile features

### 4.1 Live agent status

- A session list showing every active session on the paired core, with per-session state, model, worktree, and elapsed time.
- A **live activity view** per session: streaming turn progress, tool calls, and diff summaries as they happen (driven by the event stream from §3).
- A **fan-out dashboard** (read-only) mirroring the desktop's orchestration view — see dozens of subagents at a glance, each with state and progress.

### 4.2 Approvals (accept / reject)

- The highest-value mobile action. When a session enters `awaiting-approval`, the phone surfaces a rich approval card: what tool is being invoked, with what arguments, and the surrounding context.
- **Accept** / **Reject** are one-tap actions backed by `approval_respond`. Reject may optionally include a short reason that routes back to the agent.
- Approvals are **push-notified** (§4.3) so the user is never blocked on a gate they didn't see.

### 4.3 Notifications / push

- **Push notifications** for the events that matter: approval required, session finished, session errored, user input required, checkpoint available.
- Each notification carries a **deep link** into the relevant session (e.g., `multiplexer://session/<id>/approval`), so a tap lands the user on the exact action.
- Notification delivery uses **Expo Notifications** (per D2, which wraps APNs/FCM). The **notification payload schema** is stack-independent and defined once.

### 4.4 Live activity

- Streaming, low-latency view of agent activity: current turn, tool calls, file writes, diff hunks, terminal output.
- **Diff preview** for files the agent is editing — read-only on mobile, with an inline "approve this change" affordance where applicable.
- Designed to be glanceable: a calm summary by default, with progressive disclosure into raw detail (consistent with the "clean / progressive disclosure" design principle).

### 4.5 Terminal view

- A read-mostly terminal view of the session's terminal pane: stream output, and send keystrokes for quick interventions.
- **Not** a full Ghostty-class terminal on mobile — a lightweight, scrollable output view with a single input line for short commands. Full terminal work stays on the desktop; the phone covers "check the log / send one command."

### 4.6 Account / usage tracking

- View usage metering (tokens, sessions, time) and account status, matching the baseline "account / usage tracking" requirement.
- Read-only on mobile; account management (billing, plans) lives on the desktop/web surface.

### 4.7 Remote access (pair with desktop)

- The phone connects to the desktop's core over the remote/relay layer (§5): local (same LAN), paired (direct), relay tunnel, or SSH, with Tailscale as an option.
- **Pairing** establishes a trusted relationship between the phone and a specific desktop core, producing the WebSocket ticket used for auth.

---

## 5. Remote access

The phone must reach the desktop's core from anywhere. This is covered in depth by `plan/14-remote-and-relay.md`; this doc specifies the mobile-facing requirements and how the phone participates.

### 5.1 Connection modes

| Mode | When | How the phone connects |
|---|---|---|
| **Local** | Same LAN | Direct WebSocket to the desktop core's local address |
| **Paired** | Direct phone↔desktop | Direct connection after pairing (e.g., mDNS discovery + ticket) |
| **Relay tunnel** | Outside LAN, no inbound ports | Phone connects to a relay; desktop core dials out to the same relay; traffic is relayed |
| **SSH** | User has SSH access to the host | Phone connects over the SSH transport to the core |
| **Tailscale** | User runs Tailscale | Phone and desktop are on the same tailnet; connect over the tailnet address |

### 5.2 Pairing credential model (D37)

Remote connections are authenticated with **WebSocket ticket auth** (per PLAN-CONTEXT: "WebSocket ticket auth (5-min TTL)"). The pairing credential model is reconciled with `plan/14` (D37):

1. **QR encodes a one-time code.** The desktop core shows a pairing screen with a QR code encoding a pairing URL — `multiplexer://pair?host=<host>&port=<port>&code=<one-time-code>` (or the user types the code on a second desktop). The one-time code is single-use, short-TTL (e.g. 2 minutes), and high-entropy.
2. **Exchange.** The phone presents the one-time code to the core over the local network (or via the relay). The core validates it (single-use, short TTL), then issues a **long-lived device credential** — a **device id + a stored secret** — and a device profile (name, capabilities, default scope).
3. **Store.** The phone stores the device credential (device id + secret) in its **OS keychain** (never plaintext). The core stores the device's public identity and scope in its own keychain-backed store.
4. **Mint into short-lived tickets.** The long-lived device credential is **never used directly on the wire**. On each connection, the phone mints its device credential into a **short-lived ticket (5-min TTL)** used to authenticate the WebSocket handshake. Tickets are single-use and time-boxed, so a leaked ticket is low-risk.
5. **Passkeys / DPoP** (per PLAN-CONTEXT auth) protect the remote identity layer; the phone's keychain holds the long-lived secret material, never a raw credential in plaintext and never a long-lived bearer secret on the wire.

> **Note (D37):** This matches `plan/14` §5.1/§5.2 exactly — the long-lived device credential is the trust root, and only short-lived tickets (plus DPoP proofs) travel over the wire.

### 5.3 Security posture

- The phone holds **no agent state** and **no provider secrets** — only the pairing identity and tickets.
- All traffic is TLS (WSS). Tickets are short-lived and single-use. The core enforces per-phone authorization (which sessions a given phone may observe/steer).
- Full detail in `plan/17-security-and-secrets.md`; the mobile app follows the same "OS keychain for local secrets, never raw values in configs" rule.

---

## 6. Mobile UI

The mobile UI is a **clean, native mobile UI consistent with the desktop design system** (`plan/10-ui-pane-system.md`). It is not a scaled-down desktop — it is a purpose-built mobile surface that shares the design language (typography, color, spacing, iconography, motion) with the desktop.

### 6.1 Design principles applied

- **Beautiful:** crisp typography, deliberate spacing, smooth 60fps+ transitions — the same aesthetic bar as the desktop.
- **Clean / progressive disclosure:** calm by default; a session list and a few glanceable cards. Raw detail (logs, full diffs) is revealed on demand.
- **Blazing fast:** the phone is a thin client; UI must feel instant even on a slow link, with optimistic updates for approvals and clear loading/offline states.

### 6.2 Screen map (proposed)

| Screen | Purpose |
|---|---|
| **Sessions** | Home: list of active sessions with state badges; tap to open |
| **Session detail** | Live activity, approvals, diff preview, terminal, interrupt/stop |
| **Approval card** | Rich accept/reject surface with tool context (often reached via push deep-link) |
| **Notifications** | Inbox of past notifications, each deep-linking to its session |
| **Terminal** | Read-mostly output view + single-line input |
| **Usage / account** | Metering and account status |
| **Pairing / settings** | Pair with a desktop core, manage connections, notification prefs |

### 6.3 Design-system consistency

- The mobile app consumes the **shared design tokens** (colors, type scale, spacing, radii, motion) defined for the desktop, so the two surfaces feel like one product.
- Design tokens are shared as a **JS module** (per D2, Expo/RN), generated/exported from the single source of truth defined once.

---

## 7. Testing

Per PLAN-CONTEXT testing strategy ("Mobile: native unit + integration against shared contract; mock server for offline determinism"), the mobile app is tested at three levels:

### 7.1 Native unit tests

- Unit tests for the mobile client's own logic: the JSON-RPC client framing/parsing, the state store (session list, per-session state machine), approval/notification payload handling, and the pairing/ticket client.
- These run on-device/emulator in CI and are stack-appropriate (Jest / React Native Testing Library for the Expo/RN client).

### 7.2 Integration tests against the shared contract

- The mobile client runs its full integration suite against the **shared mock server** (§3.2) — the same deterministic, scripted contract implementation used by the web client and desktop contract tests.
- Scenarios: connect → list sessions → receive live events → approve a gate → reject a gate → send user input → interrupt → observe terminal output → read usage.
- Because the mock server is offline and deterministic, these tests are **repeatable and flake-free** in CI.

### 7.3 Contract conformance

- The mobile client is **schema-verified against the same JSON-RPC wire schema** as the desktop core, so a contract change that breaks the phone is caught in CI, not in the field.
- The shared contract-test suite is the single source of truth for "does this client speak the contract correctly."

### 7.4 CI placement

- Mobile tests run in the same CI pipeline as the rest of the product (fmt → clippy → unit+property → mutation → integration → component → e2e → coverage for the Rust core; the mobile suite runs alongside as a client-conformance gate). No blind CI.

---

## 8. Open questions

These are **pending user decisions** — referenced here, not decided unilaterally. They are tracked in `plan/20-risks-and-open-questions.md`.

1. **MVP feature scope of the mobile app** — how much of the §4 feature set ships in the MVP vs after. Approvals + notifications + live status are the highest-value first cut; terminal and fan-out dashboard can follow. (Related to PLAN-CONTEXT Q7, Orca baseline scope.) Note: the mobile app **itself is part of the MVP** (D8 — MVP = Phases 1–4); this question is only about which §4 features land in the first cut, not whether the app ships.
2. **Remote-access modes in MVP** — local + paired + relay tunnel + SSH + Tailscale are all in the architecture; the MVP may ship a subset (e.g., local + relay tunnel first). (See `plan/14`.)
3. **Push-notification infrastructure** — Expo Notifications (per D2) wraps APNs/FCM; the notification payload schema is defined once regardless.
4. **Multi-provider on mobile** — the mobile app observes/steers sessions regardless of harness; whether the phone surfaces provider-specific controls in the MVP depends on PLAN-CONTEXT Q3 (Grok-only vs multi-provider MVP).

---

## References

- `docs/PLAN-CONTEXT.md` — authoritative shared plan context (mobile required; thin-client architecture; remote/ticket auth; mobile testing).
- `plan/00-vision-and-principles.md` — mobile as differentiator #8 and baseline-bar item; North Star "steer them from their phone."
- `plan/01-competitive-analysis.md` — Orca/T3 mobile companion as baseline bar; mobile is a tie we must match.
- `plan/04-wire-contract.md` — the JSON-RPC-over-WebSocket contract the mobile client consumes.
- `plan/10-ui-pane-system.md` — the desktop design system the mobile UI must be consistent with.
- `plan/14-remote-and-relay.md` — local/paired/relay/SSH/Tailscale + WebSocket ticket auth.
- `plan/15-testing-strategy.md` — contract testing and the shared mock server.
- `plan/17-security-and-secrets.md` — keychain, passkeys/DPoP, no raw secrets on the phone.
- `plan/20-risks-and-open-questions.md` — consolidated open decisions.
