---
name: openbrowser
description: "Use when: open-browser, openbrowser, headless browser testing, semantic tree, navigate and test, headless browse without playwright, CDP testing, site mapping, knowledge graph crawl. Do not trigger for: screenshot testing, visual regression testing, cross-browser testing, Playwright specific features, CSS rendering verification, JavaScript-heavy SPA testing requiring real browser engine."
author: Chris Engelhard <chris@chrisengelhard.nl>
triggers:
  - open-browser
  - openbrowser
  - headless browser testing
  - semantic tree
  - navigate and test
  - headless browse without playwright
  - CDP testing
  - site mapping
  - knowledge graph crawl
  - test page content
  - check page structure
  - form testing without browser
  - scrape semantic state
context:
  domains:
    - testing
    - browser-automation
    - web-scraping
  concerns:
    - headless-testing
    - semantic-analysis
    - form-interaction
    - site-mapping
  technologies:
    - rust
    - deno-core
    - v8
  patterns:
    - http-only-browsing
    - semantic-tree-parsing
    - element-id-interaction
  priority: 8
  tokenBudget: 4000

negativeTriggers:
  - screenshot testing
  - visual regression testing
  - cross-browser testing
  - Playwright specific features
  - CSS rendering verification
  - JavaScript-heavy SPA testing requiring real browser engine

allowed-tools: Bash Read Write Edit

---

# open-browser

A headless browser built for AI agents. No pixels, no screenshots — just structured semantic state. HTTP + HTML parsing only, no Chromium binary, no Docker, no GPU.

## Overview

open-browser fetches URLs, parses HTML, and outputs a clean semantic tree — landmarks, headings, links, buttons, forms, and their actions — in milliseconds. Interactive elements get unique IDs (`[#1]`, `[#2]`) that AI agents use to reference them without CSS selectors.

**Core principle:** AI agents don't need screenshots. They need to know what's on a page, what they can interact with, and where they can go.

## When to Use

Use open-browser for:
- Checking page has specific heading/text
- Verifying link/button exists with correct label
- Testing form submission (login, search, signup)
- Verifying navigation links and routes
- Mapping site structure for test coverage
- Extracting semantic content from pages
- Fast smoke tests (under 200ms per page)
- Checking network requests and subresources
- PDF content extraction
- AI agent workflows that need structured state, not pixels
- Running in CI without browser dependencies

**When open-browser is the better choice:**
- Speed matters (HTTP GET + HTML parse vs full browser launch)
- You only need semantic/content assertions
- Running in CI without browser dependencies
- Mapping site structure for test coverage planning
- AI agent workflows that need structured state, not pixels

## When NOT to Use

Do not use open-browser for:
- Screenshot comparison / visual regression testing
- Testing CSS layouts and rendering
- Testing JavaScript-heavy SPA interactions
- Cross-browser testing (Chrome/Firefox/Safari)
- Testing real WebSocket connections
- Verifying pixel-perfect design
- Drag-and-drop interactions
- File upload handling
- Hover states and CSS-dependent behavior

**When Playwright is required:**
- You need real JavaScript execution (React/Vue reactivity, async data loading)
- Visual regression testing
- Cross-browser compatibility verification
- CSS-dependent behavior testing
- Complex user interactions (drag-drop, file upload, hover states)
## Command Decision Matrix

| I want to... | Command | Key flags |
|---|---|---|
| Check what's on a page | `open-browser navigate <url>` | `--format tree` or `--format json` |
| Test only interactive elements | `open-browser navigate <url>` | `--interactive-only` |
| Click a link by element ID | `open-browser interact <url> click-id <id>` | `--format json` |
| Click using CSS selector | `open-browser interact <url> click <selector>` | |
| Type into a form field | `open-browser interact <url> type-id <id> <value>` | |
| Submit a form | `open-browser interact <url> submit <selector> --field 'name=value'` | |
| Wait for element to appear | `open-browser interact <url> wait <selector>` | `--timeout-ms 5000` |
| Check network requests | `open-browser navigate <url>` | `--network-log` |
| Get navigation graph | `open-browser navigate <url>` | `--format json --with-nav` |
| Map entire site | `open-browser map <url>` | `--depth 3 --output kg.json` |
| Run interactive session | `open-browser repl` | `--js` for JavaScript |
| Automate via WebSocket | `open-browser serve` | `--host --port` |
| Clean session data | `open-browser clean` | `--cookies-only` or `--cache-only` |
| View PDF content | `open-browser navigate <url-to-pdf>` | Works automatically |
| Use JavaScript execution | `open-browser navigate <url>` | `--js --wait-ms 5000` |

