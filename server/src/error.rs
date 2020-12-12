use thiserror::Error;

#[derive(Debug, Error)]
pub enum LrthromeError {
    #[error("IO error {0}")]
    IoError(#[from] std::io::Error),
}

pub type LrthromeResult<T> = std::result::Result<T, LrthromeError>;
