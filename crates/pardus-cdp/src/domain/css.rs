use async_trait::async_trait;
use serde_json::Value;

use crate::domain::{method_not_found, CdpDomainHandler, DomainContext, HandleResult};
use crate::protocol::target::CdpSession;

pub struct CssDomain;

#[async_trait]
impl CdpDomainHandler for CssDomain {
    fn domain_name(&self) -> &'static str {
        "CSS"
    }

    async fn handle(
        &self,
        method: &str,
        params: Value,
        _session: &mut CdpSession,
        ctx: &DomainContext,
    ) -> HandleResult {
        match method {
            "getComputedStyleForNode" => {
                // No renderer - return empty computed styles.
                HandleResult::Success(serde_json::json!({
                    "computedStyle": []
                }))
            }
            "getInlineStylesForNode" => {
                let node_id = params["nodeId"].as_i64().unwrap_or(-1);
                let nm = ctx.node_map.lock().await;
                let _selector = nm.get_selector(node_id).map(|s| s.to_string());
                drop(nm);

                // ElementHandle doesn't expose style directly - return empty.
                HandleResult::Success(serde_json::json!({
                    "inlineStyle": {
                        "cssProperties": [],
                        "shorthandEntries": [],
                        "styleSheetId": format!("inline-{}", node_id),
                    }
                }))
            }
            "getMatchedStylesForNode" => {
                HandleResult::Success(serde_json::json!({
                    "matchedCSSRules": [],
                    "inlineStyle": null,
                }))
            }
            _ => method_not_found("CSS", method),
        }
    }
}
