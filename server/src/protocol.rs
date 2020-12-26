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

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::net::Ipv4Addr;

use bytes::{BufMut, Bytes, BytesMut};

use nom::bytes::complete::{tag, take_while};
use nom::combinator::{map, map_res};
use nom::multi::count;
use nom::number::complete::{le_u32, le_u8};
use nom::sequence::{pair, terminated};
use nom::IResult;

use crate::error::LrthromeError;

pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, PartialEq)]
pub struct ProtocolVersion(u8);

#[derive(Debug, PartialEq)]
pub struct Header {
    /// Current protocol version.
    ///
    /// Version is checked to ensure proper parsing on both sides.
    pub protocol_version: ProtocolVersion,

    /// Message variant to indicate parsing procedure.
    /// Field is repr as u8 in networking.
    pub variant: Variant,
}

/// Message variants for parsing procedure hint.
///
/// It is entirely feasible to house two separate version of a variant,
/// on a single protocol version.
/// In that scenario, two variants of the same purpose and implementation would co-exist.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Variant {
    /// Acknowledgement of peer connection.
    ///
    /// Server public data will be transmitted to peer.
    Established = 0,

    /// Optional peer payload to server to identify or authenticate itself.
    ///
    /// Authentication may grant higher limits in the future.
    Identify = 1,

    /// Request to check ip address against tree.
    Request = 2,

    /// Successful response indicating a longest match was found.
    ResponseOkFound = 3,

    /// Successful response indicating no result.
    ResponseOkNotFound = 4,

    /// Unsuccessful response.
    /// This response is considered fatal, and peer should attempt at another time.
    ResponseError = 5,
}

/// Server public data transmitted to peers.
/// Peer should save and update this information upon receiving.
pub struct Established<'a> {
    /// Rate limit over the span of 5 seconds, allowing burst.
    pub rate_limit: u32,

    /// Number of entries within the lookup tree.
    pub tree_size: u32,

    /// Cache time-to-live.
    /// Interval in seconds the cache will be purged and fetched again.
    pub cache_ttl: u32,

    /// Peer time-to-live.
    /// Interval that a peer's connection can stay alive without additional requests.
    pub peer_ttl: u32,

    /// Optional banner message
    pub banner: &'a str,
}

/// Optional peer request to identify/authenticate.
pub struct Identify<'n> {
    /// Identification token.
    pub identification: &'n str,
}

/// Request to check ip address against the tree.
pub struct Request<'n> {
    /// IPv4 address to check the tree for
    pub ip_address: Ipv4Addr,

    /// Number of key value pairs to read
    pub meta_count: u8,

    /// Key-value pairs
    pub meta: HashMap<&'n str, &'n str>,
}

/// Successful response indicating a longest match was found.
pub struct ResponseOkFound {
    /// IP address in which the result was found.
    pub ip_address: Ipv4Addr,

    /// Longest match prefixed for the IP address.
    pub prefix: Ipv4Addr,

    /// Prefix mask length.
    pub mask_len: u32,
}

/// Successful response indicating no result.
pub struct ResponseOkNotFound {
    /// IP address in which the result was not found.
    pub ip_address: Ipv4Addr,
}

/// Unsuccessful response.
/// This response is considered fatal, and peer should attempt at another time.
pub struct ResponseError<'a> {
    /// Corresponding error code for the message.
    /// Useful for peer-side handling of error.
    pub code: u8,

    /// Human facing error message.
    pub message: &'a str,
}

impl TryFrom<u8> for ProtocolVersion {
    type Error = LrthromeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value != PROTOCOL_VERSION {
            return Err(LrthromeError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                received: value,
            });
        }

        Ok(Self(value))
    }
}

impl Header {
    pub fn parse(input: &[u8]) -> IResult<&[u8], Header> {
        let (input, protocol_version) = map_res(le_u8, ProtocolVersion::try_from)(input)?;

        let (input, variant) = map_res(le_u8, Variant::try_from)(input)?;

        Ok((
            input,
            Header {
                protocol_version,
                variant,
            },
        ))
    }

    pub fn new(variant: Variant) -> Self {
        Self {
            protocol_version: ProtocolVersion(PROTOCOL_VERSION),
            variant,
        }
    }

    pub fn to_bytes(&self) -> BytesMut {
        let mut buf = BytesMut::new();

        buf.put_u8(self.protocol_version.0);
        buf.put_u8(self.variant.clone() as u8);

        buf
    }
}

impl TryFrom<u8> for Variant {
    type Error = LrthromeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Variant::Established as u8 => Ok(Variant::Established),
            x if x == Variant::Identify as u8 => Ok(Variant::Identify),
            x if x == Variant::Request as u8 => Ok(Variant::Request),
            x if x == Variant::ResponseOkFound as u8 => Ok(Variant::ResponseOkFound),
            x if x == Variant::ResponseOkNotFound as u8 => Ok(Variant::ResponseOkNotFound),
            x if x == Variant::ResponseError as u8 => Ok(Variant::ResponseError),
            x => Err(LrthromeError::InvalidMessageVariant(x)),
        }
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl<'a> Established<'a> {
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = Header::new(Variant::Established).to_bytes();

        buf.put_u32_le(self.rate_limit);
        buf.put_u32_le(self.tree_size);
        buf.put_u32_le(self.cache_ttl);
        buf.put_u32_le(self.peer_ttl);
        buf.put_slice(self.banner.as_bytes());
        buf.put_u8(0);

        buf.freeze()
    }
}

