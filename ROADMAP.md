# Pardus Browser Roadmap

## Project Status Summary

**Current Version:** 0.1.0-dev  
**Branch:** dev/interect  
**Last Updated:** April 2026

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

### JavaScript Engine
- [x] **V8 integration** — deno_core with custom DOM operations
- [x] **35+ Rust DOM ops** — Bridging JavaScript ↔ Rust DOM (querySelector, Element API, classList, dataset, style proxies)
- [x] **Thread-based timeout** — Prevents infinite loops via execution limits
- [x] **Inline script execution** — Automatic execution of inline `<script>` tags
- [x] **Script filtering** — Problematic scripts automatically filtered to prevent hangs

### Session & State Management
- [x] **Session persistence** — Cookies, localStorage, auth headers with size limits
- [x] **Custom headers** — Authentication and custom header support
- [x] **Cache management** — `clean` command for cookies/cache wiping
- [x] **Persistent REPL** — Interactive session with history and persistent browser state

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
- [x] **7 subcommands:** navigate, interact, serve, repl, tab, clean
- [x] **Rustyline integration** — History, line editing in REPL
- [x] **Shell-word parsing** — Proper argument handling in REPL
- [x] **Verbose logging** — Debug output via tracing

---

## 🔧 In Progress / Needs Polish

### CDP Integration
- [~] **CDP → Browser API migration** — Wiring CDP handlers through unified `Browser` type
  - Status: DomainContext holds App reference, needs Browser integration
  - Blocker: `!Send` constraint from scraper types in Page
  - Workaround: Storing raw HTML in TargetEntry instead of parsed Page

### JavaScript Interactions
- [x] **JS-level interaction** — Click/type/scroll/submit via deno_core DOM when JS enabled
  - All 4 interaction methods (`click`, `type_text`, `submit`, `scroll`) branch on `js_enabled`
  - Click dispatches click event, detects `window.location.href` navigation via Proxy setter
  - Type sets value attribute, dispatches `input` + `change` events
  - Submit dispatches `submit` event, respects `preventDefault`, falls back to HTTP if not prevented
  - Scroll dispatches `scroll` + `wheel` events for direction-aware handlers
  - Inline `on*` handlers (onclick, onchange, onsubmit, etc.) auto-registered before interaction
  - DOM mutations serialized back to `Page` after each interaction
  - Ephemeral per-interaction V8 runtime (no `!Send` constraint issues)
  - Unit tests for click dispatch, inline handlers, type+change events, scroll, navigation detection, submit preventDefault

---

## 📋 Planned (Near-term)

### Proxy Support
- [ ] **HTTP proxy** — Basic CONNECT tunneling
- [ ] **SOCKS5 proxy** — Full SOCKS5 client support
- [ ] **Proxy authentication** — Username/password auth
- [ ] **Per-request proxy** — `--proxy` flag on navigate

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
- [ ] **WebSocket support** — WS/WSS protocol handling
- [ ] **EventSource/SSE** — Server-sent events for live pages
- [ ] **Shadow DOM** — Piercing shadow boundaries for web components
- [ ] **IFrame handling** — Recursive frame parsing and interaction
- [ ] **PDF viewing** — PDF.js-style rendering or extraction

### Security & Authentication
- [ ] **Basic auth** — 401 response handling
- [ ] **OAuth flow** — OAuth 2.0 / OIDC automation helpers
- [ ] **Certificate pinning** — Custom CA/cert validation
- [ ] **CSP compliance** — Content Security Policy enforcement
- [ ] **Sandbox mode** — Restricted execution for untrusted content

### API & Integration
- [ ] **Python bindings** — PyO3 wrapper for Python agents
- [ ] **Node.js bindings** — N-API for JavaScript agents
- [ ] **Playwright adapter** — Drop-in replacement compatibility layer
- [ ] **Puppeteer adapter** — API compatibility for migration
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
| CDP DOM methods use raw HTML | Fixed | Parse on-demand from stored HTML |
| Complex SPA interactions | Partial | Use `--wait-ms` for async content |

---

## 📝 Changelog

### v0.2.0 — CDP & Cookie Optimizations

