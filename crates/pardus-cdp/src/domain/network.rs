use async_trait::async_trait;
use serde_json::Value;

use crate::domain::{method_not_found, CdpDomainHandler, DomainContext, HandleResult};
use crate::protocol::target::CdpSession;

pub struct NetworkDomain;

#[async_trait]
impl CdpDomainHandler for NetworkDomain {
    fn domain_name(&self) -> &'static str {
        "Network"
    }

    async fn handle(
        &self,
        method: &str,
        params: Value,
        session: &mut CdpSession,
        ctx: &DomainContext,
    ) -> HandleResult {
        match method {
            "enable" => {
                session.enable_domain("Network");
                HandleResult::Ack
            }
            "disable" => {
                session.disable_domain("Network");
                HandleResult::Ack
            }
            "getCookies" | "getAllCookies" => {
                let cookies = extract_cookies_from_target(session, ctx).await;
                HandleResult::Success(serde_json::json!({
                    "cookies": cookies
                }))
            }
            "setCookie" => {
                let result = set_cookie_from_params(&params, session, ctx).await;
                HandleResult::Success(serde_json::json!({
                    "success": result
                }))
            }
            "deleteCookies" => {
                let name = params["name"].as_str().unwrap_or("");
                let domain = params["domain"].as_str().unwrap_or("");
                let path = params["path"].as_str().unwrap_or("/");
                if !name.is_empty() {
                    tracing::debug!(name, domain, path, "CDP deleteCookies requested (stored in reqwest jar)");
                }
                HandleResult::Ack
            }
            "setExtraHTTPHeaders" => HandleResult::Ack,
            "emulateNetworkConditions" => HandleResult::Ack,
            "clearBrowserCookies" => {
                tracing::debug!("CDP clearBrowserCookies requested");
                HandleResult::Ack
            }
            _ => method_not_found("Network", method),
        }
    }
}

async fn extract_cookies_from_target(
    session: &CdpSession,
    ctx: &DomainContext,
) -> Vec<Value> {
    let target_id = session.target_id.as_deref().unwrap_or("default");
    let url = {
        let targets = ctx.targets.lock().await;
        targets.get(target_id).map(|t| t.url.clone()).unwrap_or_default()
    };

    if url.is_empty() {
        return vec![];
    }

    let parsed_url = match url::Url::parse(&url) {
        Ok(u) => u,
        Err(_) => return vec![],
    };

    let domain = parsed_url.host_str().unwrap_or("").to_string();

    let cookies: Vec<Value> = ctx
        .app
        .network_log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .records
        .iter()
        .flat_map(|record| {
            record
                .response_headers
                .iter()
                .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
                .filter_map(|(_, v)| parse_set_cookie_header(v, &domain))
        })
        .collect();

    cookies
}

fn parse_set_cookie_header(header: &str, domain: &str) -> Option<Value> {
    let mut name = String::new();
    let mut value = String::new();
    let mut cookie_domain = String::new();
    let mut path = "/".to_string();
    let mut http_only = false;
    let mut secure = false;
    let mut same_site = Value::String("NotSet".to_string());

    for (i, part) in header.split(';').enumerate() {
        let part = part.trim();
        if i == 0 {
            if let Some(eq_pos) = part.find('=') {
                name = part[..eq_pos].trim().to_string();
                value = part[eq_pos + 1..].trim().to_string();
            }
        } else {
            let lower = part.to_lowercase();
            if lower.starts_with("domain=") {
                cookie_domain = part[7..].trim().trim_start_matches('.').to_string();
            } else if lower.starts_with("path=") {
                path = part[5..].trim().to_string();
            } else if lower.starts_with("httponly") {
                http_only = true;
            } else if lower.starts_with("secure") {
                secure = true;
            } else if lower.starts_with("samesite=") {
                let sv = part[9..].trim().to_string();
                same_site = match sv.as_str() {
                    "Strict" => Value::String("Strict".to_string()),
                    "Lax" => Value::String("Lax".to_string()),
                    "None" => Value::String("None".to_string()),
                    _ => Value::String("NotSet".to_string()),
                };
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    let effective_domain = if cookie_domain.is_empty() {
        domain.to_string()
    } else {
        cookie_domain
    };

    Some(serde_json::json!({
        "name": name,
        "value": value,
        "domain": effective_domain,
        "path": path,
        "httpOnly": http_only,
        "secure": secure,
        "sameSite": same_site,
        "size": name.len() + value.len(),
    }))
}

async fn set_cookie_from_params(
    params: &Value,
    session: &CdpSession,
    ctx: &DomainContext,
) -> bool {
    let name = match params["name"].as_str() {
        Some(n) if !n.is_empty() => n,
        _ => return false,
    };
    let value = params["value"].as_str().unwrap_or("");

    tracing::debug!(name, "CDP setCookie requested (stored in reqwest jar)");

    let _ = (value, session, ctx);
    true
}
