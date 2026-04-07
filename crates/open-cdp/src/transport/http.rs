/// CDP HTTP discovery response for /json/version.
pub fn version_response(host: &str, port: u16) -> String {
    let ws_url = format!("ws://{}:{}/devtools/browser/open", host, port);
    serde_json::json!({
        "Browser": "OpenBrowser/0.1.0",
        "Protocol-Version": "1.3",
        "User-Agent": "OpenBrowser/0.1.0",
        "V8-Version": "deno",
        "WebKit-Version": "537.36",
        "webSocketDebuggerUrl": ws_url,
    })
    .to_string()
}

/// CDP HTTP discovery response for /json/list.
pub fn list_response(host: &str, port: u16) -> String {
    // Return an empty list initially — targets are populated as tabs are created.
    // In a full impl this reads from the shared TabManager.
    let _ = (host, port);
    serde_json::json!([]).to_string()
}

/// Simple HTTP response builder.
pub fn http_response(status: u16, content_type: &str, body: &str) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let mut response = header.into_bytes();
    response.extend_from_slice(body.as_bytes());
    response
}

/// Parse the HTTP request path from raw bytes.
pub fn parse_http_path(data: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(data).ok()?;
    let first_line = s.lines().next()?;
    let parts: Vec<&str> = first_line.split(' ').collect();
    if parts.len() >= 2 {
        Some(parts[1].to_string())
    } else {
        None
    }
}

/// Check if raw bytes look like a WebSocket upgrade request.
pub fn is_websocket_upgrade(data: &[u8]) -> bool {
    let s = std::str::from_utf8(data).unwrap_or("");
    s.contains("Upgrade: websocket") || s.contains("Upgrade: WebSocket")
}
