use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum ProtonError {
    #[error("Request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Version {0} not found.")]
    VersionNotFound(String),

    #[error("Filesystem error {0}")]
    IoError(#[from] io::Error),

    #[error("Hash mismatch")]
    HashMismatch
}
