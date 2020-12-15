use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub general: General,

    pub sources: Sources,
}

#[derive(Deserialize, Debug)]
pub struct General {
    pub bind_address: String,

    /// Temper interval in minutes to update cache from sources
    pub temper_interval: u64,

    /// Maximum rate limit per 5 seconds
    pub rate_limit: u32,
}

#[derive(Deserialize, Debug)]
pub struct Sources {
    pub remotes: Vec<String>,
}
