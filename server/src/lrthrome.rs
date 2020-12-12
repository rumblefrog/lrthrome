use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

use crate::cache::Cache;
use crate::error::LrthromeResult;

pub struct Lrthrome {
    listener: TcpListener,

    cache: Cache,

    streams: Vec<(TcpStream, SocketAddr)>,
}

impl Lrthrome {
    pub async fn new<A: ToSocketAddrs>(addr: A) -> LrthromeResult<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
            cache: Cache::new(),
            streams: Vec::new(),
        })
    }

    pub async fn up(&mut self) -> LrthromeResult<()> {
        loop {
            if let Ok(v) = self.listener.accept().await {
                self.streams.push(v);

                // Dispatch thread for processing
            }
        }
    }
}
