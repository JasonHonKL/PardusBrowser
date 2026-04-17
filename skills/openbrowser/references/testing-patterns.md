# Testing Patterns

This reference maps common testing patterns from Playwright to open-browser equivalents, and provides assertion recipes for AI-driven testing workflows.

## Playwright to open-browser Translation Table

### Navigation

| Playwright | open-browser |
|---|---|
| `await page.goto('https://example.com')` | `open-browser navigate https://example.com` |
| `await page.url()` | Parse `url` from JSON output |
| `await page.title()` | Parse `title` from JSON output |
| `await page.goBack()` | `back` in REPL |
| `await page.goForward()` | `forward` in REPL |
| `await page.reload()` | `reload` in REPL |

### Content Assertions

| Playwright | open-browser |
|---|---|
| `await page.locator('h1').textContent()` | Parse heading from semantic tree JSON |
| `await expect(page.locator('h1')).toHaveText('Title')` | Check semantic_tree for `heading (h1)` with text "Title" |
| `await page.locator('a').count()` | Check `stats.links` in JSON output |
| `await page.getByText('Submit')` | Use `--interactive-only` and find by text in output |
| `await expect(page).toHaveTitle('My Page')` | Assert `title` field in JSON output |

### Element Interaction

| Playwright | open-browser |
|---|---|
| `await page.click('a')` | `open-browser interact <url> click 'a'` |
| `await page.click('#submit-btn')` | `open-browser interact <url> click-id <id>` (find ID first) |
| `await page.fill('input[name="q"]', 'search')` | `open-browser interact <url> type-id <id> 'search'` |
| `await page.selectOption('select', 'value')` | `open-browser interact <url> click-id <id>` (limited) |
| `await page.check('#checkbox')` | Not directly supported — use click-id |
| `await page.setInputFiles('input', file)` | Not supported |

### Forms

| Playwright | open-browser |
|---|---|
| `await page.fill('#email', 'test@test.com')` | `open-browser interact <url> type-id <id> 'test@test.com'` |
| `await page.fill('#password', 'secret')` | `open-browser interact <url> type-id <id> 'secret'` |
| `await page.click('button[type="submit"]')` | `open-browser interact <url> click-id <id>` or `submit 'form'` |
| Login flow (multi-step) | Use REPL: `visit <url>` → `type #<id> <val>` → `submit <selector>` |

### Network

| Playwright | open-browser |
|---|---|
| `page.on('request', ...)` | `--network-log --format json` then parse `network_log.requests` |
| `page.on('response', ...)` | Same — all requests recorded automatically |
| `await page.waitForResponse(url)` | Not supported — use `--network-log` for all requests |

### Multi-tab

| Playwright | open-browser |
|---|---|
| `const page2 = await browser.newPage()` | `open-browser tab open <url>` or REPL `tab open <url>` |
| `await page2.bringToFront()` | REPL `tab switch <id>` |
| `browser.pages()` | `open-browser tab list` or REPL `tab list` |

## Assertion Recipes

### Assert page has heading
```bash
# Get page as JSON
RESULT=$(open-browser navigate https://example.com --format json)

# Assert heading exists (using jq)
echo "$RESULT" | jq -e '.semantic_tree.root.children[] | .. | select(.role? == "heading")'
```

### Assert page has specific link count
```bash
RESULT=$(open-browser navigate https://example.com --format json)
LINK_COUNT=$(echo "$RESULT" | jq '.semantic_tree.stats.links')
[ "$LINK_COUNT" -ge 5 ] && echo "PASS: $LINK_COUNT links found" || echo "FAIL: expected >= 5, got $LINK_COUNT"
```

### Assert form has required fields
```bash
RESULT=$(open-browser navigate https://example.com/signup --format json --with-nav)

# Check form exists with expected fields
echo "$RESULT" | jq -e '.navigation_graph.forms[] | select(.action == "/signup")'
echo "$RESULT" | jq -e '.navigation_graph.forms[].fields[] | select(.name == "email")'
```

### Assert navigation works
```bash
# Click link and verify destination
RESULT=$(open-browser interact https://example.com click-id 1 --format json)
FINAL_URL=$(echo "$RESULT" | jq -r '.url')
[ "$FINAL_URL" = "https://example.com/about" ] && echo "PASS" || echo "FAIL: landed on $FINAL_URL"
```

### Assert no failed network requests
```bash
RESULT=$(open-browser navigate https://example.com --network-log --format json)
FAILED=$(echo "$RESULT" | jq '.network_log.failed')
[ "$FAILED" -eq 0 ] && echo "PASS: no failures" || echo "FAIL: $FAILED requests failed"
```

### Assert page load under threshold
```bash
RESULT=$(open-browser navigate https://example.com --network-log --format json)
TOTAL_MS=$(echo "$RESULT" | jq '.network_log.total_time_ms')
[ "$TOTAL_MS" -lt 2000 ] && echo "PASS: ${TOTAL_MS}ms" || echo "FAIL: ${TOTAL_MS}ms exceeds 2000ms"
```

## Testing Workflow Templates

### Template 1: Login Flow Test
```bash
# Step 1: Get login page, find form fields
open-browser navigate https://example.com/login --interactive-only
# Output shows: [#1] textbox "Email" [action: fill], [#2] textbox "Password" [action: fill], [#3] button "Sign In" [action: click]

# Step 2: Fill and submit via REPL for session persistence
open-browser repl
# visit https://example.com/login
# type #1 user@example.com
# type #2 mypassword
# click #3
# visit https://example.com/dashboard
# Verify: dashboard page loads (check title/headings)
```

### Template 2: Form Validation Test
```bash
# Submit form with empty required fields
RESULT=$(open-browser interact https://example.com/contact submit 'form' --format json)

# Verify: form submission was rejected (still on same page or shows error)
URL=$(echo "$RESULT" | jq -r '.url')
# If URL unchanged, submission was rejected
```

### Template 3: Site Smoke Test
```bash
# Map site to find all pages
open-browser map https://example.com --depth 2 --output smoke-kg.json

# For each internal link in the graph, verify page loads
jq -r '.states | to_entries[] | .value.url' smoke-kg.json | while read URL; do
  STATUS=$(open-browser navigate "$URL" --format json | jq -r '.semantic_tree.stats.headings')
  echo "$URL: $STATUS headings"
done
```

### Template 4: SEO Content Check
```bash
RESULT=$(open-browser navigate https://example.com --format json)

# Check H1 exists and is unique
H1_COUNT=$(echo "$RESULT" | jq '[.. | objects | select(.role? == "heading") | select(.level? == 1)] | length')
echo "H1 count: $H1_COUNT (expected: 1)"

# Check meta info
TITLE=$(echo "$RESULT" | jq -r '.title')
echo "Title: $TITLE"

# Check navigation exists
LANDMARKS=$(echo "$RESULT" | jq '.semantic_tree.stats.landmarks')
echo "Landmarks: $LANDMARKS"
```

## What You Cannot Test with open-browser

- **Visual layout** — No CSS rendering, no screenshots
- **JavaScript interactions** — No real DOM, no event listeners, no React state
- **Async content** — Content loaded by JavaScript after page load won't appear (use `--js` for basic inline script execution only)
- **Hover states** — No mouse hover simulation
- **Drag and drop** — Not supported
- **File uploads** — Not supported
- **WebSockets** — CDP server provides WebSocket endpoint, but cannot test client WebSocket connections
- **iFrames** — Detected as subresources but content not parsed
- **Shadow DOM** — Not parsed
