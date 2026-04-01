use serde::Serialize;

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const SERVER_ERROR: i64 = -32000;

#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("Parse error")]
    ParseError,
    #[error("Invalid request")]
    InvalidRequest,
    #[error("'{0}' wasn't found")]
    MethodNotFound(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("{0}")]
    ServerError(String),
}

impl CdpError {
    pub fn code(&self) -> i64 {
        match self {
            CdpError::ParseError => PARSE_ERROR,
            CdpError::InvalidRequest => INVALID_REQUEST,
            CdpError::MethodNotFound(_) => METHOD_NOT_FOUND,
            CdpError::InvalidParams(_) => INVALID_PARAMS,
            CdpError::ServerError(_) => SERVER_ERROR,
        }
    }
}

#[derive(Serialize)]
pub struct CdpErrorBody {
    pub code: i64,
    pub message: String,
}

impl From<&CdpError> for CdpErrorBody {
    fn from(err: &CdpError) -> Self {
        Self {
            code: err.code(),
            message: err.to_string(),
        }
    }
}