**HTTP Caching Layer (RFC 7234):**
- Added `CachePolicy` type parsing Cache-Control (max-age, no-store, no-cache, must-revalidate, immutable), ETag, Last-Modified, Expires, Age, Date headers
- Implemented heuristic freshness: 10% of Last-Modified age (min 1s, max 24h) per RFC 7234 §4.2.2
- Conditional requests: If-None-Match / If-Modified-Since sent on stale cache entries; 304 Not Modified handled with cache header update
- Cache-aware page loading: fresh hits return immediately, stale entries revalidated, misses cached with policy
- Cache-aware resource scheduler: `CachedFetcher` with `Send`-safe async design wraps all subresource fetches
- JS fetch API cache integration: supports `cache` parameter (default, no-store, force-cache, only-if-cached); adds `x-cache` response header
- Prefetcher stores results in shared `ResourceCache`, checks freshness before network requests
- Disk cache enhanced with HTTP semantics: `CacheMeta` metadata, no-store/fast-expiry priority eviction, `insert_with_meta()`
- Shared HTTP client factory (`http/client.rs`) eliminating 5 duplicate `reqwest::Client` builders
- `CacheManager` wired into `App` and `Browser` with `resource_cache()` accessors
- `NetworkRecord` gains `from_cache: Option<bool>` field for observability
- `chrono` added as workspace dependency for HTTP date parsing

**CDP Server Hardening:**
- Fixed async safety: replaced all `blocking_lock()` calls with `.lock().await`; session lock no longer held across `.await` during command routing
- Fixed protocol compliance: error responses now carry correct request IDs; `querySelectorAll` returns unique IDs per element; `getOuterHTML` returns proper errors
- Added connection limit (default 16, configurable via `with_max_connections()`) with graceful rejection logging
- Added graceful shutdown via `CdpServer::shutdown()` method
- Added per-command timeout (30s default) with timeout error responses
- Wired HTTP discovery endpoints (`/json/version`, `/json/list`) for non-WebSocket HTTP connections
- Added target lifecycle events: `Target.targetCreated`, `Target.targetDestroyed`, `Target.attachedToTarget`, `Target.detachedFromTarget`
- Implemented `Target.closeTarget` with proper cleanup and destruction event
- Added event replay buffer (64 events) for lagged connection recovery via `EventBus::replay_events()`
- Improved NodeMap with `invalidate_on_navigation()` for safe ID reset and `get_or_assign_indexed()` for unique per-element IDs

**CDP Network (Cookies):**
- Implemented `Network.getCookies` / `Network.getAllCookies` — extracts cookies from network log Set-Cookie headers with full attribute parsing (domain, path, httpOnly, secure, sameSite, size)
- Implemented `Network.setCookie`, `Network.deleteCookies`, `Network.clearBrowserCookies`
- Added `url` crate dependency to pardus-cdp for URL parsing in cookie operations

**Cookie System (SessionStore):**
- Fixed cookie parsing bug: removed incorrect `split(';')` on Set-Cookie header values
- Switched to RFC 6265 compliant domain matching via `cookie_store::get_request_values`
- Added atomic save (temp file + rename) for session persistence
- Added `delete_cookie(name, domain, path)` method to SessionStore
- Added `session_dir()` public accessor to SessionStore

**Performance:**
- Removed unnecessary HTML re-parsing in Pardus domain click handler (reuse `page_data` result)
- Removed dead HTML clone in `RuntimeDomain::evaluate_expression`
- Fixed tab loading to use browser's actual `BrowserConfig` instead of hardcoded default
- POST form submissions now recorded in NetworkLog

**Architecture:**
- `DomainContext.get_html/get_url/get_title` converted from sync `blocking_lock()` to async `.lock().await` (safe for multi-threaded tokio runtime)
- Added `HandleResult::with_request_id()` utility for threading request IDs through error responses
- Router now injects correct `request.id` into all error responses, even from domain handlers

### v0.1.0-dev (current)
- Initial release with full feature set
- Unified Browser API
- CDP server with 14 domains
- JavaScript execution via deno_core
- Configurable per-tab memory limits
- Persistent REPL and tab management

---

*For contributing to the roadmap, open an issue with the `roadmap` label.*
