# Plan 21 — Plugin Architecture (extension points, credential plane, sandboxing, harness admission)

**Status:** Authored for adversarial review.
**Anchored to:** D13, D17, D12, D23, D25, and the new D41–D46.
**One-line thesis:** Core Multiplexer stays small and security-reviewable; everything deployment-specific — credential vaults, session sandboxes, harness adapters, approval policies — is a plugin with declared, least-privilege capabilities. Users extend Multiplexer by writing plugins, not PRs.

---

## 1. Purpose

The server-centric runtime (D13) makes `multiplexer-server` the natural trust boundary for agent execution. That is only true if the server itself is minimal and if the things that vary per deployment (which vault, which sandbox, which harness, which approval policy) live behind stable, capability-scoped extension seams. This doc defines those seams.

The first-party plugins that ship alongside core are the proof of the model:

1. **`plugin-1password`** — CredentialProvider bridging a 1Password service account scoped to a single automation vault into the session-cache model (D23).
2. **`plugin-container-sandbox`** — SandboxProvider running each agent session in an isolated container on the server host.
3. **`plugin-acp-harness`** — HarnessAdapter admitting Claude Code / Codex / OpenCode / ZCode via the generic ACP machinery (D17).
4. **`plugin-approval-pack`** — ApprovalPolicy implementations over the D12 4-way enum (local prompt, mobile push, declarative policy).

## 2. Non-goals

- No in-core knowledge of any specific vault vendor, container runtime, or harness beyond Grok (in-process, D10).
- No plugin UI in the MVP beyond pane registration hooks; the editor/pane system consumes plugins, it does not host arbitrary web content.
- No dynamic marketplace/signing story in MVP; plugins are local installs from a manifest + directory (marketplace is a Phase 5+ concern, D30 adjacent).

## 3. Extension points (trait seams)

All plugin traits live in `multiplexer-wire`-adjacent `multiplexer-plugin-api` so client codegen never depends on plugin internals. Traits are async, object-safe, and versioned by the plugin API semver (§7).

### 3.1 `CredentialProvider` (D43)

```rust
#[async_trait]
pub trait CredentialProvider: Send + Sync {
    /// List addressable references this provider can resolve, e.g.
    /// "1pass://automation-vault/forgejo-agent/token".
    async fn inventory(&self) -> Result<Vec<CredentialRef>, PluginError>;
    /// Resolve a reference into the SERVER-SIDE session cache only.
    /// Providers never hand secrets to plugins or clients.
    async fn resolve(&self, r: &CredentialRef, cache: &SessionCache)
        -> Result<CachedSecretId, PluginError>;
}
```

Resolution happens once at session start; values materialize only as `CachedSecretId` handles inside the server-side session cache (D23). Per-session credential injection into a sandbox (§3.2) uses those handles — a session sees only the references its task config names. **No live user-session `op` reads, ever (D23).** External vault auth (e.g., a 1Password service-account token) is itself a keychain secret on the server host.

### 3.2 `SandboxProvider` (D44)

```rust
#[async_trait]
pub trait SandboxProvider: Send + Sync {
    /// Provision an isolated execution environment for one session.
    async fn create(&self, spec: SandboxSpec) -> Result<SandboxHandle, PluginError>;
    /// Inject only the CachedSecretIds the task config declares (env/files,
    /// inside the sandbox, never on the host).
    async fn inject(&self, h: &SandboxHandle, secrets: &[CachedSecretId]) -> Result<(), PluginError>;
    async fn exec(&self, h: &SandboxHandle, cmd: ExecSpec) -> Result<ExecOutcome, PluginError>;
    async fn destroy(&self, h: &SandboxHandle) -> Result<(), PluginError>;
}
```

The default `plugin-container-sandbox` uses per-session containers with: workspace bind-mount scoped to the session worktree, no host home exposure, egress policy per session config, and teardown that shreds injected secrets. The remote-agent independent-enforcement rule (D25) is implemented HERE: confinement is enforced by the sandbox, not trusted from the client.

