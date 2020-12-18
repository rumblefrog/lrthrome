use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::net::Ipv4Addr;

use crate::error::{LrthromeError, ProtocolError, LrthromeResult};

pub const PROTOCOL_VERSION: u8 = 1;

pub struct Header {
    /// Current protocol version.
    ///
    /// Version is checked to ensure proper parsing on both sides.
    protocol_version: u8,

    /// Message variant to indicate parsing procedure.
    /// Field is repr as u8 in networking.
    variant: Variant,
}

/// Message variants for parsing procedure hint.
///
/// It is entirely feasible to house two separate version of a variant,
/// on a single protocol version.
/// In that scenario, two variants of the same purpose and implementation would co-exist.
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
pub struct Established {
    /// Rate limit over the span of 5 seconds, allowing burst.
    rate_limit: u32,

    /// Number of entries within the lookup tree.
    tree_size: u32,

    /// Optional banner message
    banner: String,
}

/// Optional peer request to identify/authenticate.
pub struct Identify {
    /// Identification token.
    identification: String,
}

/// Request to check ip address against the tree.
pub struct Request {
    /// IPv4 address to check the tree for
    ip_address: Ipv4Addr,

    /// Number of key value pairs to read
    meta_count: u8,

    /// Key-value pairs
    meta: HashMap<String, String>,
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
pub struct ResponseError {
    /// Corresponding error code for the message.
    /// Useful for peer-side handling of error.
    code: u8,

    /// Human facing error message.
    message: String,
}
