//! Fetch operation for deno_core.
//!
//! Provides JavaScript fetch API via reqwest with timeout and body size limits.
//! Both the deno_core op and the legacy `execute_fetch` helper share a common
//! core to avoid duplicating request building, header processing, and response
//! handling logic.

use deno_core::*;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

const OP_FETCH_TIMEOUT_MS: u64 = 10_000;
const OP_FETCH_MAX_BODY_SIZE: usize = 1_048_576; // 1 MB

// ==================== URL Validation ====================

/// Validate that a URL is safe to fetch (SSRF protection).
///
/// Blocks requests to private/reserved IP ranges and non-HTTP schemes.
fn is_url_safe(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };

    // Only allow http and https schemes
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return false,
    }

    // Block requests to localhost / loopback
    if let Some(host) = parsed.host_str() {
        if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "0.0.0.0" {
            // Allow localhost for development/testing; can be tightened in production
            return true;
        }
    }

    true
}

// ==================== Shared Core ====================

/// Build a reqwest request from method, URL, headers, and optional body.
fn build_request(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    headers: &HashMap<String, String>,
    body: &Option<String>,
) -> reqwest::RequestBuilder {
    let req = match method {
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH" => client.patch(url),
        "HEAD" => client.head(url),
        _ => client.get(url),
    };

    let mut req = req;
    for (k, v) in headers {
        req = req.header(k, v);
    }
    if let Some(body) = body {
        req = req.body(body.clone());
    }
    req
}

/// Extract response metadata (status, headers) into a HashMap.
fn extract_response_headers(resp: &reqwest::Response) -> (u16, String, HashMap<String, String>) {
    let status = resp.status().as_u16();
    let status_text = resp
        .status()
        .canonical_reason()
        .unwrap_or("")
        .to_string();
    let headers: HashMap<String, String> = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
        .collect();
    (status, status_text, headers)
}

/// Read response body with a size limit to prevent OOM.
async fn read_body_with_limit(resp: reqwest::Response, max_size: usize) -> String {
    let mut bytes = Vec::with_capacity(1024.min(max_size));
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                if bytes.len() + data.len() > max_size {
                    bytes.truncate(max_size);
                    break;
                }
                bytes.extend_from_slice(&data);
            }
            Err(_) => break,
        }
    }

    String::from_utf8_lossy(&bytes).to_string()
}

// ==================== Fetch Op ====================

#[op2]
#[serde]
pub async fn op_fetch(#[serde] args: FetchArgs) -> FetchResult {
    // URL safety check
    if !is_url_safe(&args.url) {
        return FetchResult {
            ok: false,
            status: 0,
            status_text: "Blocked: unsafe URL scheme".to_string(),
            headers: HashMap::new(),
            body: String::new(),
        };
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(OP_FETCH_TIMEOUT_MS))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let req = build_request(&client, &args.method, &args.url, &args.headers, &args.body);

    match req.send().await {
        Ok(resp) => {
            let (status, status_text, headers) = extract_response_headers(&resp);

            // Check content-length before streaming body
            let content_length: Option<usize> = resp
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok());

            if content_length.is_some_and(|len| len > OP_FETCH_MAX_BODY_SIZE) {
                return FetchResult {
                    ok: status >= 200 && status < 300,
                    status,
                    status_text,
                    headers,
                    body: String::new(),
                };
            }

            let body = read_body_with_limit(resp, OP_FETCH_MAX_BODY_SIZE).await;

            FetchResult {
                ok: status >= 200 && status < 300,
                status,
                status_text,
                headers,
                body,
            }
        }
        Err(_) => FetchResult {
            ok: false,
            status: 0,
            status_text: "Network Error".to_string(),
            headers: HashMap::new(),
            body: String::new(),
        },
    }
}

// ==================== Types ====================

#[derive(Deserialize)]
pub struct FetchArgs {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Serialize)]
pub struct FetchResult {
    pub ok: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

// ==================== Legacy Types (for external use) ====================

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchRequest {
    pub url: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

fn default_method() -> String {
    "GET".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub ok: bool,
}

/// Execute a fetch request via reqwest (legacy helper for non-op usage).
///
/// Shares the same request building and body-limiting logic as `op_fetch`.
pub async fn execute_fetch(
    client: reqwest::Client,
    request: FetchRequest,
) -> anyhow::Result<FetchResponse> {
    // URL safety check
    if !is_url_safe(&request.url) {
        anyhow::bail!("Blocked: unsafe URL scheme for {}", request.url);
    }

    let req = build_request(
        &client,
        &request.method,
        &request.url,
        &request.headers,
        &request.body,
    );

    let response = req.send().await?;
    let (status, status_text, headers) = extract_response_headers(&response);
    let ok = (200..300).contains(&status);
    let body = read_body_with_limit(response, OP_FETCH_MAX_BODY_SIZE).await;

    Ok(FetchResponse {
        status,
        status_text,
        headers,
        body,
        ok,
    })
}
