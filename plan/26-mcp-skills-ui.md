# 26: MCP & Skills UI (Registry, Customize Panel)

**Status:** Planning (authored by subagent, pending adversarial review)
**Owner:** UI / Configuration surface
**Depends on:** `02-architecture.md`, `05-provider-adapter-layer.md`, `10-ui-pane-system.md`, `17-security-and-secrets.md`, `21-mcp-lifecycle-supervisor.md`
**Feeds:** `15-testing-strategy.md`, `19-roadmap-and-milestones.md`, `20-risks-and-open-questions.md`

This document is consistent with `docs/PLAN-CONTEXT.md` (the authoritative shared plan
context). Where a decision is not yet settled, it is listed under **Open questions** and is
**not** decided unilaterally here. New decisions proposed here are numbered **D72+** in the
style of `docs/DECISIONS.md`; they are proposals for the decision log, not locked decisions.

**Locked decisions applied (D1, D13, D18, D21, D23, D33):** This doc reflects the locked
decisions from `docs/DECISIONS.md`:
- **D1** : Rust + GPUI, single native server binary; the Customize panel is a GPUI component
  in `multiplexer-ui`, backed by config logic in `multiplexer-core`.
- **D13** : consolidated `multiplexer-*` crate layout; config parsing/validation lives in
  `multiplexer-core`, the GPUI panel in `multiplexer-ui`.
- **D18** : bounded channels with backpressure; registry browse and live-status streams follow
  the same rule.
- **D21** : mutation-testing scope includes all core logic; the config parser/validator and
  the registry state machine are mutation targets.
- **D23** : secrets session-cache model; MCP env/headers and hook commands reference secrets
  via the same mechanism, never raw values in configs or in the UI.
- **D33** : 70% mutation score is the merge floor.

**Relationship to plan/21:** plan/21 specifies the *process* supervisor (spawn, reuse,
supervise, reap, resource limits). This doc specifies the *management surface*: the registry
of configured MCP servers, skills, and hooks, the add/edit/remove editors, the graphical
Customize panel, and the validation that runs before anything is written to disk. It does
**not** re-specify the supervisor's process design; it consumes the supervisor's live status
(plan/21 §4.3) as a read-only projection for the UI. Where this doc needs a process-level
behavior it references plan/21 rather than duplicating it.

---

## 1. Problem statement

Today MCP servers, skills, and hooks are configured by hand-editing files: `~/.grok/config.toml`
`[mcp_servers.<name>]` blocks, `SKILL.md` folders, and `~/.grok/hooks/*.json`. This is
file-only configuration with no lifecycle visibility:

1. **No inventory.** There is no single place to see what MCP servers, skills, and hooks are
   configured, where each came from (user / project / plugin / compat source), and whether it
   is enabled. The user must grep config files and directories to reconstruct the picture.
2. **No lifecycle visibility.** A configured server may be `ready`, `crashed`, or `stopped`
   (plan/21 states), but the user cannot see that from the config file. The pile-up problem
   plan/21 diagnoses (N sessions = N copies of each server) is invisible until the machine
   runs out of memory.
3. **No validation before write.** A typo in a TOML block, a malformed `SKILL.md` frontmatter,
   or a bad hook JSON is only discovered when the harness fails to load it, often silently or
   with a cryptic error. There is no schema check, no "test this server" step, and no guard
   against pasting a raw secret into a config.
4. **No discovery.** The official MCP Registry (registry.modelcontextprotocol.io) hosts
   metadata for thousands of servers, but there is no in-app way to browse it, see what a
   server does, and add it. Users copy-paste commands from READMEs instead.
5. **No trust surface for project hooks.** Project hooks run arbitrary commands on every
   relevant lifecycle event. Grok gates them behind `/hooks-trust`; Multiplexer must surface
   that trust decision explicitly rather than silently running project hooks.

The result is that the most powerful extension surface of the harness (MCP tools, skills,
hooks) is also the least managed. This doc makes it a first-class, graphical, validated
surface.

---

## 2. Why a first-class UI

The same argument that makes plan/21's supervisor a product feature applies here: the
configuration surface is where users feel the harness's power and its sharp edges.

1. **MCP is the ecosystem's growth vector.** The MCP Registry and the broader marketplace
   ecosystem are growing quickly. A client that makes adding a server a two-click, validated,
   discoverable action is measurably better than one that requires editing TOML by hand.
