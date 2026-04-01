use async_trait::async_trait;
use serde_json::Value;

use crate::domain::{method_not_found, CdpDomainHandler, DomainContext, HandleResult};
use crate::protocol::target::CdpSession;

pub struct InputDomain;

fn resolve_target_id(session: &CdpSession) -> &str {
    session.target_id.as_deref().unwrap_or("default")
}

#[async_trait]
impl CdpDomainHandler for InputDomain {
    fn domain_name(&self) -> &'static str {
        "Input"
    }

    async fn handle(
        &self,
        method: &str,
        params: Value,
        session: &mut CdpSession,
        ctx: &DomainContext,
    ) -> HandleResult {
        let target_id = resolve_target_id(session);

        match method {
            "dispatchMouseEvent" => {
                let mouse_type = params["type"].as_str().unwrap_or("");
                if mouse_type == "mousePressed" {
                    // Best-effort: check if interactive elements exist.
                    // Actual click execution is handled via the Pardus.interact domain,
                    // since Page is !Send and cannot be held across await points.
                    let _has_elements = match (ctx.get_html(target_id), ctx.get_url(target_id)) {
                        (Some(html_str), Some(url)) => {
                            let page = pardus_core::Page::from_html(&html_str, &url);
                            !page.interactive_elements().is_empty()
                        }
                        _ => false,
                    };
                }
                HandleResult::Ack
            }
            "dispatchKeyEvent" => {
                let _key = params["key"].as_str().unwrap_or("");
                HandleResult::Ack
            }
            "insertText" => {
                let text = params["text"].as_str().unwrap_or("");
                if !text.is_empty() {
                    if let (Some(html_str), Some(url)) = (ctx.get_html(target_id), ctx.get_url(target_id)) {
                        // Find a text input to type into.
                        // type_text is synchronous, so Page doesn't cross an await.
                        let page = pardus_core::Page::from_html(&html_str, &url);
                        if let Some(el) = page.query("input[type='text'], input:not([type]), textarea") {
                            let _ = pardus_core::App::type_text(&page, &el, text);
                        }
                    }
                }
                HandleResult::Ack
            }
            _ => method_not_found("Input", method),
        }
    }
}
