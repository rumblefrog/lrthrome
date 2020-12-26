// Lrthrome - Fast and light TCP-server based IPv4 CIDR filter lookup server over minimal binary protocol, and memory footprint
// Copyright (C) 2021  rumblefrog

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use async_trait::async_trait;

use cidr::Ipv4Cidr;

use crate::error::LrthromeResult;

mod remote;

pub use remote::Remote;

#[async_trait]
pub trait Fetcher {
    /// Check if fetcher has update available.
    ///
    /// If false, the fetcher will be skipped
    async fn has_update(&self) -> bool;

    async fn iterate_cidr(&self) -> LrthromeResult<Box<dyn Iterator<Item = Ipv4Cidr>>>;
}

pub struct Sources {
    sources: Vec<Box<dyn Fetcher>>,
}

impl Sources {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn register(&mut self, source: Box<dyn Fetcher>) {
        self.sources.push(source);
    }

    pub fn sources(&self) -> &Vec<Box<dyn Fetcher>> {
        &self.sources
    }
}
