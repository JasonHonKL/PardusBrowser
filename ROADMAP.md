# Pardus Browser Roadmap

## Project Status Summary

**Current Version:** 0.4.0-dev  
**Branch:** dev/interect  
**Last Updated:** April 2, 2026

---

## ✅ Completed (Stable)

### Core Engine
- [x] **Semantic tree output** — ARIA roles, headings, landmarks, interactive elements
- [x] **Navigation graph** — Internal routes, external links, form descriptors with field metadata
- [x] **Multiple output formats** — Markdown (default), tree, JSON
- [x] **Interactive-only mode** — Strip static content, show only actionable elements
- [x] **Action annotations** — navigate, click, fill, toggle, select tags on elements
- [x] **Fast HTTP parsing** — GET + HTML parse typically under 200ms
- [x] **Zero Chrome dependencies** — Pure Rust, no browser binary needed

### Page Interaction
- [x] **Click actions** — Link navigation, button clicks with form auto-detection
- [x] **Form handling** — Text input, checkboxes, radio buttons, selects
- [x] **Form submission** — Automatic CSRF token collection, POST/GET submission
- [x] **Wait for selectors** — Polled re-fetch until element appears
- [x] **Scroll pagination** — Detects `?page=`, `?offset=`, `/page/N` patterns
- [x] **JS-level interaction** — Click/type/scroll/submit via deno_core DOM when JS enabled
  - All 4 interaction methods branch on `js_enabled`
  - Inline `on*` handlers auto-registered before interaction
  - DOM mutations serialized back to `Page` after each interaction

### JavaScript Engine
- [x] **V8 integration** — deno_core with custom DOM operations
- [x] **35+ Rust DOM ops** — Bridging JavaScript ↔ Rust DOM (querySelector, Element API, classList, dataset, style proxies)
- [x] **Thread-based timeout** — Prevents infinite loops via execution limits
- [x] **Inline script execution** — Automatic execution of inline `<script>` tags
- [x] **Script filtering** — Problematic scripts automatically filtered to prevent hangs