## Workflow

### Pattern 1: Page Content Verification

```
1. Navigate to URL: open-browser navigate <url> --format json
2. Parse JSON output
3. Assert: check semantic_tree for expected headings, links, roles
4. Assert: check stats (landmarks, links, headings, actions counts)
```

### Pattern 2: Form Interaction Testing

```
1. Navigate to form page: open-browser navigate <url> --interactive-only
2. Identify form fields by element IDs ([#1], [#2], etc.)
3. Type into fields: open-browser interact <url> type-id <id> <value>
4. Submit form: open-browser interact <url> submit 'form' --field 'name=value'
5. Verify result: check response page semantic tree
```

### Pattern 3: Navigation Flow Testing

```
1. Navigate to start page: open-browser navigate <url> --format json --with-nav
2. Check navigation_graph for expected internal links
3. Click link: open-browser interact <url> click-id <id>
4. Verify navigation landed on correct page (check title, headings)
```

### Pattern 4: Site Coverage Mapping

```
1. Map site: open-browser map <url> --depth 3 --output kg.json
2. Parse kg.json
3. Count states and transitions
4. Verify all expected routes are discovered
5. Check for unreachable pages (gaps in the graph)
```

### Pattern 5: Network Request Verification

```
1. Navigate with network logging: open-browser navigate <url> --network-log --format json
2. Parse network_log from JSON output
3. Assert: expected requests were made (check URLs, status codes, timing)
4. Assert: no failed requests
5. Assert: subresource count matches expectations
```

### Pattern 6: Session Persistence Testing

```
1. Start REPL: open-browser repl
2. Navigate to login page: visit <login-url>
3. Type username: type #<id> <username>
4. Submit form: submit <selector>
5. Navigate to protected page: visit <protected-url>
6. Verify session persisted (cookies maintained)
4. Navigate to protected page: visit <protected-url>
5. Verify session persisted (cookies maintained)
```

## Output Formats

| Format | Flag | When to use |
|--------|------|-------------|
| Markdown | `--format md` (default) | Quick visual inspection, human-readable |
| Tree | `--format tree` | See hierarchy and nesting clearly |
| JSON | `--format json` | Programmatic parsing, assertions, CI |

JSON output structure for assertions:
- `url` — final URL (after redirects)
- `title` — page title
- `semantic_tree.root` — tree with `role`, `children`, `text`
- `semantic_tree.stats` — `{ landmarks, links, headings, actions }`
- `navigation_graph.internal_links` — `[{ url, label }]`
- `navigation_graph.external_links` — `[url]`
- `navigation_graph.forms` — `[{ action, method, fields }]`
- `network_log.requests` — `[{ method, url, status, content_type, timing_ms }]`

## Architecture Context

```
open-browser
├── crates/open-core    Browser type, HTML parsing, semantic tree, interaction, tabs
├── crates/open-debug   Network debugger, request recording, subresource discovery
├── crates/open-cdp     CDP WebSocket server (14 domains)
├── crates/open-kg      Knowledge Graph, BFS crawler, state fingerprinting
└── crates/open-cli     CLI binary (navigate, interact, map, tab, serve, repl, clean)
```

**Key architectural facts for testing:**
- No rendering engine — HTTP GET + HTML parse only (except optional V8 via `--js`)
- Element IDs are per-page, not persistent across navigations
- Tab state does NOT persist across CLI invocations (use REPL or CDP for persistence)
- JS execution is limited: only inline scripts, no external scripts, setTimeout/setInterval are no-ops
- PDF detection is automatic (content-type sniffing), no flags needed

## Known Limitations