### 3.3 `HarnessAdapter` (D45)

```rust
#[async_trait]
pub trait HarnessAdapter: Send + Sync {
    fn id(&self) -> HarnessId;                       // "grok" | "claude-code" | "codex" | …
    async fn start(&self, s: &SandboxHandle, cfg: HarnessConfig)
        -> Result<ProviderEventStream, PluginError>; // canonical events (D16)
    async fn decide(&self, pending: &PermissionRequested) -> FourWay; // D12 plumbing only
}
```

Grok in-process remains a core adapter (D10). All external harnesses enter as HarnessAdapter plugins riding the generic ACP machinery (D17). Adding a harness is a plugin, never a core PR.

### 3.4 `ApprovalPolicy` (D46)

```rust
#[async_trait]
pub trait ApprovalPolicy: Send + Sync {
    /// Map a pending PermissionRequested to a D12 decision. Policies may
    /// escalate to a human (local prompt / mobile push) or apply declarative
    /// rules ("reads auto-allow; writes and network egress always gate").
    async fn decide(&self, pending: &PermissionRequested, ctx: &SessionCtx)
        -> Result<FourWay, PluginError>;
}
```

Policies compose (ordered chain, first non-`defer` wins). The approval gate is a security boundary extending D12/D25: policy plugins require explicit user consent at install and are mutation-gated like core (D21).

## 4. Manifest & capabilities (D42)

```toml
# plugin.toml
[plugin]
id = "1password"
api = "1"                # plugin API semver major
kind = "credential"      # credential | sandbox | harness | approval | pane

[capabilities]
credential-read = ["1pass://automation-vault/*"]
network = ["https://my.1password.com"]   # egress allow-list, deny by default
```

- Capabilities are **declared, user-approved at install, and enforced** by the plugin host. No ambient authority: a plugin receives only the API handles its capabilities grant.
- Plugins run **out-of-process** against a versioned JSON-RPC sidecar of the same style as the wire (D20 discipline): the host can crash-isolate, resource-limit, and kill them. (WASM-in-process is a future optimization, not v1.)
- Unsigned plugins warn loudly; signed distribution is Phase 5+.

## 5. Threat-model mapping

- **Credential readability ("agent reads the vault"):** secrets exist only server-side in the session cache; sessions receive injected, task-scoped secrets inside a sandbox; clients never see any of it.
- **Credential abuse-in-session ("hijacked session USES its access"):** scoped per-agent tokens + sandbox egress policy + ApprovalPolicy gates on destructive operations.
- **Plugin compromise:** capabilities + process isolation bound a malicious plugin to its declared slice; the credential provider is the highest-value target and therefore gets the strictest review bar (see §7 testing).

## 6. Milestone mapping (per the D40 spine)

- Phase 1 additions: `multiplexer-plugin-api` crate, manifest + host, capability enforcement (unit + property + mutation per D21/D33).
- Phase 4 pull-forward (parallel-safe per D40): container SandboxProvider + remote relay enforcement wiring (D25).
- First-party plugins land with their phases: 1Password + approval-pack early (they harden the MVP), ACP harness adapters with multi-harness admission, pane plugins with Phase 2.

## 7. Compatibility & testing

- Plugin API semver: `api = "1"`; breaking bumps are additive-major with a deprecation window. Trait sketches above are normative for names, not signatures.
- Every plugin API boundary is fuzzed and mutation-gated; `plugin-1password` and the sandbox host additionally require integration tests against a real service-account vault and a real container runtime respectively. No mock-only security tests.

## 8. Open questions (for adversarial review)

1. Pane plugins (D42 `pane` kind): render via GPUI hooks only, or webview panes with a hardened origin model? Recommend GPUI-only for MVP.
2. Should `SandboxProvider` also own per-session egress network policy config, or does that live in session config consumed by the provider? Recommend the latter.
3. Credential rotation TTLs: enforce minimums at the API level or leave to providers? Recommend providers, with a lint warning.
