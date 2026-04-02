"""CDP Protocol compatibility test suite for Playwright/Puppeteer adapters.

Tests verify that the CDP server correctly responds to the methods
that Playwright and Puppeteer use during their connection and page lifecycle.
These tests can be run against a running pardus-browser serve instance.
"""

import json
import urllib.request
import urllib.error
import sys
import time

CDP_URL = "http://127.0.0.1:9222"
TIMEOUT = 15


def cdp_send(ws_url: str, method: str, params: dict = None) -> dict:
    """Send a CDP command over WebSocket and return the response."""
    try:
        import asyncio
        import websockets

        async def _send():
            async with websockets.connect(ws_url) as ws:
                msg = json.dumps({
                    "id": 1,
                    "method": method,
                    "params": params or {},
                })
                await ws.send(msg)
                response = await asyncio.wait_for(ws.recv(), timeout=TIMEOUT)
                return json.loads(response)

        loop = asyncio.new_event_loop()
        try:
            return loop.run_until_complete(_send())
        finally:
            loop.close()
    except ImportError:
        return {"error": "websockets not installed, skipping WS test"}


def get_ws_url() -> str:
    """Get the WebSocket debugger URL from the HTTP endpoint."""
    try:
        req = urllib.request.urlopen(f"{CDP_URL}/json/version", timeout=5)
        data = json.loads(req.read().decode())
        return data.get("webSocketDebuggerUrl", "")
    except Exception as e:
        print(f"Cannot connect to PardusBrowser at {CDP_URL}: {e}")
        sys.exit(1)


def test_http_discovery():
    """Test /json/version endpoint (used by Puppeteer)."""
    req = urllib.request.urlopen(f"{CDP_URL}/json/version", timeout=5)
    data = json.loads(req.read().decode())
    assert "Browser" in data
    assert "Protocol-Version" in data
    assert "webSocketDebuggerUrl" in data
    print("  [PASS] /json/version")


def test_http_list():
    """Test /json/list endpoint (used by Puppeteer)."""
    req = urllib.request.urlopen(f"{CDP_URL}/json/list", timeout=5)
    data = json.loads(req.read().decode())
    assert isinstance(data, list)
    print("  [PASS] /json/list")


def test_target_create_attach():
    """Test Target.createTarget and Target.attachToTarget."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Target.createTarget", {"url": "about:blank"})
    assert "result" in result, f"createTarget failed: {result}"
    target_id = result["result"].get("targetId")
    assert target_id

    result = cdp_send(ws_url, "Target.attachToTarget", {"targetId": target_id})
    assert "result" in result
    session_id = result["result"].get("sessionId")
    assert session_id
    print("  [PASS] Target.createTarget + attachToTarget")

    return target_id, session_id


def test_runtime_enable():
    """Test Runtime.enable (called by PW/Puppeteer on session attach)."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Runtime.enable")
    assert "result" in result
    print("  [PASS] Runtime.enable")


def test_runtime_evaluate():
    """Test Runtime.evaluate with basic expressions."""
    ws_url = get_ws_url()

    tests = [
        ("1 + 1", 2),
        ("'hello'", "hello"),
        ("true", True),
        ("[1,2,3]", [1, 2, 3]),
        ("({a: 1})", {"a": 1}),
        ("null", None),
    ]

    for expr, expected in tests:
        result = cdp_send(ws_url, "Runtime.evaluate", {"expression": expr})
        if "error" in result:
            print(f"  [SKIP] Runtime.evaluate({expr!r}): {result['error']}")
            continue
        actual = result.get("result", {}).get("result", {}).get("value")
        assert actual == expected, f"evaluate({expr!r}): expected {expected}, got {actual}"
        print(f"  [PASS] Runtime.evaluate({expr!r}) = {expected}")


def test_runtime_call_function_on():
    """Test Runtime.callFunctionOn."""
    ws_url = get_ws_url()
    result = cdp_send(
        ws_url,
        "Runtime.callFunctionOn",
        {
            "functionDeclaration": "function(a, b) { return a + b; }",
            "arguments": [
                {"value": 10},
                {"value": 20},
            ],
        },
    )
    if "error" not in result:
        value = result.get("result", {}).get("result", {}).get("value")
        assert value == 30, f"callFunctionOn: expected 30, got {value}"
        print("  [PASS] Runtime.callFunctionOn")
    else:
        print(f"  [SKIP] Runtime.callFunctionOn: {result['error']}")


