# 17: Cores / Resources inspector audit

**Date:** 2026-08-12
**Scope:** Resources tab (`InspectorTab::Resources`), `core_rows`, `sample_cores`, `multiplexer-resman`, desktop pump/refresh.
**Method:** Read-only. No `cargo`. Compared live code to `plan/24-resource-manager.md`, `plan/32-list-rows.md` cores section, `plan/33-inspector-customize.md` section 6.2.
**Verdict:** The Cores tab is a list of `cpuN` rows with a reserved badge and a Unicode block bar. It is not the plan/24 node view. It does not pin, cap, enable, or contain anything. Live sampling hardcodes reserved to cores 0 and 1. Click only expands an accordion.

FINDINGS: 8

Honesty first: most of the product gap is **wiring**, not a missing crate. `multiplexer-resman` already has a core bitmap, a session manager, Windows `JobContainment`, and tests for kill-on-close. The running desktop never feeds that manager into the rail. Painting a prettier bar will not make reservation or Job Objects real.

---

## Layer split

| Layer | What exists | What plan/24 (and 32/33) require |
|---|---|---|
| Engine (`multiplexer-resman` bitmap + manager) | `CoreBitmap` with static reserve of 0,1 when `n_cores > 2`, `set_enabled`, allocate/free. `ResourceManager` binds a session to fake or Job containment. | Same, plus `NodeState` / `CoreState` (enabled, usage, pinned session, RAM). Fleet bitmap + free-RAM. |
| Engine (telemetry) | `sample_cores` / `sample_cores_from` / `format_core_bar`. CPU only. New `sysinfo::System` per call. | Long-lived `System`, `refresh_cpu_usage` at monitor cadence (D64). RAM from sysinfo plus Job/cgroup accounting. |
| Engine (containment) | `JobContainment` (Windows) with kill-on-close. `FakeContainment` for tests and `SessionRuntime`. Terminal capture explicitly skips Job assignment. | Every agent / MCP / terminal tree in a Job/cgroup. Inspector shows that tree. |
| UI (shell + desktop) | `CoreRow { index, usage, reserved }`. `core_rows` is a vertical list. Click = `toggle_right_row`. Live pump passes reserved `[0, 1]`. | Clickable enable/disable (24) or at least `toggle_core_reserved` (32). `CoreCell` 4-wide grid (33). RAM. Job/session viz. Real GPUI bars, not `█░` text. |

The fallback dump `resource_detail` still starts with the static sentence `Job Object kill-on-close is armed.` That sentence is not backed by the desktop process tree.

---

## F1. Live path treats reserved as cpu0-1; each sample is a throwaway System

- **Severity:** Major
- **Layer:** Both. Telemetry API samples every sysinfo CPU, but the host reserved list and first-sample usage make the live rail dishonest.
- **Plan:** 24 section 4.3 / D59 reserve 0,1 for the app, then pin sessions to the remaining enabled pool. D64: one long-lived `System`, refresh no faster than `MINIMUM_CPU_UPDATE_INTERVAL`, at the power-adaptive cadence.
- **Evidence:**
  - `sample_cores(reserved)` (`crates/multiplexer-resman/src/telemetry.rs` 34-38) builds a **new** `System`, calls `refresh_cpu_usage` once, maps **all** `sys.cpus()`. The `reserved` slice is only a badge filter (`reserved.contains(&index)`). It is not a sample set. Comment on the same fn: first call may report 0% because usage is a delta; this fn does not sleep and does not keep the `System`.
  - Desktop init and the 1.5 s pump both pass `&[0, 1]` (`apps/multiplexer-desktop/src/main.rs` 80, 549-560). Every live row that is not index 0 or 1 is unreserved, regardless of `CoreBitmap` or user intent.
  - Manual Reload calls `sample_cores(&(0..8).collect())` then `reserved: c.reserved || c.index < 2` (`main.rs` 265-273). That marks **every** index in `0..8` reserved (the list is the reserved set), then ORs 0 and 1. After Reload, cpu2-cpu7 show "reserved" until the next pump overwrites them with `[0, 1]` again.
  - Pump only runs while the Resources tab is selected. Other tabs freeze the last sample. Cadence is 1500 ms, not plan/16 1 s / 5 s / 15 s, and not the sysinfo floor on a retained instance.
- **Why it matters:** The hot path looks like "we only care about cpu0 and cpu1." All logical CPUs are enumerated, but reservation is a hardcoded pair and usage is a first-refresh (often 0%). Reload and pump disagree. D64 is not implemented.

---

## F2. Reserved toggle is not real

