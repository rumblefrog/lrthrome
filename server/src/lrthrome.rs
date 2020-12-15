use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::select;
use tokio::stream::StreamExt;
use tokio::sync::{mpsc, watch, RwLock};
use tokio::time::{sleep, Duration};
use tokio_util::codec::{BytesCodec, Decoder, Framed};

use bytes::{Bytes, BytesMut};

use ratelimit_meter::{KeyedRateLimiter, GCRA};

use futures::sink::SinkExt;

use crate::cache::Cache;
use crate::error::LrthromeResult;
use crate::protocol::{Request, Response};
use crate::sources::Sources;

pub struct Lrthrome {
    /// TCP listener bind for the lrthrome server.
    listener: TcpListener,

    /// Shared data between peers and the server.
    ///
    /// Only cache field maintain RwLock, as it's the only field mutable
    shared: Arc<Shared>,

    /// Mapping of peer socket address to peer structure.
    ///
    /// The key is cleared as soon as peer disconnects.
    ///
    /// There could be multiple peers per IP address.
    peers: HashMap<SocketAddr, PeerRegistry>,

    /// Main event loop receiver.
    ///
    /// Operates on cache feedback & peer updates
    rx: mpsc::UnboundedReceiver<Message>,

    /// Structure containing compile-time registered sources,
    /// with data populated at run-time from the config file.
    ///
    /// Temper will utilize the sources to refresh its cache.
    sources: Sources,

    /// Cache time-to-live.
    ///
    /// The amount of time between temperance.
    cache_ttl: Duration,

    /// Peer time-to-live.
    ///
    /// The amount of time a peer is allowed to keep their connection open
    /// without making an additional request to refresh the timeout.
    peer_ttl: Duration,

    /// Ratelimiter for individual IP address.
    ///
    /// Note that the key is `IpAddr` rather than SocketAddr.
    /// As the ratelimit applies globally to a single address,
    /// shared between the IP address's connections.
    ratelimiter: KeyedRateLimiter<IpAddr, GCRA>,

    /// Rate limit meter for IP address.
    ///
    /// Peer that exceeds this will be force disconnected.
    rate_limit: NonZeroU32,
}

/// Enum of message variants & data,
/// in which is passed to the main thread and computed.
enum Message {
    /// Upon repeating timer of `cache_ttl`.
    CacheTick,

    /// Upon repeating timer of `peer_ttl`.
    PeerTick,

    PeerFrame(SocketAddr, BytesMut),

    /// Upon peer disconnect or force disconnect.
    PeerDisconnected(SocketAddr),
}

/// Data structures that's shared between peers and the server.
///
/// Only cache retains RwLock for mutability
struct Shared {
    /// IPv4 Radix cache tree.
    ///
    /// Will be write-locked when tempered
    cache: RwLock<Cache>,

    /// Main event loop sender.
    ///
    /// This will be cloned to peers.
    /// Used by peers to send message back to main thread.
    tx: mpsc::UnboundedSender<Message>,
}

struct PeerRegistry {
    /// Instant of the last request.
    ///
    /// Used to compare to the duration of `peer_ttl` for force-disconnecting peers.
    last_request: Instant,

    /// Peer shutdown sender channel.
    ///
    /// Will drop connection once sent.
    tx_shutdown: watch::Sender<bool>,

    /// Peer sending channel.
    ///
    /// For main thread to pass information back to the `Peer`
    tx_bytes: mpsc::UnboundedSender<Bytes>,
}

struct Peer {
    /// Socket address identifier.
    addr: SocketAddr,

    /// Wrap the TcpStream around bytes allows chunked based level operation
    /// rather than raw bytes.
    frame: Framed<TcpStream, BytesCodec>,

    /// Peer shutdown receiver channel.
    ///
    /// Will drop connection if received.
    rx_shutdown: watch::Receiver<bool>,

    /// Peer receiving channel.
    ///
    /// This is used to receive bytes to write to `Peer`'s socket
    rx_bytes: mpsc::UnboundedReceiver<Bytes>,
}

impl Lrthrome {
    pub async fn new<A>(addr: A, sources: Sources, rate_limit: NonZeroU32) -> LrthromeResult<Self>
    where
        A: ToSocketAddrs,
    {
        let (tx, rx) = mpsc::unbounded_channel();

        Ok(Self {
            listener: TcpListener::bind(addr).await?,
            shared: Arc::new(Shared::new(tx)),
            peers: HashMap::new(),

            // Default cache time-to-live to 24 hours.
            cache_ttl: Duration::from_secs(86400),

            // Default peer time-to-live to 15 seconds.
            peer_ttl: Duration::from_secs(15),
            ratelimiter: KeyedRateLimiter::new(rate_limit, Duration::from_secs(5)),
            rate_limit,
            sources,
            rx,
        })
    }

    #[allow(dead_code)]
    pub fn cache_ttl(&mut self, dur: Duration) {
        self.cache_ttl = dur;
    }

    #[allow(dead_code)]
    pub fn peer_ttl(&mut self, dur: Duration) {
        self.peer_ttl = dur;
    }

