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

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    #[serde(rename(deserialize = "General"))]
    pub general: General,

    #[serde(rename(deserialize = "Sources"))]
    pub sources: Sources,
}

#[derive(Deserialize)]
pub struct General {
    pub bind_address: String,

    /// Cache time-to-live.
    /// Interval in seconds the cache will be purged and fetched again.
    pub cache_ttl: u32,

    /// Peer time-to-live.
    /// Interval that a peer's connection can stay alive without additional requests.
    pub peer_ttl: u32,

    /// Maximum rate over the span of 5 seconds.
    /// Multiple connections on a single IP address are aggregated together.
    pub rate_limit: u32,

    /// Banner message sent to clients upon established.
    pub banner: String,
}

#[derive(Deserialize)]
pub struct Sources {
    pub remotes: Vec<String>,

    #[serde(rename = "GeoLite")]
    pub geolite: GeoLite,
}

#[derive(Deserialize)]
pub struct GeoLite {
    #[serde(rename = "ASN")]
    pub asn: GeoLiteAsn,

    #[serde(rename = "City")]
    pub city: GeoLiteCity,

    #[serde(rename = "Country")]
    pub country: GeoLiteCountry,
}

#[derive(Deserialize)]
pub struct GeoLiteAsn {
    pub database_path: String,

    pub asns: Vec<u32>,
}

#[derive(Deserialize)]
pub struct GeoLiteCity {
    pub database_path: String,

    pub cities: Vec<u32>,
}

#[derive(Deserialize)]
pub struct GeoLiteCountry {
    pub database_path: String,

    pub countries: Vec<u32>,
}