2. **Skills and hooks are the trust surface.** Skills are folders of instructions and scripts;
   hooks are commands that run on lifecycle events, including the blocking `PreToolUse` event
   that can deny a tool call. Users need a clear, graphical view of what is active and what
   runs, because these are the surfaces that execute code on their behalf.
3. **Lifecycle visibility is the differentiator.** No major client (Grok CLI, Claude, Cursor,
   Orca) offers a combined graphical view of MCP servers *and* their live process state
   (plan/21). Multiplexer's server-centric runtime makes this natural: the UI is a pure view
   of server truth (plan/10 §8.2), so the Customize panel can show the live fleet for free.
4. **Validation prevents silent breakage.** A schema-checked, testable editor turns "the
   harness silently ignored my server" into "here is exactly what is wrong before I save."
   This is a reliability win that compounds across every server, skill, and hook the user adds.

This maps onto Multiplexer's core architecture: a single native binary owns config, processes,
and the read model. The Customize panel is the graphical face of that ownership.

---

## 3. Design goals

1. **One surface for MCP servers, skills, and hooks.** A single Customize panel with tabs for
   each category, plus a marketplace/registry browse for MCP servers. No more hunting through
   files.
2. **Add / edit / remove for all three.** Graphical editors for MCP server config (stdio and
   remote), `SKILL.md` metadata, and hook definitions, writing to the correct source file with
   correct scope.
3. **Live lifecycle status.** MCP servers show their plan/21 state (`ready` / `crashed` /
   `stopped` / `spawned`) as a read-only projection of the supervisor, with enable/disable
   toggles that the supervisor reacts to.
4. **Validate before write.** Every edit is validated against a schema before it is written;
   the user sees errors inline, not after the harness fails to load.
5. **No plaintext secrets.** Env vars, headers, and hook commands reference secrets via the
   session-cache model (D23) or `${VAR}` expansion; the UI never stores or displays raw
   secret values.
6. **Explicit trust for project hooks.** Project hooks require an explicit trust grant before
   they run, surfaced in the UI, consistent with Grok's `/hooks-trust` model.

---

## 4. Proposed architecture

### 4.1 Placement in the runtime

The Customize panel is a GPUI component in `multiplexer-ui` (D1, D13). It is a pure view: it
reads config and live status from the server read model over the JSON-RPC contract (plan/04)
and issues commands back through it. It never writes files directly; the server owns all
config writes.

