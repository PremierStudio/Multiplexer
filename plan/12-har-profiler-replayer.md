# 12 — HAR Profiler / Replayer

> **Status:** Authoritative plan doc. Consistent with `docs/PLAN-CONTEXT.md` (the shared plan context). If any statement here conflicts with PLAN-CONTEXT, PLAN-CONTEXT wins and the conflict should be flagged in `plan/20-risks-and-open-questions.md`.
>
> **Purpose:** Specify the built-in HAR (HTTP Archive) profiler/replayer — capture network traffic via CDP on the system browser, visualize request waterfalls, and replay recorded sessions. This is core differentiator #4 and an explicit user ask: it lets the agent and user make "super smart efficient decisions" about performance. No competitor ships this.
>
> **Inputs:** `plan/11-system-browser-integration.md` (CDP transport), `plan/02-architecture.md` (server-centric runtime, SQLite read model), `plan/10-ui-pane-system.md` (right-bar pane), `plan/06-orchestration-engine.md` (agent context feed), `plan/15-testing-strategy.md` (TDD gates).

---

## 1. The core idea

Multiplexer captures the network traffic of the code it runs — the browser it drives and the HTTP activity of the embedded harness — and turns it into **insight** and **action**:

- **Profile:** record a session's network activity into a HAR file, then visualize it as a request waterfall (per-request DNS / connect / SSL / wait / receive timing bars).
- **Replay:** re-issue the recorded requests against the same targets, compare responses, and measure performance deltas between runs.
- **Decide:** feed the analysis into the agent's context so the agent can make "super smart efficient decisions" — e.g. *"this request is the bottleneck, here's the waterfall, here's the fix."*

This is a core differentiator (PLAN-CONTEXT §"Core differentiators", item 4). No competitor ships HAR tooling: Orca bundles Chromium but has no HAR, T3 Code has none, Superset/Conductor/OpenCode/Codex have none. It turns Multiplexer from "a place to run agents" into "a place to *understand* what the agent's code actually did on the network."

### 1.1 Why it matters to the user

| Problem today | Multiplexer's answer |
|---|---|
| "Why is my page slow?" — guesswork, DevTools buried in a separate browser | One-click capture in the pane you already have open, next to the agent that wrote the code |
| "Did my change make it faster?" — manual before/after timing | Replay the recorded session and diff the waterfall + metrics |
| "Which request is the bottleneck?" — eyeballing network tabs | Waterfall with blocking-resource detection and per-request timing |
| "The agent can't see the network" — agent works blind | HAR analysis is fed into the agent's context as first-class evidence |
| "I don't want to leak my API bodies" — privacy fear | Sensitive bodies are not stored by default |

### 1.2 Scope boundaries

- **Capture source #1 (primary):** the system browser driven via CDP (`plan/11`). This is where the user's web app runs.
- **Capture source #2 (secondary):** the embedded harness's own HTTP activity (agent tool calls that hit the network — e.g. `web_fetch`, search, provider API calls). This is a differentiator *within* a differentiator: we can see what the agent itself did on the network, in-process.
- **Out of scope for MVP:** capturing arbitrary non-browser, non-harness processes on the host (that is a system-level packet capture, not a HAR capture). Noted as an open question.

---

## 2. HAR capture

### 2.1 The HAR format

HAR 1.2 is the interchange format (a JSON document). We build and consume it. Top-level shape:

```json
{
  "log": {
    "version": "1.2",
    "creator": { "name": "Multiplexer", "version": "0.1.0" },
    "pages": [ { "startedDateTime": "...", "id": "page_1", "title": "...", "pageTimings": {} } ],
    "entries": [
      {
        "startedDateTime": "...",
        "time": 123.4,
        "request":  { "method": "GET", "url": "...", "httpVersion": "HTTP/2", "headers": [], "queryString": [], "cookies": [], "headersSize": -1, "bodySize": 0 },
        "response": { "status": 200, "statusText": "OK", "httpVersion": "HTTP/2", "headers": [], "cookies": [], "content": { "size": 0, "mimeType": "text/html", "text": "" }, "redirectURL": "", "headersSize": -1, "bodySize": 0 },
        "cache": {},
        "timings": { "blocked": 0, "dns": 0, "connect": 0, "ssl": 0, "send": 0, "wait": 0, "receive": 0 },
        "serverIPAddress": "93.184.216.34",
        "connection": "123",
        "comment": ""
      }
    ]
  }
}
```

