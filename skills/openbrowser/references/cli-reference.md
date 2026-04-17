# CLI Reference

## Global Flags

| Flag | Description |
| --- | --- |
| `--format <md\|tree\|json>` | Output format (default: md) |
| `--interactive-only` | Show only interactive elements |
| `--js` | Enable JavaScript execution via V8/deno_core |
| `--wait-ms <ms>` | Wait time for async JS rendering (default: varies) |
| `--header <header>` | Custom HTTP header (can be repeated) |
| `--network-log` | Capture network request table |
| `--with-nav` | Include navigation graph in JSON output |
| `-v` | Verbose logging |

## Subcommands

### navigate

Navigate to a URL and output the semantic tree.

```bash
open-browser navigate <url> [--format md|tree|json] [--interactive-only] [--js] [--wait-ms N] [--header "Key: Value"] [--network-log] [--with-nav] [-v]
```

Examples:

```bash
# Default markdown tree
open-browser navigate https://example.com

# JSON with full data
open-browser navigate https://example.com --format json --with-nav

# Only buttons, links, inputs
open-browser navigate https://example.com --interactive-only

# With JavaScript execution
open-browser navigate https://example.com --js --wait-ms 5000

# Custom auth header
open-browser navigate https://api.example.com --header "Authorization: Bearer token"

# Network debugging
open-browser navigate https://example.com --network-log --format json
```

JSON output fields: `url`, `title`, `semantic_tree` (`root`, `stats`), `navigation_graph` (`internal_links`, `external_links`, `forms`), `network_log` (`total_requests`, `total_bytes`, `total_time_ms`, `requests[]`)

### interact

Interact with page elements at HTTP level.

```bash
open-browser interact <url> <action> [args] [--format json] [--js] [--wait-ms N]
```

Actions:

- `click <selector>` — Click element by CSS selector (follows links, submits forms)
- `click-id <id>` — Click element by its ID number (e.g., `click-id 1` for `[#1]`)
- `type <selector> <value>` — Type into field by CSS selector
- `type-id <id> <value>` — Type into field by element ID
- `submit <selector> --field 'name=value'` — Submit form with field values
- `wait <selector> --timeout-ms N` — Wait for element to appear
- `scroll --direction down|up|to-top|to-bottom` — Scroll/detect pagination

Examples:

```bash
# Click link by element ID
open-browser interact https://example.com click-id 1

# Type into search field
open-browser interact https://example.com type-id 3 'search query'

# Submit login form
open-browser interact https://example.com submit 'form' --field 'user=admin' --field 'pass=secret'

# Wait for dynamic content with JS
open-browser interact https://example.com wait '.result-list' --js --timeout-ms 5000

# Scroll/paginate
open-browser interact 'https://example.com/news?page=1' scroll --direction down
```

How interactions work:

- **click (link):** resolves href, HTTP GET, returns new page
- **click (button):** finds enclosing `<form>`, collects all fields including hidden CSRF tokens, submits via HTTP
- **type:** returns field selector + value
- **submit:** collects all form fields, merges with `--field` values, HTTP POST/GET
- **wait:** checks HTML for selector, re-fetches if not found
- **scroll:** detects pagination patterns in URL (`?page=`, `?offset=`, `/page/N`)

### map

Map a site's structure into a knowledge graph via BFS crawl.

```bash
open-browser map <url> [--depth N] [--max-pages N] [--output <file>] [--no-pagination] [-v]
```

| Flag | Default | Description |
| --- | --- | --- |
| `--depth` | 3 | Maximum crawl depth |
| `--max-pages` | 50 | Maximum pages to crawl |
| `--output` | required | Output JSON file path |
| `--no-pagination` | false | Skip pagination discovery |
| `-v` | — | Verbose logging |

Examples:

```bash
# Standard site map
open-browser map https://example.com --output kg.json

# Shallow crawl
open-browser map https://example.com --depth 1 --output kg.json

# Deep crawl
open-browser map https://example.com --depth 5 --max-pages 200 --output kg.json
```

### tab

Manage multiple browser tabs.

```bash
open-browser tab open <url> [--js]
open-browser tab list
open-browser tab info
open-browser tab navigate <url>
```

Note: Tab state does NOT persist across CLI invocations. Use REPL or CDP server for persistent tabs.

### serve

Start Chrome DevTools Protocol WebSocket server.

```bash
open-browser serve [--host <addr>] [--port <port>] [--timeout <seconds>]
```

| Flag | Default | Description |
| --- | --- | --- |
| `--host` | localhost | Bind address |
| `--port` | default | Port number |
| `--timeout` | none | Inactivity timeout in seconds |

Implemented CDP domains: Browser, Target, Page, Runtime, DOM, Network, Emulation, Input, CSS, Log, Console, Security, Performance, Open (custom)

### repl

Start interactive session with persistent state.

```bash
open-browser repl [--js] [--format md|tree|json] [--wait-ms N]
```

### clean

Clean session data (cookies, cache, localStorage).

```bash
open-browser clean [--cookies-only] [--cache-only] [--cache-dir <path>]
```

## PDF Support

PDF URLs are detected automatically by Content-Type. No flags needed:

```bash
open-browser navigate https://example.com/report.pdf
```

Works with all formats: `--format json`, `--format tree`, `--format md`
