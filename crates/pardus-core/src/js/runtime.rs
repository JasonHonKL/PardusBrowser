//! JavaScript execution runtime.
//!
//! Uses deno_core (V8) to execute JavaScript with thread-based timeouts.
//! Provides a minimal `document` and `window` shim via ops that interact with the DOM.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use deno_core::*;
use scraper::{Html, Selector};
use url::Url;

use super::dom::DomDocument;
use super::extension::pardus_dom;

// ==================== Configuration ====================

const SCRIPT_TIMEOUT_MS: u64 = 2000; // 2s per script
const MAX_SCRIPT_SIZE: usize = 100_000; // 100KB
const MAX_SCRIPTS: usize = 20;
const EVENT_LOOP_TIMEOUT_MS: u64 = 500;
const EVENT_LOOP_MAX_POLLS: usize = 3;
const THREAD_JOIN_GRACE_MS: u64 = 2000;

/// Analytics/tracking patterns to skip (all lowercase for case-insensitive matching)
const ANALYTICS_PATTERNS: &[&str] = &[
    "google-analytics",
    "gtag(",
    "ga('",
    "gtag('",
    "facebook.com/tr",
    "fbq(",
    "fbq('",
    "hotjar",
    "hj(",
    "hj('",
    "mixpanel",
    "amplitude",
    "segment.com",
    "datalayer", // was dataLayer, but we lowercase the input
    "gtm.js",
    "googletagmanager",
    "adsbygoogle",
    "ads.js",
    "doubleclick",
    "newrelic",
    "nrqueue",
    "fullstory",
    "intercom",
    "zendesk",
    "helpscout",
    "heap.io",
    "logrocket",
    // Framework patterns that cause hangs
    "react",
    "reactdom",
    "vue",
    "vuejs",
    "angular",
    "next.js",
    "nuxt",
    "ember",
    "backbone",
    "knockout",
    "jquery",
    "webpack",
    "babel",
    "polyfill",
    "core-js",
    "regenerator",
    // Module loaders that may cause issues
    "systemjs",
    "requirejs",
    "amd",
    // Lazy loading / dynamic imports that may hang
    "import(",
    "__webpack_require__",
    // Service workers and PWA
    "serviceworker",
    "navigator.serviceworker",
    // Mutation observer loops
    "mutationobserver",
    // ResizeObserver loops
    "resizeobserver",
    // IntersectionObserver patterns
    "intersectionobserver",
    // WebGL that may crash
    "webgl",
    "getcontext(\"webgl\")",
    // Web Workers
    "worker",
    "webworker",
    "sharedworker",
    // WebSocket connections
    "websocket",
    "new websocket",
];

/// Patterns that indicate scripts likely to hang or cause issues
const PROBLEMATIC_PATTERNS: &[&str] = &[
    // Infinite loop patterns
    "while(true)",
    "while (true)",
    "for(;;)",
    "for (;;)",
    "while(1)",
    "while (1)",
    // Event listener spam
    "addeventlistener",
    "attachevent",
    // Timer spam
    "setinterval",
    // Animation loops
    "requestanimationframe",
    "requestidlecallback",
    // Promise chains that may not resolve
    "promise.resolve().then",
    "promise.resolve().catch",
    // Async generators
    "async function*",
    // Deprecated patterns
    "arguments.callee",
    // Eval usage (often dynamic code)
    "eval(",
    "new function(",
    // Function constructor
    "function()",
];

// ==================== Script Extraction ====================

#[derive(Debug, Clone)]
struct ScriptInfo {
    name: String,
    code: String,
}

/// Extract inline scripts from HTML, filtering out analytics/tracking.
fn extract_scripts(html: &str) -> Vec<ScriptInfo> {
    let doc = Html::parse_document(html);
    let selector = match Selector::parse("script") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    doc.select(&selector)
        .enumerate()
        .filter_map(|(i, el)| {
            let is_module = el.value().attr("type") == Some("module");

            // Skip external scripts (src attribute)
            if el.value().attr("src").is_some() {
                return None;
            }

            let mut code = el.text().collect::<String>();

            // Transform module syntax for basic support
            if is_module {
                code = transform_module_syntax(&code);
            }

            // Skip empty scripts
            if code.trim().is_empty() {
                return None;
            }

            // Skip large scripts
            if code.len() > MAX_SCRIPT_SIZE {
                return None;
            }

            // Skip analytics/tracking scripts
            if is_analytics_script(&code) {
                return None;
            }

            // Skip scripts with problematic patterns (infinite loops, event spam, etc.)
            if is_problematic_script(&code) {
                return None;
            }

            Some(ScriptInfo {
                name: format!("inline_script_{}.js", i),
                code,
            })
        })
        .take(MAX_SCRIPTS)
        .collect()
}

