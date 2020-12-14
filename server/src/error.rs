use thiserror::Error;

#[derive(Debug, Error)]
pub enum LrthromeError {
    #[error("IO error {0}")]
    IoError(#[from] std::io::Error),

    #[error("Reqwest error {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Mismatching protocol version, expected {0}, received {1}")]
    ProtocolMismatch(u8, u8),

    #[error("Invalid net address {0}")]
    InvalidAddress(#[from] std::net::AddrParseError),

    #[error("Unable to parse int {0}")]
    InvalidInt(#[from] std::num::ParseIntError),

    #[error("Invalid CIDR {0}")]
    InvalidCIDR(#[from] cidr::NetworkParseError),

    #[error("Stream shutdown watch channel error {0}")]
    ShutdownWatchError(#[from] tokio::sync::watch::error::SendError<bool>),
}

pub type LrthromeResult<T> = std::result::Result<T, LrthromeError>;