We treat HAR as the **serialization** format, not the in-memory model. In-memory we keep a richer typed model (see §3) and serialize to HAR for export/interop.

### 2.2 CDP Network domain mapping

Capture is driven by the CDP `Network` domain on the browser target (`plan/11` provides the transport). The events we subscribe to and how they map:

| CDP event | HAR contribution |
|---|---|
| `Network.requestWillBeSent` | Create entry; record method, url, headers, postData, timestamp, wallTime, requestId |
| `Network.requestWillBeSentExtraInfo` | Associated headers (incl. cookie header) + `associatedCookies` |
| `Network.responseReceived` | Record status, statusText, headers, mimeType, `fromDiskCache`/`fromPrefetchCache`, `encodedDataLength` |
| `Network.responseReceivedExtraInfo` | Response headers + `setCookie` info |
| `Network.dataReceived` | Accumulate `dataLength` / `encodedDataLength` for receive timing + body size |
| `Network.loadingFinished` | Finalize entry: total `encodedDataLength`, compute `time` and `timings` |
| `Network.loadingFailed` | Mark entry failed; record `errorText`, `canceled`, `blockedReason` |
| `Network.requestServedFromCache` | Mark entry as served from cache |
| `Network.webSocketCreated/FrameSent/FrameReceived/Closed` | WebSocket entries (HAR has no native WS type; we store as a custom entry or omit bodies) |
| `Page.frameStartedLoading` / `Page.loadEventFired` | Page boundaries → `pages[]` and page timings |

**Timing computation.** CDP gives us timestamps per event (monotonic `timestamp` + `wallTime`). We derive the HAR `timings` fields:

- `blocked` = time from request start to when the request actually began (queued/blocked).
- `dns` = from `Network.dnsTiming`-style info when available; else 0.
- `connect` = from `Network.connectTiming` when available; else 0.
- `ssl` = from `Network.connectTiming.sslStart/sslEnd` when available.
- `send` = from request start to `Network.requestWillBeSentExtraInfo`/first byte sent.
- `wait` = from request sent to `Network.responseReceived` (TTFB).
- `receive` = from `responseReceived` to `loadingFinished` (body download).

Where CDP does not expose a sub-phase (e.g. DNS/connect for HTTP/2 multiplexed requests), we leave the field `0` and mark it "not measured" in the UI rather than fabricating a value. **Never invent timings** — the waterfall must be honest about what CDP actually reported.

### 2.3 Capture lifecycle

- **Start:** `Network.enable` on the target, subscribe to the events above, open a new `pages[]` entry.
- **Stream:** events are appended to an in-memory capture buffer and mirrored to the UI in real time (the waterfall updates live).
- **Stop:** `Network.disable`, finalize all open entries, close the page, persist the capture (see §3).
- **Attach/detach:** a capture can be attached to a browser tab, to a whole browser session, or to a specific agent turn (see §6).

### 2.4 Capturing the embedded harness's HTTP activity

Because the harness is embedded in-process (`plan/03`), we can instrument its HTTP activity directly rather than via CDP. Two mechanisms:

1. **Tool-level instrumentation:** the vendored `xai-grok-tools` network tools (`web_fetch`, search, provider API calls) are wrapped so each network call emits a `HarEvent` into the same capture pipeline. This gives us HAR entries for what the *agent* did, not just what the browser did.
2. **HTTP client hook:** if the harness uses a shared HTTP client (e.g. `reqwest`), we install a middleware that records request/response/timing per call.

This is a genuine differentiator: no competitor can show "here is the network traffic your agent generated" because no competitor embeds the harness in-process. It is what makes the agent's performance decisions evidence-based.

---

## 3. HAR storage

### 3.1 Data model

In-memory we keep a typed model; SQLite is the durable store (consistent with the event-sourced read model in `plan/02`/`plan/06`). Core tables:

