//! Fetch operation for deno_core.
//!
//! Provides JavaScript fetch API via reqwest with timeout, body size limits,
//! and HTTP cache compliance (ETag, Last-Modified, conditional requests).

use deno_core::*;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use crate::cache::ResourceCache;

const OP_FETCH_TIMEOUT_MS: u64 = 10_000;
const OP_FETCH_MAX_BODY_SIZE: usize = 1_048_576;

fn get_fetch_client() -> &'static reqwest::Client {
    crate::http::client::fetch_client()
}

fn get_fetch_cache() -> &'static Arc<ResourceCache> {
    static CACHE: OnceLock<Arc<ResourceCache>> = OnceLock::new();
    CACHE.get_or_init(|| Arc::new(ResourceCache::new(100 * 1024 * 1024)))
}

fn is_url_safe(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };

    match parsed.scheme() {
        "http" | "https" => {}
        _ => return false,
    }

    if let Some(host) = parsed.host_str() {
        if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "0.0.0.0" {
            return true;
        }
    }

    true
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchCacheMode {
    Default,
    NoStore,
    ForceCache,
    OnlyIfCached,
}

impl FetchCacheMode {
    fn from_str(s: &str) -> Self {
        match s {
            "no-store" => Self::NoStore,
            "force-cache" => Self::ForceCache,
            "only-if-cached" => Self::OnlyIfCached,
            _ => Self::Default,
        }
    }
}

#[op2]
#[serde]
pub async fn op_fetch(#[serde] args: FetchArgs) -> FetchResult {
    if !is_url_safe(&args.url) {
        return FetchResult {
            ok: false,
            status: 0,
            status_text: "Blocked: unsafe URL scheme".to_string(),
            headers: HashMap::new(),
            body: String::new(),
        };
    }

    let client = get_fetch_client();
    let cache_mode = args.cache.as_deref()
        .map(FetchCacheMode::from_str)
        .unwrap_or(FetchCacheMode::Default);

    let is_get = args.method.eq_ignore_ascii_case("get");
    let cache = get_fetch_cache();

    if is_get && cache_mode != FetchCacheMode::NoStore {
        if let Some(entry) = cache.get(&args.url) {
            let guard = entry.read().unwrap();

            match cache_mode {
                FetchCacheMode::ForceCache | FetchCacheMode::OnlyIfCached => {
                    if guard.is_fresh() || cache_mode == FetchCacheMode::ForceCache {
                        let body = String::from_utf8_lossy(&guard.content).to_string();
                        let status = 200u16;
                        let mut headers: HashMap<String, String> = guard.content_type
                            .as_ref()
                            .map(|ct| vec![("content-type".to_string(), ct.clone())])
                            .unwrap_or_default()
                            .into_iter()
                            .collect();
                        headers.insert("x-cache".to_string(), "hit".to_string());
                        drop(guard);
                        return FetchResult {
                            ok: true,
                            status,
                            status_text: "OK".to_string(),
                            headers,
                            body,
                        };
                    }
                    if cache_mode == FetchCacheMode::OnlyIfCached {
                        drop(guard);
                        return FetchResult {
                            ok: false,
                            status: 504,
                            status_text: "Gateway Timeout (cache miss)".to_string(),
                            headers: HashMap::new(),
                            body: String::new(),
                        };
                    }
                }
                FetchCacheMode::Default => {
                    if guard.is_fresh() {
                        let body = String::from_utf8_lossy(&guard.content).to_string();
                        let mut headers: HashMap<String, String> = guard.content_type
                            .as_ref()
                            .map(|ct| vec![("content-type".to_string(), ct.clone())])
                            .unwrap_or_default()
                            .into_iter()
                            .collect();
                        headers.insert("x-cache".to_string(), "hit".to_string());
                        drop(guard);
                        return FetchResult {
                            ok: true,
                            status: 200,
                            status_text: "OK".to_string(),
                            headers,
                            body,
                        };
                    }

                    if guard.cache_policy.has_validator {
                        let cond_headers = guard.conditional_headers();
                        drop(guard);

                        let mut request = client.get(&args.url);
                        for (name, value) in cond_headers.iter() {
                            request = request.header(name, value);
                        }
                        for (k, v) in &args.headers {
                            let k_lower = k.to_lowercase();
                            if k_lower != "if-none-match" && k_lower != "if-modified-since" {
                                request = request.header(k.as_str(), v.as_str());
                            }
                        }

                        match request.send().await {
                            Ok(resp) => {
                                let status = resp.status().as_u16();
                                let status_text = resp.status().canonical_reason().unwrap_or("").to_string();
                                let mut headers: HashMap<String, String> = resp.headers()
                                    .iter()
                                    .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
                                    .collect();

                                if status == 304 {
                                    cache.update_from_304(&args.url, resp.headers());
                                    if let Some(entry) = cache.get(&args.url) {
                                        let guard = entry.read().unwrap();
                                        let body = String::from_utf8_lossy(&guard.content).to_string();
                                        headers.insert("x-cache".to_string(), "hit (304)".to_string());
                                        drop(guard);
                                        return FetchResult {
                                            ok: true,
                                            status: 200,
                                            status_text: "OK".to_string(),
                                            headers,
                                            body,
                                        };
                                    }
                                }

                                let body = read_body_with_limit(resp, OP_FETCH_MAX_BODY_SIZE).await;
                                if (200..300).contains(&status) {
                                    cache.insert(&args.url, bytes::Bytes::from(body.clone()), None, &reqwest::header::HeaderMap::new());
                                }
                                headers.insert("x-cache".to_string(), "miss".to_string());
                                return FetchResult {
                                    ok: (200..300).contains(&status),
                                    status,
                                    status_text,
                                    headers,
                                    body,
                                };
                            }
                            Err(_) => {}
                        }
                    }
                    // Fall through to full fetch
                }
                FetchCacheMode::NoStore => {}
            }
        }
    }

    let req = build_request(&client, &args.method, &args.url, &args.headers, &args.body);

    match req.send().await {
        Ok(resp) => {
            let (status, status_text, mut headers) = extract_response_headers(&resp);

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

            if is_get && cache_mode != FetchCacheMode::NoStore && (200..300).contains(&status) {
                cache.insert(
                    &args.url,
                    bytes::Bytes::from(body.clone()),
                    headers.get("content-type").cloned(),
                    &reqwest::header::HeaderMap::new(),
                );
            }

            headers.insert("x-cache".to_string(), "miss".to_string());

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

#[derive(Deserialize)]
pub struct FetchArgs {
    pub url: String,
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    #[serde(default)]
    pub cache: Option<String>,
}

#[derive(Serialize)]
pub struct FetchResult {
    pub ok: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

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

pub async fn execute_fetch(
    client: reqwest::Client,
    request: FetchRequest,
) -> anyhow::Result<FetchResponse> {
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
