use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::net::Ipv4Addr;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::error::{LrthromeError, LrthromeResult};

pub const PROTOCOL_VERSION: u8 = 1;

// 01
// 44 B4 08 16
// 02
// 66 6f 6f 00 62 61 72 00
pub struct Request {
    protocol_version: u8,

    ip_address: Ipv4Addr,

    // meta_count: u8,
    meta: HashMap<String, String>, //key=value,
}

impl Request {
    pub fn new(data: &Vec<u8>) -> LrthromeResult<Self> {
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
    protocol_version: u8, // 1

    in_filter: bool, // 1

    limit: u8, // 1

    ip_address: Ipv4Addr, // 4
}

impl Response {
    pub fn to_vec(&self) -> LrthromeResult<Vec<u8>> {
        let p = Vec::with_capacity(7);
        let mut payload = Cursor::new(p);

        payload.write_u8(PROTOCOL_VERSION)?;
        payload.write_u8(((self.in_filter as u8) << 7) | self.limit)?;
        payload.write_u32::<LittleEndian>(self.ip_address.into())?;

        Ok(payload.into_inner())
    }
}

trait ReadCString {
    fn read_cstring(&mut self) -> LrthromeResult<String>;
}

impl ReadCString for Cursor<&Vec<u8>> {
    fn read_cstring(&mut self) -> LrthromeResult<String> {
        let end = self.get_ref().len() as u64;
        let mut buf = [0; 1];
        let mut str_vec = Vec::with_capacity(256);

        while self.position() < end {
            self.read(&mut buf)?;
            if buf[0] == 0 {
                break;
            } else {
                str_vec.push(buf[0]);
            }
        }

        Ok(String::from_utf8_lossy(&str_vec[..]).into_owned())
    }
}
