# Changelog

### v0.4.0 — WebSocket Full Implementation

**WebSocket Support:**
- Added `WebSocketConnection` module (`crates/open-core/src/websocket/connection.rs`)
  - Async connect with configurable timeout
  - `send_text()`, `send_binary()` for outgoing messages
  - `recv()` returns `(WebSocketFrame, Vec<u8>)` for incoming messages
  - Automatic Ping/Pong handling
  - Connection statistics tracking (frames sent/received, bytes)
  - Unique connection ID generation via URL hashing

- Added `WebSocketManager` module (`crates/open-core/src/websocket/manager.rs`)
  - Connection pooling with per-origin limits (`max_per_origin`)
  - Configurable security policy (`block_private_ips`, `block_loopback`)
  - CDP event bus integration for real-time notifications
  - Event emission: `Network.webSocketCreated`, `Network.webSocketClosed`, `Network.webSocketFrameSent`, `Network.webSocketFrameReceived`

- Added `WebSocketConfig` for connection settings
  - `max_per_origin`: Maximum concurrent connections per origin (default: 6)
  - `connect_timeout_secs`: Connection timeout (default: 30s)
  - `max_message_size`: Maximum message size (default: 10MB)
  - `block_private_ips`: Block private IP addresses (default: true)
  - `block_loopback`: Block loopback addresses (default: true)

- SSRF Protection for WebSocket
  - Blocks private IPv4: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
  - Blocks IPv6 unique local: fc00::/7
  - Blocks IPv6 link-local: fe80::/10
  - Blocks IPv6 loopback: ::1
  - Blocks cloud metadata: metadata.google.internal, 169.254.169.254, 100.100.100.200
  - Blocks localhost hostname

- Added `ResourceType::WebSocket` to `open-debug` crate

- Dependencies added:
  - `tokio-tungstenite = "0.26"` — Async WebSocket client
  - `tungstenite = "0.26"` — WebSocket protocol

- Test coverage: 30 unit tests
  - Config tests (2)
  - Manager tests (15)
  - IPv6 tests (3)
  - Event bus tests (2)
  - URL validation tests (5)
  - Connection limit tests (2)
  - Permissive policy tests (1)

### v0.3.0 — SSE, WebSocket, SSRF Protection

**Server-Sent Events (EventSource):**
- Added `SseParser` — streaming SSE parser per HTML Living Standard (BOM stripping, multi-line data, chunked input, 48 tests)
- Added `SseClient` — async SSE connection with reqwest, runs on dedicated 8-thread tokio background runtime
- Added `SseManager` — thread-safe connection manager (DashMap) with `open`/`close`/`drain_events_js`
- Auto-reconnect with exponential backoff (max 5 attempts, 30s cap), honors server `retry:` field
- `Last-Event-ID` header sent on reconnect for gapless streams
- SSRF protection via `UrlPolicy` — blocks private IPs, loopback, metadata endpoints, file:// scheme
- 4 deno_core ops: `op_sse_open`, `op_sse_close`, `op_sse_ready_state`, `op_sse_url`
- `drain_events_js()` generates JS dispatch code consumed by runtime event loop
- `EventSource` Web API in bootstrap.js (CONNECTING/OPEN/CLOSED states, onopen/onmessage/onerror, addEventListener)
- `MessageEvent` class, `__sse_dispatch` global for Rust→JS event dispatch
- SSE event drain phase in `js/runtime.rs` after each event loop poll
- `spawn_sse_connection_on()` for testability (decouples runtime from connection spawning)
- 88 unit tests: 48 parser, 17 client (with local TCP test servers), 20 manager, 3 url_policy

**WebSocket (Full Implementation):**
- Added `WebSocketConnection` — wraps tokio-tungstenite for WS/WSS with TLS support
- `connect()`, `send_text()`, `send_binary()`, `recv()`, `recv_text()`, `close()` API
- Automatic Ping/Pong handling, frame-level statistics (`WebSocketStats`)
- UrlPolicy validation on connect, connection ID via blake3 hash
- Added `WebSocketManager` — connection pooling with per-origin limits
- CDP event emission: `Network.webSocketCreated`, `Network.webSocketClosed`, `Network.webSocketFrameSent`, `Network.webSocketFrameReceived`
- SSRF protection: Blocks private IPs (RFC 1918), loopback, link-local (169.254.x.x, fe80::/10), cloud metadata endpoints
- IPv6 support: Blocks loopback (::1), link-local (fe80::/10), unique local (fc00::/7)
- 30 unit tests covering security, lifecycle, event bus, URL validation

**SSRF Protection:**
- Added `UrlPolicy` — validates all URLs before fetching
- Blocks private IPs (10.x, 172.16-31.x, 192.168.x), loopback, link-local (169.254.x), multicast
- Blocks cloud metadata endpoints (AWS 169.254.169.254, GCP, Azure, Alibaba 100.100.100.200)
- Blocks non-HTTP(S) schemes (file://, ftp://, data://, javascript:)
- Three modes: `default()` (strict), `permissive()` (localhost allowed), `allowlist()`
- Wired into `BrowserConfig` and JS `fetch()` API (15 unit tests)

**Interceptor Pipeline:**
- Fixed `run_before_request` to compose Redirect/Mock across all interceptors (previously returned on first match)

**Preload Scanner Rewrite:**
- Replaced single `RegexSet` with 6 per-tag-type classifiers (`classify_link`, `classify_script`)
- Proper CORS attribute extraction (`crossorigin`, `use-credentials`, `anonymous`)
- `modulepreload` recognition, better priority classification (preload/modulepreload → High, async/defer → Low)

**Streaming Parser Simplification:**
- Removed `lol_html` dependency; now uses regex-based `PreloadScanner` + `LazyDom`

**Cache:**
- `CachePolicy::is_fresh()` returns `true` for `immutable` resources (bypasses freshness lifetime calculation)

**Navigation Graph:**
- Added `Clone` + `Deserialize` derives to `NavigationGraph`, `Route`, `FormDescriptor`, `FieldDescriptor`

**Parser:**
- `LazyDom`: added `from_bytes()` (infallible), `Default` impl, fixed `select()` lifetime, removed `Send + Sync` bound
- Early scanner: relaxed image prefetch to `priority <= High` (was Critical only)

**Resource Module:**
- `FetchResult::error()` simplified to take `String`
- `CachedFetcher` wrapped in `Arc`, `PriorityQueue::peek()` returns references

**JS:**
- Fixed DOM tree bug where `child_id` return value was unused in `js/dom.rs`
- Conditional JS compilation: `#[cfg(feature = "js")]` guards on interaction methods in `browser.rs`

**Prefetcher:**
- Now takes shared `client: reqwest::Client` + `cache: Arc<ResourceCache>` (no duplicate client creation)

**Toolchain:**
- Added `rust-toolchain.toml` pinning to nightly

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
- Added `url` crate dependency to open-cdp for URL parsing in cookie operations

**Cookie System (SessionStore):**
- Fixed cookie parsing bug: removed incorrect `split(';')` on Set-Cookie header values
- Switched to RFC 6265 compliant domain matching via `cookie_store::get_request_values`
- Added atomic save (temp file + rename) for session persistence
- Added `delete_cookie(name, domain, path)` method to SessionStore
- Added `session_dir()` public accessor to SessionStore

**Performance:**
- Removed unnecessary HTML re-parsing in Open domain click handler (reuse `page_data` result)
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
