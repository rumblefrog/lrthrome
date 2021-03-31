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

use std::str::FromStr;

use async_trait::async_trait;

use reqwest::Client;

use cidr::Ipv4Cidr;

use crate::error::LrthromeResult;

use super::Fetcher;

pub struct Remote {
    endpoints: Vec<String>,
}

impl Remote {
    pub fn new(endpoints: Vec<String>) -> Self {
        Self { endpoints }
    }
}

#[async_trait]
impl Fetcher for Remote {
    // It is uncertain until the file is fetched again
    // Not all endpoints has E-tag to verify
    async fn has_update(&self) -> bool {
        true
    }

    async fn iterate_cidr(&self) -> LrthromeResult<Box<dyn Iterator<Item = Ipv4Cidr>>> {
        let client = Client::new();

        let mut cidrs = Vec::new();

        for endpoint in &self.endpoints {
            if let Ok(res) = client.get(endpoint).send().await {
                if let Ok(resp) = res.text().await {
                    for line in resp.lines() {
                        if let Ok(cidr) = Ipv4Cidr::from_str(line) {
                            cidrs.push(cidr);
                        }
                    }
                }
            }
        }

        Ok(Box::new(cidrs.into_iter()))
    }
}
