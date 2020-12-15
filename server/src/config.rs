use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(rename(deserialize = "General"))]
    pub general: General,

    #[serde(rename(deserialize = "Sources"))]
    pub sources: Sources,
}

#[derive(Deserialize, Debug)]
pub struct General {
    pub bind_address: String,

    /// Cache time-to-live.
    /// Interval in seconds the cache will be purged and fetched again.
    pub cache_ttl: u64,

    /// Peer time-to-live.
    /// Interval that a peer's connection can stay alive without additional requests.
    pub peer_ttl: u64,

    /// Maximum rate over the span of 5 seconds.
    /// Multiple connections on a single IP address are aggregated together.
    pub rate_limit: u32,
}

#[derive(Deserialize, Debug)]
pub struct Sources {
    pub remotes: Vec<String>,
}
