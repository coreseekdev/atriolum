use thiserror::Error;

#[derive(Error, Debug)]
pub enum IngestError {
    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("payload too large: {0}")]
    PayloadTooLarge(String),

    #[error("decompression error: {0}")]
    Decompression(String),

    #[error("invalid envelope: {0}")]
    InvalidEnvelope(String),

    #[error("protocol error: {0}")]
    Protocol(#[from] atriolum_protocol::ProtocolError),

    #[error("store error: {0}")]
    Store(#[from] atriolum_store::StoreError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
