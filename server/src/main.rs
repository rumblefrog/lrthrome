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

#[macro_use]
extern crate log;

use std::env::var;
use std::num::NonZeroU32;

use env_logger::Env;

mod cache;
mod config;
mod error;
mod lrthrome;
mod protocol;
mod sources;

use config::Config;
use lrthrome::Lrthrome;
use sources::{Remote, Sources};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let el_env = Env::default().filter_or("LRTHROME_LOG_LEVEL", "info");

    env_logger::init_from_env(el_env);

    let config_loc = var("LRTHROME_CONFIG").unwrap_or_else(|_| "config.toml".into());

    let config: Config = toml::from_slice(&std::fs::read(config_loc)?)?;

    let mut sources = Sources::new();

    sources.register(Box::new(Remote::new(config.sources.remotes)));

    let mut lrthrome = Lrthrome::new(
        config.general.bind_address,
        sources,
        NonZeroU32::new(config.general.rate_limit).unwrap(),
    )
    .await?;

    lrthrome
        .cache_ttl(config.general.cache_ttl)
        .peer_ttl(config.general.peer_ttl)
        .banner(config.general.banner);

    info!("Lrthrome started");

    lrthrome.up().await?;

    info!("Lrthrome shutting down");

    Ok(())
}
