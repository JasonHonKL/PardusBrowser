# pardus-browser

A headless browser built for AI agents. No pixels, no screenshots — just structured semantic state.

```
$ pardus-browser navigate https://example.com

00:00  pardus-browser navigate https://example.com
00:05  connected — parsing semantic state…
       document  [role: document]
       └── region  [role: region]
           ├── heading (h1)  "Example Domain"
           └── link  "Learn more"  → https://iana.org/domains/example
00:05  semantic tree ready — 0 landmarks, 1 links, 1 headings, 1 actions
00:05  agent-ready: structured state exposed · no pixel buffer · 0 screenshots
```

## Why

AI agents don't need screenshots. They need to know what's on a page, what they can interact with, and where they can go. `pardus-browser` fetches a URL, parses the HTML, and outputs a clean semantic tree — landmarks, headings, links, buttons, forms, and their actions — in milliseconds, not seconds.

No Chromium binary. No Docker. No GPU. Just HTTP + HTML parsing.

## Demo

[demo.mp4](demo/demo.mp4)

## Features

- **Semantic tree output** — ARIA roles, headings, landmarks, interactive elements
- **Page interaction** — Click links, submit forms, type into fields, wait for selectors, scroll
- **3 output formats** — Markdown (default), tree, JSON
- **Navigation graph** — Internal routes, external links, form descriptors with fields
- **Interactive-only mode** — Strip static content, show only actionable elements
- **Action annotations** — Every interactive element tagged with `navigate`, `click`, `fill`, `toggle`, or `select`
- **Network debugger** — DevTools-style request table with subresource discovery and parallel fetching
- **Session persistence** — Cookies, headers, localStorage across requests
- **CDP server** — Chrome DevTools Protocol WebSocket endpoint for automation
- **Fast** — HTTP GET + HTML parse, typically under 200ms
- **Zero dependencies on Chrome** — Pure Rust, no browser binary needed

## Install

From source (requires Rust nightly):

```bash
# Install nightly toolchain
rustup install nightly

# Clone and build
git clone https://github.com/user/pardus-browser.git
cd pardus-browser
cargo +nightly install --path crates/pardus-cli
```

## Usage

### Navigate to a URL

```bash
# Default: Markdown tree
pardus-browser navigate https://example.com

# Raw tree format
pardus-browser navigate https://example.com --format tree

# JSON with navigation graph
pardus-browser navigate https://example.com --format json --with-nav

# Only interactive elements
pardus-browser navigate https://example.com --interactive-only

# Custom headers
pardus-browser navigate https://api.example.com --header "Authorization: Bearer token"

# Enable JavaScript execution (EXPERIMENTAL - may hang on complex sites)
pardus-browser navigate https://example.com --js

# JS with custom wait time (ms) for async rendering
pardus-browser navigate https://example.com --js --wait-ms 5000

# Verbose logging
pardus-browser navigate https://example.com -v

# Capture and display network request table
pardus-browser navigate https://example.com --network-log

# Network log with JSON output
pardus-browser navigate https://example.com --format json --network-log
```

### Output formats

**Markdown (default)** — clean semantic tree with role annotations:

```
document  [role: document]
├── banner  [role: banner]
│   ├── link "Home"  → /
│   ├── link "Products"  → /products
│   └── button "Sign In"
├── main  [role: main]
│   ├── heading (h1) "Welcome to Example"
│   ├── region "Hero"
│   │   ├── text "The fastest way to build"
│   │   └── link "Get Started"  → /signup
│   └── form "Search"  [role: form]
│       ├── textbox "Search..."  [action: fill]
│       └── button "Go"  [action: click]
└── contentinfo  [role: contentinfo]
    ├── link "Privacy"  → /privacy
    └── link "Terms"  → /terms
```

**JSON** — structured data with full navigation graph:

```bash
pardus-browser navigate https://example.com --format json --with-nav
```

Returns:

```json
{
  "url": "https://example.com/",
  "title": "Example Domain",
  "semantic_tree": {
    "root": { "role": "document", "children": [...] },
    "stats": { "landmarks": 4, "links": 12, "headings": 3, "actions": 2 }
  },
  "navigation_graph": {
    "internal_links": [
      { "url": "/products", "label": "Products" },
      { "url": "/signup", "label": "Get Started" }
    ],
    "external_links": ["https://github.com/..."],
    "forms": [
      {
        "action": "/search",
        "method": "GET",
        "fields": [
          { "name": "q", "field_type": "text", "action": "fill" },
          { "name": "go", "field_type": "submit", "action": "click" }
        ]
      }
    ]
  },
  "network_log": {
    "total_requests": 4,
    "total_bytes": 6432,
    "total_time_ms": 312,
    "failed": 0,
    "requests": [
      {
        "id": 1, "method": "GET", "type": "document",
        "initiator": "navigation", "description": "document · navigation",
        "url": "https://example.com/", "status": 200,
        "content_type": "text/html", "body_size": 4304, "timing_ms": 142
      }
    ]
  }
}
```

