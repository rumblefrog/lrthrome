use std::net::Ipv4Addr;

use cidr::Cidr;
use treebitmap::IpLookupTable;

use crate::error::LrthromeResult;
use crate::sources::Sources;

/// Wrapper around prefix tree structure.
///
/// Includes convenient methods for tempering and existence check.
pub struct Cache(IpLookupTable<Ipv4Addr, bool>);

impl Cache {
    pub fn new() -> Self {
        Self(IpLookupTable::new())
    }

    pub fn longest_match(&self, addr: Ipv4Addr) -> Option<(Ipv4Addr, u32)> {
        self.0.longest_match(addr).map(|i| (i.0, i.1))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub async fn temper(&mut self, sources: &Sources) -> LrthromeResult<()> {
        for source in sources.sources() {
            if !source.has_update() {
                continue;
            }

            let iter = source.iterate_cidr().await?;

            for cidr in iter {
                self.0
                    .insert(cidr.first_address(), cidr.network_length() as u32, true);
            }
        }

        let mem_usage = self.0.mem_usage();

        info!(
            "Lookup table size: (node: {}) (results: {})",
            mem_usage.0, mem_usage.1
        );

        Ok(())
    }
}