    /// Start the main event loop.
    ///
    /// Handles the connections as well as `Lrthrome`.rx events.
    pub async fn up(&mut self) -> LrthromeResult<()> {
        self.start_timers();
        self.temper_cache().await?;

        loop {
            select! {
                _ = tokio::signal::ctrl_c() => {
                    // Exit to main
                    return Ok(());
                }
                Ok((stream, addr)) = self.listener.accept() => {
                    let (tx_shutdown, rx_shutdown) = watch::channel(false);
                    let (tx_bytes, rx_bytes) = mpsc::unbounded_channel();

                    self.peers.insert(addr, PeerRegistry::new(tx_shutdown, tx_bytes));

                    self.process_peer(Peer::new(addr, stream, rx_shutdown, rx_bytes));
                }
                Some(message) = self.rx.recv() => {
                    match message {
                        Message::CacheTick => self.temper_cache().await?,
                        Message::PeerTick => self.sweep_peers()?,
                        Message::PeerFrame(addr, buf) => {
                            match Request::new(buf.as_ref()) {
                                Ok(req) => {
                                    if let Some(peer) = self.peers.get_mut(&addr) {
                                        // Peer reached ratelimit, disconnect
                                        if self.ratelimiter.check(addr.ip()).is_err() {
                                            Self::shutdown_peer(peer, &addr);

                                            continue;
                                        }

                                        peer.last_request = Instant::now();

                                        let c = self.shared.cache.read().await;

                                        let resp = Response {
                                            in_filter: c.exist(req.ip_address),
                                            limit: self.rate_limit.get() as u8,
                                            ip_address: req.ip_address,
                                        };

                                        if let Err(e) = peer.tx_bytes.send(resp.to_buf()) {
                                            error!("Unable to send response to {}: {}", addr, e);
                                        }
                                    }
                                },
                                Err(_) => {
                                    if let Some(peer) = self.peers.get_mut(&addr) {
                                        Self::shutdown_peer(peer, &addr)
                                    }
                                }
                            }
                        },
                        Message::PeerDisconnected(addr) => {
                            self.peers.remove(&addr);
                        }
                    }
                }
            }
        }
    }

    fn shutdown_peer(peer: &mut PeerRegistry, addr: &SocketAddr) {
        if let Err(e) = peer.tx_shutdown.send(true) {
            error!("Unable to shutdown peer {}: {}", addr, e);
        }
    }

    async fn temper_cache(&mut self) -> LrthromeResult<()> {
        let mut c = self.shared.cache.write().await;

        c.temper(&self.sources).await?;

        Ok(())
    }

    fn sweep_peers(&mut self) -> LrthromeResult<()> {
        for c in self.peers.values() {
            if c.last_request.elapsed() > self.peer_ttl {
                c.tx_shutdown.send(true)?;
            }
        }

        Ok(())
    }

    fn process_peer(&mut self, peer: Peer) {
        let shared = self.shared.clone();

        let mut peer = peer;
        tokio::spawn(async move {
            loop {
                select! {
                    _ = peer.rx_shutdown.changed() => {
                        let _ = shared.tx.send(Message::PeerDisconnected(peer.addr));

                        // Exiting this function will drop peer, dropping the connection
                        return;
                    }
                    Some(bytes) = peer.rx_bytes.recv() => {
                        if let Err(e) = peer.frame.send(bytes).await {
                            error!("Unable to send bytes to {}: {}", peer.addr, e);
                        }
                    }
                    Some(message) = peer.frame.next() => {
                        match message {
                            Ok(buf) => {
                                let _ = shared.tx.send(Message::PeerFrame(peer.addr, buf));
                            },
                            Err(_) => {
                                let _ = shared.tx.send(Message::PeerDisconnected(peer.addr));

                                return;
                            }
                        }
                    }
                }
            }
        });
    }

    /// Starts background timers.
    ///
    /// Peer & Cache TTL timers will initialize here.
    fn start_timers(&mut self) {
        let shared = self.shared.clone();
        let cache_ttl = self.cache_ttl;

        tokio::spawn(async move {
            loop {
                sleep(cache_ttl).await;

                if let Err(e) = shared.tx.send(Message::CacheTick) {
                    error!("Unable to send cache tick: {0}", e);
                }
            }
        });

        let shared = self.shared.clone();
        let peer_ttl = self.peer_ttl;

        tokio::spawn(async move {
            loop {
                sleep(peer_ttl).await;

                if let Err(e) = shared.tx.send(Message::PeerTick) {
                    error!("Unable to send cache tick: {0}", e);
                }
            }
        });
    }
}

impl Shared {
    pub fn new(tx: mpsc::UnboundedSender<Message>) -> Self {
        Self {
            cache: RwLock::new(Cache::new()),
            tx,
        }
    }
}

impl PeerRegistry {
    pub fn new(tx_shutdown: watch::Sender<bool>, tx_bytes: mpsc::UnboundedSender<Bytes>) -> Self {
        Self {
            last_request: Instant::now(),
            tx_shutdown,
            tx_bytes,
        }
    }
}

impl Peer {
    pub fn new(
        addr: SocketAddr,
        stream: TcpStream,
        rx_shutdown: watch::Receiver<bool>,
        rx_bytes: mpsc::UnboundedReceiver<Bytes>,
    ) -> Self {
        Self {
            addr,
            frame: BytesCodec::new().framed(stream),
            rx_shutdown,
            rx_bytes,
        }
    }
}