### Network debugger

Capture and display all network requests in a DevTools-style table:

```bash
pardus-browser navigate https://example.com --network-log
```

```
00:00  pardus-browser navigate https://example.com
00:00  connected — parsing semantic state…
       # Network — 4 requests — 4.6 KB — 312ms total

         Method  Type        Resource                URL                                         Status  Size     Time
         —       ——————       —————————                 —————————————————                               ——————   ————————   ——————
         1       GET         document                 document · navigation                        200     4.2 KB   142ms
         2       GET         stylesheet               stylesheet · css2                            200     128 B    45ms
         3       GET         stylesheet               stylesheet · styles.css                      200     2.1 KB   89ms
         4       GET         script                   script · script.js                           200     0 B      23ms
00:00  semantic tree ready — 0 landmarks, 1 links, 1 headings, 1 actions
00:00  agent-ready: structured state exposed · no pixel buffer · 0 screenshots
```

The network debugger:
- Records the main page request (status, timing, size, headers)
- Discovers all subresources from HTML (`<link>`, `<script>`, `<img>`, `<video>`, `<audio>`, `<iframe>`, `<embed>`, `<object>`, inline CSS `url()`)
- Fetches all discovered subresources in parallel (concurrency limit of 6)
- Includes `network_log` in JSON output when using `--format json --network-log`

### CDP server

Start a Chrome DevTools Protocol WebSocket server for automation:

```bash
# Start on default host/port
pardus-browser serve

# Custom host and port
pardus-browser serve --host 0.0.0.0 --port 9222

# With inactivity timeout
pardus-browser serve --timeout 60
```

### Clean cache

```bash
# Wipe everything
pardus-browser clean

# Only cookies
pardus-browser clean --cookies-only

# Only cache
pardus-browser clean --cache-only

# Custom cache directory
pardus-browser clean --cache-dir /path/to/cache
```

### Tab management

```bash
# Open a new tab (fetches page and shows summary)
pardus-browser tab open https://example.com

# Open with JS execution
pardus-browser tab open https://example.com --js

# List all open tabs
pardus-browser tab list

# Show active tab info
pardus-browser tab info

# Navigate the active tab
pardus-browser tab navigate https://example.com/page2
```

**Note:** Tab state does not persist across CLI invocations. For persistent tab sessions, use the CDP server or the `Browser` type programmatically.

### Programmatic usage

The `Browser` type unifies navigation, interaction, and tab management into a single API:

```rust
use pardus_core::Browser;

let mut browser = Browser::new(BrowserConfig::default());

// Navigate (creates a tab automatically)
let tab = browser.navigate("https://example.com").await?;

// Interact — click updates the tab automatically if navigation occurs
let result = browser.click("a").await?;

// Chain interactions
browser.type_text("input[name='q']", "search query")?;
browser.submit("form", &state).await?;

// Tab management
let id = browser.create_tab("https://example.com/page2");
browser.switch_to(id).await?;
browser.go_back().await?;

// Access current state
let page = browser.current_page().unwrap();
let tree = page.semantic_tree();
```

### Page interaction

Interact with pages using the `interact` subcommand. Works at the HTTP level — clicks follow links and submit forms, no rendering engine required.

```bash
# Click a link — follows href, returns new page
pardus-browser interact https://example.com click 'a'

# Click a submit button — finds enclosing form, submits it
pardus-browser interact https://example.com click 'button[type="submit"]'

# Type into a field (returns the field state)
pardus-browser interact https://example.com type 'input[name="q"]' 'search query'

# Submit a form with field values
pardus-browser interact https://example.com submit 'form' --field 'q=rust+language'

# Wait for a CSS selector to appear (with timeout)
pardus-browser interact https://example.com wait '.result-list' --timeout-ms 5000

# Scroll — detects URL pagination (?page=, ?offset=, /page/N)
pardus-browser interact 'https://example.com/news?page=1' scroll --direction down

# JSON output for the result page
pardus-browser interact https://example.com click 'a' --format json

# Enable JS execution before interaction
pardus-browser interact https://example.com wait '.dynamic-content' --js --wait-ms 3000
```

**How interactions work:**

| Action | Mechanism |
|--------|-----------|
| `click` (link) | Resolves href, HTTP GET, returns new page |
| `click` (button) | Finds enclosing `<form>`, collects all fields (including hidden CSRF tokens), submits via HTTP |
| `type` | Returns field selector + value (accumulate in `FormState` before submit) |
| `submit` | Collects all form fields from HTML, merges with `--field` values, HTTP POST/GET |
| `wait` | Checks current HTML for selector match; polls by re-fetching if not found |
| `scroll` | Detects pagination patterns in URL (`?page=`, `?offset=`, `?start=`, `/page/N`) |

## Architecture

