use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("walk error: {0}")]
    Walk(#[from] walkdir::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type BridgeResult<T> = Result<T, BridgeError>;
