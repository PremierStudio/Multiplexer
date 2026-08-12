# 23: First-Party Tailscale Integration

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** Remote / Core runtime
**Depends on:** `02-architecture.md`, `14-remote-and-relay.md`, `17-security-and-secrets.md`, `04-wire-contract.md`
**Feeds:** `15-testing-strategy.md`, `16-performance.md`, `19-roadmap-and-milestones.md`, `20-risks-and-open-questions.md`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context). Where a decision is not yet settled, it is listed under **Open questions** and is
**not** decided unilaterally here. New decisions proposed here are numbered **D51+** in the
style of `docs/DECISIONS.md`; they are proposals for the decision log, not locked decisions.

**Locked decisions applied (D1, D13, D24, D25, D37, D38):** This doc reflects the locked
decisions from `docs/DECISIONS.md`:
- **D1** : Rust + GPUI, single native server binary; Tailscale integration is a component of
  that binary, not a sidecar.
- **D13** : consolidated `multiplexer-*` crate layout; the Tailscale client lives in
  `multiplexer-remote` (discovery + serve) with the wire contract unchanged.
- **D24** : the relay is a TLS-terminating pipe, not E2EE; Tailscale serve is a private-network
  alternative that does not depend on our relay at all.
- **D25** : the remote agent independently enforces permission modes, worktree confinement, and
  approval gating; this applies to a Tailscale-reached remote host exactly as it does to SSH.
- **D37** : pairing issues a long-lived device credential minted into short-lived tickets;
  Tailscale identity is a transport-level trust signal, not a substitute for ticket auth.
- **D38** : local tickets are keychain-only; Tailscale never causes a plaintext token file.

---

## 1. Problem statement

Multiplexer's server-centric runtime is designed to be reached from anywhere: local, paired,
relay tunnel, and SSH (plan/14). But each of those paths has a cost:

- **Relay tunnel** requires us to operate (or pay for) a relay and requires the client to reach
  the internet. It is the "connect from anywhere" path, but it is not the lowest-friction path
  for a user who already runs Tailscale.
- **SSH** requires the user to configure keys, a remote agent, and port forwarding. It is
  powerful but not automatic.
- **Paired / LAN** works only on the same network and does not survive NAT or roaming.

Meanwhile, a large fraction of the target audience (developers, homelab users, and the
multi-machine "delegate work to a fleet" workload) already runs **Tailscale**. When Tailscale is
running, the user's machines are already on a private, authenticated, encrypted network with
stable DNS names (`machine.tailnet.ts.net`) and stable IPs (`100.x`). Multiplexer currently
ignores this entirely: it treats Tailscale as an optional add-on (plan/14 §1, §9) and offers no
first-party integration.

The result is a missed opportunity and a real friction point:

1. **Manual setup.** To reach a session from another machine today, the user must configure the
   relay, SSH, or LAN pairing by hand. If they already run Tailscale, none of that should be
   necessary.
2. **No machine discovery.** The UI has no notion of "the other machines on my tailnet that run
   Multiplexer." The user cannot see a fleet or pick a machine to delegate work to.
3. **No private JSON-RPC endpoint.** The core could expose its JSON-RPC-over-WebSocket contract
   on the tailnet via `tailscale serve`, giving a stable, authenticated, private endpoint with
   no public ports and no relay dependency.

This doc proposes making Tailscale a **first-party integration**: when Tailscale is running,
Multiplexer automatically uses MagicDNS and Tailscale features so a session can run on one
machine while the user controls it from another, and so many machines (up to 100) can delegate
work.

---

## 2. Why first-party (not "just use SSH")

The natural objection is "Tailscale is just a network; SSH already works over it." That is true
but misses the point. First-party integration buys four things that "SSH over Tailscale" does
not:

1. **Automatic discovery.** The Local API (`tailscale status --json`) returns the full netmap:
   every peer, its `DNSName`, `TailscaleIPs`, `Online` state, and `Tags`. Multiplexer can render
   a live machine picker ("which machine should run this session?") with zero user configuration.
   SSH gives you a network, not a directory of Multiplexer-capable peers.