| Limitation | Impact | Workaround |
|---|---|---|
| No real JS rendering | Cannot test JS-heavy SPAs | Use `--js` for basic inline scripts, use Playwright for real JS |
| No screenshots | No visual regression testing | Use Playwright for visual tests |
| No CSS rendering | Cannot test CSS-dependent behavior | Use Playwright for layout tests |
| No cross-browser | Only tests server-rendered HTML | Use Playwright for cross-browser |
| External scripts not executed | Dynamic content from external JS won't appear | By design — only inline scripts |
| setTimeout/setInterval no-ops | Timed behaviors won't execute | By design — prevents infinite loops |
| Element IDs per-page | IDs change between navigations | Re-navigate to get fresh IDs before interacting |

## Error Handling

| Situation | Response |
|---|---|
| Element ID not found | Re-navigate to get fresh element IDs, use `--interactive-only` to list available IDs |
| Form submission fails | Check CSRF tokens in form fields, verify form action/method |
| Navigation timeout | Increase `--wait-ms`, check if URL is reachable with `curl` |
| Build failure on Linux | Install `cmake` and `clang`, set `LIBCLANG_PATH=/usr/lib` |
| JS execution hangs | Avoid complex JS, only inline scripts are supported |
| PDF not detected | Verify server sends `Content-Type: application/pdf` header |
| CDP connection refused | Check `serve` is running, verify host/port, check firewall |

## Quick Tests

**Should trigger:**
- "Test the login flow with open-browser"
- "Map the site structure for testing coverage"
- "Navigate to a URL and check the semantic tree"
- "Interact with a form using open-browser"
- "Start a CDP server for browser automation"
- "Check what headings are on this page"
- "Verify the navigation links on the homepage"
- "Extract the semantic structure of this URL"

**Should not trigger:**
- "Take a screenshot of the page"
- "Test across Chrome, Firefox, and Safari"
- "Run visual regression tests"
- "Use Playwright to automate tests"
- "Check if the CSS layout is correct"
- "Test drag and drop functionality"
- "Verify pixel-perfect rendering"

**Functional:**
- `open-browser navigate https://example.com --format json` returns valid JSON with semantic_tree
- `open-browser interact https://example.com click-id 1` follows link by element ID
- `open-browser map https://example.com --depth 1 --output kg.json` produces valid knowledge graph
- `open-browser repl` starts interactive session with persistent state
- `open-browser navigate <pdf-url>` automatically extracts PDF content

## References

- `references/installation.md` — Build from source, Arch Linux extras, Docker
- `references/cli-reference.md` — All commands, flags, output formats, examples
- `references/testing-patterns.md` — Playwright vs open-browser translations, assertion recipes
- `references/repl-and-cdp.md` — REPL commands, CDP server for automation
- `references/knowledge-graph.md` — Site mapping, state fingerprinting, transition types
- `references/semantic-roles.md` — HTML element to ARIA role/action mapping table
- `references/programmatic-usage.md` — Rust API, Browser type, tab management
- `references/examples.md` — End-to-end examples for common testing scenarios
- `references/troubleshooting.md` — Build issues, runtime errors, fixes, workarounds
## Examples

See `references/examples.md` for complete end-to-end examples including:

- Navigate and verify page content
- Login flow with REPL
- Form submission via CLI
- Network debugging
- Site mapping and coverage verification
- PDF extraction
- Tab management
- CDP automation with Python
- Smoke testing multiple pages
- SEO audit script

## Best Practices

- **Always get fresh element IDs** before interacting — IDs are per-page and not persistent across navigations
- **Use `--interactive-only` first** when you need to interact with elements, to quickly find available IDs
- **Use `--format json` in scripts** for reliable programmatic parsing
- **Use REPL for multi-step flows** that require session persistence (cookies, login state)
- **Use CDP server for external automation** from Python, Node.js, or other test frameworks
- **Use `--network-log` for performance assertions** and to verify subresource loading
- **Map the site first** when testing unknown sites, to understand state space and reachable pages
- **Prefer `--format tree`** for quick visual debugging, `--format md` for default readability
- **Check `--with-nav`** when you need form descriptors and internal link graphs
- **Use Playwright as fallback** for anything requiring screenshots, real JS rendering, or complex interactions

## Related Skills

| Skill | Purpose | When to use |
|---|---|---|
| **playwright-cli** | Full browser automation with real rendering | When you need screenshots, JS-heavy SPA testing, visual regression |
| **bowser** | Headless browsing, parallel sessions | When you need background browser automation |
