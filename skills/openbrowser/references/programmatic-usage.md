# Programmatic Usage (Rust API)

The `Browser` type from `open-core` unifies navigation, interaction, and tab management into a single API.

## Setup

```rust
use open_core::Browser;
use open_core::BrowserConfig;

let mut browser = Browser::new(BrowserConfig::default());
```

## Navigation

```rust
// Navigate to URL (creates a tab automatically)
let tab = browser.navigate("https://example.com").await?;
```

## Interaction

```rust
// Click using CSS selector — updates tab automatically if navigation occurs
let result = browser.click("a").await?;

// Click using element ID — easier for AI agents
let result = browser.click_by_id(1).await?;  // Click element with ID [#1]

// Type using element ID
let result = browser.type_by_id(3, "search query").await?;  // Type into element [#3]

// Type using CSS selector
browser.type_text("input[name='q']", "search query")?;

// Submit a form (FormState accumulates field values)
let state = browser.current_form_state()?;
browser.submit("form", &state).await?;
```

## Tab Management

```rust
// Create a new tab
let id = browser.create_tab("https://example.com/page2");

// Switch to tab
browser.switch_to(id).await?;

// Go back in history
browser.go_back().await?;
```

## Accessing State

```rust
// Get current page
let page = browser.current_page().unwrap();

// Get semantic tree
let tree = page.semantic_tree();

// Find element by ID
if let Some(element) = page.find_by_element_id(1) {
    println!("Element selector: {}", element.selector);
}
```

## Architecture

The `Browser` type owns:
- HTTP client (reqwest)
- Tab state (multiple tabs with independent history)
- Session persistence (cookies, headers, localStorage)
- Optional JavaScript execution (deno_core)

Internal pipeline: fetch via reqwest -> parse HTML with scraper -> build semantic tree with ARIA roles -> detect interactive elements and assign IDs.

PDF URLs detected by content-type (`application/pdf`) are automatically routed to PDF extraction (pdf-extract/lopdf) instead of HTML parsing.

## Crate Dependencies

```
open-browser
├── crates/open-core    Browser type, HTML parsing, semantic tree, interaction, tabs
├── crates/open-debug   Network debugger, request recording, subresource discovery
├── crates/open-cdp     CDP WebSocket server (14 domains)
├── crates/open-kg      Knowledge Graph, BFS crawler, state fingerprinting
└── crates/open-cli     CLI binary
```

When using programmatically, you primarily interact with `open-core`. The other crates provide additional functionality:
- `open-debug` for network logging
- `open-cdp` for CDP server integration
- `open-kg` for site mapping