```
captures(id, name, started_at, ended_at, source, browser_profile, agent_turn_id, page_count, entry_count, total_bytes, status)
pages(id, capture_id, page_id, title, started_at, load_event_at, timings_json)
entries(id, capture_id, page_id, request_id, method, url, host, path, status, mime_type,
        started_at, wall_time, time_ms, blocked_ms, dns_ms, connect_ms, ssl_ms, send_ms,
        wait_ms, receive_ms, request_headers_json, response_headers_json,
        request_body_ref, response_body_ref, request_bytes, response_bytes,
        from_cache, failed, error_text, server_ip, connection_id, is_websocket)
bodies(id, capture_id, entry_id, kind, mime_type, size, storage_ref, sensitive)   -- see §3.3
```

The `entries` table is the workhorse: it is denormalized for fast waterfall queries and metric aggregation (no JSON parsing at read time for the hot path).

### 3.2 Storage location & lifecycle

- **Location:** SQLite in the core's data directory (same DB family as the read model). Captures are first-class records, not loose files.
- **Export/import:** a capture can be exported to a `.har` file (for sharing, DevTools interop, or CI) and imported back. Export is a serialization of the SQLite rows to HAR JSON.
- **Retention:** captures are retained by default with a configurable cap (count and/or total bytes). Old captures are pruned oldest-first unless pinned. Pinning a capture (e.g. one you're replaying against) protects it from pruning.
- **Size management:** bodies are the dominant cost. See §3.3 for the default policy that keeps captures small.

### 3.3 Privacy & body storage (default: don't store sensitive bodies)

The default posture is **privacy-first**: we do not store sensitive request/response bodies unless the user opts in.

- **Default:** store only **headers + metadata + sizes + timings**. Bodies are captured in memory for the live waterfall (so the user can inspect the current session) but **not persisted** to SQLite.
- **Opt-in body capture:** a per-capture toggle "store response bodies" / "store request bodies". When off (default), `bodies` rows are not written.
- **Sensitive filtering:** even when body capture is on, we redact by content-type and by header. Never persist bodies whose `Content-Type` is `application/json` with auth-shaped content, or any body carrying an `Authorization`/`Cookie`/`Set-Cookie`/`X-Api-Key` header. Redaction is applied at write time, not display time.
- **Keychain rule:** per PLAN-CONTEXT, secrets live in the OS keychain and configs use `op://Vault/Item/field` references only. HAR bodies are **never** a place to store secrets; the redaction layer is the enforcement point.
- **UI affordance:** the pane shows a clear "bodies not stored" indicator and a one-click "capture bodies for this session" toggle, so the privacy default is visible, not silent.

---

## 4. Waterfall visualization

The waterfall is a **right-bar pane** (`plan/10`). It renders one horizontal timing bar per request, segmented into the HAR timing phases.

### 4.1 Layout

```
┌──────────────────────────────────────────────────────────────┐
│ HAR · capture "checkout flow" · 42 req · 1.8s · [Replay] [Export] │
├──────────────────────────────────────────────────────────────┤
│  #  Method  URL                          DNS Cnt SSL Snd Wt Rcv │
│  1  GET     /index.html                   ██  ██  ██  █  ████ │
│  2  GET     /app.js                       ██  ██  ██  █  ██████│
│  3  GET     /api/cart                     ██  ██  ██  █  ████████│
│  4  GET     /api/pricing                  ██  ██  ██  █  ██████████│
│  ...                                     (bars aligned to a shared time axis) │
├──────────────────────────────────────────────────────────────┤
│  Summary: 1.8s total · 1 blocking resource · 3 cache hits     │
│  Slowest: /api/pricing (412ms wait) · Suggest: cache it       │
└──────────────────────────────────────────────────────────────┘
```

- **Shared time axis:** all bars share one horizontal time scale (the capture's timeline), so you can see which requests are serialized vs. parallel and where the critical path is.
- **Phase colors:** DNS / connect / SSL / send / wait / receive each get a distinct color, matching the HAR `timings` fields.
- **Row detail:** clicking a row expands headers, timing breakdown, and (if captured) the body.
- **Filters:** by method, host, status, mime type, cache hit/miss, failed. A "blocking" filter highlights requests on the critical path.
- **Live mode:** during capture the waterfall streams in real time; rows appear as requests are issued and bars fill as phases complete.

### 4.2 Waterfall computation

The waterfall is a pure function of the capture's entries: `Vec<Entry> -> WaterfallModel`. It computes, per entry, the phase offsets (start + cumulative phase durations) and the shared time axis (min start → max end). Because it is pure, it is trivially unit-testable and property-testable (see §8). The UI renders the model; it never computes timing itself.

### 4.3 Critical path / blocking detection

A request is **blocking** if it lies on the critical path — the longest chain of dependent requests from navigation start to load event. We compute this from the dependency structure (a request is a dependency of another if it was initiated by it, per CDP `initiator` info) plus the shared timeline. Blocking resources are highlighted and surfaced in the summary ("1 blocking resource").

---

## 5. Replay

Replay re-issues the recorded requests against the same targets and measures the delta. It is the "did my change make it faster?" tool.

### 5.1 The replay engine

A dedicated `ReplayEngine` in the core:

1. **Load** a capture (from SQLite or an imported `.har`).
2. **Re-issue** each entry's request (method, URL, headers, body) in the recorded order, using the same HTTP stack the browser/harness used (or a configurable client). Replay does **not** require a browser — it is a plain HTTP client replay, which makes it fast and deterministic.
3. **Record** the replayed response: status, headers, timing (per-phase where measurable), size.
4. **Compare** replayed vs. recorded: status match, body hash match, per-request timing delta, aggregate delta.
5. **Emit** a `ReplayReport` (see below).

### 5.2 Replay modes

| Mode | Behavior | Use case |
|---|---|---|
| **Baseline replay** | Re-issue against the same targets as recorded | Reproduce a session, verify it still behaves the same |
| **A/B replay** | Replay capture A and capture B (or A twice around a code change) and diff | "Did my change make it faster?" |
| **Target override** | Replay against a different base URL (e.g. localhost vs. staging) | Test the same flow against a different environment |
| **Throttled replay** | Apply latency/bandwidth shaping per request | Simulate slow networks, find what breaks |

### 5.3 The ReplayReport

```rust
pub struct ReplayReport {
    pub capture_id: CaptureId,
    pub mode: ReplayMode,
    pub started_at: DateTime<Utc>,
    pub total_entries: usize,
    pub replayed: usize,
    pub failed: usize,               // status mismatch, timeout, connection error
    pub total_time_ms: u64,          // recorded vs replayed
    pub recorded_total_ms: u64,
    pub replayed_total_ms: u64,
    pub per_entry: Vec<EntryDelta>,  // per-request recorded vs replayed
    pub body_mismatches: usize,      // entries whose response body hash differs
    pub slowest_entries: Vec<EntryDelta>,
}
```

The report is both a UI object (rendered in the pane) and a **structured artifact the agent can read** (see §6).

### 5.4 Replay safety

- Replay **re-issues real requests** to real servers. This can have side effects (POSTs, writes). Default posture: replay is **read-only-safe by default** — we warn on and require confirmation for non-idempotent methods (POST/PUT/DELETE/PATCH), and we offer a "dry-run" mode that issues only idempotent requests.
- Replay never sends stored bodies that were redacted at capture time (they were never stored).
- Replay is rate-limited and can be pointed at a target override to avoid hammering production.

---

## 6. Agent integration

The agent uses HAR data to make performance decisions. This is the "super smart efficient decisions" ask made concrete.

### 6.1 Feeding HAR analysis into agent context

HAR data enters the agent's context as **structured evidence**, not raw dumps. We render a capture/report into a compact, token-efficient text block the agent can reason over:

```
HAR capture "checkout flow" (42 requests, 1.8s total, 3 cache hits)
  Slowest 5:
    GET /api/pricing       412ms wait   (blocking)  1.2KB
    GET /api/cart          388ms wait   (blocking)  0.9KB
    GET /app.js            301ms wait               240KB
    GET /api/recommend     250ms wait               1.1KB
    GET /index.html        120ms wait                18KB
  Blocking resources: /api/pricing, /api/cart
  Cache misses: /api/recommend (no Cache-Control), /app.js (no ETag)
  Suggestion: add Cache-Control to /api/pricing; parallelize /api/cart
```

This is fed into the agent turn via the same mechanism the harness uses for tool results (a `har_analysis` tool result / context block). The agent can then act: propose a fix, edit the code, and the user can **replay** to verify.

### 6.2 Agent-initiated capture & analysis

- **Capture on demand:** the agent can request a capture of the current browser session (a tool call) and receive the analysis back.
- **Analyze existing capture:** the agent can ask for the analysis of any stored capture or any replay report.
- **Replay on demand:** the agent can trigger a replay and read the `ReplayReport` to verify its own change ("I added caching; replay shows /api/pricing 412ms → 41ms").

### 6.3 Turn-attached captures

A capture can be **attached to an agent turn** (via the orchestration read model, `plan/06`). This means: when the agent runs the app in the browser and something is slow, the capture is recorded against that turn, so the user can later ask "what did the agent do, and what did the network do?" together. This ties HAR into the event-sourced history rather than leaving it as an orphan file.

### 6.4 Token budget

HAR analysis is rendered **summarized by default** (top-N slowest, blocking, cache misses, aggregate metrics) with a "full detail" opt-in. This keeps agent context lean and avoids blowing the context window on a 200-request capture. The summarizer is a pure function (unit-testable) and is the same one used for the UI summary strip.

---

## 7. Performance analysis

The analysis layer computes metrics and suggestions from a capture or a replay report.

### 7.1 Metrics

| Metric | Definition |
|---|---|
| **Total load time** | Navigation start → load event (or last entry end) |
| **Per-request timing** | Full `time` + per-phase breakdown (DNS/connect/SSL/send/wait/receive) |
| **Blocking resources** | Requests on the critical path (see §4.3) |
| **Cache hits / misses** | From `fromDiskCache`/`fromPrefetchCache`/`requestServedFromCache` + response cache headers |
| **Bundle sizes** | Sum of response bytes by mime type (js/css/img), largest-first |
| **Request count** | Total + by host/mime |
| **Failed requests** | Status ≥ 400, `loadingFailed`, timeouts |
| **Serialization** | Long stretches where only one request is in flight (parallelism opportunity) |
| **Replay delta** | Recorded vs. replayed totals and per-entry (from `ReplayReport`) |

### 7.2 Suggestions (rule-based, evidence-backed)

The analysis layer emits concrete, actionable suggestions. Each is a pure rule over the metrics, so it is testable and the agent can cite it:

| Rule | Suggestion |
|---|---|
| `wait_ms` dominates and resource is cacheable | "Add Cache-Control / ETag to `<url>` (412ms wait, no cache headers)" |
| Resource on critical path, large body | "This blocking resource is 240KB; consider code-splitting / lazy-load" |
| Many requests to same host, no parallelism | "N requests to `<host>` are serialized; consider HTTP/2 multiplexing or batching" |
| Cache miss with no validator | "`<url>` misses cache every load; add a validator" |
| Replay delta large | "Replay shows `<url>` 412ms → 41ms after change" |
| Failed requests | "`<url>` failed with `<error>`; check server/network" |

Suggestions are **evidence-backed**: every suggestion carries the metric values that triggered it, so the agent (and user) can trust it rather than treat it as a guess.

### 7.3 Analysis output

Analysis produces a structured `HarAnalysis` (metrics + suggestions) that is rendered three ways: the UI summary strip, the full pane, and the token-efficient agent context block (§6.1). All three derive from the same pure computation — no divergent logic.

---

## 8. Testing

TDD at inception is non-negotiable (PLAN-CONTEXT §Testing). HAR is a great fit because most of the pipeline is pure and deterministic.

### 8.1 Unit tests (co-located `#[cfg(test)]`)

- **HAR parsing/building:** parse a fixture `.har` into the typed model and back; round-trip equality. Property-based (proptest): arbitrary entry sets serialize → parse → serialize identically.
- **CDP event → entry mapping:** feed synthetic `Network.requestWillBeSent`/`responseReceived`/`loadingFinished` sequences and assert the resulting entry's fields and timings. Property-based over event orderings.
- **Timing computation:** given synthetic CDP timestamps, assert the derived HAR `timings` fields; assert we emit `0`/"not measured" rather than fabricating when a phase is absent.
- **Waterfall computation:** given an entry set, assert phase offsets, shared axis, and critical-path/blocking detection. Property-based: random DAGs of request dependencies → blocking set is consistent.
- **Metrics & suggestions:** given a capture, assert each metric and each rule fires with the right evidence. Property-based over generated captures.
- **Replay engine (mocked client):** given a mock HTTP client, assert re-issue order, header/body fidelity, timing capture, and `ReplayReport` construction. Property-based over request sets.
- **Redaction/privacy:** assert sensitive bodies are never persisted by default; assert redaction by content-type and by auth-shaped header.

### 8.2 Mutation tests (cargo-mutants)

The pure functions (timing computation, waterfall, metrics, suggestions, redaction, HAR serialization) are prime mutation targets. CI gates: ≥85% line, ≥80% branch, ≥70% mutation score killed (PLAN-CONTEXT). The redaction layer and the timing computation are **mandatory** high-mutation-score areas — a mutant that leaks a body or fabricates a timing must be killed.

### 8.3 Integration tests (real core + mock agent)

- **Real browser capture:** drive a real browser via CDP (headless) against a local test server, capture, and assert the resulting capture contains the expected entries with sane timings. This is the "real browser capture" integration test.
- **Harness HTTP capture:** run the embedded harness against a mock network tool and assert its HTTP activity lands in the capture pipeline.
- **Replay against a live local server:** record a capture against a local server, replay it, assert the `ReplayReport` matches (statuses, body hashes, sane deltas).
- **Read-model assertion:** after a capture, assert the SQLite read model reflects the capture (counts, totals) — consistent with `plan/06`.

### 8.4 Component (GPUI) tests

- Waterfall pane renders the `WaterfallModel` correctly; snapshot tests for pane layouts (PLAN-CONTEXT §Testing).
- Summary strip renders metrics/suggestions from a `HarAnalysis` fixture.

### 8.5 E2E tests

- Drive the real app: open a browser tab, capture, stop, view waterfall, replay, view report. Assert the full loop works headless.

### 8.6 CI order

fmt → clippy (deny warnings) → unit+property → mutation → integration → component → e2e → coverage. All green before merge. No blind CI.

---

## 9. Open questions

Per PLAN-CONTEXT, these are pending decisions; this doc does **not** decide them unilaterally. They are tracked in `plan/20-risks-and-open-questions.md`.

1. **MVP depth of replay:** is replay (re-issuing requests) in the MVP, or is MVP capture + waterfall + analysis only, with replay following? Replay is the "did my change help" loop and the user's explicit ask, so the default is to include it — but it adds the `ReplayEngine` and its safety surface.
2. **Harness HTTP capture scope:** how much of the embedded harness's HTTP activity do we instrument in the MVP (all network tools vs. only `web_fetch`/search)? Instrumenting everything is more valuable but touches more vendored code.
3. **Non-browser, non-harness capture:** do we ever capture arbitrary host processes (system-level)? Default is no for MVP; noted for completeness.
4. **Body-capture default:** we default to **not storing** sensitive bodies. Confirm this privacy-first default is acceptable (vs. storing bodies locally by default with redaction only).
5. **Replay side-effect policy:** confirm the default "warn + confirm on non-idempotent methods, dry-run available" posture for re-issuing real requests.
6. **Retention defaults:** confirm the default retention cap (count + bytes) and pruning policy for stored captures.

---

## References

- `docs/PLAN-CONTEXT.md` — authoritative shared plan context (differentiator #4, architecture, testing gates).
- `plan/11-system-browser-integration.md` — CDP transport that HAR capture rides on.
- `plan/02-architecture.md` — server-centric runtime, SQLite read model.
- `plan/10-ui-pane-system.md` — right-bar pane hosting the waterfall.
- `plan/06-orchestration-engine.md` — turn-attached captures, agent context feed.
- `plan/15-testing-strategy.md` — TDD gates this doc's §8 conforms to.
- `plan/20-risks-and-open-questions.md` — consolidated open decisions.