fn is_analytics_script(code: &str) -> bool {
    let lower = code.to_lowercase();
    ANALYTICS_PATTERNS.iter().any(|p| lower.contains(p))
}

fn is_problematic_script(code: &str) -> bool {
    let lower = code.to_lowercase();
    PROBLEMATIC_PATTERNS.iter().any(|p| lower.contains(p))
}

fn transform_module_syntax(code: &str) -> String {
    let mut result = String::new();

    for line in code.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("import ") || trimmed.starts_with("import{") || trimmed.starts_with("import(") {
            continue;
        }

        if trimmed.starts_with("export default ") {
            result.push_str(&trimmed[15..]);
            result.push('\n');
            continue;
        }

        if trimmed.starts_with("export const ") {
            result.push_str(&trimmed[7..]);
            result.push('\n');
            continue;
        }
        if trimmed.starts_with("export let ") {
            result.push_str(&trimmed[7..]);
            result.push('\n');
            continue;
        }
        if trimmed.starts_with("export var ") {
            result.push_str(&trimmed[7..]);
            result.push('\n');
            continue;
        }
        if trimmed.starts_with("export function ") {
            result.push_str(&trimmed[7..]);
            result.push('\n');
            continue;
        }
        if trimmed.starts_with("export class ") {
            result.push_str(&trimmed[7..]);
            result.push('\n');
            continue;
        }
        if trimmed.starts_with("export {") || trimmed.starts_with("export{") {
            let inner = trimmed.trim_start_matches("export ");
            result.push_str(inner);
            result.push('\n');
            continue;
        }
        if trimmed.starts_with("export = ") {
            continue;
        }

        result.push_str(line);
        result.push('\n');
    }

    result
}

// ==================== Runtime Creation ====================

/// Create a deno runtime with our DOM extension.
fn create_runtime(dom: Rc<RefCell<DomDocument>>, base_url: &Url) -> anyhow::Result<JsRuntime> {
    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![pardus_dom::init()],
        ..Default::default()
    });

    // Store DOM in op state
    runtime.op_state().borrow_mut().put(dom);

    // Store timer queue in op state
    runtime.op_state().borrow_mut().put(super::timer::TimerQueue::new());

    // Set up window.location from base_url
    let location_js = format!(
        r#"
        window.location = {{
            href: "{}",
            origin: "{}",
            protocol: "{}",
            host: "{}",
            hostname: "{}",
            pathname: "{}",
            search: "{}",
            hash: "{}"
        }};
    "#,
        base_url.as_str(),
        base_url.origin().ascii_serialization(),
        base_url.scheme(),
        base_url.host_str().unwrap_or(""),
        base_url.host_str().unwrap_or(""),
        base_url.path(),
        base_url.query().unwrap_or(""),
        base_url.fragment().unwrap_or("")
    );

    runtime.execute_script("location.js", location_js)?;

    Ok(runtime)
}

// ==================== Thread-Based Execution ====================

/// Result of script execution in a thread.
struct ThreadResult {
    dom_html: Option<String>,
    #[allow(dead_code)]
    error: Option<String>,
}

