use async_trait::async_trait;

use cidr::Ipv4Cidr;

use crate::error::LrthromeResult;

mod remote;

pub use remote::Remote;

#[async_trait]
pub trait Fetcher {
    fn has_update(&self) -> bool;

    async fn iterate_cidr(&self) -> LrthromeResult<Box<dyn Iterator<Item = Ipv4Cidr>>>;
}

pub struct Sources {
    sources: Vec<Box<dyn Fetcher>>,
}

impl Sources {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn register(&mut self, source: Box<dyn Fetcher>) {
        self.sources.push(source);
    }

    pub fn sources(&self) -> &Vec<Box<dyn Fetcher>> {
        &self.sources
    }
}
