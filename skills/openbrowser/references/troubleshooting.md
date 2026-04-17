# Troubleshooting

Common issues when using open-browser and their solutions.

## Build and Installation Issues

### Error: `is 'cmake' not installed?`

**Cause:** The `boring-sys2` crate requires cmake to build BoringSSL, but cmake is not installed or not on PATH.

**Fix:**
```bash
# Arch Linux
sudo pacman -S cmake

# Debian/Ubuntu
sudo apt install cmake

# macOS
brew install cmake
```

---

### Error: `Unable to find libclang`

**Cause:** The `bindgen` crate needs `libclang.so` to generate FFI bindings, but cannot find it.

**Fix on Arch Linux:**
```bash
# Install clang
sudo pacman -S clang

# Set LIBCLANG_PATH (required even after installation)
export LIBCLANG_PATH=/usr/lib

# Make it permanent
echo 'export LIBCLANG_PATH=/usr/lib' >> ~/.bashrc
```

**Fix on Debian/Ubuntu:**
```bash
sudo apt install clang libclang-dev
```

**Fix on macOS:**
```bash
brew install llvm
export LIBCLANG_PATH=$(brew --prefix llvm)/lib
```

---

### Error: `const_type_id` feature error

**Cause:** open-browser requires Rust nightly for `const_type_id` (used by deno_core).

**Fix:**
```bash
rustup install nightly
# Always use cargo +nightly
cargo +nightly install --path crates/open-cli --features js
```

---

## Runtime Issues

### Element ID not found

**Cause:** Element IDs are per-page and change between navigations. An ID from a previous navigation will not be valid on the current page.

**Fix:**
1. Re-navigate to the current page to get fresh element IDs
2. Use `--interactive-only` to see only interactive elements with their current IDs

```bash
open-browser navigate https://example.com --interactive-only
```

---

### Form submission fails or returns wrong page

**Cause 1:** Missing CSRF token. Forms with hidden CSRF fields need the token included.

**Fix:** Use the `submit` command which automatically collects all hidden fields, or manually include the token via `--field`.

**Cause 2:** Wrong form action or method.

**Fix:** Inspect the form with `--format json --with-nav` to verify `action` and `method`:
```bash
open-browser navigate https://example.com --format json --with-nav
```

---

### JavaScript execution hangs or produces no output

**Cause:** open-browser only supports inline scripts. External scripts are not fetched or executed, and `setTimeout`/`setInterval` are no-ops.

**Fix:**
- Do not rely on `--js` for complex SPAs
- Increase `--wait-ms` if the inline script needs time to execute
- For JS-heavy sites, use Playwright instead

```bash
# Give JS more time
open-browser navigate https://example.com --js --wait-ms 5000
```

---

### Navigation timeout

**Cause:** The target URL is unreachable, slow, or requires authentication.

**Fix:**
1. Verify the URL with curl:
   ```bash
   curl -I https://example.com
   ```
2. Increase timeout if needed (via `--wait-ms` for JS, or check network connectivity)
3. Add required authentication headers:
   ```bash
   open-browser navigate https://example.com --header "Authorization: Bearer token"
   ```

---

### PDF content not extracted

**Cause:** The server is not sending `Content-Type: application/pdf` or the URL does not actually point to a PDF.

**Fix:**
1. Verify the content type:
   ```bash
   curl -I https://example.com/report.pdf | grep -i content-type
   ```
2. If the server sends `application/octet-stream` or `text/html`, open-browser cannot auto-detect the PDF.

---

### CDP server connection refused

**Cause:** The server is not running, the port is blocked, or the wrong host/port is being used.

**Fix:**
1. Verify the server is running:
   ```bash
   open-browser serve --host 127.0.0.1 --port 9222
   ```
2. Test the connection:
   ```bash
   curl http://127.0.0.1:9222/json
   ```
3. Check firewall rules if binding to non-localhost addresses

---

### `--interactive-only` shows no elements

**Cause:** The page has no interactive elements (links, buttons, inputs) or they are loaded by JavaScript.

**Fix:**
- Without JS: the page genuinely has no interactive HTML elements
- With JS: try `--js --wait-ms 3000` to allow inline scripts to generate elements

---

### Network log shows fewer requests than expected

**Cause:** open-browser discovers subresources from the initial HTML only. Resources injected by JavaScript after load are not fetched.

**Fix:**
- This is by design for the HTTP-only mode
- Use `--js` to execute inline scripts that may generate additional resource references
- For complete network tracing of JS-heavy sites, use Playwright

---

## Performance Issues

### Page loads are slow

**Cause:** Large HTML pages, slow network, or excessive subresource fetching when using `--network-log`.

**Fix:**
- Use `--interactive-only` to skip parsing non-interactive content
- Avoid `--network-log` if you don't need subresource data
- Use `--format tree` or `--format md` instead of `--format json` if you don't need programmatic parsing

---

### Map command takes too long or produces huge output

**Cause:** Site has many pages, deep pagination, or duplicate states.

**Fix:**
- Reduce `--depth` and `--max-pages`
- Use `--no-pagination` to skip pagination discovery
- Remember that identical fingerprints are deduplicated — large outputs usually mean genuinely diverse states

```bash
open-browser map https://example.com --depth 1 --max-pages 20 --output kg.json
```

## Known Limitations (By Design)

| Behavior | Reason | Workaround |
|---|---|---|
| External scripts not executed | Prevents dependency hell and infinite loops | Only inline scripts run with `--js` |
| `setTimeout`/`setInterval` no-ops | Prevents hangs from waiting on timers | Increase `--wait-ms` instead |
| No screenshot capability | No rendering engine | Use Playwright for visual tests |
| No drag-and-drop | HTTP-only architecture | Use Playwright for complex interactions |
| No file upload | No browser file picker | Use Playwright or direct API calls |
| iFrames not parsed | Security/complexity | Use Playwright for iframe content |
| Shadow DOM not parsed | Parsing limitation | Use Playwright for web components |
| Element IDs per-page | IDs depend on current DOM state | Re-fetch IDs before each interaction |
| Tabs don't persist across CLI calls | Stateless CLI design | Use REPL or CDP server for persistence |
