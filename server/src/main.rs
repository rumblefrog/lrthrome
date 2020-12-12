use std::env::var;

mod error;
mod config;
mod sources;
mod cache;
mod lrthrome;

use config::Config;
use lrthrome::Lrthrome;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_loc = var("LRTHROME_CONFIG")
        .unwrap_or("config.toml".into());

    let config: Config = toml::from_slice(&std::fs::read(config_loc)?)?;

    let lrthrome = Lrthrome::new(config.general.bind_address).await?;

    Ok(())
}
