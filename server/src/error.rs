// Lrthrome - Fast and light TCP-server based IPv4 CIDR filter lookup server over minimal binary protocol, and memory footprint
// Copyright (C) 2021  rumblefrog
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use thiserror::Error;
#[derive(Debug, Error)]
pub enum LrthromeError {
    #[error("IO error {0}")]
    IoError(#[from] std::io::Error),

    #[error("Reqwest error {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("CSV error {0}")]
    CsvError(#[from] csv::Error),

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
    InvalidCidr(#[from] cidr::NetworkParseError),

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
