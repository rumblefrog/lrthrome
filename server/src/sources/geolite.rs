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
use std::str::FromStr;

use async_trait::async_trait;

use cidr::Ipv4Cidr;

use csv::Reader;

use crate::config::GeoLite as GeoLiteConfig;
use crate::error::LrthromeResult;

use super::Fetcher;

pub struct GeoLite {
    asn_path: String,
    geo_paths: [String; 2],

    // Combine city & country geoname ids, O(1) lookup.
    geoname_ids: HashMap<String, ()>,
    // ASN is kept separate in event of duplicate key.
    asns: HashMap<String, ()>,
}

impl GeoLite {
    pub fn new(config: GeoLiteConfig) -> Self {
        let asn_path = config.asn.database_path;
        let geo_paths = [config.city.database_path, config.country.database_path];

        let asns = {
            let mut t = HashMap::new();

            for id in config.asn.asns {
                t.insert(id.to_string(), ());
            }

            t
        };

        let geoname_ids = {
            let mut t = HashMap::new();

            let geos = [config.city.cities, config.country.countries];

            let ids: Vec<&u32> = geos.iter().flat_map(|s| s.iter()).collect();

            for id in ids {
                t.insert(id.to_string(), ());
            }

            t
        };

        Self {
            asn_path,
            geo_paths,
            geoname_ids,
            asns,
        }
    }
}

#[async_trait]
impl Fetcher for GeoLite {
    // Re-read each database as database file may auto-updating.
    async fn has_update(&self) -> bool {
        true
    }

    async fn iterate_cidr(&self) -> LrthromeResult<Box<dyn Iterator<Item = Ipv4Cidr>>> {
        let mut cidrs = Vec::new();

        for geo in self.geo_paths.iter() {
            match Reader::from_path(geo) {
                Ok(mut r) => {
                    for result in r.records() {
                        let record = result?;

                        if let Some(geo_id) = record.get(1) {
                            if self.geoname_ids.contains_key(geo_id) {
                                if let Some(network) = record.get(0) {
                                    if let Ok(cidr) = Ipv4Cidr::from_str(network) {
                                        cidrs.push(cidr);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => warn!("Unable to open {}. Skipped.", geo),
            }
        }

        match Reader::from_path(&self.asn_path) {
            Ok(mut r) => {
                for result in r.records() {
                    let record = result?;

                    if let Some(asn) = record.get(1) {
                        if self.asns.contains_key(asn) {
                            if let Some(network) = record.get(0) {
                                if let Ok(cidr) = Ipv4Cidr::from_str(network) {
                                    cidrs.push(cidr);
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => warn!("Unable to open {}. Skipped.", self.asn_path),
        }

        Ok(Box::new(cidrs.into_iter()))
    }
}
