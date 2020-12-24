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
        .peer_ttl(config.general.peer_ttl);

    info!("Lrthrome started");

    lrthrome.up().await?;

    info!("Lrthrome shutting down");

    Ok(())
}
