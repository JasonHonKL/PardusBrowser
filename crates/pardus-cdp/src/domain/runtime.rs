use async_trait::async_trait;
use serde_json::Value;

use crate::domain::{method_not_found, CdpDomainHandler, DomainContext, HandleResult};
use crate::protocol::message::CdpEvent;
use crate::protocol::target::CdpSession;

pub struct RuntimeDomain;

fn resolve_target_id(session: &CdpSession) -> &str {
    session.target_id.as_deref().unwrap_or("default")
}

#[async_trait]
impl CdpDomainHandler for RuntimeDomain {
    fn domain_name(&self) -> &'static str {
        "Runtime"
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
                session.enable_domain("Runtime");
                let target_id = resolve_target_id(session);
                let origin = {
                    let targets = ctx.targets.lock().await;
                    targets.get(target_id).map(|t| t.url.clone()).unwrap_or_default()
                };
                let ctx_id = session.create_execution_context(origin, "".to_string());
                let _ = ctx.event_bus.send(CdpEvent {
                    method: "Runtime.executionContextCreated".to_string(),
                    params: serde_json::json!({
                        "context": {
                            "id": ctx_id,
                            "origin": "",
                            "name": "",
                            "auxData": { "isDefault": true, "type": "default" }
                        }
                    }),
                    session_id: Some(session.session_id.clone()),
                });
                HandleResult::Ack
            }
            "disable" => {
                for ec in &session.execution_contexts {
                    let _ = ctx.event_bus.send(CdpEvent {
                        method: "Runtime.executionContextDestroyed".to_string(),
                        params: serde_json::json!({ "executionContextId": ec.id }),
                        session_id: Some(session.session_id.clone()),
                    });
                }
                session.execution_contexts.clear();
                session.disable_domain("Runtime");
                HandleResult::Ack
            }
            "evaluate" => {
                let expression = params["expression"].as_str().unwrap_or("");
                let result = evaluate_expression(expression, session, ctx).await;
                HandleResult::Success(result)
            }
            "callFunctionOn" => {
                let function = params["functionDeclaration"].as_str().unwrap_or("");
                let result = evaluate_expression(function, session, ctx).await;
                HandleResult::Success(result)
            }
            "getProperties" => {
                HandleResult::Success(serde_json::json!({ "result": [] }))
            }
            _ => method_not_found("Runtime", method),
        }
    }
}

async fn evaluate_expression(
    expression: &str,
    session: &CdpSession,
    ctx: &DomainContext,
) -> Value {
    if expression.is_empty() {
        return serde_json::json!({
            "result": { "type": "undefined" }
        });
    }

    let target_id = resolve_target_id(session);
    let (base_url, has_js) = {
        let targets = ctx.targets.lock().await;
        match targets.get(target_id) {
            Some(entry) => (
                entry.url.clone(),
                entry.js_enabled,
            ),
            None => (String::new(), false),
        }
    };

    if has_js {
        let _ = base_url;
        serde_json::json!({
            "result": {
                "type": "undefined",
                "description": "JS execution not available in this build"
            }
        })
    } else {
        serde_json::json!({
            "result": {
                "type": "undefined",
                "description": "JS execution not enabled for this target"
            }
        })
    }
}