/// Execute scripts in a separate thread with timeout, graceful termination, and no leaks.
fn execute_scripts_with_timeout(
    html: String,
    base_url: String,
    scripts: Vec<ScriptInfo>,
    timeout_ms: u64,
) -> Option<String> {
    let result = Arc::new(Mutex::new(ThreadResult {
        dom_html: None,
        error: None,
    }));
    let result_clone = result.clone();
    let terminated = Arc::new(AtomicBool::new(false));
    let terminated_clone = terminated.clone();

    let handle = thread::spawn(move || {
        // Parse base URL
        let base = match Url::parse(&base_url) {
            Ok(u) => u,
            Err(e) => {
                *result_clone.lock().unwrap() = ThreadResult {
                    dom_html: None,
                    error: Some(format!("Invalid base URL: {}", e)),
                };
                return;
            }
        };

        // Create DOM from HTML
        let dom = Rc::new(RefCell::new(DomDocument::from_html(&html)));

        // Create runtime
        let mut runtime = match create_runtime(dom.clone(), &base) {
            Ok(r) => r,
            Err(e) => {
                *result_clone.lock().unwrap() = ThreadResult {
                    dom_html: None,
                    error: Some(format!("Failed to create runtime: {}", e)),
                };
                return;
            }
        };

        // Execute bootstrap.js
        let bootstrap = include_str!("bootstrap.js");
        if let Err(e) = runtime.execute_script("bootstrap.js", bootstrap) {
            *result_clone.lock().unwrap() = ThreadResult {
                dom_html: None,
                error: Some(format!("Bootstrap error: {}", e)),
            };
            return;
        }

        if terminated_clone.load(Ordering::Relaxed) {
            return;
        }

        // Execute each script with termination checks between them
        for script in scripts {
            if terminated_clone.load(Ordering::Relaxed) {
                return;
            }
            if let Err(e) = runtime.execute_script(script.name.clone(), script.code) {
                // Log error but continue with next script
                eprintln!("[JS] Script {} error: {}", script.name, e);
            }
        }

        if terminated_clone.load(Ordering::Relaxed) {
            return;
        }

        // Fire DOMContentLoaded event after all scripts
        let _ = runtime.execute_script("dom_content_loaded.js", r#"
    (function() {
        if (typeof _fireDOMContentLoaded === 'function') _fireDOMContentLoaded();
        var event = new Event('DOMContentLoaded', { bubbles: true, cancelable: false });
        document.dispatchEvent(event);
    })();
"#);

        // Run event loop with bounded timeout (not infinite)
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => return,
        };
        for _ in 0..EVENT_LOOP_MAX_POLLS {
            if terminated_clone.load(Ordering::Relaxed) {
                return;
            }
            let _ = rt.block_on(async {
                let _ = tokio::time::timeout(
                    Duration::from_millis(EVENT_LOOP_TIMEOUT_MS),
                    runtime.run_event_loop(PollEventLoopOptions::default()),
                )
                .await;
            });
        }

        // Drain expired timers (delay=0 callbacks)
        {
            let op_state_rc = runtime.op_state();
            let state_rc = op_state_rc.borrow();
            if let Some(queue) = state_rc.try_borrow::<super::timer::TimerQueue>() {
                if !queue.is_at_limit() {
                    let timer_js = queue.get_expired_timer_callbacks_js();
                    if !timer_js.is_empty() {
                        drop(state_rc);
                        let _ = runtime.execute_script("timers.js", timer_js);
                        let op_state_mut = runtime.op_state();
                        let mut state_mut = op_state_mut.borrow_mut();
                        if let Some(queue_mut) = state_mut.try_borrow_mut::<super::timer::TimerQueue>() {
                            queue_mut.mark_delay_zero_fired();
                        }
                    }
                }
            }
        }

        // Serialize DOM back to HTML
        let output = dom.borrow().to_html();
        *result_clone.lock().unwrap() = ThreadResult {
            dom_html: Some(output),
            error: None,
        };
    });

    // Wait for thread with timeout
    let start = Instant::now();
    loop {
        if handle.is_finished() {
            break;
        }
        if start.elapsed() >= Duration::from_millis(timeout_ms) {
            // Signal termination
            terminated.store(true, Ordering::SeqCst);
            eprintln!("[JS] Execution timed out after {}ms, waiting for thread to finish...", timeout_ms);

            // Give the thread a grace period to finish after termination signal
            let grace_start = Instant::now();
            loop {
                if handle.is_finished() {
                    break;
                }
                if grace_start.elapsed() >= Duration::from_millis(THREAD_JOIN_GRACE_MS) {
                    eprintln!("[JS] Thread did not finish within grace period, returning original HTML");
                    return None;
                }
                thread::sleep(Duration::from_millis(10));
            }
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    // One final check after the loop (fixes race condition where thread finishes between
    // is_finished() check and elapsed() check)
    let guard = result.lock().unwrap();
    guard.dom_html.clone()
}

// ==================== Main Entry Point ====================

/// Execute all scripts in the given HTML and return the modified HTML.
///
/// This uses deno_core (V8) to execute JavaScript. We provide a minimal
/// `document` and `window` shim via ops that interact with the DOM.
///
/// Thread-based timeout ensures we don't hang on complex scripts.
pub async fn execute_js(html: &str, base_url: &str, wait_ms: u32) -> anyhow::Result<String> {
    // Parse base URL
    let base = match Url::parse(base_url) {
        Ok(u) => u,
        Err(_) => return Ok(html.to_string()),
    };

    // Extract scripts from HTML
    let scripts = extract_scripts(html);

    // If no scripts, return original HTML
    if scripts.is_empty() {
        return Ok(html.to_string());
    }

    eprintln!(
        "[JS] Found {} inline script(s) to execute for {}",
        scripts.len(),
        base.as_str()
    );

    // Calculate total timeout: per-script timeout * number of scripts, max 30s
    let total_timeout = ((scripts.len() as u64) * SCRIPT_TIMEOUT_MS).min(30_000);
    let timeout = total_timeout.max(wait_ms as u64);

    // Execute in a separate thread with timeout
    let result = execute_scripts_with_timeout(html.to_string(), base_url.to_string(), scripts, timeout);

    match result {
        Some(modified_html) => Ok(modified_html),
        None => {
            // Timeout or error - return original HTML
            Ok(html.to_string())
        }
    }
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== extract_scripts Tests ====================

    #[test]
    fn test_extract_scripts_empty_html() {
        let scripts = extract_scripts("<html></html>");
        assert!(scripts.is_empty());
    }

    #[test]
    fn test_extract_scripts_no_scripts() {
        let html = r#"<html><body><p>Hello</p></body></html>"#;
        let scripts = extract_scripts(html);
        assert!(scripts.is_empty());
    }

    #[test]
    fn test_extract_scripts_simple_inline() {
        let html = r#"
            <html><body>
                <script>document.body.innerHTML = 'Hello';</script>
            </body></html>
        "#;
        let scripts = extract_scripts(html);
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].name, "inline_script_0.js");
        assert!(scripts[0].code.contains("document.body.innerHTML"));
    }

    #[test]
    fn test_extract_scripts_multiple_scripts() {
        let html = r#"
            <html><body>
                <script>var a = 1;</script>
                <script>var b = 2;</script>
                <script>var c = 3;</script>
            </body></html>
        "#;
        let scripts = extract_scripts(html);
        assert_eq!(scripts.len(), 3);
    }

    #[test]
    fn test_extract_scripts_skips_external() {
        let html = r#"
            <html><body>
                <script src="external.js"></script>
                <script>inline code</script>
            </body></html>
        "#;
        let scripts = extract_scripts(html);
        assert_eq!(scripts.len(), 1);
        assert!(scripts[0].code.contains("inline code"));
    }

    #[test]
    fn test_extract_scripts_transforms_module() {
        let html = r#"
            <html><body>
                <script type="module">import { foo } from './bar.js';
export const x = 1;
export function hello() {}</script>
                <script>regular script</script>
            </body></html>
        "#;
        let scripts = extract_scripts(html);
        assert_eq!(scripts.len(), 2);
        assert!(scripts[0].code.contains("const x = 1;"));
        assert!(scripts[0].code.contains("function hello() {}"));
        assert!(!scripts[0].code.contains("import "));
        assert!(!scripts[0].code.contains("export "));
        assert!(scripts[1].code.contains("regular script"));
    }

    #[test]
    fn test_extract_scripts_skips_empty() {
        let html = r#"
            <html><body>
                <script></script>
                <script>   </script>
                <script>real code</script>
            </body></html>
        "#;
        let scripts = extract_scripts(html);
        assert_eq!(scripts.len(), 1);
    }

    #[test]
    fn test_extract_scripts_skips_large() {
        let large_code: String = "x".repeat(MAX_SCRIPT_SIZE + 1);
        let html = format!(
            r#"<html><body><script>{}</script></body></html>"#,
            large_code
        );
        let scripts = extract_scripts(&html);
        assert!(scripts.is_empty());
    }

    #[test]
    fn test_extract_scripts_limits_count() {
        let mut scripts_html = String::from("<html><body>");
        for i in 0..60 {
            scripts_html.push_str(&format!("<script>var a{} = {};</script>", i, i));
        }
        scripts_html.push_str("</body></html>");

        let scripts = extract_scripts(&scripts_html);
        assert_eq!(scripts.len(), MAX_SCRIPTS);
    }

    // ==================== is_analytics_script Tests ====================

    #[test]
    fn test_is_analytics_script_google() {
        assert!(is_analytics_script("gtag('event', 'click');"));
        assert!(is_analytics_script("ga('send', 'pageview');"));
        assert!(is_analytics_script("google-analytics.com/analytics.js"));
    }

    #[test]
    fn test_is_analytics_script_facebook_pixel() {
        assert!(is_analytics_script("fbq('track', 'PageView');"));
        assert!(is_analytics_script("facebook.com/tr?id=123"));
    }

    #[test]
    fn test_is_analytics_script_hotjar() {
        assert!(is_analytics_script("hj('trigger', 'button');"));
        assert!(is_analytics_script("hotjar.identify({userId: 123});"));
    }

    #[test]
    fn test_is_analytics_script_segment() {
        assert!(is_analytics_script("segment.com/analytics.js"));
        assert!(is_analytics_script("mixpanel.track('Event');"));
    }

    #[test]
    fn test_is_analytics_script_not_analytics() {
        assert!(!is_analytics_script("function doSomething() { return 1; }"));
        assert!(!is_analytics_script("const app = { name: 'MyApp' };"));
        assert!(!is_analytics_script("document.querySelector('.btn').click();"));
    }

    #[test]
    fn test_is_analytics_script_case_insensitive() {
        assert!(is_analytics_script("GOOGLE-ANALYTICS.com/script.js"));
        assert!(is_analytics_script("GTag('event');"));
        // Note: dataLayer becomes datalayer when lowercased, so test with lowercase
        assert!(is_analytics_script("dataLayer.push({});"));
    }

    #[test]
    fn test_is_analytics_script_googletagmanager() {
        assert!(is_analytics_script("googletagmanager.com/gtm.js"));
        assert!(is_analytics_script("gtm.js"));
        assert!(is_analytics_script("dataLayer.push({event: 'click'});"));
    }

    #[test]
    fn test_is_analytics_script_ads() {
        assert!(is_analytics_script("adsbygoogle.push({});"));
        assert!(is_analytics_script("doubleclick.net/ad.js"));
    }

    // ==================== execute_js Tests ====================

    #[tokio::test]
    async fn test_execute_js_no_scripts() {
        let html = "<html><body><p>Hello</p></body></html>";
        let result = execute_js(html, "https://example.com", 100).await.unwrap();
        assert_eq!(result, html);
    }

    #[tokio::test]
    async fn test_execute_js_invalid_url() {
        let html = "<html><body><p>Hello</p></body></html>";
        let result = execute_js(html, "not-a-url", 100).await.unwrap();
        assert_eq!(result, html);
    }

    #[tokio::test]
    async fn test_execute_js_with_analytics_skipped() {
        let html = r#"
            <html><body>
                <script>gtag('event', 'click');</script>
            </body></html>
        "#;
        let result = execute_js(html, "https://example.com", 100).await.unwrap();
        assert!(result.contains("<html>"));
    }

    // ==================== is_problematic_script Tests ====================

    #[test]
    fn test_is_problematic_script_infinite_loops() {
        assert!(is_problematic_script("while(true) { }"));
        assert!(is_problematic_script("while (true) { }"));
        assert!(is_problematic_script("for(;;) { }"));
        assert!(is_problematic_script("for (;;) { }"));
        assert!(is_problematic_script("while(1) { }"));
        assert!(is_problematic_script("while (1) { }"));
    }

    #[test]
    fn test_is_problematic_script_event_listeners() {
        assert!(is_problematic_script("element.addEventListener('click', handler)"));
        assert!(is_problematic_script("setInterval(function() {}, 100)"));
        assert!(is_problematic_script("requestAnimationFrame(render)"));
    }

    #[test]
    fn test_is_problematic_script_react_vue() {
        assert!(is_analytics_script("const React = require('react')"));
        assert!(is_analytics_script("import Vue from 'vue'"));
        assert!(is_analytics_script("angular.module('app')"));
    }

    #[test]
    fn test_is_problematic_script_safe_code() {
        // These should NOT be flagged as problematic
        assert!(!is_problematic_script("function add(a, b) { return a + b; }"));
        assert!(!is_problematic_script("const x = 1;"));
        assert!(!is_problematic_script("document.body.innerHTML = 'Hello';"));
    }

    #[tokio::test]
    async fn test_execute_js_skips_problematic_scripts() {
        let html = r#"
            <html><body>
                <script>while(true) { }</script>
                <script>document.body.innerHTML = 'Safe';</script>
            </body></html>
        "#;
        let result = execute_js(html, "https://example.com", 100).await.unwrap();
        assert!(result.contains("Safe"));
    }
}
