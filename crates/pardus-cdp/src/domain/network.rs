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
        _params: Value,
        session: &mut CdpSession,
        _ctx: &DomainContext,
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
                // Cookies would come from a session store if available.
                // For now return empty.
                HandleResult::Success(serde_json::json!({
                    "cookies": []
                }))
            }
            "setCookie" => {
                HandleResult::Success(serde_json::json!({
                    "success": true
                }))
            }
            "deleteCookies" => HandleResult::Ack,
            "setExtraHTTPHeaders" => HandleResult::Ack,
            "emulateNetworkConditions" => HandleResult::Ack,
            _ => method_not_found("Network", method),
        }
    }
}