2. **MagicDNS names in the UI.** Instead of IPs or SSH host aliases, the UI shows
   `machine.tailnet.ts.net` names that resolve locally via the tailnet DNS at `100.100.100.100`
   (https://tailscale.com/docs/features/magicdns). These names are stable, human-readable, and
   already authenticated by the tailnet.
3. **A private JSON-RPC endpoint via `tailscale serve`.** The core can expose its wire contract
   on the tailnet with `tailscale serve`, giving a stable HTTPS endpoint reachable only by
   tailnet peers. This is the same contract as local/relay/SSH (plan/14 §1.1: one wire contract,
   four transports), so the client code is unchanged; only the transport and its trust signal
   differ.
4. **Fleet delegation.** With discovery + a machine picker, "many machines (up to 100) can
   delegate work" becomes a first-class UI action rather than a manual SSH chore. This is the
   multi-machine story plan/14 §7 gestures at but does not automate.

First-party integration is also a **differentiator**: no major competitor (Orca, T3 Code,
Superset, Conductor) treats Tailscale as a first-class control-surface feature. T3 Code uses
Tailscale as a transport option; we would use it as a discovery + delegation layer on top of the
server-centric runtime.

---

## 3. Design goals

1. **Auto-detect.** When Tailscale is running (BackendState == Running, Self.Online, a
   CurrentTailnet is present), Multiplexer automatically enables the Tailscale surface. When it
   is absent or stopped, Multiplexer degrades gracefully to the existing local/relay/SSH paths
   with no error and no configuration burden.
2. **MagicDNS names in the UI.** Every reachable Multiplexer peer is shown by its
   `machine.tailnet.ts.net` name, not by IP or SSH alias. The user never types a tailnet IP.
3. **Machine picker.** The UI lists tailnet peers that advertise a Multiplexer endpoint, marks
   their `Online` state, and lets the user pick a machine to observe or to delegate work to.
4. **No public ports.** All Tailscale exposure uses `tailscale serve` (private tailnet HTTPS via
   MagicDNS), never `tailscale funnel` (public). Nothing is exposed to the public internet by
   default.
5. **Graceful degrade.** If Tailscale is not installed, not running, or not logged in, the
   feature is simply absent. No crash, no blocking, no forced install.
6. **Same wire contract.** The JSON-RPC-over-WebSocket contract (plan/04) is unchanged. Tailscale
   is a transport + discovery layer, exactly as plan/14 frames it.

---

## 4. Proposed architecture

Tailscale integration is a component of `multiplexer-remote` (D13). It has three cooperating
parts: **node discovery** (Local API), **identity** (who is this peer, what can it do), and
**serve** (expose the core's JSON-RPC endpoint on the tailnet). It sits alongside the existing
remote/relay layer and reuses its auth and wire contract.

### 4.1 Placement in the runtime

```
┌───────────────────────────────────────────────────────────────┐
│                     MULTIPLEXER SERVER                        │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  ORCHESTRATION ENGINE (event-sourced, plan/06)          │  │
│  └───────────────┬─────────────────────────────────────────┘  │
│                  │  JSON-RPC over WebSocket (plan/04)         │
│  ┌───────────────▼─────────────────────────────────────────┐  │
│  │  REMOTE / RELAY (multiplexer-remote, plan/14)           │  │
│  │  local │ paired │ relay tunnel │ SSH │ TAILSCALE        │  │
│  │  ┌───────────────────────────────────────────────────┐  │  │
│  │  │  TAILSCALE INTEGRATION                            │  │  │
│  │  │  ┌────────────┐ ┌────────────┐ ┌───────────────┐  │  │  │
│  │  │  │ Discovery  │ │ Identity   │ │ Serve         │  │  │  │
│  │  │  │ (Local API)│ │ (peer map) │ │ (tailscale    │  │  │  │
│  │  │  │            │ │            │ │  serve)       │  │  │  │
│  │  │  └────────────┘ └────────────┘ └───────────────┘  │  │  │
│  │  └───────────────────────────────────────────────────┘  │  │
│  └───────────────┬─────────────────────────────────────────┘  │
│                  │  ticket + DPoP auth (plan/17)              │
│  ┌───────────────▼─────────────────────────────────────────┐  │
│  │  TAILSCALE DAEMON (user-installed, tailscaled)          │  │
│  │  Local API 100.100.100.100 │ tailscale serve │ netmap   │  │
│  └─────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

The integration is **not** a sidecar and does **not** embed the Tailscale daemon. It talks to
the user's existing `tailscaled` via the **Local API** (a local HTTP API on the tailnet's
`100.100.100.100` resolver / local socket). This keeps us out of the business of running a
WireGuard daemon and means the feature works with any stock Tailscale install.

### 4.2 Node discovery via the Local API

The Local API is the primary discovery mechanism. It requires **no admin token** and no cloud
API key, which is why it is the recommended path (https://tailscale.com/docs/features/magicdns,
https://tailscale.com/api). Two access routes:

- **CLI:** `tailscale status --json` returns the full netmap.
- **Local API:** `GET /localapi/v0/status` on the local socket returns the same structure.

Rust crates that wrap this:
- `tslocal` (https://github.com/bouk/tslocal)
- `tailscale-localapi` (https://github.com/jtdowney/tailscale-localapi)

We evaluate both and pick one (or write a thin client) behind a `TailscaleClient` trait so the
backend is swappable and testable with a mock. The status payload gives us, per the docs:

| Field | Use |
|---|---|
| `BackendState` | `Running` means the daemon is up and connected |
| `Self` | this machine's node: `DNSName`, `TailscaleIPs`, `Online`, `Tags` |
| `Peer` map | every tailnet peer: `DNSName`, `TailscaleIPs`, `Online`, `Tags`, `LastSeen` |
| `CurrentTailnet` | the tailnet name and MagicDNS-enabled flag |

**Detection rule:** the Tailscale surface is enabled only when `BackendState == Running`, `Self`
is present and `Online`, and `CurrentTailnet` is non-empty. Otherwise the surface is hidden and
the existing paths (local/relay/SSH) are used unchanged.

**Peer filtering:** not every tailnet peer runs Multiplexer. Discovery filters peers to those
that advertise a Multiplexer endpoint (see §4.4) and marks the rest as "available on tailnet but
not running Multiplexer" (or hides them). `Online` drives the picker's live state.

### 4.3 Identity

Identity answers "who is this peer, and what may it do?" It has two layers:

1. **Transport identity (Tailscale).** The tailnet already authenticates every peer and
   encrypts traffic between them. A connection that arrives over the tailnet is from a verified
   tailnet node with a stable `DNSName` and `Tags`. This is a strong transport-level trust
   signal, but it is **not** a substitute for application auth.
2. **Application identity (Multiplexer).** Per D37 and plan/17, every connection still presents
   a **ticket + DPoP proof** at the WebSocket handshake. Tailscale identity does not bypass this.
   A tailnet peer is trusted at the transport layer, then authenticated and scoped at the
   application layer exactly like any other remote client.

**Tags and ACLs:** the tailnet's ACL policy (HuJSON, deny-by-default) and node tags
(`tag:prod`, etc.) are the operator's way to segment the fleet. Multiplexer reads `Tags` from the
netmap and surfaces them in the picker, but it does **not** enforce tailnet ACLs itself; that is
Tailscale's job. Multiplexer enforces its own permission modes and worktree confinement on top
(plan/17 §7), and a Tailscale-reached remote host independently enforces them per D25.

### 4.4 Serve: private JSON-RPC endpoint

`tailscale serve` exposes a local service on the tailnet over HTTPS via MagicDNS
(https://tailscale.com/docs/features/tailscale-serve). This is the right fit for the JSON-RPC
relay:

- **Serve (private)** exposes the core only to tailnet peers. This is the default and the
  recommended mode.
- **Funnel (public)** exposes it to the public internet. This is **not** used by default and is
  gated behind an explicit user opt-in (see §6).

The core runs its existing WebSocket listener (the same one used for local/relay/SSH, plan/04)
and `tailscale serve` fronts it on `https://<machine>.<tailnet>.ts.net:<port>`. Because the
contract is unchanged, any thin client (desktop, mobile, web) that can reach the tailnet can
connect to this endpoint with the same ticket + DPoP auth.

**How serve is driven:** `tailscale serve` is configured via the CLI (`tailscale serve --bg
--https=443 http://127.0.0.1:<port>`) or the Local API. Multiplexer invokes the CLI (or the Local
API) to set up and tear down the serve mapping, and records the resulting URL in the read model
so the picker can advertise it.

**Advertise:** the core advertises its Multiplexer endpoint so other peers' pickers can find it.
This is a small, signed record (node identity + serve URL + protocol version) that peers read via
discovery. It contains **no secrets** (see §6).

### 4.5 Graceful degrade

The whole feature is conditional on the detection rule in §4.2. If Tailscale is absent, stopped,
or not logged in:

- The Tailscale surface (picker, serve, discovery) is simply not shown.
- The existing local/relay/SSH paths continue to work unchanged.
- No error is surfaced beyond an informational "Tailscale not detected" hint in the remote
  settings, and no install is forced.

This keeps Tailscale a first-class but **optional** integration, consistent with plan/14's
"optional, first-class provider" framing.

---

## 5. Proposed decisions (D51+)

These are proposals for `docs/DECISIONS.md`, in its style. They are **not** locked; they are
offered for the decision log.

### D51. Tailscale is a first-party integration, not an add-on (PROPOSED)
- **Decision:** When Tailscale is running, Multiplexer automatically enables discovery, MagicDNS
  naming, machine picking, and `tailscale serve` for its JSON-RPC endpoint. It is a first-class
  surface of `multiplexer-remote`, not a bolt-on.
- **Rationale:** The target audience already runs Tailscale; ignoring it forces manual relay/SSH
  setup and leaves the multi-machine delegation story unautomated.

### D52. Discovery via the Local API, no admin token (PROPOSED)
- **Decision:** Node discovery uses the **Local API** (`tailscale status --json` /
  `GET /localapi/v0/status`) behind a `TailscaleClient` trait. The Cloud API
  (api.tailscale.com, needs `tskey-api` or OAuth) is used **only** for admin fleet management,
  never for the per-user discovery path.
- **Rationale:** The Local API needs no admin token and no cloud key, so the core feature works
  with a stock Tailscale install and holds no extra credential.

### D53. Serve, not Funnel, by default (PROPOSED)
- **Decision:** The core's JSON-RPC endpoint is exposed with **`tailscale serve`** (private
  tailnet HTTPS via MagicDNS) by default. **`tailscale funnel`** (public) is never enabled
  without an explicit user opt-in.
- **Rationale:** Serve gives a stable, authenticated, private endpoint with no public ports and
  no relay dependency. Funnel is a public-exposure risk and is not needed for the multi-machine
  story.

### D54. Tailscale identity is transport trust, not app auth (PROPOSED)
- **Decision:** A tailnet connection is trusted at the transport layer (Tailscale authenticates
  and encrypts it) but still requires the standard **ticket + DPoP** handshake and is scoped per
  connection (plan/17). Tailscale identity never bypasses application auth.
- **Rationale:** Consistent with D37/D38 and plan/17's "the core never trusts the transport."
  A compromised tailnet node must not gain control by virtue of being on the tailnet.

### D55. Tailscale-reached remote hosts independently enforce policy (PROPOSED)
- **Decision:** A remote host reached over Tailscale (like one reached over SSH) runs the remote
  agent, which **independently enforces** permission modes, worktree confinement, and approval
  gating (D25). Tailscale does not relax remote-side enforcement.
- **Rationale:** The tailnet authenticates the transport, not the operations. The remote agent
  remains the enforcement point for what actually executes on the remote host.

### D56. Advertised endpoint records contain no secrets (PROPOSED)
- **Decision:** The discovery/advertise record carries only node identity, serve URL, and
  protocol version. It never carries tickets, tokens, or key material.
- **Rationale:** Discovery records are visible to tailnet peers; they must be capability-free so
  a leaked record grants nothing (the ticket + DPoP handshake still gates access).

---

## 6. Security

Tailscale integration follows plan/17's principles: least privilege, fail closed, auditability.
The tailnet is a strong transport, but it is not a security boundary by itself.

1. **ACLs are Tailscale's job.** The tailnet's deny-by-default HuJSON ACL policy and node tags
   (`tag:prod`) segment the fleet at the network layer
   (https://tailscale.com/docs/features/access-control/acls). Multiplexer reads `Tags` for the
   picker but does not re-implement ACLs. The operator controls who can reach which node.
2. **No secrets in MagicDNS names.** `DNSName` is `machine.tailnet.ts.net`; it is a node
   identifier, never a secret. We never embed credentials in a hostname or in the advertise
   record (D56).
3. **Serve, not Funnel, by default.** The default is private tailnet exposure. Funnel (public)
   requires explicit opt-in and is treated as a public-exposure risk (D53).
4. **App auth still applies.** Every connection presents ticket + DPoP and is scoped (D54). A
   tailnet peer is not implicitly a controller; it is a remote client like any other.
5. **Remote hosts independently enforce policy.** A Tailscale-reached remote host runs the
   remote agent with independent permission/worktree/approval enforcement (D55, D25).
6. **No new credential surface.** The Local API path holds no admin token. If the Cloud API is
   used for admin fleet management, its `tskey-api`/OAuth credential lives in the OS keychain
   (D23/D38), never in configs or plaintext.
7. **Auditability.** Discovery events, serve setup/teardown, and tailnet connections are events
   in the read model, replayable for review (plan/17 §1.5).

**Threat model additions (relative to plan/17 §9):**

| Threat | Mitigation |
|---|---|
| Rogue tailnet node connects to the core | Ticket + DPoP handshake and per-connection scope (D54); tailnet ACLs limit who can reach the node |
| Tailnet peer reads another peer's advertise record | Records contain no secrets (D56); they grant nothing without a valid ticket |
| Funnel accidentally enabled | Serve-only by default; Funnel requires explicit opt-in (D53) |
| Compromised tailnet node drives a remote host | Remote agent independently enforces policy (D55, D25) |
| Tailscale daemon compromised | The daemon is user-installed and trusted; Multiplexer holds no tailnet credential beyond what the daemon already has |

---

## 7. Testing strategy

Tailscale integration is tested under the project's TDD-at-inception gate chain (fmt → clippy →
unit+property → mutation → integration → component → e2e → coverage), per plan/15. **No live
tailnet in CI.**

### 7.1 Unit tests (status parsing, detection)

Co-located `#[cfg(test)]` modules over the `TailscaleClient` trait and the status parser:
- **Status parse:** parse realistic `tailscale status --json` fixtures (Self, Peer map,
  CurrentTailnet, Online, Tags) into the typed model; assert every field maps correctly.
- **Detection rule:** given fixture statuses, assert the surface is enabled only when
  `BackendState == Running`, `Self` is Online, and `CurrentTailnet` is present. Cover absent,
  stopped, and logged-out states.
- **Peer filtering:** given a peer map, assert only Multiplexer-advertising peers appear in the
  picker and `Online` state is correct.
- **Advertise record:** assert the record contains only identity/URL/version and never secrets
  (property test over random fields).

### 7.2 Property tests (proptest)

- **Status parser:** arbitrary JSON-shaped status inputs never panic and never produce an
  invalid typed model (round-trip / field-presence invariants).
- **Detection:** over arbitrary combinations of BackendState / Self / CurrentTailnet, the
  enabled/disabled decision is consistent and deterministic.
- **Advertise record:** over arbitrary node fields, the serialized record never contains a
  secret-shaped field.

### 7.3 Integration tests (mock Local API)

- **Mock localapi:** run a mock `tailscale status --json` server (or a fake `tailscaled` Local
  API) and assert the `TailscaleClient` reads it correctly.
- **Serve setup/teardown:** mock the `tailscale serve` CLI invocation; assert the core records
  the resulting URL and tears it down cleanly.
- **End-to-end over mock tailnet:** connect a mock client to the core's serve-fronted WebSocket
  endpoint with a valid ticket + DPoP proof; assert the wire contract works unchanged over the
  tailnet transport.
- **No live tailnet in CI:** all of the above use fixtures and mocks. A live-tailnet smoke test
  is CI-optional and gated behind an explicit opt-in flag.

### 7.4 Mutation testing

cargo-mutants over the status parser, detection rule, and advertise-record serializer. CI gates:
≥85% line, ≥80% branch, ≥70% mutation score killed (D21, D33). The detection rule and parser are
prime mutation targets.

### 7.5 E2E

Drive the real app headless against a mock tailnet; assert that when Tailscale is "running" the
picker appears with the expected peers, and when it is "absent" the surface is hidden and the
local path still works. This is the direct regression test for graceful degrade.

---

## 8. Open questions / risks

These are flagged, not decided here:

1. **Rust client crate.** `tslocal` vs `tailscale-localapi` vs a thin hand-written client. Both
   are small; we must evaluate maintenance, Windows support, and API coverage before choosing.
2. **Cloud API scope.** Whether admin fleet management (device list, ACL inspection) via the
   Cloud API ships at all, and if so whether it uses `tskey-api` or OAuth. The per-user
   discovery path does not need it (D52).
3. **Tailscale SSH.** `tailscale set --ssh` with node-identity auth and policy-file SSH
   (https://tailscale.com/docs/features/tailscale-ssh) is an optional bootstrap for the remote
   agent. Whether it is a supported path alongside OpenSSH (plan/14 §3) is open.
4. **Serve port and TLS.** The exact serve port, whether to use `--https=443` or a custom port,
   and how the serve URL is discovered by peers needs a concrete decision.
5. **Fleet delegation semantics.** "Up to 100 machines delegate work" needs a definition of what
   delegation means (observe-only vs control vs full remote-agent) and how it maps to the
   existing multi-machine model (plan/14 §7). Cross-core orchestration is deferred there and
   remains deferred here.
6. **Windows specifics.** Tailscale on Windows runs `tailscaled` as a service; the Local API
   socket path and CLI invocation differ from Unix. Windows-first (D9) means this must be tested
   first.
7. **MVP scope.** Whether discovery + serve ships in MVP (Phase 4, plan/19) or is staged later
   is a roadmap decision for plan/19.

**Flagged consistency note:** this doc is consistent with PLAN-CONTEXT (server-centric runtime,
one wire contract / four transports, ticket + DPoP auth, Tailscale serve as an endpoint
provider). If any locked decision flips (e.g. stack, crate layout, remote model), the affected
sections (§4, §5) must be revisited.

---

## 9. Consistency with plan/14, plan/22, plan/24

- **plan/14 (Remote & Relay):** This doc is the concrete realization of plan/14's "Tailscale
  serve is an endpoint provider" framing. It resolves plan/14 §9 open question #2 (Tailscale
  posture) by proposing first-party status (D51), and it reuses plan/14's one-contract /
  four-transports model, ticket + DPoP auth, and multi-machine model. It does **not** change the
  relay tunnel; Tailscale serve is an alternative endpoint provider, not a replacement for the
  relay.
- **plan/22 (anticipated):** plan/22 is not yet authored. If it covers multi-machine fleet
  coordination or device management, this doc's discovery + machine picker (§4.2, §4.4) is the
  discovery substrate it should build on. Any overlap must be reconciled when plan/22 lands.
- **plan/24 (anticipated):** plan/24 is not yet authored. If it covers remote-agent / SSH
  bootstrap or fleet security, this doc's Tailscale SSH option (§8.3) and independent-enforcement
  posture (D55) must be consistent with it. Reconcile when plan/24 lands.

---

## References

- `docs/PLAN-CONTEXT.md`: authoritative shared plan context (remote/relay line, security
  posture, testing).
- `docs/DECISIONS.md`: locked decisions D1-D40 (D1, D13, D24, D25, D37, D38 applied here).
- `plan/04-wire-contract.md`: the JSON-RPC-over-WebSocket contract this layer transports.
- `plan/14-remote-and-relay.md`: the four remote kinds, ticket auth, multi-machine model.
- `plan/17-security-and-secrets.md`: OS keychain, ticket + DPoP, permission modes, D25.
- `plan/19-roadmap-and-milestones.md`: staging of the remote matrix (Phase 4).
- `plan/20-risks-and-open-questions.md`: consolidated open decisions.
- https://tailscale.com/docs/features/magicdns: MagicDNS, netmap, 100.100.100.100 resolver.
- https://tailscale.com/docs/features/tailscale-serve: Serve (private) vs Funnel (public).
- https://tailscale.com/docs/features/access-control/acls: deny-by-default HuJSON ACLs, tags.
- https://tailscale.com/docs/features/tailscale-ssh: Tailscale SSH, node-identity auth.
- https://tailscale.com/api: Cloud API (devices list, tskey-api / OAuth).
- https://github.com/bouk/tslocal: Rust Local API client.
- https://github.com/jtdowney/tailscale-localapi: Rust Local API client.
