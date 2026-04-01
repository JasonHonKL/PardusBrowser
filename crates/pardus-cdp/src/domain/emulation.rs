use async_trait::async_trait;
use serde_json::Value;

use crate::domain::{method_not_found, CdpDomainHandler, DomainContext, HandleResult};
use crate::protocol::target::CdpSession;

pub struct EmulationDomain;

#[async_trait]
impl CdpDomainHandler for EmulationDomain {
    fn domain_name(&self) -> &'static str {
        "Emulation"
    }

    async fn handle(
        &self,
        method: &str,
        params: Value,
        _session: &mut CdpSession,
        _ctx: &DomainContext,
    ) -> HandleResult {
        match method {
            "setDeviceMetricsOverride" => {
                // Store viewport config - no renderer, so metadata-only.
                let _width = params["width"].as_u64().unwrap_or(1280);
                let _height = params["height"].as_u64().unwrap_or(720);
                let _scale = params["deviceScaleFactor"].as_f64().unwrap_or(1.0);
                HandleResult::Ack
            }
            "clearDeviceMetricsOverride" => HandleResult::Ack,
            "setUserAgentOverride" => {
                // Would need to rebuild reqwest client with new UA.
                // For now, just ack.
                let _ua = params["userAgent"].as_str().unwrap_or("");
                HandleResult::Ack
            }
            "setTouchEmulationEnabled" => HandleResult::Ack,
            _ => method_not_found("Emulation", method),
        }
    }
}
