use std::io;
use std::path::PathBuf;
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
    #[error("Invalid library name: '{0}'")]
    InvalidLibraryName(String),
    #[error("Library not found at path: {0}")]
    LibraryNotFound(PathBuf),
    #[error("Invalid Maven coordinate: '{0}'")]
    InvalidMavenCoordinate(String),
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
