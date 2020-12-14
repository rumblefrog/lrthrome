use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::select;
use tokio::stream::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::watch::{channel, Sender};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use tokio_util::codec::{BytesCodec, Decoder};

use futures::sink::SinkExt;

use crate::cache::Cache;
use crate::error::LrthromeResult;
use crate::protocol::{Request, Response};
use crate::sources::Sources;

pub struct Lrthrome {
    listener: TcpListener,

    cache: Arc<RwLock<Cache>>,

    clients: HashMap<SocketAddr, Client>,

    temper_sources: Sources,

    temper_interval: u64,

    client_ttl: u64,
}

struct Client {
    last_request: Instant,

    shutdown: Sender<bool>,
}

enum Action {
    Request(SocketAddr),
    Shutdown(SocketAddr),
}

impl Lrthrome {
    pub async fn new<A: ToSocketAddrs>(
        addr: A,
        temper_sources: Sources,
        temper_interval: u64,
        client_ttl: u64,
    ) -> LrthromeResult<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
            cache: Arc::new(RwLock::new(Cache::new())),
            clients: HashMap::new(),
            temper_sources,
            temper_interval,
            client_ttl,
        })
    }

    fn connection_heartbeats(&self) -> LrthromeResult<()> {
        for (_, v) in &self.clients {
            if v.last_request.elapsed() > Duration::from_secs(self.client_ttl) {
                v.shutdown.send(true)?;
            }
        }

        Ok(())
    }

    fn handle_connection(
        &mut self,
        addr: SocketAddr,
        stream: TcpStream,
        c_tx: mpsc::UnboundedSender<Action>,
    ) {
        let (tx, mut rx) = channel(false);

        self.clients.insert(
            addr,
            Client {
                last_request: Instant::now(),
                shutdown: tx,
            },
        );

        let cache = self.cache.clone();
        tokio::spawn(async move {
            let mut framed = BytesCodec::new().framed(stream);

            loop {
                select! {
                    _ = rx.changed() => {
                        let _ = c_tx.send(Action::Shutdown(addr));

                        break;
                    }
                    Some(message) = framed.next() => {
                        match message {
                            Ok(buf) => {
                                if let Ok(req) = Request::new(buf.as_ref()) {
                                    let c = cache.read().await;

                                    let resp = Response {
                                        in_filter: c.exist(req.ip_address),
                                        limit: 0,
                                        ip_address: req.ip_address,
                                    }.to_buf();

                                    if let Err(e) = framed.send(resp).await {
                                        error!("Unable to send response {0}", e);
                                    }
                                }

                                let _ = c_tx.send(Action::Request(addr));
                            }
                            Err(_) => {
                                let _ = c_tx.send(Action::Shutdown(addr));

                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    async fn temper_cache(&mut self) -> LrthromeResult<()> {
        let mut cache = self.cache.write().await;

        cache.temper(&self.temper_sources).await?;

        Ok(())
    }

    pub async fn up(&mut self) -> LrthromeResult<()> {
        self.temper_cache().await?;

        let (heartbeat_tx, mut heartbeat_rx) = channel(Instant::now());
        let client_ttl = self.client_ttl.clone();
        tokio::spawn(async move {
            loop {
                heartbeat_tx
                    .send(Instant::now())
                    .expect("Unable to send heartbeat seq");

                sleep(Duration::from_secs(client_ttl)).await;
            }
        });

        let (temper_tx, mut temper_rx) = channel(Instant::now());
        let temper_interval = self.temper_interval.clone();
        tokio::spawn(async move {
            loop {
                temper_tx
                    .send(Instant::now())
                    .expect("Unable to send temper seq");

                sleep(Duration::from_secs(60 * temper_interval)).await;
            }
        });

        let (c_tx, mut c_rx) = mpsc::unbounded_channel();

        loop {
            select! {
                Ok(_) = temper_rx.changed() => {
                    self.temper_cache().await?;
                }
                Ok(_) = heartbeat_rx.changed() => {
                    self.connection_heartbeats()?;
                }
                Some(action) = c_rx.recv() => {
                    match action {
                        Action::Request(addr) => {
                            if let Some(v) = self.clients.get_mut(&addr) {
                                v.last_request = Instant::now();
                            }
                        },
                        Action::Shutdown(addr) => {
                            self.clients.remove(&addr);
                        }
                    }
                }
                Ok((stream, addr)) = self.listener.accept() => {
                    info!("Client connected {}", addr);

                    self.handle_connection(addr, stream, c_tx.clone());
                }
            }
        }
    }
}
