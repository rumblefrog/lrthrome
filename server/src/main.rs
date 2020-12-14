#![feature(associated_type_bounds)]
#[macro_use]
extern crate log;

use std::env::var;

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
    env_logger::init();

    let config_loc = var("LRTHROME_CONFIG").unwrap_or("config.toml".into());

    let config: Config = toml::from_slice(&std::fs::read(config_loc)?)?;

    let mut sources = Sources::new();

    sources.register(Box::new(Remote::new(config.sources.remotes)));

    let mut lrthrome = Lrthrome::new(
        config.general.bind_address,
        sources,
        config.general.temper_interval,
    )
    .await?;

    lrthrome.up().await?;

    Ok(())
}