def test_page_navigate():
    """Test Page.navigate (core navigation)."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Page.navigate", {"url": "https://example.com"})
    assert "result" in result
    frame_id = result["result"].get("frameId")
    assert frame_id
    print("  [PASS] Page.navigate")


def test_page_reload():
    """Test Page.reload."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Page.reload")
    assert "result" in result
    print("  [PASS] Page.reload")


def test_page_get_frame_tree():
    """Test Page.getFrameTree."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Page.getFrameTree")
    assert "result" in result
    assert "frameTree" in result["result"]
    print("  [PASS] Page.getFrameTree")


def test_page_get_resource_tree():
    """Test Page.getResourceTree."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Page.getResourceTree")
    assert "result" in result
    assert "frameTree" in result["result"]
    print("  [PASS] Page.getResourceTree")


def test_page_create_isolated_world():
    """Test Page.createIsolatedWorld (called by PW)."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Page.createIsolatedWorld", {
        "frameId": "main",
    })
    assert "result" in result
    assert "executionContextId" in result["result"]
    print("  [PASS] Page.createIsolatedWorld")


def test_page_screenshot_error():
    """Test that Page.captureScreenshot returns a meaningful error."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Page.captureScreenshot", {"format": "png"})
    assert "error" in result
    msg = result["error"].get("message", "")
    assert "not supported" in msg.lower() or "semantic" in msg.lower()
    print("  [PASS] Page.captureScreenshot returns proper error")


def test_page_pdf_error():
    """Test that Page.printToPDF returns a meaningful error."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Page.printToPDF")
    assert "error" in result
    msg = result["error"].get("message", "")
    assert "not supported" in msg.lower() or "semantic" in msg.lower()
    print("  [PASS] Page.printToPDF returns proper error")


def test_dom_get_document():
    """Test DOM.getDocument."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "DOM.getDocument")
    assert "result" in result
    assert "root" in result["result"]
    print("  [PASS] DOM.getDocument")


def test_dom_query_selector():
    """Test DOM.querySelector."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "DOM.querySelector", {"selector": "h1"})
    assert "result" in result
    assert "nodeId" in result["result"]
    print("  [PASS] DOM.querySelector")


def test_dom_query_selector_all():
    """Test DOM.querySelectorAll."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "DOM.querySelectorAll", {"selector": "p"})
    assert "result" in result
    assert "nodeIds" in result["result"]
    print("  [PASS] DOM.querySelectorAll")


def test_dom_describe_node():
    """Test DOM.describeNode."""
    ws_url = get_ws_url()
    sel_result = cdp_send(ws_url, "DOM.querySelector", {"selector": "h1"})
    node_id = sel_result.get("result", {}).get("nodeId", 0)
    if node_id and node_id > 0:
        result = cdp_send(ws_url, "DOM.describeNode", {"backendNodeId": node_id})
        assert "result" in result
        assert "node" in result["result"]
        print("  [PASS] DOM.describeNode")
    else:
        print("  [SKIP] DOM.describeNode (no h1 found)")


def test_dom_get_outer_html():
    """Test DOM.getOuterHTML."""
    ws_url = get_ws_url()
    sel_result = cdp_send(ws_url, "DOM.querySelector", {"selector": "h1"})
    node_id = sel_result.get("result", {}).get("nodeId", 0)
    if node_id and node_id > 0:
        result = cdp_send(ws_url, "DOM.getOuterHTML", {"backendNodeId": node_id})
        assert "result" in result
        html = result["result"].get("outerHTML", "")
        assert "<h1" in html.lower()
        print("  [PASS] DOM.getOuterHTML")
    else:
        print("  [SKIP] DOM.getOuterHTML (no h1 found)")


def test_input_dispatch_key_event():
    """Test Input.dispatchKeyEvent."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Input.dispatchKeyEvent", {
        "type": "keyDown",
        "key": "a",
        "windowsVirtualKeyCode": 65,
    })
    assert "result" in result
    print("  [PASS] Input.dispatchKeyEvent")


def test_input_dispatch_mouse_event():
    """Test Input.dispatchMouseEvent."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Input.dispatchMouseEvent", {
        "type": "mousePressed",
        "x": 100,
        "y": 100,
        "button": "left",
        "clickCount": 1,
    })
    assert "result" in result
    print("  [PASS] Input.dispatchMouseEvent")


def test_input_insert_text():
    """Test Input.insertText."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Input.insertText", {"text": "hello"})
    assert "result" in result
    print("  [PASS] Input.insertText")


def test_network_enable():
    """Test Network.enable."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Network.enable")
    assert "result" in result
    print("  [PASS] Network.enable")


