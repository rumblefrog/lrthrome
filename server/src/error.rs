use thiserror::Error;

#[derive(Debug, Error)]
pub enum LrthromeError {
    #[error("IO error {0}")]
    IoError(#[from] std::io::Error),

    #[error("Reqwest error {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Mismatching protocol version, expected {0}, received {1}")]
    ProtocolMismatch(u8, u8),

    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("Invalid net address {0}")]
    InvalidAddress(#[from] std::net::AddrParseError),

    #[error("Unable to parse int {0}")]
    InvalidInt(#[from] std::num::ParseIntError),

    #[error("Invalid CIDR {0}")]
    InvalidCIDR(#[from] cidr::NetworkParseError),

    #[error("Stream shutdown watch channel error {0}")]
    ShutdownWatchError(#[from] tokio::sync::watch::error::SendError<bool>),
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Mismatching protocol version, expected {expected}, received {received}")]
    VersionMismatch {
        expected: u8,
        received: u8,
    },

    #[error("Invalid message variant {0}")]
    InvalidMessageVariant(u8),
}

pub type LrthromeResult<T> = std::result::Result<T, LrthromeError>;
