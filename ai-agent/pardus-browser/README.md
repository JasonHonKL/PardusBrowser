# Pardus Browser Agent

An AI agent that uses pardus-browser for web navigation via OpenAI function calling.

## Features

- 🌐 **Multi-instance browser management** - Each conversation can spawn multiple isolated browser instances
- 🤖 **OpenAI-compatible** - Works with OpenAI, OpenRouter, or any OpenAI-compatible API
- 🔧 **9 browser tools** - navigate, click, fill, submit, scroll, list, and more
- 📝 **Semantic tree output** - LLM-friendly markdown representation of pages
- 🧹 **Automatic cleanup** - Browser processes killed on exit

## Quick Start

### 1. Install Dependencies

```bash
cd ai-agent/pardus-browser
npm install
```

### 2. Configure

Copy `.env.example` to `.env` and fill in your API key:

```bash
cp .env.example .env
# Edit .env with your API key
```

Or set environment variable:
```bash
export OPENAI_API_KEY=your_key_here
```

Or create config file at `~/.pardus-agent/config.json`:
```json
{
  "apiKey": "your_key_here",
  "baseURL": "https://api.openai.com/v1",
  "model": "gpt-4"
}
```

### 3. Run

Build first:
```bash
npm run build
```

Interactive mode:
```bash
npm start
```

Single query:
```bash
npm start "Find the latest version of Node.js on nodejs.org"
```

Development mode (no build needed):
```bash
npm run dev
```

## Using with OpenRouter

```bash
# In .env or environment:
OPENAI_BASE_URL=https://openrouter.ai/api/v1
OPENAI_API_KEY=your_openrouter_key
OPENAI_MODEL=anthropic/claude-3-opus

npm run dev
```

## Testing

The project uses Node.js's built-in test runner (available in Node 18+).

### Run all tests:
```bash
npm run build
npm test
```

### Watch mode (re-run on changes):
```bash
npm run build
npm run test:watch
```

### Coverage report:
```bash
npm run build
npm run test:coverage
```

### Type checking:
```bash
npm run lint
```

## Architecture

```
src/
├── __tests__/              # Test files
│   ├── core/
│   │   ├── types.test.ts
│   │   ├── BrowserManager.test.ts
│   │   └── ...
│   ├── tools/
│   │   ├── definitions.test.ts
│   │   └── executor.test.ts
│   ├── agent/
│   │   └── Agent.test.ts
│   ├── llm/
│   │   └── prompts.test.ts
│   ├── integration.test.ts
│   └── test-utils.ts       # Mock utilities
├── core/
│   ├── BrowserInstance.ts  # CDP WebSocket wrapper
│   ├── BrowserManager.ts   # Multi-instance management
│   ├── types.ts            # Type definitions
│   └── index.ts            # Re-exports
├── tools/
│   ├── definitions.ts      # OpenAI tool schemas (9 tools)
│   ├── executor.ts        # Tool call handler
│   └── index.ts
├── llm/
│   ├── client.ts          # OpenAI-compatible client
│   ├── prompts.ts          # System prompt
│   └── index.ts
├── agent/
│   ├── Agent.ts            # Main orchestration
│   └── index.ts
└── index.ts               # CLI entry point
```

## Available Tools

| Tool | Description |
|------|-------------|
| `browser_new` | Create a new browser instance |
| `browser_navigate` | Navigate to URL, return semantic tree |
| `browser_click` | Click element by ID |
| `browser_fill` | Fill input field |
| `browser_submit` | Submit form |
| `browser_scroll` | Page scrolling |
| `browser_get_state` | Get current page state |
| `browser_list` | List active instances |
| `browser_close` | Close instance |

## Example Conversation

```
> Find the latest version of Node.js

🤔 Thinking...

[Tool] browser_new: {}
[Tool Result] Success

[Tool] browser_navigate: {"instance_id": "browser_abc123", "url": "https://nodejs.org", "wait_ms": 3000}
[Tool Result] Success

[Tool] browser_click: {"instance_id": "browser_abc123", "element_id": "#5"}
[Tool Result] Success

The latest version of Node.js is v20.12.0 (LTS). You can download it from the
downloads page or use a version manager like nvm.

>
```

## Building

```bash
npm run build        # Compile TypeScript to dist/
npm run clean        # Remove dist/
npm run lint         # Type check without emit
```

## Requirements

- Node.js 18+ (for built-in test runner)
- pardus-browser installed and in PATH
- OpenAI API key (or compatible service)

## Configuration Options

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAI_API_KEY` | Your API key | Required |
| `OPENAI_BASE_URL` | API base URL | `https://api.openai.com/v1` |
| `OPENAI_MODEL` | Model to use | `gpt-4` |
| `BROWSER_TIMEOUT` | Default timeout (ms) | `30000` |
| `BROWSER_PROXY` | Default proxy URL | None |
| `DEBUG` | Enable verbose logging | `false` |

### Config File

Create `~/.pardus-agent/config.json`:

```json
{
  "apiKey": "your_key",
  "baseURL": "https://api.openai.com/v1",
  "model": "gpt-4-turbo",
  "timeout": 60000
}
```

## License

MIT
