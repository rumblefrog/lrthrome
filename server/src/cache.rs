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
        // Create a new instance in order to purge prefixes that may not exist anymore
        self.0 = IpLookupTable::new();

        for source in sources.sources() {
            if !source.has_update().await {
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
