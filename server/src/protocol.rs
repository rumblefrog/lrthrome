use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::net::Ipv4Addr;

use bytes::{BufMut, Bytes, BytesMut};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::error::{LrthromeError, LrthromeResult};

pub const PROTOCOL_VERSION: u8 = 1;

// 01
// 44 B4 08 16
// 02
// 66 6f 6f 00 62 61 72 00
pub struct Request {
    pub protocol_version: u8,

    pub ip_address: Ipv4Addr,

    // meta_count: u8,
    pub meta: HashMap<String, String>, //key=value,
}

impl Request {
    pub fn new(data: &[u8]) -> LrthromeResult<Self> {
        let mut cursor = Cursor::new(data);

        let protocol_version = cursor.read_u8()?;

        if protocol_version != PROTOCOL_VERSION {
            return Err(LrthromeError::ProtocolMismatch(
                PROTOCOL_VERSION,
                protocol_version,
            ));
        }

        let ip_address = Ipv4Addr::from(cursor.read_u32::<LittleEndian>()?);

        let meta_count = cursor.read_u8()?;

        let mut meta: HashMap<String, String> = HashMap::new();

        for _ in 0..=meta_count {
            meta.insert(cursor.read_cstring()?, cursor.read_cstring()?);
        }

        Ok(Request {
            protocol_version,
            ip_address,
            meta,
        })
    }
}

pub struct Response {
    // protocol_version
    pub in_filter: bool, // 1

    pub rate_limit: u32, // 1

    pub ip_address: Ipv4Addr, // 4
}

impl Response {
    pub fn to_buf(&self) -> Bytes {
        let mut buf = BytesMut::new();

        buf.put_u8(PROTOCOL_VERSION);
        buf.put_u8(self.in_filter as u8);
        buf.put_u32_le(self.rate_limit);
        buf.put_u32_le(self.ip_address.into());

        buf.freeze()
    }
}

trait ReadCString {
    fn read_cstring(&mut self) -> LrthromeResult<String>;
}

impl ReadCString for Cursor<&[u8]> {
    fn read_cstring(&mut self) -> LrthromeResult<String> {
        let end = self.get_ref().len() as u64;
        let mut buf = [0; 1];
        let mut str_vec = Vec::with_capacity(256);

        while self.position() < end {
            self.read_exact(&mut buf)?;
            if buf[0] == 0 {
                break;
            } else {
                str_vec.push(buf[0]);
            }
        }

        Ok(String::from_utf8_lossy(&str_vec[..]).into_owned())
    }
}
