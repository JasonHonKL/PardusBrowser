# Examples

Complete end-to-end examples for common testing scenarios with open-browser.

## Example 1: Navigate and Verify Page Content

Verify that `https://example.com` has the expected heading and at least one link.

```bash
RESULT=$(open-browser navigate https://example.com --format json)

# Check title
echo "$RESULT" | jq -r '.title'
# Expected: "Example Domain"

# Check there is an h1 heading
# Count all headings (H1 specificity depends on JSON structure)
H1_COUNT=$(echo "$RESULT" | jq '[.. | objects | select(.role? == "heading")] | length')
echo "Heading count: $H1_COUNT"
# Expected: >= 1 (check output for h1 specifically)

# Check there is at least 1 link
LINKS=$(echo "$RESULT" | jq '.semantic_tree.stats.links')
echo "Links: $LINKS"
# Expected: >= 1
```

## Example 2: Login Flow with REPL

Test a complete login flow with session persistence using the REPL.

```bash
# Start REPL with JS enabled
open-browser repl --js

# Inside the REPL:
visit https://example.com/login
# Output shows interactive elements with IDs
# e.g., [#1] textbox "Email" [action: fill]
#       [#2] textbox "Password" [action: fill]
#       [#3] button "Sign In" [action: click]

type #1 user@example.com
type #2 mypassword
click #3

# Verify redirect to dashboard
visit https://example.com/dashboard
# Check dashboard headings are present

exit
```

## Example 3: Form Submission via CLI

Submit a contact form with fields and verify the thank-you page.

```bash
# Step 1: Navigate and identify form fields
open-browser navigate https://example.com/contact --interactive-only
# Output shows:
# [#1] textbox "Name" [action: fill]
# [#2] textbox "Email" [action: fill]
# [#3] textbox "Message" [action: fill]
# [#4] button "Send" [action: click]

# Step 2: Submit via interact
RESULT=$(open-browser interact https://example.com/contact submit 'form' \
  --field 'name=John Doe' \
  --field 'email=john@example.com' \
  --field 'message=Hello!' \
  --format json)

# Step 3: Verify we landed on the thank-you page
FINAL_URL=$(echo "$RESULT" | jq -r '.url')
echo "Final URL: $FINAL_URL"
# Expected: "https://example.com/thank-you"

# Step 4: Verify heading on thank-you page
THANK_YOU_HEADING=$(echo "$RESULT" | jq -r '.. | objects | select(.role? == "heading") | .text' | head -1)
echo "Heading: $THANK_YOU_HEADING"
# Expected: contains "Thank You"
```

## Example 4: Network Debugging

Capture all network requests made when loading a page.

```bash
RESULT=$(open-browser navigate https://example.com --network-log --format json)

# Total requests and timing
echo "$RESULT" | jq '{total_requests: .network_log.total_requests, total_bytes: .network_log.total_bytes, total_time_ms: .network_log.total_time_ms, failed: .network_log.failed}'

# List all request URLs
echo "$RESULT" | jq -r '.network_log.requests[] | "\(.method) \(.status) \(.timing_ms)ms \(.url)"'
```

Expected output:
```
GET 200 142ms https://example.com/
GET 200 45ms https://example.com/styles.css
GET 200 23ms https://example.com/script.js
```

## Example 5: Site Mapping

Map a site and verify specific routes are reachable.

```bash
# Generate knowledge graph
open-browser map https://example.com --depth 2 --output kg.json

# Verify key pages were found
for PATH in "/" "/about" "/contact"; do
  FOUND=$(jq -r --arg p "$PATH" '.states | to_entries[] | select(.value.url | endswith($p)) | .key' kg.json)
  if [ -n "$FOUND" ]; then
    echo "PASS: $PATH found"
  else
    echo "FAIL: $PATH not found"
  fi
done

# Verify there are no dead links (unverified transitions)
UNVERIFIED=$(jq '[.transitions[] | select(.verified == false)] | length' kg.json)
echo "Unverified transitions: $UNVERIFIED"
# Expected: 0
```