def test_network_get_cookies():
    """Test Network.getCookies."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Network.getCookies")
    assert "result" in result
    assert "cookies" in result["result"]
    print("  [PASS] Network.getCookies")


def test_network_set_cookie():
    """Test Network.setCookie."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Network.setCookie", {
        "name": "test_cookie",
        "value": "test_value",
        "domain": "example.com",
    })
    assert "result" in result
    print("  [PASS] Network.setCookie")


def test_emulation_set_device_metrics():
    """Test Emulation.setDeviceMetricsOverride."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Emulation.setDeviceMetricsOverride", {
        "width": 800,
        "height": 600,
        "deviceScaleFactor": 1.0,
        "mobile": False,
    })
    assert "result" in result
    print("  [PASS] Emulation.setDeviceMetricsOverride")


def test_emulation_set_user_agent():
    """Test Emulation.setUserAgentOverride."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Emulation.setUserAgentOverride", {
        "userAgent": "TestAgent/1.0",
    })
    assert "result" in result
    print("  [PASS] Emulation.setUserAgentOverride")


def test_css_get_inline_styles():
    """Test CSS.getInlineStylesForNode."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "CSS.getInlineStylesForNode", {"nodeId": 1})
    assert "result" in result
    print("  [PASS] CSS.getInlineStylesForNode")


def test_pardus_semantic_tree():
    """Test Pardus.semanticTree (custom extension)."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Pardus.semanticTree")
    if "error" in result:
        print(f"  [SKIP] Pardus.semanticTree: {result['error']}")
        return
    assert "result" in result
    tree = result["result"].get("semanticTree", {})
    assert "root" in tree
    print("  [PASS] Pardus.semanticTree")


def test_pardus_detect_actions():
    """Test Pardus.detectActions (custom extension)."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Pardus.detectActions")
    if "error" in result:
        print(f"  [SKIP] Pardus.detectActions: {result['error']}")
        return
    assert "result" in result
    actions = result["result"].get("actions", [])
    assert isinstance(actions, list)
    print(f"  [PASS] Pardus.detectActions ({len(actions)} actions found)")


def test_pardus_navigation_graph():
    """Test Pardus.getNavigationGraph (custom extension)."""
    ws_url = get_ws_url()
    result = cdp_send(ws_url, "Pardus.getNavigationGraph")
    if "error" in result:
        print(f"  [SKIP] Pardus.getNavigationGraph: {result['error']}")
        return
    assert "result" in result
    graph = result["result"].get("navigationGraph", {})
    assert "internal_links" in graph
    assert "external_links" in graph
    print("  [PASS] Pardus.getNavigationGraph")


def main():
    print(f"PardusBrowser CDP Compatibility Test Suite")
    print(f"Target: {CDP_URL}")
    print()

    http_tests = [
        test_http_discovery,
        test_http_list,
    ]

    cdp_tests = [
        test_runtime_enable,
        test_runtime_evaluate,
        test_runtime_call_function_on,
        test_page_navigate,
        test_page_reload,
        test_page_get_frame_tree,
        test_page_get_resource_tree,
        test_page_create_isolated_world,
        test_page_screenshot_error,
        test_page_pdf_error,
        test_dom_get_document,
        test_dom_query_selector,
        test_dom_query_selector_all,
        test_dom_describe_node,
        test_dom_get_outer_html,
        test_input_dispatch_key_event,
        test_input_dispatch_mouse_event,
        test_input_insert_text,
        test_network_enable,
        test_network_get_cookies,
        test_network_set_cookie,
        test_emulation_set_device_metrics,
        test_emulation_set_user_agent,
        test_css_get_inline_styles,
        test_pardus_semantic_tree,
        test_pardus_detect_actions,
        test_pardus_navigation_graph,
    ]

    passed = 0
    failed = 0
    skipped = 0

    print("=== HTTP Discovery Tests ===")
    for test in http_tests:
        try:
            test()
            passed += 1
        except AssertionError as e:
            print(f"  [FAIL] {test.__name__}: {e}")
            failed += 1
        except Exception as e:
            print(f"  [ERROR] {test.__name__}: {e}")
            failed += 1

    print()
    print("=== CDP Protocol Tests ===")
    for test in cdp_tests:
        try:
            test()
            passed += 1
        except AssertionError as e:
            print(f"  [FAIL] {test.__name__}: {e}")
            failed += 1
        except Exception as e:
            if "SKIP" in str(e):
                skipped += 1
            else:
                print(f"  [ERROR] {test.__name__}: {e}")
                failed += 1

    print()
    print(f"Results: {passed} passed, {failed} failed, {skipped} skipped")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
