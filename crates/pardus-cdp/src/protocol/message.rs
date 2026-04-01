use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CdpErrorBody;

/// Incoming CDP command from a client.
#[derive(Deserialize, Debug)]
pub struct CdpRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Successful CDP response.
#[derive(Serialize)]
pub struct CdpResponse {
    pub id: u64,
    pub result: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// CDP event pushed from server to client.
#[derive(Serialize, Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// CDP error response.
#[derive(Serialize)]
pub struct CdpErrorResponse {
    pub id: u64,
    pub error: CdpErrorBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}
