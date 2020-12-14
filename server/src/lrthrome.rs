use std::collections::HashMap;
use std::net::{Shutdown, SocketAddr};
use std::sync::Arc;
use std::time::Instant;

use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::watch::{channel, Sender};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::cache::Cache;
use crate::error::LrthromeResult;
use crate::sources::Sources;
use crate::protocol::{Request, Response};

pub struct Lrthrome {
    listener: TcpListener,

    cache: Arc<RwLock<Cache>>,

    clients: HashMap<SocketAddr, Client>,

    temper_sources: Sources,

    temper_interval: u64,
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
    ) -> LrthromeResult<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr).await?,
            cache: Arc::new(RwLock::new(Cache::new())),
            clients: HashMap::new(),
            temper_sources,
            temper_interval,
        })
    }

    fn connection_heartbeats(&self) -> LrthromeResult<()> {
        for (_, v) in &self.clients {
            if v.last_request.elapsed() > Duration::from_secs(10) {
                v.shutdown.send(true)?;
            }
        }

        Ok(())
    }

    fn handle_connection(
        &mut self,
        addr: SocketAddr,
        stream: TcpStream,
        c_tx: mpsc::Sender<Action>,
    ) {
        let (tx, mut rx) = channel(false);

        self.clients.insert(
            addr,
            Client {
                last_request: Instant::now(),
                shutdown: tx,
            },
        );

        tokio::spawn(async move {
            let mut buf = Vec::with_capacity(512);

            loop {
                select! {
                    _ = rx.changed() => {
                        stream.shutdown(Shutdown::Both).expect("Unable to shut down client stream");

                        if c_tx.send(Action::Shutdown(addr)).await.is_err() {
                            error!("Main receiver shutdown");
                        }
                    }
                    Ok(ready) = stream.readable() => {
                        if let Ok(n) = stream.try_read(&mut buf) {

                            if let Ok(req) = Request::new(&buf) {

                            }


                            if c_tx.send(Action::Request(addr)).await.is_err() {
                                error!("Main receiver shutdown");
                            }
                        }
                    }
                }
            }
        });
    }

    pub async fn up(&mut self) -> LrthromeResult<()> {
        self.connection_heartbeats()?;

        let (temper_tx, mut temper_rx) = channel(Instant::now());

        let temper_interval = self.temper_interval.clone();
        tokio::spawn(async move {
            temper_tx
                .send(Instant::now())
                .expect("Unable to send temper seq");

            sleep(Duration::from_secs(60 * temper_interval)).await;
        });

        let (c_tx, mut c_rx) = mpsc::channel(64);

        loop {
            select! {
                Ok(v) = temper_rx.changed() => {
                    let mut cache = self.cache.write().await;

                    cache.temper(&self.temper_sources).await?;
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
                    self.handle_connection(addr, stream, c_tx.clone());
                }
            }
        }
    }
}