- **Severity:** Critical (lying control). Plan/32 already asked for a model flip; plan/24 asks for an allocator event.
- **Layer:** UI. The bitmap can reserve and disable. The rail never calls it.
- **Plan:** 24 section 4.5: click toggles `enabled`, emits `CoreEnabled` / `CoreDisabled`, allocator drops the core, pinned sessions re-pin. 32: `toggle_core_reserved(index)` flips `cores[i].reserved`, no accordion, Reload **preserves** user flags by index.
- **Evidence:**
  - `core_rows` (`crates/multiplexer-shell/src/inspector_model.rs` 58-79) paints a `reserved` badge. It has no action, no `enabled` field, no `selected` from user toggle.
  - Desktop `inspector_row_el` (`main.rs` about 1881-1886) on left click only `toggle_right_row(id)`. That is the accordion key (`core:0`), not a reserve flip. The inspector test (`inspector_model.rs` 284-294) asserts expand, not reserved.
  - Grep of first-party UI finds no `toggle_core_reserved`, no `set_enabled`, no `CoreEnabled` / `CoreDisabled`. `CoreBitmap::set_enabled` / `reserve` exist (`crates/multiplexer-resman/src/bitmap.rs` 68-88) and are unused by desktop.
  - Toolbar copy is "Refresh reserved cores" (`apps/multiplexer-desktop/src/inspector.rs` 46-49). Reload resamples usage. It does not toggle, and it does not merge prior flags (F1).
  - Fallback dump hardcodes `Reserved cores: 0, 1 (app)` (`workspace.rs` 647-649) even when `ws.cores` is empty or when Reload marked 0..8.
- **Why it matters:** The badge looks like policy. Click looks like a control. Neither changes the allocator pool. The UI cannot disable a core for agent use. That is the plan/24 visual's whole point.

---

## F3. No RAM

- **Severity:** Major
- **Layer:** Engine telemetry + UI. Neither side has a RAM figure to project.
- **Plan:** 24 section 4.5 `NodeState { ram_total, ram_used }`. RAM from sysinfo plus per-tree Job/cgroup accounting. Section 4.4 / D60: 4 GiB tree / 2 GiB process caps. Fleet scheduler needs free-RAM.
- **Evidence:**
  - `CoreSample` and `CoreRow` are `{ index, usage, reserved }` only (`telemetry.rs` 6-11, `workspace.rs` 130-136). No bytes, no cap, no pressure.
  - `telemetry.rs` calls `refresh_cpu_usage` only. No `refresh_memory`, no `total_memory` / `used_memory`. `format_core_bar` is CPU percent text.
  - `core_rows` and `resource_detail` never print GiB, RSS, or a cap. Status strip counts cores (`status.rs`), not RAM.
  - No `NodeState` / `CoreState` type in first-party crates (only the plan/24 sketch).
- **Why it matters:** Plan/24's killer visual is "every machine, every core, every gigabyte." Without RAM, the fleet allocator cannot exist and the orphan-RAM story (10.4 GB / 27.9 GB in plan/24 section 1) cannot be shown.

---

## F4. No Job Object visualization (static caption is a lie)

- **Severity:** Critical
- **Layer:** Engine exists in tests. UI claims it is armed. The running desktop does not show or own a job.
- **Plan:** 24 sections 3 / 4.2 / D58: every process tree in a Job Object (Windows) with kill-on-close. Visual is a projection of that containment: sessions on cores, tree limits, reaped vs live.
- **Evidence:**
  - `resource_detail` (`workspace.rs` 647-649) always prefixes `Job Object kill-on-close is armed.` There is no job handle, pid, process count, or memory cap in `Workspace`.
  - `core_rows` has no job, session, or tree fields. Expand shows the text usage bar only (`main.rs` inspector row: meta when `expanded`).
  - Desktop inspector depends on `sample_cores`, not `ResourceManager` or `JobContainment`. `SessionRuntime` uses `ResourceManager::fake(8)` (`crates/multiplexer-core/src/runtime.rs` 17-49). Desktop `Server::with_local` is not that runtime's resman on the rail.
  - Terminal capture states the skip in-file: "Job assignment is skipped. multiplexer-resman `JobContainment` is not used" (`crates/multiplexer-terminal/src/capture.rs` 1-6).
  - `JobContainment` is real and tested (`crates/multiplexer-resman/tests/containment_job.rs`). Nothing in the inspector lists a job, a child pid, `ActiveProcessLimit`, or kill-on-close state.
- **Why it matters:** The dump tells the user the orphan fix is on. The product process trees (grok turn, shell cmd, PTY) are not shown as contained. A caption is not a visual and not a guarantee.

---

## F5. Bars are text, not a visual

- **Severity:** Major
- **Layer:** UI. `format_core_bar` is the same idea in the engine crate and is unused by the rail.
- **Plan:** 24 section 3.5 "beautiful live visual" / node view. 33 section 6.2: `CoreCell` run wrapped as a 4-wide GPUI grid; reserved cores highlighted; **do not** render `tiny_usage_bar`. 32: public `usage_bar` on the row, then stop calling the private dump bar.
- **Evidence:**
  - `core_rows` sets `subtitle` to `{usage:.1}%` and `meta` to `usage_bar(c.usage, 10)` (`inspector_model.rs` 72-75). `usage_bar` (`crates/multiplexer-shell/src/bars.rs` 4-10) is `"█".repeat` + `"░".repeat`.
  - Desktop paints `meta` as a `div().child(meta)` string, and only when the row is expanded (`main.rs` inspector row). Collapsed cores are title + percent + optional badge. No fill rect, no grid, no `CoreCell` kind (`ListRowSpec` has no `kind`).
  - `tiny_usage_bar` still lives on `resource_detail` (`workspace.rs` 897-907), 8 ticks, same block characters.
  - `format_core_bar` (`telemetry.rs` 43-53) produces `████░░░░░░ 41%`. Call sites outside `telemetry.rs` tests: none.