```
┌───────────────────────────────────────────────────────────────┐
│                     MULTIPLEXER SERVER                        │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  CONFIG LAYER (multiplexer-core)                        │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────────┐ │  │
│  │  │ MCP registry │ │ Skills       │ │ Hooks            │ │  │
│  │  │ (parse/merge)│ │ (SKILL.md)   │ │ (JSON)           │ │  │
│  │  └──────┬───────┘ └──────┬───────┘ └────────┬─────────┘ │  │
│  │         │  validate + write (schema, D23)   │            │  │
│  │  ┌──────▼───────────────────────────────────▼─────────┐  │  │
│  │  │  VALIDATOR (JSON/TOML schema, secret refs, https)  │  │  │
│  │  └──────────────────────┬─────────────────────────────┘  │  │
│  └─────────────────────────┼────────────────────────────────┘  │
│                            │  config events (add/remove/enable)│
│  ┌─────────────────────────▼────────────────────────────────┐  │
│  │  MCP LIFECYCLE SUPERVISOR (plan/21)                      │  │
│  │  live status: ready / crashed / stopped / spawned        │  │
│  └─────────────────────────┬────────────────────────────────┘  │
│                            │  read-model projection (JSON-RPC) │
│  ┌─────────────────────────▼────────────────────────────────┐  │
│  │  CUSTOMIZE PANEL (multiplexer-ui, GPUI)                  │  │
│  │  tabs: MCP Servers · Skills · Hooks · Marketplace        │  │
│  └──────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

The config layer is the single owner of config writes, matching the supervisor's single-owner
model for processes (plan/21 §4.1). The UI issues `config.add_server`, `config.edit_server`,
`config.remove_server`, `config.toggle_server`, and the analogous skill/hook commands; the
config layer validates, writes, and emits events that both the supervisor and the read model
react to.

### 4.2 Source of truth and scope

The source of truth is the same set of files grok-build reads, resolved with the same scope
merge order (per https://docs.x.ai/build/settings):

| Scope | Path | What it may hold |
|---|---|---|
| User | `~/.grok/config.toml` (or `$GROK_HOME`) | MCP servers, skills paths, hooks, models |
| Project | `.grok/config.toml` in the repo | MCP servers, plugins, permission rules only |
| Managed | `~/.grok/managed_config.toml` | enterprise-served defaults (read-only) |
| Compat | `~/.claude.json`, `.cursor/mcp.json`, `.mcp.json` | imported MCP configs, merged below `config.toml` |

Grok walks from the current directory up to the git root reading each `.grok/config.toml`, and
a project server with the same name as a user one replaces it entirely (per
https://docs.x.ai/build/features/mcp-servers). Multiplexer mirrors this merge order so the
panel shows exactly what the harness will load. `grok inspect` is the verification command; the
panel's "what's loaded" view is the same projection.

Each MCP server entry carries the fields plan/21 §4.2 defines: `name`, `identity` (the reuse
key), `transport` (`stdio` or `http`/`sse`), `command`/`url`, `env`/`headers` (resolved via the
secrets session cache, D23), `scope`, and `enabled`. The panel edits these; the supervisor
reacts to the resulting config events (plan/21 §4.2).

### 4.3 MCP server editor

The editor supports both transports, matching the Grok `config.toml` shape
(https://docs.x.ai/build/settings, https://docs.x.ai/build/features/mcp-servers):

**stdio** (`[mcp_servers.<name>]`):
- `command` (e.g. `npx`), `args` (e.g. `["-y", "@modelcontextprotocol/server-filesystem", "/path"]`)
- `env` (map of `NAME = "${VAR}"` or `op://` refs, never raw values)
- `cwd` (working directory, optional)
- `startup_timeout_sec` (default 30), `tool_timeout_sec` (default 6000)

**remote** (`url` + `headers`):
- `url` (e.g. `https://mcp.linear.app/mcp`)
- `headers` (map, e.g. `Authorization = "Bearer ${LINEAR_API_KEY}"`)
- OAuth servers trigger a browser flow on first use; tokens stored under
  `~/.grok/mcp_credentials.json` (per the MCP servers doc), never in the config.

The editor is a form, not a raw TOML textarea, for the common fields, with an "advanced" raw
view for power users. Every field is validated live (see §4.6). A "Test connection" button
asks the supervisor to spawn the server and report whether it reaches `ready` (plan/21 §4.3),
surfacing the result inline.

### 4.4 Registry browse (marketplace)

