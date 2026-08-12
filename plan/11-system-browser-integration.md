# 11 — System-Browser Integration

**Status:** Draft (consistent with `docs/PLAN-CONTEXT.md`)
**Owner:** Multiplexer planning fan-out
**Scope:** Detect/import the user's installed browsers, launch/authorize them, drive them via CDP (Chrome DevTools Protocol), and surface them as a first-class pane, a Design Mode, and an agent browser-automation tool. **No bundled Chromium.**

**Locked decisions applied:** M1 (browser security tests), D27 (CI headless browser sourcing), D28 (HAR is CDP-only caveat).

---

## 1. The Core Idea

Most agentic coding tools that need a browser do one of two things: they **bundle** a browser (Orca embeds its own Chromium, ~100MB+ and a hidden data-collection surface) or they **shell out** to a headless driver with no real user profile. Multiplexer does neither.

**We detect the user's installed system browsers, import them, launch/authorize them, and drive them over CDP.** The browser the agent uses is the browser the user already has — with the user's real profile, cookies, extensions, and logged-in sessions. This is:

- **A core differentiator** (explicit user ask; see PLAN-CONTEXT differentiator #3 and the "no bundled Chromium" non-goal).
- **A size/startup win** — no 100MB+ download, no embedded runtime to keep patched.
- **A privacy/trust win** — the user's browser stays the user's browser; no hidden bundled data collection.
- **A fidelity win** — previews and agent automation run against the real engine the user ships to.

The architecture is a **browser manager** owned by the server runtime (the single native binary). It is not a client-side widget: browser processes, CDP connections, and automation live server-side, so the desktop, mobile, and web clients all observe and control the same browser session over the shared JSON-RPC-over-WebSocket contract (see `plan/04`).

```
┌─────────────────────────────── Server runtime (single native binary) ───────────────────────────────┐
│                                                                                                      │
│  ┌─────────────────────────────┐   ┌──────────────────────────────┐   ┌───────────────────────────┐  │
│  │  Browser Registry           │   │  Browser Manager             │   │  CDP Client (per target)  │  │
│  │  · detect installed         │──▶│  · launch / authorize        │──▶│  · navigate / DOM / eval  │  │
│  │  · import profiles          │   │  · remote-debugging-port     │   │  · screenshots / network  │  │
│  │  · platform backends        │   │  · lifecycle / cleanup       │   │  · WebSocket transport    │  │
│  └─────────────────────────────┘   └──────────────────────────────┘   └───────────────────────────┘  │
│                                            │                                                         │
│                                            ▼                                                         │
│  ┌───────────────────────────────────────────────────────────────────────────────────────────────┐  │
│  │  BrowserService (JSON-RPC methods) — drives panes, Design Mode, agent browser tools, HAR      │  │
│  └───────────────────────────────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
        │  authenticated JSON-RPC over WebSocket (plan/04)
        ▼
   Desktop (GPUI) · Mobile · Web  — thin shells: render CDP frames, forward clicks, show HAR
```

**Design principle:** the browser manager is a *library* (like the embedded harness), not a subprocess protocol. We vendor a CDP client crate in-process and talk to the browser over a local WebSocket. No ACP-style overhead between us and the browser.

---

## 2. Browser Detection

### 2.1 The Browser Registry abstraction

A `BrowserRegistry` maps a stable `BrowserId` to a concrete `BrowserSpec` describing *how* to find, launch, and (where applicable) authorize each browser. Detection is platform-specific; the registry is the single cross-platform interface.

```rust
pub enum BrowserKind { Chrome, Edge, Firefox, Safari, Arc, Brave, Chromium, Other(String) }

pub struct BrowserSpec {
    pub id: BrowserId,          // stable, e.g. "chrome", "msedge", "firefox"
    pub kind: BrowserKind,
    pub display_name: String,
    pub executable: PathBuf,    // resolved on detection
    pub version: Option<String>,
    pub protocol: BrowserProtocol, // Cdp | WebDriverBidi | None(unsupported)
    pub profile_dir: Option<PathBuf>, // user's real profile, if importable
    pub launch_args: Vec<String>,     // protocol-specific launch flags
}

pub trait BrowserDetector {
    fn detect(&self) -> Vec<BrowserSpec>;   // all installed browsers
    fn resolve(&self, id: &BrowserId) -> Option<BrowserSpec>;
}
```

### 2.2 Detection sources by platform

| Platform | Primary sources | Notes |
|---|---|---|
| **Windows** | Registry `HKLM\SOFTWARE\...\App Paths\chrome.exe` / `msedge.exe` / `firefox.exe`; `HKLM\SOFTWARE\Clients\StartMenuInternet`; per-user `HKCU` equivalents; `%ProgramFiles%` / `%ProgramFiles(x86)%` / `%LocalAppData%` well-known paths | Edge ships with Windows; Chrome/Brave/Arc install to `App Paths`. Use the `winreg` crate. |
| **macOS** | `/Applications/*.app` bundles; `~/Applications`; `mdfind`/Spotlight for non-standard installs | Read `Info.plist` for `CFBundleIdentifier` + version. Safari is a system app at `/Applications/Safari.app`. |
| **Linux** | `which`/`command -v` for `google-chrome`, `chromium`, `firefox`, `brave-browser`, `microsoft-edge`; `~/.local/share/applications/*.desktop`; snap/flatpak paths | Use `std::process::Command` + PATH lookup; parse `.desktop` `Exec=` lines. |

Detection runs **lazily and cached**: once at startup (fast, registry/PATH only) and refreshed on demand (e.g., after the user installs a browser, or a "rescan" button). Detection must be **fast** — it runs on the cold-start path and must not push us past the <300ms target. Registry/PATH reads are sub-millisecond; version probing (running `--version`) is deferred to a background task.

### 2.3 Version & capability probing

After detection, a background task probes each candidate:

- Run `--version` (Chromium-family) or `--version`/`about:config` equivalents to read the version.
- Determine the **protocol** the browser speaks: Chromium-family → CDP; Firefox → WebDriver BiDi (see §8); Safari → WebDriver (Safari's remote automation) or a lighter integration.
- Mark browsers we cannot drive (e.g., a browser with remote debugging disabled by policy) as **importable-but-not-drivable**, and surface that in the UI rather than failing silently.

---

## 3. Browser Import & Authorization

### 3.1 Import

"Import" means: **adopt the user's existing profile** so the launched browser has their cookies, extensions, and logins. We do **not** copy the profile (that would fork state and break session continuity); we *point* the launched browser at the real profile directory.

- **Chromium-family:** launch with `--user-data-dir=<profile_dir>` (or omit it to use the default profile) plus `--remote-debugging-port`. Using the real profile means the user's logged-in sessions (GitHub, Linear, cloud consoles) are available to previews and the agent immediately.
- **Firefox:** use the real profile via `-profile <dir>` and enable remote control (see §8).
- **Safari:** enable "Allow Remote Automation" (WebDriver) — a one-time user toggle.

**Import is explicit and consent-based.** We never silently hijack a profile. The first launch of a browser shows a clear dialog: *"Multiplexer will open Chrome with your existing profile and remote debugging enabled. This lets the agent drive it. Continue?"* The user can opt to use a **fresh throwaway profile** instead (isolated, no cookies) — important for agent automation that should not touch the user's real sessions.

### 3.2 Authorization

Two distinct authorization concerns:

1. **Browser-level automation consent** — the one-time permission to drive the browser (the dialog above). Stored per-browser in the OS keychain / local config, revocable.
2. **Web OAuth flows** — when the agent or a preview needs to sign in to a service, we drive the real browser through the OAuth flow (navigate, let the user complete it, capture the resulting cookies/tokens into the profile). Because we use the real profile, these sessions persist across launches. This is the same "authorize a provider" flow the provider-adapter layer uses (`plan/05`), but executed *through* the browser rather than a headless token exchange.

### 3.3 Controlled launch mode

Launching for CDP requires `--remote-debugging-port`. We launch with:

```
chrome.exe --remote-debugging-port=0 --user-data-dir=<profile> \
           --remote-allow-origins=http://127.0.0.1:<port> \
           --no-first-run --no-default-browser-check
```

- `--remote-debugging-port=0` asks Chromium to **pick a random free port** and print it to stderr — we parse it from the process output. This avoids port collisions and predictable ports (security, §9).
- `--remote-allow-origins` restricts which WebSocket origins may connect — we pin it to our own localhost origin.
- We capture the browser's stderr/stdout to learn the actual debugging port and to detect early crash/exit.

The `BrowserManager` owns the child process: it tracks PID, port, health, and guarantees cleanup (kill on app exit, on pane close, or on explicit "stop browser").

---

## 4. CDP Driving

### 4.1 Transport & client

We vendor a Rust CDP client (e.g., `chromiumoxide` or a thin hand-rolled client over `tokio-tungstenite`). The transport is a local WebSocket to `ws://127.0.0.1:<port>/devtools/page/<targetId>` (or the browser-level endpoint). We keep a **typed, minimal client** — we only need the domains we use, not the full surface:

| CDP domain | Purpose |
|---|---|
| `Page` | navigate, reload, lifecycle, captureScreenshot |
| `Runtime` | evaluate JS, inspect globals |
| `DOM` | query elements, get outer/inner HTML, attributes |
| `Network` | request/response capture → feeds HAR (`plan/12`) |
| `Input` | dispatch mouse/keyboard events (Design Mode clicks, agent clicks) |
| `Target` | enumerate/create/attach to tabs |
| `Emulation` | viewport size, device metrics (mobile preview) |
| `Accessibility` | a11y tree for agent automation (robust selectors) |

### 4.2 Core operations

```rust
pub struct CdpSession { /* ws connection, target id, event stream */ }

impl CdpSession {
    async fn navigate(&self, url: &Url) -> Result<()>;
    async fn evaluate(&self, js: &str) -> Result<RemoteValue>;
    async fn query_selector(&self, css: &str) -> Result<Option<NodeId>>;
    async fn outer_html(&self, node: NodeId) -> Result<String>;
    async fn click(&self, node: NodeId) -> Result<()>;
    async fn screenshot(&self, clip: Option<Rect>) -> Result<Vec<u8>>; // PNG
    async fn set_viewport(&self, w: u32, h: u32) -> Result<()>;
    async fn events(&self) -> impl Stream<Item = CdpEvent>; // page/network/dom events
}
```

These operations power **preview** (navigate + screenshot + DOM), **debugging** (inspect DOM, evaluate JS, network), and **agent automation** (navigate/click/fill/screenshot). The same session is shared across all consumers; the manager multiplexes.

### 4.3 Frame streaming to clients

For the preview pane, we do **not** ship raw video. We stream **screenshots + DOM deltas** over the JSON-RPC contract: the pane renders a screenshot, and on DOM/network events we send incremental updates (or a fresh screenshot on demand). This keeps the wire contract small and lets mobile clients render the same preview. (A full video path is a possible future enhancement; see Open Questions.)

---

## 5. Design Mode (baseline bar)

Design Mode is an Orca baseline feature we must match: **click any UI element in the browser → send its HTML, CSS, and a cropped screenshot into the agent.**

### 5.1 How it works over CDP

1. **Enter Design Mode:** the manager injects a small overlay script into the page that:
   - Highlights the element under the cursor (outline + label).
   - On click, captures the element's bounding rect and **cancels the default action** (so the click doesn't navigate).
2. **On click**, the client (or the injected script) reports the element:
   - `outerHTML` / `innerHTML` of the element.
   - Computed styles (`getComputedStyle`) for the relevant CSS.
   - The element's **bounding rect** (for cropping).
3. The manager takes a **cropped screenshot** of exactly that rect via `Page.captureScreenshot` with a clip.
4. The bundle — HTML + CSS + cropped PNG — is handed to the agent as a structured tool input (see §7).

```rust
pub struct DesignModeCapture {
    pub html: String,
    pub css: String,          // computed styles for the element
    pub screenshot_png: Vec<u8>, // cropped to the element rect
    pub selector: String,     // best-effort CSS selector for re-targeting
    pub url: String,
}
```

### 5.2 Routing to the agent

The capture is delivered to the embedded harness as a **browser tool call** (a structured message the agent can read and act on). The agent can then: restyle the element, fix a bug, generate a matching component, or ask follow-up questions — all with the actual rendered element in context. This is the "browser element → agent" loop that makes Design Mode a differentiator rather than a screenshot tool.

---

## 6. Preview Pane

The browser is a **right-bar pane** (per the pane-system spec, `plan/10`). It is one of several interchangeable right-bar panes (browser / HAR / files / diff / terminal / agent activity) and can **pop out to its own window**.

### 6.1 Pane capabilities

- **Address bar** + back/forward/reload.
- **Live preview** of the current page (screenshot + DOM deltas, §4.3).
- **Viewport presets** (desktop / tablet / mobile) via `Emulation.setDeviceMetricsOverride`.
- **Design Mode toggle** (§5).
- **Network indicator** — a badge linking to the HAR pane (`plan/12`).
- **Target/tab switcher** — enumerate open tabs via `Target.getTargets`.
- **Pop-out** to a standalone window (same underlying session, new view).

### 6.2 Interaction model

The pane is a *view* over the server-owned browser session. Clicks in the pane forward `Input` events (or, in Design Mode, trigger capture). Because the browser lives server-side, the **mobile app can open the same preview** — a differentiator over desktop-only competitors.

---

## 7. Agent Browser Automation

The embedded grok-build harness gets **browser tools** so the agent can drive the browser directly. These are registered as tools in the vendored `xai-grok-tools` set (following the crate's tool pattern), backed by the `BrowserManager`.

### 7.1 Tool surface

| Tool | Description |
|---|---|
| `browser_navigate(url)` | Navigate the active tab to a URL, wait for load. |
| `browser_click(selector)` | Click the first element matching a CSS selector. |
| `browser_fill(selector, value)` | Set an input's value (with proper input events). |
| `browser_evaluate(js)` | Run JS in the page and return the result. |
| `browser_screenshot()` | Capture the current viewport (or a region) as PNG. |
| `browser_get_html(selector?)` | Read outer HTML of the page or a subtree. |
| `browser_design_capture(selector)` | Programmatic Design Mode capture (HTML+CSS+cropped PNG). |
| `browser_wait_for(selector)` | Wait until a selector appears (polling DOM). |
| `browser_list_tabs()` / `browser_switch_tab(id)` | Multi-tab control. |

### 7.2 Isolation & consent

Agent automation is **opt-in per session** and, by default, runs in a **throwaway profile** (no user cookies) unless the user explicitly grants access to the real profile. The agent cannot silently drive the user's logged-in browser. A visible indicator shows "agent is driving the browser" and the user can **pause/stop** automation at any time (an interrupt that kills the in-flight CDP call and cancels the agent turn — wired into the provider-adapter `interrupt_turn`).

### 7.3 Selector robustness

For reliability we prefer the **Accessibility tree** (`Accessibility.getFullAXTree`) to build stable selectors, falling back to CSS selectors and XPath. This mirrors how real browser-automation frameworks target elements and reduces flakiness from class-name churn.

---

## 8. Cross-Browser Differences

CDP is **Chromium-specific**. Chrome, Edge, Brave, Arc, and Chromium all speak CDP. **Firefox and Safari do not.**

| Browser | Protocol | Multiplexer support |
|---|---|---|
| Chrome / Edge / Brave / Arc / Chromium | **CDP** | Full: preview, Design Mode, agent automation, HAR. |
| Firefox | **WebDriver BiDi** (modern) / Marionette (legacy) | **BiDi adapter** — a thin translation layer exposing the same `BrowserService` API; feature subset (no full HAR capture parity). |
| Safari | **WebDriver** (remote automation) | **Lighter integration**: navigate + screenshot + basic DOM via WebDriver; Design Mode/agent automation limited. |

**HAR is CDP-only (D28).** HAR capture is **Chromium-family only** (via CDP `Network` events). **Firefox (BiDi) and Safari (WebDriver) get reduced or no HAR** — the `Network`-domain feed that drives the HAR profiler/replayer (`plan/12`) is not available on those protocols. This is an honest caveat, not a universal win: the HAR pane is a full-fidelity feature on Chrome/Edge/Brave/Arc/Chromium and is degraded or unavailable on Firefox/Safari (the UI degrades gracefully, per §8.1).

### 8.1 The protocol-adapter boundary

We define an internal `BrowserDriver` trait (mirroring the provider-adapter pattern from `plan/05`):

```rust
pub trait BrowserDriver {
    async fn navigate(&self, url: &Url) -> Result<()>;
    async fn evaluate(&self, js: &str) -> Result<RemoteValue>;
    async fn query_selector(&self, css: &str) -> Result<Option<String>>;
    async fn click(&self, selector: &str) -> Result<()>;
    async fn screenshot(&self, clip: Option<Rect>) -> Result<Vec<u8>>;
    async fn get_html(&self, selector: Option<&str>) -> Result<String>;
    async fn events(&self) -> impl Stream<Item = BrowserEvent>;
}
```

- **CdpDriver** implements it over CDP (full fidelity).
- **BidiDriver** implements it over WebDriver BiDi for Firefox (navigate, DOM, evaluate, screenshot, click — the essentials).
- **WebDriverDriver** implements a reduced subset for Safari.

The `BrowserService` talks only to `BrowserDriver`, so panes, Design Mode, and agent tools are **protocol-agnostic**. Where a capability is missing (e.g., HAR network capture on Firefox), the driver reports `Unsupported` and the UI degrades gracefully (e.g., the HAR pane shows "not available for Firefox").

### 8.2 Default & fallback

- **Default:** prefer a Chromium-family browser (Chrome or Edge on Windows) for full fidelity — this is the recommended path and what we optimize for.
- **Fallback:** if the user's default is Firefox, we use the BiDi adapter; if Safari, the WebDriver adapter. We never force a browser the user doesn't have.

---

## 9. Security

Launching a browser with remote debugging is a real attack surface: a remote-debugging port lets anyone who can reach it drive the browser, read cookies, and exfiltrate data. We treat this as a **privileged, localhost-only, short-lived capability**.

### 9.1 Controls

| Control | Implementation |
|---|---|
| **Random port** | `--remote-debugging-port=0` → OS-assigned random port, parsed from stderr. No predictable ports. |
| **Localhost-only** | Bind to `127.0.0.1` only; never `0.0.0.0`. Verify the listening address before connecting. |
| **Origin allow-list** | `--remote-allow-origins=http://127.0.0.1:<port>` pins which WebSocket origins may connect. |
| **Token / auth** | Where the browser supports it, require a per-launch token in the CDP connection; reject connections without it. |
| **Short-lived** | The debugging session lives only while the browser is managed; killed on pane close, app exit, or explicit stop. |
| **No remote exposure** | The debugging port is **never** tunneled/relayed. Remote/mobile clients reach the browser only through the authenticated JSON-RPC contract, never the raw CDP port. |
| **Profile consent** | Real-profile use is explicit and revocable; agent automation defaults to a throwaway profile. |
| **Process hygiene** | Track the child PID; guarantee termination (including on crash/panic) to avoid orphaned debugging browsers. |

### 9.2 Threat model notes

- **Local attacker** (another process on the same machine): mitigated by random port + origin allow-list + token; the port is not discoverable without reading our process output.
- **Remote attacker**: cannot reach the port (localhost-only, never relayed). The only remote path is the authenticated WebSocket contract, which is covered by `plan/17` (auth, DPoP, ticket TTL).
- **Malicious page**: a page the agent navigates to cannot reach the debugging endpoint (it's not on the page's origin and is localhost-only); we also avoid evaluating untrusted JS with elevated privileges.

---

## 10. Testing

TDD at inception applies here as everywhere (see `plan/15`). The browser layer is highly testable because CDP is a well-defined protocol and headless Chromium is available in CI.

### 10.1 Unit tests

- **Browser detection** — table-driven tests per platform: given a fake registry/PATH/`Info.plist`, assert the resolved `BrowserSpec` (executable, kind, protocol). Property tests: detection is deterministic and idempotent; unknown entries are skipped.
- **Launch-arg construction** — given a `BrowserSpec`, assert the exact command line (port=0, allow-origins, profile, no-first-run).
- **Port parsing** — parse the debugging port from simulated stderr output (including malformed/absent lines).
- **Selector building** — a11y-tree → selector fallback logic.
- **Design Mode capture assembly** — given HTML/CSS/rect fixtures, assert the `DesignModeCapture` bundle.

### 10.2 Security tests (M1 — mutation-gated)

The browser security controls (§9.1) are **mutation-gated and tested** (unit + integration), per locked decision M1. These are mandatory: a mutant that opens the port, drops the token check, or leaks the debugging session **must be killed** by the mutation suite (cargo-mutants, `plan/15`).

- **Localhost-only bind** — assert the listening address is bound to `127.0.0.1` only, never `0.0.0.0`/`::`; a mutant that binds to a non-loopback address is killed.
- **Random port** — assert the port is OS-assigned (`--remote-debugging-port=0`) and not predictable; a mutant that hard-codes or defaults a fixed port is killed.
- **Token rejection** — assert a CDP connection **without** the per-launch token is rejected, and one with the correct token is accepted; a mutant that drops the token check is killed.
- **Origin allow-list** — assert a WebSocket connection from a disallowed origin is refused.
- **Short-lived session** — assert the debugging session is **killed on pane close** (and on app exit / explicit stop); a mutant that leaks the session past close is killed.
- **No remote exposure** — assert the debugging port is never tunneled/relayed; the only remote path is the authenticated JSON-RPC contract.
- **Process hygiene** — assert **no orphaned debugging browser after a panic**: force a panic mid-session and assert the child PID is terminated and the port released; a mutant that skips cleanup on the panic path is killed.

### 10.3 Integration tests (real browser, headless)

**CI browser sourcing (D27):** CI obtains its headless browser via a **pinned `playwright`/`chromium` download in CI only** — it is **not shipped to users**. This resolves the "no bundled Chromium" (product non-goal) vs "CI needs a browser" (test requirement) contradiction: the pinned download lives in the CI image/cache, never in the shipped binary or installer. Pin an exact Chromium revision (not "latest") so integration results are reproducible; the download is cached across runs.

- Launch a **headless Chromium** (`--headless=new --remote-debugging-port=0`) in CI; drive it through the `CdpDriver`:
  - navigate → assert URL/title; DOM query; evaluate; click; screenshot (assert valid PNG dimensions).
  - Design Mode: inject overlay, click an element, assert the captured HTML/CSS/cropped rect.
  - Network: assert request/response events are captured (feeds HAR).
- **Firefox BiDi** integration (where CI can install it): navigate + screenshot + evaluate through `BidiDriver`.
- **Lifecycle:** launch → connect → kill → assert no orphaned process and port released.
- **Agent tools:** run the embedded harness with the browser tools against a local test page; assert a full navigate→fill→click→screenshot sequence.

### 10.4 Component tests (GPUI)

- **Preview pane** — render with a mocked `BrowserDriver` (fixture screenshots + DOM); snapshot the layout; assert address-bar state, viewport preset switching, Design Mode toggle, and pop-out.
- **Browser picker** — render the detected-browser list; assert selection and the import/consent dialog.

### 10.5 E2E

- Drive the real app headless: open the browser pane, navigate to a local test server, enter Design Mode, click an element, and assert the agent receives the capture and responds. This is the full loop that beats competitors with no e2e.

### 10.6 CI gates

All of the above run in the standard gate order (fmt → clippy → unit+property → mutation → integration → component → e2e → coverage). Mutation tests target the detection/launch/port-parsing logic (pure, high-value) **and the security controls (§10.2)**. Integration tests that need a real browser are gated to run where a headless Chromium is available (CI image), and are skipped (not silently passed) otherwise.

---

## 11. Open Questions

These reference pending decisions from PLAN-CONTEXT / `plan/20`; we do not decide them unilaterally.

1. **MVP browser scope** — do we ship full CDP (Chromium-family) only in the MVP and defer Firefox BiDi / Safari adapters, or ship all three? (Recommendation: Chromium-family first, BiDi adapter as a stretch, Safari deferred — but this is a scope call.)
2. **Real-profile vs throwaway default** — should agent automation default to the real profile (fidelity) or a throwaway profile (isolation)? We recommend throwaway-by-default with explicit opt-in to the real profile, but the default posture is a product decision.
3. **Preview transport** — screenshots + DOM deltas now, or invest in a video/streaming path later? (Recommendation: screenshots + deltas for MVP; video is a possible enhancement.)
4. **Which browser is the Windows default** — Chrome vs Edge for the recommended full-fidelity path. (Edge ships with Windows; Chrome is the most common. Product decision.)
5. **Bundled fallback** — PLAN-CONTEXT is explicit that we **never** bundle Chromium. This doc assumes that holds even when no system browser is detected (we show a "no drivable browser found" state rather than bundling). Flagging for confirmation, since it affects the "no browser installed" UX.
6. **HAR parity across protocols** — full CDP network capture vs reduced capture on Firefox/Safari. Whether HAR must be identical across browsers is a `plan/12` coordination question.

---

*Next: `plan/12-har-profiler-replayer.md` — network capture via CDP, waterfalls, and session replay (shares the CDP session and the `BrowserService`).*