- **Why it matters:** Plan/24 sells a live machine diagram. What shipped is a debug list whose "bar" is a font-dependent string, hidden until expand. That is the plan/33 problem statement ("ASCII bars inside a paragraph") with a row wrapper.

---

## F6. Inspector is not wired to the resource manager

- **Severity:** Critical
- **Layer:** Both (engine unused by host; UI has no session/pin projection)
- **Plan:** 24 section 4.1: orchestration asks resman to spawn contained, limited, pinned groups. Visual is the read-model projection of `NodeState` / allocations.
- **Evidence:**
  - Desktop maps `sample_cores` to `CoreRow` only (`main.rs` 80-87, 265-273, 552-559). No `ResourceManager`, no `SessionAlloc`, no `alloc_of`, no pinned core set.
  - `CoreRow` has no `pinned_session`, no `enabled`. `core_rows` cannot show which session owns a core.
  - `SessionRuntime` allocates one fake core per start (`CORES_PER_SESSION = 1`, `runtime.rs` 17-18, 73-75). Desktop chat send does not go through that path for the inspector.
  - Status line is `N cores` = `ws.cores.len()` (`status.rs`), i.e. last sysinfo sample length, not free-enabled count from the bitmap.
- **Why it matters:** The allocator and the rail are two products. You can "reserve" in the UI (you cannot, F2) without changing allocations, and you can allocate in tests without the UI noticing. The killer feature is the join of those two.

---

## F7. No CoreCell grid, files section, or row actions (plan/33)

- **Severity:** Major
- **Layer:** UI
- **Plan:** 33 section 6.2: section `cores.header`, `CoreCell` per sample (`id` `core.{index}`, badge `R`, action Refresh), then section `files.header` plus file rows. Toolbar Reload maps to `RefreshCores`. Invariant: non-section rows have 1 to 3 actions.
- **Evidence:**
  - `core_rows` emits `core:{index}` (colon, not dot), no section header, no `RowKind::CoreCell`, no `actions` field on `ListRowSpec` (`widgets.rs` 90-104).
  - Files are a separate inspector tab (`InspectorTab::Files`) plus a dump tail inside `resource_detail`, not a Files section under Cores.
  - Empty state is `core:empty` / "No core samples" with no Refresh action (`inspector_model.rs` 59-62).
- **Why it matters:** Even the Phase 0.4 projector is unfinished. The rail is still a tab over a list, not the specified control surface.

---

## F8. No node, no enabled bit, no fleet

- **Severity:** Major
- **Layer:** Engine model + UI. Local node is not represented; 1-100 is unstarted.
- **Plan:** 24 sections 4.5-4.6 `NodeState` (id, cores, RAM, sessions, enabled). Fleet scheduler: core bitmap + free-RAM + heartbeat. Click disable whole node.
- **Evidence:** `Workspace` holds `Vec<CoreRow>` only. No node id, no heartbeat, no session list on the Resources tab. `CoreBitmap` is per-process and not exposed on the wire. No `resman.*` methods in `multiplexer-wire`.
- **Why it matters:** Plan/19 calls plan/24 a killer feature across Phase 1 (local) and Phase 4 (fleet). Local node state is the Phase 1 slice. It is not on screen.

---

## What the stack does get right

- `CoreBitmap` reserves 0,1 when there are more than two cores and refuses to allocate them (`bitmap.rs` 34-36, 58-64). Tests cover skip-reserved and enable/disable.
- `JobContainment` is a real kill-on-close Job Object with integration tests. That is the D58 primitive. It is not what the inspector shows.
- `sample_cores_from` is pure and property-tested. The host should keep using it after a long-lived `System` refresh.
- Resources is a first-class tab. Palette `/cores`, term builtin `cores`, and Reload all reach the same host resample.
- Shell projector is pure: `inspector_rows` maps `ws.cores` without sysinfo. That split is correct. The host fill is what is wrong.

None of that substitutes for a live `NodeState`, a real reserved toggle, RAM, or a Job/session visual.

---

## Suggested order (not in scope for this audit)

1. Host: keep one `System`, sample all CPUs, merge reserved/enabled from `CoreBitmap` (or `ws.cores` flags) by index. Stop passing `0..8` as reserved. Stop claiming Job Objects in a static string.
2. UI: `toggle_core_reserved` / `set_enabled` on click (plan/32), not accordion. Preserve flags across Reload.
3. Engine+UI: `NodeState` with RAM (sysinfo + job accounting). Project RAM and pinned session on the rail.
4. Visual: `CoreCell` grid + GPUI fill bars. Delete dump `tiny_usage_bar` from the live body.
5. Wire desktop sessions (and later MCP/terminal) through `ResourceManager` + `JobContainment`, then show the tree. Until then, do not say kill-on-close is armed.
