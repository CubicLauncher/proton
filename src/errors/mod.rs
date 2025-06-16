use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtonError {
    #[error("Request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Version {0} not found.")]
    VersionNotFound(String),

    #[error("Filesystem error {0}")]
    IoError(#[from] io::Error),

    #[error("Hash mismatch")]
    HashMismatch,

    #[error("Concurrency Error")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Other error: {0}")]
    Other(String),
}
impl From<Box<dyn std::error::Error + Send + Sync>> for ProtonError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        ProtonError::Other(err.to_string())
    }
}
impl From<async_zip::error::ZipError> for ProtonError {
    fn from(err: async_zip::error::ZipError) -> Self {
        ProtonError::Other(format!("Zip extraction error: {}", err))
    }
}

