# Semantic Roles

open-browser maps HTML elements to ARIA roles and actions. This is the complete mapping reference.

## Element-to-Role Mapping

| HTML Element | ARIA Role | Action Type | Notes |
|---|---|---|---|
| `<html>` / `<body>` | `document` | — | Root node |
| `<header>` | `banner` | — | Page header |
| `<nav>` | `navigation` | — | Navigation landmark |
| `<main>` | `main` | — | Primary content landmark |
| `<aside>` | `complementary` | — | Sidebar content |
| `<footer>` | `contentinfo` | — | Page footer |
| `<section>` / `[role=region]` | `region` | — | Content section |
| `<form>` | `form` | — | Form container |
| `<form role=search>` | `search` | — | Search form |
| `<article>` | `article` | — | Self-contained content |
| `<h1>`–`<h6>` | `heading (hN)` | — | Headings (level 1-6) |
| `<a href>` | `link` | `navigate` | Hyperlink |
| `<button>` | `button` | `click` | Clickable button |
| `<input type=text/email/...>` | `textbox` | `fill` | Text input fields |
| `<input type=submit>` | `button` | `click` | Submit button |
| `<input type=checkbox>` | `checkbox` | `toggle` | Checkbox |
| `<input type=radio>` | `radio` | `toggle` | Radio button |
| `<select>` | `combobox` | `select` | Dropdown select |
| `<textarea>` | `textbox` | `fill` | Multi-line text input |
| `<img>` | `img` | — | Image (not interactive) |
| `<ul>` / `<ol>` | `list` | — | List container |
| `<li>` | `listitem` | — | List item |
| `<table>` | `table` | — | Table container |
| `<tr>` | `row` | — | Table row |
| `<td>` | `cell` | — | Table data cell |
| `<th>` | `columnheader` / `rowheader` | — | Table header cell |
| `<dialog>` | `dialog` | — | Dialog/modal |
| `[role=...]` | custom | varies | Explicit ARIA role |
| `[tabindex]` | varies | varies | Focusable element |

## Action Types

| Action | Meaning | Interaction Command |
|---|---|---|
| `navigate` | Follows link to new URL | `click-id <id>` or `click <selector>` |
| `click` | Triggers button/action | `click-id <id>` or `click <selector>` |
| `fill` | Type text into input | `type-id <id> <value>` or `type <selector> <value>` |
| `toggle` | Toggle checkbox/radio | `click-id <id>` |
| `select` | Select from dropdown | `click-id <id>` (limited) |

## Interactive Elements

Elements tagged with action types are assigned unique element IDs (`[#1]`, `[#2]`, etc.) in the semantic tree output. These IDs are used with `click-id` and `type-id` commands.

Example output:
```
document  [role: document]
├── banner  [role: banner]
│   ├── [#1] link "Home"  → /
│   ├── [#2] link "Products"  → /products
│   └── [#3] button "Sign In"
├── main  [role: main]
│   ├── heading (h1) "Welcome to Example"
│   └── form "Search"  [role: form]
│       ├── [#4] textbox "Search..."  [action: fill]
│       └── [#5] button "Go"  [action: click]
└── contentinfo  [role: contentinfo]
    └── [#6] link "Privacy"  → /privacy
```

Elements `[#1]` through `[#6]` are interactive and can be targeted with `click-id` or `type-id`.

## Non-Interactive Elements

Elements without action types (landmarks, headings, text, images, lists, tables) are structural only. They cannot be interacted with but provide context for understanding page structure.

## Using `--interactive-only`

The `--interactive-only` flag strips all non-interactive elements, showing only actionable items. Useful for:
- Quickly finding element IDs for interaction
- Verifying which elements are clickable/fillable
- Getting a concise view of what an AI agent can do on the page

```bash
open-browser navigate https://example.com --interactive-only
```

Output shows only elements with actions:
```
[#1] link "Home"  → /  [action: navigate]
[#2] link "Products"  → /products  [action: navigate]
[#3] button "Sign In"  [action: click]
[#4] textbox "Search..."  [action: fill]
[#5] button "Go"  [action: click]
[#6] link "Privacy"  → /privacy  [action: navigate]
```
