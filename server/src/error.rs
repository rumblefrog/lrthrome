use thiserror::Error;
#[derive(Debug, Error)]
pub enum LrthromeError {
    #[error("IO error {0}")]
    IoError(#[from] std::io::Error),

    #[error("Reqwest error {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Malformed payload")]
    MalformedPayload,

    #[error("Exceeded ratelimit")]
    Ratelimited,

    #[error("Mismatching protocol version, expected {expected}, received {received}")]
    VersionMismatch { expected: u8, received: u8 },

    #[error("Invalid message variant {0}")]
    InvalidMessageVariant(u8),

    #[error("Invalid net address {0}")]
    InvalidAddress(#[from] std::net::AddrParseError),

    #[error("Unable to parse int {0}")]
    InvalidInt(#[from] std::num::ParseIntError),

    #[error("Invalid CIDR {0}")]
    InvalidCIDR(#[from] cidr::NetworkParseError),

    #[error("Stream shutdown watch channel error {0}")]
    ShutdownWatchError(#[from] tokio::sync::watch::error::SendError<bool>),
}

impl LrthromeError {
    pub fn code(&self) -> u8 {
        match *self {
            LrthromeError::MalformedPayload => 0,
            LrthromeError::Ratelimited => 1,
            LrthromeError::VersionMismatch {
                expected: _,
                received: _,
            } => 2,
            LrthromeError::InvalidMessageVariant(_) => 3,
            _ => 255,
        }
    }
}

pub type LrthromeResult<T> = std::result::Result<T, LrthromeError>;
