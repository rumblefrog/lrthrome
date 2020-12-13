use thiserror::Error;

#[derive(Debug, Error)]
pub enum LrthromeError {
    #[error("IO error {0}")]
    IoError(#[from] std::io::Error),

    #[error("Reqwest error {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Stream shutdown watch channel error {0}")]
    ShutdownWatchError(#[from] tokio::sync::watch::error::SendError<bool>),
}

pub type LrthromeResult<T> = std::result::Result<T, LrthromeError>;
