//! Central HTTP send wrapper that runs interceptors.

use pardus_debug::{Initiator, ResourceType};

use super::{InterceptAction, InterceptorManager, RequestContext};

/// Send an HTTP request through the interception layer.
///
/// If the manager has no interceptors, this is equivalent to `client.execute(req)`.
/// Otherwise, before-request interceptors are run first.
///
/// Returns `Ok(None)` if the request was blocked.
/// Returns `Ok(Some(response))` otherwise.
pub async fn send_intercepted(
    client: &reqwest::Client,
    mgr: &InterceptorManager,
    builder: reqwest::RequestBuilder,
    resource_type: ResourceType,
    initiator: Initiator,
    is_navigation: bool,
) -> anyhow::Result<Option<reqwest::Response>> {
    // Fast path: no interceptors registered
    if mgr.is_empty() {
        let response = builder.send().await?;
        return Ok(Some(response));
    }

    // Build the request to inspect it
    let request = builder.build()?;

    // Construct RequestContext from the built request
    let url = request.url().to_string();
    let method = request.method().to_string();
    let headers = request
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = request
        .body()
        .and_then(|b| b.as_bytes())
        .map(|b| b.to_vec());

    let mut ctx = RequestContext {
        url,
        method,
        headers,
        body,
        resource_type,
        initiator,
        is_navigation,
    };

    // Run before-request interceptors
    match mgr.run_before_request(&mut ctx).await {
        InterceptAction::Block => Ok(None),
        InterceptAction::Redirect(new_url) => {
            let response = client.get(&new_url).send().await?;
            Ok(Some(response))
        }
        InterceptAction::Mock(mock) => {
            // For mock responses, we need to construct a synthetic reqwest::Response.
            // Since reqwest doesn't expose a simple constructor, we create one via
            // http::Response and convert it.
            let http_response = http::Response::builder()
                .status(http::StatusCode::from_u16(mock.status).unwrap_or(http::StatusCode::OK))
                .body(mock.body)
                .unwrap_or_else(|_| {
                    http::Response::builder()
                        .status(http::StatusCode::OK)
                        .body(Vec::new())
                        .unwrap()
                });
            Ok(Some(reqwest::Response::from(http_response)))
        }
        // Continue or Modify — rebuild the request from the (potentially modified) context
        InterceptAction::Continue | InterceptAction::Modify(_) => {
            let mut new_builder = match ctx.method.as_str() {
                "POST" => client.post(&ctx.url),
                "PUT" => client.put(&ctx.url),
                "PATCH" => client.patch(&ctx.url),
                "DELETE" => client.delete(&ctx.url),
                "HEAD" => client.head(&ctx.url),
                _ => client.get(&ctx.url),
            };

            for (name, value) in &ctx.headers {
                new_builder = new_builder.header(name.as_str(), value.as_str());
            }

            if let Some(body) = ctx.body {
                new_builder = new_builder.body(body);
            }

            let response = new_builder.send().await?;
            Ok(Some(response))
        }
    }
}
