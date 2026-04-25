---
name: OpenBrowser Only Automation
description: Use OpenBrowser as the exclusive browser engine for navigation, extraction, and web interaction tasks in this repository.
---

# OpenBrowser-only execution policy

Use this skill whenever the task involves browsing pages, extracting page content, clicking forms, or completing web workflows.

## Hard constraints

1. Use only OpenBrowser surfaces:
   - `browser_*` tools from `ai-agent/open-browser` when available.
   - or `open-browser` CLI commands (`Maps`, `interact`, `repl`, `serve`, `tab`, `map`).
2. Do not use Playwright, Puppeteer, Selenium, Cypress, raw Chromium scripts, or alternative browser stacks.
3. Treat OpenBrowser as semantic-first: plan from semantic tree/state and element IDs, never from pixel coordinates.

## Preferred runtime mode

For multi-step tasks, keep one persistent browser session:

- Prefer `browser_new -> browser_navigate -> ... -> browser_close` when browser tools are available.
- Otherwise use `open-browser repl --format llm` (or `open-browser serve` for CDP clients).

Avoid long workflows with repeated one-shot `open-browser interact <url> ...` calls, because each CLI invocation starts fresh state.

## Canonical stateful workflow

1. Open/create session.
2. Navigate to target URL. (Note: You can also navigate directly to .pdf URLs for automatic semantic extraction.)
3. Read semantic state (`browser_get_state` or equivalent page output).
4. Choose action target by current element ID `[#ID]` AND its indicated action `[action: click/fill/select]`.
5. Execute one action (`click`, `type/fill`, `select`, `submit`, `scroll`, `wait`).
   - *For forms:* Use `type-id` for multiple input fields sequentially, then use `click-id` on the submit button, or use the `submit` command on the form element.
6. Re-read state after every mutation or navigation.
7. Repeat until success criteria are met.
8. Close the session.

## ID-first interaction policy

- Prefer element-ID operations (`click-id`, `type-id`, `browser_click`, `browser_fill`) over brittle text-only guesses.
- IDs are ephemeral across navigation and DOM updates; refresh state before each follow-up action.
- If a flow opens a new tab/window, switch context explicitly (`browser_tab_switch` or `open-browser tab`).

## Dynamic pages and timing

- Use explicit waits (`browser_wait`, `wait`, or CLI `--wait-ms`) after async actions.
- Enable JS mode when required by the site (`--js` for CLI flows).
- On stale/invalid element failures, re-read state and re-plan from new IDs instead of blind retries.

## Advanced Strategies for AI

- **Site Exploration (Knowledge Graph):** If you are unsure about the website's structure or need to find a specific state/page without trial-and-error, use the `map` command (e.g., `open-browser map <url> --depth 2 --output kg.json`). Read the resulting graph to plan your navigation path.
- **Action Annotations:** Always rely on the `[action: ...]` tag in the semantic tree to determine what interactions are valid for a specific `[#ID]`. Do not guess interactions based on text alone.
- **PDF Handling:** Do not search for external PDF parsers. Simply `Maps` to the PDF URL using OpenBrowser to receive a fully parsed semantic tree with headings and text blocks.

## Output defaults

- Prefer concise LLM-oriented output (`--format llm`), or JSON when structured extraction is required.
- For extracted answers, include source context (URL and relevant section/text).

## Safety and policy boundaries

- Respect URL policy restrictions (private/loopback/link-local/metadata endpoints can be blocked).
- Respect sandbox and upload limits from project defaults.
- If policy blocks a request, report the constraint explicitly; do not bypass with another browser stack.

## Known capability boundaries

- `Page.printToPDF` is unsupported in semantic-only mode.
- Screenshot support requires the optional `screenshot` build feature; assume unavailable unless confirmed.
- Prefer implemented Open-domain primitives (`semanticTree`, `interact`, `getActionPlan`, `autoFill`, `getCoverage`, `wait`). If a higher-level tool returns `method not found`, fall back to supported primitives.

## Quick CLI examples

```bash
# Single-page semantic read (works for HTML and PDFs)
open-browser navigate "[https://example.com/report.pdf](https://example.com/report.pdf)" --format llm

# Form filling and interacting by ID (AI preferred method)
open-browser interact "[https://example.com/login](https://example.com/login)" type-id 1 "my_username" --format llm
open-browser interact "[https://example.com/login](https://example.com/login)" type-id 2 "my_password" --format llm
open-browser interact "[https://example.com/login](https://example.com/login)" click-id 3 --format llm

# Mapping a site structure before interacting
open-browser map "[https://example.com](https://example.com)" --depth 2 --format json

# Stateful multi-step flow
open-browser repl --format llm