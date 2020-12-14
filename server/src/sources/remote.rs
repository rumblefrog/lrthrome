use std::str::FromStr;

use async_trait::async_trait;

use reqwest::Client;

use cidr::Ipv4Cidr;

use crate::error::LrthromeResult;

use super::Fetcher;

pub struct Remote {
    endpoints: Vec<String>,
}

impl Remote {
    pub fn new(endpoints: Vec<String>) -> Self {
        Self { endpoints }
    }
}

#[async_trait]
impl Fetcher for Remote {
    // It is uncertain until the file is fetched again
    // Not all endpoints has E-tag to verify
    fn has_update(&self) -> bool {
        true
    }

    async fn iterate_cidr(&self) -> LrthromeResult<Box<dyn Iterator<Item = Ipv4Cidr>>> {
        let client = Client::new();

        let mut cidrs = Vec::new();

        for endpoint in &self.endpoints {
            let resp = client.get(endpoint).send().await?.text().await?;

            for line in resp.lines().into_iter() {
                if let Ok(cidr) = Ipv4Cidr::from_str(line) {
                    cidrs.push(cidr);
                }
            }
        }

        Ok(Box::new(cidrs.into_iter()))
    }
}
