# Knowledge Graph (Site Mapping)

Map a site's functional structure into a deterministic state graph. Nodes are view-states (semantic tree hash + resource fingerprint), edges are verified transitions.

## Command

```bash
open-browser map <url> [--depth N] [--max-pages N] [--output <file>] [--no-pagination] [-v]
```

| Flag | Default | Description |
|---|---|---|
| `--depth` | 3 | Maximum crawl depth |
| `--max-pages` | 50 | Maximum pages to crawl |
| `--output` | required | Output JSON file path |
| `--no-pagination` | false | Skip pagination discovery |
| `-v` | — | Verbose logging |

## How It Works

1. **BFS crawl** — Starting from the root URL, visits pages breadth-first up to `--depth` and `--max-pages`
2. **State fingerprinting** — Each page gets a composite ID: blake3 hash of semantic tree structure (roles + interactivity, not text) + resource URLs + URL path
3. **Deduplication** — Pages with identical fingerprints are merged (same layout, different copy = same state)
4. **Transition discovery** — For each page, discovers: link clicks, hash navigation (`#section`), pagination (`?page=N`, `/page/N`), and optional form submissions
5. **Verification** — Each transition is followed and the target state is confirmed

## Transition Types

| Type | Trigger | Example |
|---|---|---|
| `link_click` | Click internal link | `<a href="/about">About</a>` |
| `hash_navigation` | Hash/anchor link | `<a href="#features">Features</a>` |
| `pagination` | URL-based pagination | `?page=2`, `/page/2`, `?offset=20` |
| `form_submit` | Form submission | `<form action="/search">` |

## Output Structure

```json
{
  "root_url": "https://example.com",
  "built_at": "2026-04-02T14:30:00Z",
  "stats": {
    "total_states": 12,
    "total_transitions": 23,
    "verified_transitions": 21,
    "max_depth_reached": 3,
    "pages_crawled": 12,
    "crawl_duration_ms": 5420
  },
  "states": {
    "<fingerprint_hash>": {
      "url": "https://example.com/",
      "title": "Example Corp",
      "fingerprint": {
        "url_path": "/",
        "tree_hash": "...",
        "resource_set_hash": "..."
      },
      "semantic_tree": { ... },
      "resource_urls": ["..."]
    }
  },
  "transitions": [
    {
      "from": "<hash>",
      "to": "<hash>",
      "trigger": { "type": "link_click", "url": "/about", "label": "About Us" },
      "verified": true,
      "outcome": { "status": 200, "final_url": "...", "matched_prediction": true }
    }
  ]
}
```

## Usage Examples

```bash
# Standard site map
open-browser map https://example.com --output kg.json

# Shallow crawl (homepage only + direct links)
open-browser map https://example.com --depth 1 --output kg.json

# Deep crawl with high page limit
open-browser map https://example.com --depth 5 --max-pages 200 --output kg.json

# Skip pagination (only follow direct links)
open-browser map https://example.com --output kg.json --no-pagination

# Verbose logging for debugging
open-browser map https://example.com -v --output kg.json
```

## Testing Use Cases

### 1. Coverage Verification
Map the site, then assert all expected routes were discovered:
```bash
open-browser map https://example.com --depth 2 --output kg.json
EXPECTED=("/" "/about" "/products" "/contact")
for PAGE in "${EXPECTED[@]}"; do
  FOUND=$(jq -r --arg p "$PAGE" '.states | to_entries[] | select(.value.url | endswith($p)) | .key' kg.json)
  [ -n "$FOUND" ] && echo "PASS: $PAGE found" || echo "FAIL: $PAGE not found"
done
```

### 2. Dead Link Detection
```bash
open-browser map https://example.com --depth 3 --output kg.json
# Find unverified transitions (potential dead links)
jq '[.transitions[] | select(.verified == false)] | length' kg.json
```

### 3. Duplicate State Detection
```bash
# States with same tree_hash have identical layouts
jq '.states | to_entries | group_by(.value.fingerprint.tree_hash) | map(select(length > 1)) | length' kg.json
```

### 4. Transition Coverage Report
```bash
jq -r '.stats | "States: \(.total_states), Transitions: \(.total_transitions), Verified: \(.verified_transitions), Pages: \(.pages_crawled), Duration: \(.crawl_duration_ms)ms"' kg.json
```
