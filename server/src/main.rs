#[macro_use]
extern crate log;

use std::env::var;
use std::num::NonZeroU32;

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

    let config_loc = var("LRTHROME_CONFIG").unwrap_or_else(|_| "config.toml".into());

    let config: Config = toml::from_slice(&std::fs::read(config_loc)?)?;

    let mut sources = Sources::new();

    sources.register(Box::new(Remote::new(config.sources.remotes)));

    let mut lrthrome = Lrthrome::new(
        config.general.bind_address,
        sources,
        NonZeroU32::new(10).unwrap(),
    )
    .await?;

    info!("Lrthrome started");

    lrthrome.up().await?;

    Ok(())
}