### Security
- [x] **SSRF protection** — `UrlPolicy` validates all fetched URLs
  - Blocks private IPs (10.x, 172.16-31.x, 192.168.x), loopback, link-local (169.254.x), multicast
  - Blocks cloud metadata endpoints (AWS 169.254.169.254, GCP, Azure, Alibaba)
  - Blocks non-HTTP(S) schemes (file://, ftp://, data://, javascript:)
  - Configurable modes: default, permissive, allowlist
  - Wired into BrowserConfig and JS fetch API
  - IPv4 and IPv6 validation with bracket notation support

### Session & State Management
- [x] **Session persistence** — Cookies, localStorage, auth headers with size limits
- [x] **Custom headers** — Authentication and custom header support
- [x] **Cache management** — `clean` command for cookies/cache wiping
- [x] **Persistent REPL** — Interactive session with history and persistent browser state

### Proxy Support
- [x] **HTTP/HTTPS proxy** — Full support via reqwest `Proxy::http()`/`Proxy::https()`
- [x] **SOCKS5 proxy** — Full SOCKS5 client support via reqwest `socks` feature
- [x] **Proxy authentication** — Username/password auth via URL (`http://user:pass@host:port`)
- [x] **Per-request proxy** — `--proxy`, `--proxy-http`, `--proxy-https` flags on all commands
- [x] **Environment variable support** — Respects `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`
- [x] **No-proxy exclusions** — Comma-separated host bypass list support

### Tab Management
- [x] **Multi-tab support** — Multiple tabs with independent state
- [x] **History navigation** — Back/forward per tab
- [x] **Tab activation** — Switch between active tabs
- [x] **Tab info/list** — View open tabs and current state

### Network & Debugging
- [x] **Network debugger** — DevTools-style request table with subresource discovery
- [x] **Parallel fetching** — Stylesheets, scripts, images fetched concurrently
- [x] **Request recording** — Full HTTP request/response logging
- [x] **Subresource discovery** — Automatic parsing of CSS/JS/image references

### Server-Sent Events
- [x] **SSE parser** — Streaming parser per HTML Living Standard (BOM stripping, multi-line data, chunked input, 48 tests)
- [x] **SSE client** — Async client on dedicated 8-thread tokio background runtime with auto-reconnect
- [x] **SSE manager** — Thread-safe connection manager (DashMap) with open/close/poll/drain
- [x] **JS EventSource API** — Web-standard `EventSource` class with `MessageEvent`, event callbacks
- [x] **4 deno_core ops** — `op_sse_open`, `op_sse_close`, `op_sse_ready_state`, `op_sse_url`
- [x] **Auto-reconnect** — Exponential backoff (max 5 attempts, 30s cap), honors `retry:` field
- [x] **Last-Event-ID** — Sent on reconnect for gapless event streams
- [x] **SSRF protection** — UrlPolicy validates SSE URLs (blocks private IPs, loopback, metadata)
- [x] **88 unit tests** — Parser (48), client with local TCP servers (17), manager (20), url_policy (3)

### CDP (Chrome DevTools Protocol)
- [x] **WebSocket server** — CDP endpoint on ws://127.0.0.1:9222
- [x] **14 domain handlers:**
  - [x] Browser — Version info, permission management
  - [x] Target — Tab creation, attachment, destruction
  - [x] Page — Navigation, reload, screenshot hooks
  - [x] DOM — Node tree, query selectors, node highlighting
  - [x] Network — Request/response interception, events
  - [x] Runtime — Script execution, console API
  - [x] Input — Mouse/keyboard event dispatch
  - [x] CSS — Stylesheet inspection, computed styles
  - [x] Console — Console message capture
  - [x] Log — Log entry events
  - [x] Security — Certificate info, security state
  - [x] Emulation — Viewport, user agent, touch emulation
  - [x] Performance — Metrics collection
  - [x] Pardus — Custom domain for Pardus-specific features
- [x] **Event bus** — Real-time events to CDP clients
- [x] **Node mapping** — backendNodeId ↔ selector translation

### CLI & UX
- [x] **8 subcommands:** navigate, interact, serve, repl, tab, map, clean
- [x] **Rustyline integration** — History, line editing in REPL
- [x] **Shell-word parsing** — Proper argument handling in REPL
- [x] **Verbose logging** — Debug output via tracing

### WebSocket
- [x] **WebSocket connection** — `WebSocketConnection` wrapping tokio-tungstenite for WS/WSS
- [x] **WebSocket manager** — Connection pooling, per-origin limits, CDP events

### Knowledge Graph
- [x] **BFS site crawler** — Configurable depth, max-pages, delay
- [x] **State fingerprinting** — blake3-based semantic + network fingerprints
- [x] **Transition discovery** — Links, hash navigation, pagination detection
- [x] **JSON graph output** — State graph with verified transitions

---

## 🔧 In Progress / Needs Polish

### CDP Integration
- [x] **CDP → Browser API migration** — Wiring CDP handlers through unified `Browser` type
  - Status: DomainContext now provides `create_browser()` method to create temporary Browser instances from App config
  - Resolution: The `!Send` constraint from scraper types (Cell in Html) prevents storing Browser directly, so we create Browser on-demand in handlers
  - Result: Navigation methods (`navigate()`, `reload()`) now use the unified Browser API while keeping DomainContext Send+Sync
  - Note: JS evaluation via Runtime domain stubbed - full integration requires exposing JS runtime through Browser

---

## 📋 Planned (Near-term)

### Screenshots (Optional)
- [ ] **HTML→PNG rendering** — For when pixels actually matter
- [ ] **Element screenshots** — Capture specific element bounds
- [ ] **Viewport clipping** — Configurable resolution
- [ ] **CDP screenshot API** — Page.captureScreenshot compliance

---

## 🚀 Future Roadmap (2026+)

### AI Agent Features
- [ ] **LLM-friendly output** — Optimized token formats for common LLM context windows
- [ ] **Action planning** — Suggested next actions based on page state
- [ ] **Auto-form filling** — AI-guided form completion with validation
- [ ] **Smart wait conditions** — Wait for "content loaded" not just selectors
- [ ] **Session recording** — Replayable action sequences

### Performance & Scale
- [x] **Connection pooling** — Reuse TCP connections across requests
- [x] **HTTP/2 push** — Client-side push simulation (early `<head>` scanning + speculative fetch) and optional h2 PUSH_PROMISE reception
- [x] **Caching layer** — HTTP cache compliance (ETag, Last-Modified)
  - RFC 7234 CachePolicy with Cache-Control, ETag, Last-Modified, Expires, Age, Date header parsing
  - Freshness lifetime: max-age, Expires, heuristic (10% LM-factor per RFC 7234 §4.2.2), immutable
  - Conditional requests: If-None-Match (ETag), If-Modified-Since (Last-Modified), 304 Not Modified handling
  - Cache-aware page loading, resource scheduler, JS fetch API (with cache modes: default/no-store/force-cache/only-if-cached), prefetcher
  - Disk cache with HTTP semantics (no-store eviction, stale entry priority eviction)
  - Shared HTTP client factory eliminating duplicate reqwest::Client construction
  - `from_cache` flag on NetworkRecord for observability
- [ ] **Request deduplication** — Avoid parallel fetches of same resource
- [x] **Memory limits** — Configurable per-tab memory caps

### Web Standards
- [x] **WebSocket support** — Full WS/WSS protocol handling with `WebSocketManager` and `WebSocketConnection`
  - `WebSocketConnection`: `connect()`, `send_text()`, `send_binary()`, `recv()`, `close()` methods
  - `WebSocketManager`: Connection pooling, per-origin limits, lifecycle management
  - SSRF protection: Blocks private IPs, loopback, link-local, cloud metadata endpoints
  - CDP events: `Network.webSocketCreated`, `Network.webSocketClosed`, `Network.webSocketFrameSent`, `Network.webSocketFrameReceived`
  - IPv6 support: Blocks loopback (`::1`), link-local (`fe80::/10`), unique local (`fc00::/7`)
  - 30 unit tests covering security, lifecycle, event bus, and URL validation
- [x] **EventSource/SSE** — Full SSE implementation: parser (48 tests), async client with auto-reconnect, thread-safe manager, JS EventSource Web API, 4 deno_core ops, SSRF protection
- [ ] **Shadow DOM** — Piercing shadow boundaries for web components
- [ ] **IFrame handling** — Recursive frame parsing and interaction
- [ ] **PDF viewing** — PDF.js-style rendering or extraction

### Security & Authentication
- [x] **SSRF protection** — URL policy blocking private IPs, metadata endpoints, non-HTTP schemes
- [ ] **Basic auth** — 401 response handling
- [ ] **OAuth flow** — OAuth 2.0 / OIDC automation helpers
- [x] **Certificate pinning** — Custom CA/cert validation (SPKI hash + CA cert, 738 lines in `tls/pinning.rs`)
- [ ] **CSP compliance** — Content Security Policy enforcement
- [x] **Sandbox mode** — Restricted execution for untrusted content (off/strict/moderate/minimal, 289 lines in `sandbox/mod.rs`)

### API & Integration
- [ ] **Python bindings** — PyO3 wrapper for Python agents
- [ ] **Node.js bindings** — N-API for JavaScript agents
- [x] **Playwright adapter** — Drop-in compatibility layer (Python + Node.js)
  - `adapters/python/pardus-playwright/` — sync_api and async_api context managers
  - `adapters/node/pardus-playwright/` — npm package @pardus/playwright
  - Launches pardus-browser serve, connects via connectOverCDP()
  - Adds .pardus extension namespace for semantic tree, navigation graph, element IDs
- [x] **Puppeteer adapter** — API compatibility for migration
  - `adapters/node/pardus-puppeteer/` — npm package @pardus/puppeteer
  - Launches pardus-browser serve, connects via puppeteer.connect()
- [x] **CDP enhancements for adapter support**
  - Runtime.evaluate wired to V8 with proper RemoteObject serialization
  - Runtime.callFunctionOn with argument passing
  - Network domain emits requestWillBeSent/responseReceived/loadingFinished events
  - Input.dispatchMouseEvent/dispatchKeyEvent wired to pardus-core interactions
  - DOM.getOuterHTML returns real HTML via scraper
  - Emulation.setDeviceMetricsOverride/setUserAgentOverride update BrowserConfig
  - Page.captureScreenshot/printToPDF return proper error messages
  - Page.addScriptToEvaluateOnNewDocument, getResourceTree, createIsolatedWorld
  - CSS.getInlineStylesForNode parses real inline style attributes
  - Full method coverage expansion across all 14 domains
- [ ] **Docker image** — Official container with health checks

### Developer Experience
- [ ] **HAR export** — HTTP Archive format for request logs
- [ ] **Coverage reporting** — CSS/JS usage statistics
- [ ] **Accessibility audit** — Automated a11y checks
- [ ] **Visual regression** — Diff screenshots for testing
- [ ] **REPL improvements** — Auto-completion, syntax highlighting

---

## 📊 Metrics & Targets

| Metric | Current | Target |
|--------|---------|--------|
| Cold start | ~50ms | ~30ms |
| Page parse (typical) | ~150ms | ~100ms |
| JS execution timeout | 3s | Configurable |
| CDP domains | 14/20 | 20/20 |
| Test coverage | ~60% | 85% |
| Binary size | ~15MB | <10MB |

---

## 🐛 Known Issues

| Issue | Status | Workaround |
|-------|--------|------------|
| External scripts not executed | By design | Only inline scripts supported |
| setTimeout/setInterval no-ops | By design | Prevents infinite loops |
| Complex SPA interactions | Partial | Use `--wait-ms` for async content |

---

*For contributing to the roadmap, open an issue with the `roadmap` label.*
