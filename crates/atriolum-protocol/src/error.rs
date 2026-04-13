use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("invalid authentication: {0}")]
    InvalidAuth(String),

    #[error("invalid DSN: {0}")]
    InvalidDsn(String),

    #[error("invalid envelope: {0}")]
    InvalidEnvelope(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