```
pardus-browser
├── crates/pardus-core    Core library — Browser type, HTML parsing, semantic tree, navigation graph, interaction, tabs
├── crates/pardus-debug   Network debugger — request recording, subresource discovery, table output
├── crates/pardus-cdp     CDP WebSocket server — Chrome DevTools Protocol for automation
└── crates/pardus-cli     CLI binary
```

**pardus-core** — The engine. The `Browser` type is the main entry point — it owns the HTTP client, tab state, and provides navigation + interaction as a single cohesive API. Internally, it fetches pages via `reqwest`, parses HTML with `scraper`, and builds semantic trees mapping ARIA roles and interactive states. Provides page interaction (click, type, submit, wait, scroll) with automatic tab updates on navigation. Includes tab management, history navigation, session persistence (cookies, headers, localStorage), and optional JavaScript execution via deno_core. Outputs Markdown, tree, or JSON.

**pardus-debug** — Network debugging. Records all HTTP requests to a shared `NetworkLog`, discovers subresources from parsed HTML (stylesheets, scripts, images, fonts, media), fetches them in parallel, and formats DevTools-style request tables.

**pardus-cdp** — Chrome DevTools Protocol server. Exposes a WebSocket endpoint for browser automation. Includes domain handlers (DOM, Runtime, Network, Page), event bus, target management, message routing, and session lifecycle.

**pardus-cli** — The `pardus-browser` command-line tool. Provides `navigate`, `interact`, `tab`, `serve`, and `clean` subcommands. All commands use the unified `Browser` type.

## Semantic roles detected

| Element | Role | Action |
|---------|------|--------|
| `<a href>` | `link` | `navigate` |
| `<button>` | `button` | `click` |
| `<input type=text>` | `textbox` | `fill` |
| `<input type=submit>` | `button` | `click` |
| `<input type=checkbox>` | `checkbox` | `toggle` |
| `<select>` | `combobox` | `select` |
| `<textarea>` | `textbox` | `fill` |
| `<img>` | `img` | — |
| `<h1>`–`<h6>` | `heading (hN)` | — |
| `<nav>` | `navigation` | — |
| `<main>` | `main` | — |
| `<header>` | `banner` | — |
| `<footer>` | `contentinfo` | — |
| `<form>` | `form` | — |
| `<article>` | `article` | — |
| `<ul>/<ol>` | `list` | — |
| `<table>` | `table` | — |
| `[role=...]` | custom | varies |
| `[tabindex]` | varies | varies |

## Roadmap

### Done

- [x] **Semantic tree output** — ARIA roles, headings, landmarks, interactive elements
- [x] **Navigation graph** — Internal routes, external links, form descriptors
- [x] **Multiple output formats** — Markdown, tree, JSON
- [x] **Interactive-only mode** — Strip static content, show only actionable elements
- [x] **Action annotations** — navigate, click, fill, toggle, select
- [x] **Page interaction** — Click links, submit forms, type fields, wait for selectors, scroll
- [x] **Custom headers** — Pass authentication and custom headers
- [x] **Cache management** — Clean cookies and cache
- [x] **Network debugger** — Request table with subresource discovery and parallel fetching
- [x] **Session persistence** — Cookies, localStorage, auth headers with size limits
- [x] **CDP WebSocket server** — Domain handlers, event bus, target management, message routing
- [x] **Unified Browser API** — Single `Browser` type combining navigation, interaction, and tab management

### Experimental

- [~] **JavaScript execution** — V8 via deno_core with custom DOM ops
  - Infrastructure complete (deno_core, 35+ Rust ops, bootstrap.js)
  - **Currently disabled** — hangs on JS-heavy sites
  - Works on simple inline scripts
  - Needs: smarter script filtering, async callback handling

- [~] **Full DOM API** — querySelector, event dispatching, complete Element API
  - querySelector / querySelectorAll with CSS selectors (fast-path + scraper fallback)
  - Element API: cloneNode, insertBefore, replaceChild, contains, etc.
  - classList, dataset, style proxies
  - 35+ Rust ops bridging JS <-> Rust DOM
  - Comment nodes, outerHTML, getElementsByTagName, getElementsByClassName
  - **Not usable yet** — blocked by JS execution being disabled

### Planned

- [ ] **CDP → Browser migration** — Wire CDP domain handlers through unified `Browser` (blocked by `!Send` constraint)
- [ ] **JS-level interaction** — Click/type/scroll via deno_core DOM when JS is enabled
- [ ] **Proxy support** — HTTP/SOCKS proxies
- [ ] **Screenshots** — Optional, for when pixels actually matter

## Known Issues

| Issue | Status | Workaround |
|-------|--------|------------|
| JS execution hangs on complex sites | Open | Don't use `--js` flag |
| External scripts not executed | By design | Only inline scripts supported |
| setTimeout/setInterval no-ops | By design | Prevents infinite loops |

## Requirements

- **Rust nightly** required (deno_core uses `const_type_id` feature)
- Install: `rustup install nightly`

## License

MIT License