## Example 6: PDF Extraction

Navigate to a PDF URL and extract its structure.

```bash
RESULT=$(open-browser navigate https://example.com/report.pdf --format json)

# Check page title (from PDF metadata or first heading)
echo "$RESULT" | jq -r '.title'

# Count headings extracted from PDF
HEADING_COUNT=$(echo "$RESULT" | jq '[.. | objects | select(.role? == "heading")] | length')
echo "Headings found: $HEADING_COUNT"

# List all headings
echo "$RESULT" | jq -r '.. | objects | select(.role? == "heading") | "\(.level // ""): \(.text)"'
```

## Example 7: Tab Management

Open multiple tabs in the REPL and switch between them.

```bash
open-browser repl

# Inside REPL:
visit https://example.com
# Now at example.com

tab open https://httpbin.org
# Opened tab 2

tab list
# Tabs (2 total):
#   * [2] Ready — httpbin.org — https://httpbin.org
#     [1] Ready — Example Domain — https://example.com

tab switch 1
# Back to example.com

click #1
# Click first interactive element

tab switch 2
# Back to httpbin.org

exit
```

## Example 8: CDP Automation with Python

Start the CDP server and control it from Python.

```bash
# Terminal 1: start CDP server
open-browser serve --host 127.0.0.1 --port 9222
```

```python
# test_cdp.py
import asyncio
import websockets
import json

async def test_navigation():
    uri = "ws://127.0.0.1:9222"
    async with websockets.connect(uri) as ws:
        # Navigate to example.com
        await ws.send(json.dumps({
            "id": 1,
            "method": "Page.navigate",
            "params": {"url": "https://example.com"}
        }))
        response = await ws.recv()
        print(response)

        # Evaluate title
        await ws.send(json.dumps({
            "id": 2,
            "method": "Runtime.evaluate",
            "params": {"expression": "document.title"}
        }))
        response = await ws.recv()
        print(response)

asyncio.run(test_navigation())
```

## Example 9: Smoke Test Multiple Pages

Run a quick smoke test across multiple pages.

```bash
#!/bin/bash
URLS=(
  "https://example.com/"
  "https://example.com/about"
  "https://example.com/products"
  "https://example.com/contact"
)

for URL in "${URLS[@]}"; do
  RESULT=$(open-browser navigate "$URL" --format json)
  STATUS=$(echo "$RESULT" | jq -r '.semantic_tree.stats.headings')
  if [ "$STATUS" -gt 0 ]; then
    echo "PASS: $URL ($STATUS headings)"
  else
    echo "FAIL: $URL (no headings found)"
  fi
done
```

## Example 10: SEO Audit

Check basic SEO elements on a page.

```bash
RESULT=$(open-browser navigate https://example.com --format json)

# 1. Title exists and is not empty
TITLE=$(echo "$RESULT" | jq -r '.title')
[ -n "$TITLE" ] && echo "PASS: Title present ($TITLE)" || echo "FAIL: Missing title"

# 2. Exactly one H1 (if JSON includes level field, filter with | select(.level? == 1))
H1_COUNT=$(echo "$RESULT" | jq '[.. | objects | select(.role? == "heading")] | length')
[ "$H1_COUNT" -ge 1 ] && echo "PASS: At least one heading" || echo "FAIL: No headings found"

# 3. Has navigation landmark
NAV=$(echo "$RESULT" | jq '[.. | objects | select(.role? == "navigation")] | length')
[ "$NAV" -ge 1 ] && echo "PASS: Navigation landmark present" || echo "FAIL: No navigation landmark"

# 4. Has main landmark
MAIN=$(echo "$RESULT" | jq '[.. | objects | select(.role? == "main")] | length')
[ "$MAIN" -ge 1 ] && echo "PASS: Main landmark present" || echo "FAIL: No main landmark"
```