The panel includes a Marketplace tab backed by the official MCP Registry REST API
(https://modelcontextprotocol.io/registry/about, https://modelcontextprotocol.io/registry/registry-aggregators):

- Base URL `https://registry.modelcontextprotocol.io`; endpoints `GET /v0.1/servers`
  (cursor-paginated, `limit` + `cursor`, optional `updated_since`), and per-server
  `GET /v0.1/servers/{serverName}/versions` / `.../versions/{version}` (URL-encoded names).
- The registry hosts **metadata only, not binaries**: `server.json` describes the server's
  name (reverse-DNS, e.g. `io.github.username/server`), title, description, version, and
  `packages` (npm / pypi / nuget / oci / mcpb) with the install command or remote URL
  (https://modelcontextprotocol.io/registry/package-types). Multiplexer renders this metadata
  and, on "Add", fills the server editor from it; it does not download or execute anything.
- The registry is in preview; breaking changes or data resets may occur. The browse is a
  **cached, read-only** view: the server fetches pages on demand (bounded, D18), caches them
  locally, and degrades gracefully if the registry is unreachable. It is not a hard dependency
  of the panel.
- The registry is not intended to be consumed directly by host apps; downstream marketplaces
  implement the same OpenAPI spec. Multiplexer may add marketplace sources later (Grok already
  supports `[[marketplace.sources]]` in `config.toml`), but the MVP browse targets the official
  registry API directly.

### 4.5 Skills and hooks editors

**Skills** are folders containing a `SKILL.md` with YAML frontmatter
(https://docs.x.ai/build/features/skills-plugins-marketplaces). Grok discovers them from
`./.grok/skills/` (walked to repo root), `~/.grok/skills/`, enabled plugins' `skills/`
directories, and `[skills] paths` in `config.toml`. It also reads Claude Code
(`.claude/skills/`) and Cursor (`.cursor/skills/`) skill dirs, and user-level
`~/.agents/skills/`. The panel:

- Lists discovered skills with their `name`, `description`, `when-to-use`, `user-invocable`,
  and `metadata` (author, short-description) from the frontmatter.
- Add / edit / remove: creating a skill creates a `SKILL.md` folder in the chosen scope
  (user `~/.grok/skills/` or project `.grok/skills/`), edits the YAML frontmatter, and manages
  the body and any script files. The frontmatter fields are validated (see §4.6); extra keys
  are preserved (Grok ignores unknown keys).
- Slash-command integration: user-invocable skills appear as `/<skill-name>`; the panel shows
  which skills are invocable and lets the user toggle `user-invocable`.

**Hooks** are JSON files (https://docs.x.ai/build/features/hooks). Personal hooks live in
`~/.grok/hooks/*.json`; project hooks in `<project>/.grok/hooks/*.json`; Claude
(`.claude/settings.json`) and Cursor (`.cursor/hooks.json`) hook files are read too. The panel:

- Lists hooks grouped by event (`SessionStart`, `SessionEnd`, `UserPromptSubmit`,
  `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionDenied`, `Stop`,
  `StopFailure`, `Notification`, `SubagentStart`, `SubagentStop`, `PreCompact`,
  `PostCompact`).
- Add / edit / remove: a hook is a `matcher` (regex against tool name, omit for all), a
  `type` (`command` or `http` with a `url`), and a `timeout` (seconds, default 5). The editor
  writes the JSON file in the chosen scope.
- Highlights the **blocking** event `PreToolUse`, which can `deny` a tool call by writing
  `{ "decision": "deny", "reason": "..." }` to stdout (exit 2 denies; everything else is
  fail-open). The panel makes it explicit which hooks can block tool execution.
- Project hooks require trust before they run (see §6).

### 4.6 Validation

Every edit is validated before write, in the config layer, against a schema:

- **MCP servers:** a JSON Schema / TOML schema for the `[mcp_servers.<name>]` block. stdio
  requires `command`; remote requires a valid `url`. `env`/`headers` values must be secret
  references (`${VAR}`, `op://`, or `env:VAR`), never raw secret-looking values (D23). Remote
  `url` must be `https` (or `http://localhost` for local dev), never plaintext `http` to a
  remote host.
- **Skills:** `SKILL.md` frontmatter is validated against the documented fields (`name`,
  `description`, `when-to-use`, `paths`, `allowed-tools`, `argument-hint`,
  `user-invocable`, `disable-model-invocation`, `metadata`). Malformed YAML is rejected
  inline.
- **Hooks:** hook JSON is validated against the `{ hooks: { <event>: [ { matcher, hooks:
  [ { type, command|url, timeout } ] } ] } }` shape. Unknown events and malformed
  `matcher` regexes are rejected.
- **Test before save:** the MCP editor offers "Test connection" (spawn via the supervisor);
  the hooks editor offers a dry-run of the command against a sample event payload.

Validation is a pure function over the config value, which makes it unit- and property-
testable (see §7). The validator is the single gate before any write, so a bad edit can never
reach disk.

### 4.7 Live status from the supervisor

The MCP Servers tab shows each server's live state as a read-only projection of the plan/21
supervisor: `spawned`, `ready`, `crashed`, `stopped`, plus the reference count and resource
usage where available. The user can:

- **Toggle enable/disable** (the `enabled` flag and the disabled list grok-build persists).
  The supervisor reacts to the config event (plan/21 §4.2).
- **See crash/restart state** (plan/21 §4.6) and the backoff status, so a permanently failed
  server is visible rather than silently absent.
- **Trigger teardown** of an idle server, which the supervisor handles (plan/21 §4.5).

This is the differentiator: the panel is the first place a user can see both what is
configured and what is actually running.

---

## 5. Key design decisions (proposed D72+)

These are proposals for `docs/DECISIONS.md`, in its style. They are **not** locked; they are
offered for the decision log.

### D72. Config write ownership: the server owns all config writes (PROPOSED)
- **Decision:** The config layer in `multiplexer-core` is the single owner of config writes
  for MCP servers, skills, and hooks. The UI issues commands; it never writes files directly.
- **Rationale:** Mirrors the supervisor's single-owner model for processes (plan/21 §4.1).
  One writer means validation always runs before write and the read model always reflects
  reality.

### D73. Validate before write, schema-gated (PROPOSED)
- **Decision:** Every config edit is validated against a schema (JSON/TOML for MCP, YAML
  frontmatter for skills, JSON for hooks) before it is written. The validator is a pure
  function and the single gate to disk.
- **Rationale:** Prevents silent breakage where the harness ignores a malformed config. A pure
  validator is unit- and property-testable and mutation-gated (D21, D33).

### D74. No plaintext secrets in the UI or config (PROPOSED)
- **Decision:** Env vars, headers, and hook commands reference secrets via the session-cache
  model (D23) or `${VAR}` expansion. The UI never stores or displays raw secret values; it
  shows a masked reference and a "reveal" only for the reference, never the resolved value.
- **Rationale:** Consistent with D23 and plan/17. Prevents secret leakage through config files
  or UI screenshots.

### D75. Registry browse is a cached, read-only view (PROPOSED)
- **Decision:** The Marketplace tab consumes the official MCP Registry REST API
  (`https://registry.modelcontextprotocol.io/v0.1/servers`) as a cached, read-only, bounded
  (D18) view. It renders `server.json` metadata and fills the editor on "Add"; it never
  downloads or executes server binaries.
- **Rationale:** The registry hosts metadata, not binaries
  (https://modelcontextprotocol.io/registry/about). A read-only, cached view keeps the panel
  fast and resilient to registry preview instability.

### D76. Project hooks require explicit trust (PROPOSED)
- **Decision:** Project hooks (and project MCP/LSP servers) require an explicit trust grant
  before they run, surfaced in the Customize panel, consistent with Grok's `/hooks-trust`
  model. The decision is stored per-folder.
- **Rationale:** Project hooks run arbitrary commands on lifecycle events, including the
  blocking `PreToolUse`. Trust must be explicit and visible, not silent.

---

## 6. Security considerations

The Customize panel is a management surface for code that executes on the user's behalf
(MCP servers run arbitrary commands via npx; hooks run commands on lifecycle events; skills
are folders of instructions and scripts). It follows plan/17's principles: least privilege,
fail closed, auditability.

1. **No plaintext secrets.** Env vars, headers, and hook commands reference secrets via the
   session-cache model (D23) or `${VAR}` expansion. The UI shows masked references, never
   resolved values. Validation rejects raw secret-looking values in configs (D74).
2. **Confirm destructive actions.** Removing an MCP server, skill, or hook requires an
   explicit confirmation, especially when the server is currently `ready` (removing it tears
   down a live process via the supervisor, plan/21 §4.5). The confirmation states the
   consequence.
3. **Project hooks require trust.** Project hooks do not run until the user grants trust for
   the folder (D76). The panel surfaces which hooks are pending trust, which are trusted, and
   what each hook runs. This is the same gate Grok applies (`/hooks-trust`), made visible.
4. **Remote servers are https-only.** Remote MCP `url` values must be `https` (or
   `http://localhost` for local dev). This prevents credential-bearing headers from being sent
   over plaintext.
5. **Registry metadata is untrusted input.** `server.json` from the registry is rendered as
   data, never executed. The "Add" flow fills the editor from metadata but the user reviews
   and saves it; nothing from the registry runs automatically.
6. **Auditability.** Every add/edit/remove/enable/disable and every trust grant is an event in
   the read model, replayable for review, consistent with plan/17's auditability principle and
   plan/21's event-sourced model.

---

## 7. Testing strategy

The config layer and the panel are tested under the project's TDD-at-inception gate chain
(fmt → clippy → unit+property → mutation → integration → component → e2e → coverage), per
plan/15.

### 7.1 Unit tests (config parse / validate)

Co-located `#[cfg(test)]` modules over the config layer:
- **MCP parse:** parse `[mcp_servers.<name>]` blocks (stdio and remote) into the registry
  entry struct; assert field mapping and scope merge order (user < project < managed < compat).
- **Validator:** for each schema, feed valid and invalid configs and assert the exact error.
  stdio-without-`command`, remote-without-`url`, plaintext-`http` remote, raw-secret-looking
  env value, malformed `SKILL.md` frontmatter, malformed hook JSON, unknown hook event.
- **Round-trip:** parse → serialize → parse is identity for every supported config shape.

### 7.2 Property tests (proptest)

- **Validator completeness:** generated configs that pass the validator always parse and load;
  generated configs that fail always produce a typed error (no silent acceptance).
- **Scope merge:** arbitrary combinations of user/project/managed/compat configs merge
  deterministically; a project server with the same name as a user one always replaces it.
- **Round-trip identity:** arbitrary valid config values serialize → parse → serialize
  identically.

### 7.3 Integration tests (real core + mock supervisor)

- **Add → supervisor reaction:** issue `config.add_server`; assert the config file is written
  correctly and the supervisor spawns the server (mock MCP server, per plan/21 §8.3).
- **Edit → identity change:** edit a server's `url`; assert the supervisor spawns a new
  instance and drains the old one (plan/21 §4.4).
- **Remove → teardown:** remove a `ready` server; assert it is torn down and the config file
  no longer contains it.
- **Registry browse:** mock the registry REST API; assert pagination, caching, and graceful
  degradation when unreachable.

### 7.4 Mutation testing

cargo-mutants over the config parser, validator, and scope-merge logic. CI gates: ≥85% line,
≥80% branch, ≥70% mutation score killed (D21, D33). The validator is a prime mutation target:
a surviving mutant that lets a bad config through must be killed by the unit/property tests.

### 7.5 Component tests (GPUI panel)

- Render the Customize panel in the headless GPUI harness; assert the tab structure, the
  server list with live status badges, and the editor forms.
- **Form validation:** a bad field shows an inline error and disables Save; a valid form
  enables Save.
- **Snapshot tests:** golden snapshots of the panel layout (per plan/10 §9.2).

### 7.6 E2E

Drive the real app headless; assert that adding a server through the panel results in a
working MCP tool call, that removing it tears it down, and that a project hook requires trust
before running. This is the direct regression test for the original problem (file-only config
with no lifecycle visibility).

---

## 8. Open questions / risks

These are flagged, not decided here:

1. **Raw TOML/JSON editing vs form-only.** This doc proposes a form with an "advanced" raw
   view. Whether the MVP ships the raw view, and how much of the form is generated from the
   schema vs hand-built, is a UX/effort decision for plan/19.
2. **Marketplace sources beyond the official registry.** Grok supports
   `[[marketplace.sources]]` and Claude/Cursor marketplaces. Whether Multiplexer's Marketplace
   tab aggregates multiple sources in MVP, or ships the official registry browse only, is open.
3. **Scope of skills editing.** Creating a skill folder and editing frontmatter is in scope;
   whether the panel also manages the skill's script files (a mini file editor) or defers to
   the main editor (plan/09) is open.
4. **Trust model granularity.** Grok stores trust per-folder in `~/.grok/trusted_folders.toml`
   covering project MCP and LSP servers too. Whether Multiplexer reuses that file or introduces
   its own trust store, and whether trust is per-folder or per-hook, needs a decision.
5. **Registry preview instability.** The MCP Registry is in preview; breaking changes or data
   resets may occur. The browse is designed to degrade gracefully, but the exact fallback
   (empty state vs a bundled curated list) is open.
6. **Interaction with grok-build's centralizing MCP management.** Upstream is centralizing MCP
   management server-side (gateway catalog, per `docs/UPSTREAM-TRAJECTORY.md`). How the panel
   reconciles with managed/gateway servers, and with the `[compat.claude]`/`[compat.cursor]`
   disable flags, needs a decision as upstream evolves (track via D31).

**Flagged consistency note:** this doc is consistent with PLAN-CONTEXT (server-centric
runtime, event-sourced orchestration, bounded channels, secrets session-cache model) and with
plan/21 (the panel consumes the supervisor's live status as a read-only projection and does
not duplicate its process design). If any locked decision flips (e.g. stack, crate layout),
the affected sections (§4, §5) must be revisited.

---

*Next: `plan/27-*.md`; see `plan/19-roadmap-and-milestones.md` for the ordering of remaining
plan docs.*
