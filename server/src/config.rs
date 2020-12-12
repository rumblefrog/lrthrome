use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub general: General,

    pub sources: Sources,
}

#[derive(Deserialize)]
pub struct General {
    pub bind_address: String,
}

#[derive(Deserialize)]
pub struct Sources {
    pub remotes: Vec<String>,
}
