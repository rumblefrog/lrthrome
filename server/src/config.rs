use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub general: General,

    pub sources: Sources,
}

#[derive(Deserialize)]
pub struct General {
    pub bind_address: String,

    /// Temper interval in minutes to update cache from sources
    pub temper_interval: u64,
}

#[derive(Deserialize)]
pub struct Sources {
    pub remotes: Vec<String>,
}