impl<'n> Identify<'n> {
    pub fn parse(input: &'n [u8]) -> IResult<&'n [u8], Identify<'n>> {
        let (input, identification) = parse_cstring(input)?;

        Ok((input, Identify { identification }))
    }
}

impl<'n> Request<'n> {
    pub fn parse(input: &'n [u8]) -> IResult<&'n [u8], Request<'n>> {
        let (input, ip_address) = map(le_u32, Ipv4Addr::from)(input)?;
        let (input, meta_count) = le_u8(input)?;

        let (input, v) = count(pair(parse_cstring, parse_cstring), meta_count as usize)(input)?;

        Ok((
            input,
            Request {
                ip_address,
                meta_count,
                meta: v.into_iter().collect(),
            },
        ))
    }
}

impl ResponseOkFound {
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = Header::new(Variant::ResponseOkFound).to_bytes();

        buf.put_u32_le(u32::from(self.ip_address));
        buf.put_u32_le(u32::from(self.prefix));
        buf.put_u32_le(self.mask_len);

        buf.freeze()
    }
}

impl ResponseOkNotFound {
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = Header::new(Variant::ResponseOkNotFound).to_bytes();

        buf.put_u32_le(u32::from(self.ip_address));

        buf.freeze()
    }
}

impl<'a> ResponseError<'a> {
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = Header::new(Variant::ResponseError).to_bytes();

        buf.put_u8(self.code);
        buf.put_slice(self.message.as_bytes());
        buf.put_u8(0);

        buf.freeze()
    }
}

fn parse_cstring(input: &[u8]) -> IResult<&[u8], &str> {
    map_res(
        terminated(take_while(|b| b != 0), tag([0])),
        std::str::from_utf8,
    )(input)
}

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    #[rustfmt::skip]
    fn parse_valid_header() {
        let payload: &[u8] = &[
            PROTOCOL_VERSION, 0x00,
        ];

        let h = Header::parse(payload).unwrap();

        assert_eq!(
            h.1,
            Header {
                protocol_version: ProtocolVersion(1),
                variant: Variant::Established,
            }
        );
    }

    #[test]
    #[rustfmt::skip]
    fn parse_invalid_version_header() {
        let payload: &[u8] = &[
            0x64, 0x01,
        ];

        assert_ne!(payload[0], PROTOCOL_VERSION);

        let h = Header::parse(payload);

        assert!(h.is_err());
    }

    #[test]
    #[rustfmt::skip]
    fn parse_invalid_variant_header() {
        let payload: &[u8] = &[
            PROTOCOL_VERSION, 0x64,
        ];

        let h = Header::parse(payload);

        assert!(h.is_err());
    }

    #[test]
    #[rustfmt::skip]
    fn parse_valid_identify() {
        let payload: &[u8] = &[
            PROTOCOL_VERSION, Variant::Identify as u8,
            0x66, 0x69, 0x73, 0x68, 0x79, 0x00, // fishy
        ];

        let h = Header::parse(payload).unwrap();

        assert_eq!(h.1.variant, Variant::Identify);

        let i = Identify::parse(h.0).unwrap();

        assert_eq!(i.1.identification, "fishy");
    }

    #[test]
    #[rustfmt::skip]
    fn parse_valid_request() {
        let payload: &[u8] = &[
            PROTOCOL_VERSION, Variant::Request as u8,
            0x01, 0x01, 0x01, 0x01, // IP address
            0x02, // Meta count
            0x66, 0x6f, 0x6f, 0x00, // 0th pair's key

            0x57, 0x65, 0x20, 0x6c,
            0x69, 0x76, 0x65, 0x20,
            0x69, 0x6e, 0x20, 0x61,
            0x20, 0x74, 0x77, 0x69,
            0x6c, 0x69, 0x67, 0x68,
            0x74, 0x20, 0x77, 0x6f,
            0x72, 0x6c, 0x64, 0x00, // 0th pair's value

            0x62, 0x61, 0x72, 0x00, // 1th pair's key

            0x61, 0x6e, 0x64, 0x20,
            0x74, 0x68, 0x65, 0x72,
            0x65, 0x20, 0x61, 0x72,
            0x65, 0x20, 0x6e, 0x6f,
            0x20, 0x66, 0x72, 0x69,
            0x65, 0x6e, 0x64, 0x73,
            0x20, 0x61, 0x74, 0x20,
            0x64, 0x75, 0x73, 0x6b, 0x00, // 1th pair's value
        ];

        let h = Header::parse(payload).unwrap();

        assert_eq!(h.1.variant, Variant::Request);

        let r = Request::parse(h.0).unwrap();

        assert_eq!(r.1.ip_address, Ipv4Addr::new(1, 1, 1, 1));
        assert_eq!(r.1.meta_count, 2);
        assert_eq!(r.1.meta["foo"], "We live in a twilight world");
        assert_eq!(r.1.meta["bar"], "and there are no friends at dusk");
    }
}
