use std::net::Ipv4Addr;
use std::{collections::HashMap, convert::TryFrom};

use nom::{IResult, Err, bytes::complete::{tag, take_while}, combinator::{map, map_res}, multi::count, number::complete::{le_u32, le_u8}, sequence::{pair, terminated, tuple}};
use nom::error::{ParseError, FromExternalError};

use crate::error::{LrthromeError, LrthromeResult, ProtocolError};

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
    /// Server public data will be transmitted to peer.
    Established = 0,

    /// Optional peer payload to server to identify or authenticate itself.
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
    rate_limit: u32,

    /// Number of entries within the lookup tree.
    tree_size: u32,

    /// Optional banner message
    banner: &'a str,
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
    ip_address: Ipv4Addr,

    /// Longest match prefixed for the IP address.
    prefix: Ipv4Addr,

    /// Prefix mask length.
    mask_len: u32,
}

/// Successful response indicating no result.
pub struct ResponseOkNotFound {
    /// IP address in which the result was not found.
    ip_address: Ipv4Addr,
}

/// Unsuccessful response.
/// This response is considered fatal, and peer should attempt at another time.
pub struct ResponseError<'a> {
    /// Corresponding error code for the message.
    /// Useful for peer-side handling of error.
    code: u8,

    /// Human facing error message.
    message: &'a str,
}

impl TryFrom<u8> for ProtocolVersion {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value != PROTOCOL_VERSION {
            return Err(ProtocolError::VersionMismatch{
                expected: PROTOCOL_VERSION,
                received: value,
            });
        }

        Ok(Self(value))
    }
}


impl Header {
    fn parse(input: &[u8]) -> IResult<&[u8], Header>  {
        let (input, protocol_version) = map_res(
            le_u8,
            ProtocolVersion::try_from,
        )(input)?;

        let (input, variant) = map_res(le_u8, Variant::try_from)(input)?;

        Ok((
            input,
            Header {
                protocol_version,
                variant,
            },
        ))
    }
}

impl TryFrom<u8> for Variant {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Variant::Established as u8 => Ok(Variant::Established),
            x if x == Variant::Identify as u8 => Ok(Variant::Identify),
            x if x == Variant::Request as u8 => Ok(Variant::Request),
            x if x == Variant::ResponseOkFound as u8 => Ok(Variant::ResponseOkFound),
            x if x == Variant::ResponseOkNotFound as u8 => Ok(Variant::ResponseOkNotFound),
            x if x == Variant::ResponseError as u8 => Ok(Variant::ResponseError),
            x => Err(ProtocolError::InvalidMessageVariant(x)),
        }
    }
}

impl<'n> Identify<'n> {
    fn parse(input: &'n [u8]) -> IResult<&'n [u8], Identify<'n>> {
        let (input, identification) = parse_cstring(input)?;

        Ok((input, Identify { identification }))
    }
}

impl<'n> Request<'n> {
    fn parse(input: &'n [u8]) -> IResult<&'n [u8], Request<'n>> {
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

fn parse_cstring(input: &[u8]) -> IResult<&[u8], &str> {
    map_res(
        terminated(take_while(|b| b != 0), tag([0])),
        std::str::from_utf8,
    )(input)
}

mod tests {
    use super::*;

    #[test]
    fn parse_valid_header() {
        let payload: &[u8] = &[PROTOCOL_VERSION, 0x00];

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
    fn parse_invalid_version_header() {
        let payload: &[u8] = &[
            0x64, 0x01,
        ];

        assert_ne!(payload[0], PROTOCOL_VERSION);

        let h = Header::parse(payload);

        assert!(h.is_err());
    }

    #[test]
    fn parse_invalid_variant_header() {
        let payload: &[u8] = &[
            PROTOCOL_VERSION, 0x64,
        ];

        let h = Header::parse(payload);

        assert!(h.is_err());
    }

    #[test]
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
