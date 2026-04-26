# REPL and CDP Server

## Interactive REPL

Start a persistent interactive session where browser state (tabs, pages, cookies, history) is preserved across commands.

```bash
# Start REPL with defaults
open-browser repl

# Enable JS execution by default
open-browser repl --js

# Set default output format and JS wait time
open-browser repl --format json --wait-ms 5000
```

The REPL prompt shows current URL context:
```
open> visit https://example.com
  document  [role: document]
  └── region  [role: region]
      ├── heading (h1)  "Example Domain"
      └── link  "Learn more"  → https://iana.org/domains/example
  0 landmarks, 1 links, 1 headings, 1 actions

open [https://example.com]> tab open https://httpbin.org
Opened tab 2: httpbin.org

open [https://httpbin.org]> tab list
Tabs (2 total):
  * [2] Ready — httpbin.org — https://httpbin.org
    [1] Ready — Example Domain — https://example.com

open [https://httpbin.org]> tab switch 1
Switched to tab 1: https://example.com

open [https://example.com]> click 'a'
Navigated to: https://iana.org/domains/example

open [https://iana.org/domains/example]> back
open [https://example.com]> exit
Bye.
```

### REPL Commands

| Command | Description |
|---|---|
| `visit <url>` / `open <url>` | Navigate to URL |
| `click <selector>` | Click an element using CSS selector |
| `click #<id>` | Click an element by its ID (e.g., `click #1`) |
| `type <selector> <value>` | Type into a field using CSS selector |
| `type #<id> <value>` | Type into a field by ID (e.g., `type #3 hello`) |
| `submit <selector> [name=value...]` | Submit a form |
| `scroll [down\|up\|to-top\|to-bottom]` | Scroll the page |
| `wait <selector> [timeout_ms]` | Wait for element |
| `back` / `forward` | Navigate history |
| `reload` | Reload current page |
| `tab list` | List all open tabs |
| `tab open <url>` | Open new tab |
| `tab switch <id>` | Switch to tab by ID |
| `tab close [id]` | Close tab |
| `tab info` | Show active tab info |
| `js [on\|off]` | Toggle JS execution |
| `format md\|tree\|json` | Change output format |
| `wait-ms <ms>` | Set JS wait time |
| `help` | Show available commands |
| `exit` / `quit` | Exit REPL |

### When to Use REPL vs CLI

| Scenario | Use REPL | Use CLI |
|---|---|---|
| Login flow with session persistence | X | |
| Multi-step form interaction | X | |
| Quick single-page check | | X |
| CI/CD test automation | | X (or CDP) |
| Tab management across steps | X | |
| Scripting in bash/python | | X |

## CDP Server

Start a Chrome DevTools Protocol WebSocket server for automation.

```bash
# Start on default host/port
open-browser serve

# Custom host and port
open-browser serve --host 0.0.0.0 --port 9222

# With inactivity timeout
open-browser serve --timeout 60
```

### CDP Flags

| Flag | Default | Description |
|---|---|---|
| `--host` | localhost | Bind address |
| `--port` | default | Port number |
| `--timeout` | none | Inactivity timeout in seconds |

### Implemented CDP Domains

| Domain | Purpose |
|---|---|
| Browser | Browser-level operations |
| Target | Target/tab management |
| Page | Page navigation, lifecycle |
| Runtime | JavaScript execution |
| DOM | DOM tree access |
| Network | Network interception, monitoring |
| Emulation | Device emulation |
| Input | Input event dispatch |
| CSS | CSS access |
| Log | Log entry access |
| Console | Console API |
| Security | Security state |
| Performance | Performance metrics |
| Open | Custom extensions |

### CDP Usage Pattern

```bash
# 1. Start the server
open-browser serve --port 9222 &

# 2. Connect via WebSocket
# Using wscat: wscat -c ws://localhost:9222
# Using python: websockets.connect('ws://localhost:9222')
# Using node: new WebSocket('ws://localhost:9222')

# 3. Send CDP commands
{"id": 1, "method": "Page.navigate", "params": {"url": "https://example.com"}}
{"id": 2, "method": "Runtime.evaluate", "params": {"expression": "document.title"}}
```

### When to Use CDP vs REPL vs CLI

| Scenario | Use CDP | Use REPL | Use CLI |
|---|---|---|---|
| External automation (Python/Node) | X | | |
| Real-time event streaming | X | | |
| Persistent browser with programmatic control | X | | |
| Quick interactive debugging | | X | |
| Bash script integration | | | X |
| CI/CD pipeline tests | X | | X |
