use serde_json::Value;

use crate::domain::{DomainContext, HandleResult};
use crate::error::{CdpError, CdpErrorBody};
use crate::protocol::message::{CdpErrorResponse, CdpRequest, CdpResponse};
use crate::protocol::registry::DomainRegistry;
use crate::protocol::target::CdpSession;

pub struct CdpRouter {
    registry: DomainRegistry,
}

impl CdpRouter {
    pub fn new(registry: DomainRegistry) -> Self {
        Self { registry }
    }

    pub async fn route(
        &self,
        request: CdpRequest,
        session: &mut CdpSession,
        ctx: &DomainContext,
    ) -> Result<CdpResponse, CdpErrorResponse> {
        let (domain, method) = split_method(&request.method).map_err(|e| CdpErrorResponse {
            id: request.id,
            error: CdpErrorBody::from(&e),
            session_id: request.session_id.clone(),
        })?;

        let handler = self.registry.get(domain).ok_or_else(|| {
            let err = CdpError::MethodNotFound(request.method.clone());
            CdpErrorResponse {
                id: request.id,
                error: CdpErrorBody::from(&err),
                session_id: request.session_id.clone(),
            }
        })?;

        match handler.handle(method, request.params, session, ctx).await {
            HandleResult::Success(result) => Ok(CdpResponse {
                id: request.id,
                result,
                session_id: request.session_id,
            }),
            HandleResult::Error(err) => {
                let err_with_id = CdpErrorResponse {
                    id: request.id,
                    ..err
                };
                Err(err_with_id)
            }
            HandleResult::Ack => Ok(CdpResponse {
                id: request.id,
                result: Value::Object(serde_json::Map::new()),
                session_id: request.session_id,
            }),
        }
    }
}

fn split_method(method: &str) -> Result<(&str, &str), CdpError> {
    method
        .split_once('.')
        .ok_or_else(|| CdpError::InvalidRequest)
}
