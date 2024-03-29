// Lrthrome - Fast and light TCP-server based IPv4 CIDR filter lookup server over minimal binary protocol, and memory footprint
// Copyright (C) 2021  rumblefrog
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio::select;
use tokio::sync::{mpsc, watch, RwLock};
use tokio::time::{sleep, Duration};
use tokio_stream::StreamExt;
use tokio_util::codec::{BytesCodec, Decoder, Framed};

use bytes::{Bytes, BytesMut};

use ratelimit_meter::{KeyedRateLimiter, GCRA};

use futures::sink::SinkExt;

use crate::error::LrthromeResult;
use crate::protocol::{
    Established, Header, Request, ResponseError, ResponseOkFound, ResponseOkNotFound, Variant,
};
use crate::sources::Sources;
use crate::{cache::Cache, error::LrthromeError};

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
    cache_ttl: u32,

    /// Peer time-to-live.
    ///
    /// The amount of time a peer is allowed to keep their connection open
    /// without making an additional request to refresh the timeout.
    peer_ttl: u32,

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

    /// Banner message sent to clients upon established.
    banner: String,
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
            cache_ttl: 86400,

            // Default peer time-to-live to 15 seconds.
            peer_ttl: 15,
            ratelimiter: KeyedRateLimiter::new(rate_limit, Duration::from_secs(5)),
            banner: "".to_string(),
            rate_limit,
            sources,
            rx,
        })
    }

    pub fn cache_ttl(&mut self, dur: u32) -> &mut Self {
        self.cache_ttl = dur;

        self
    }

    pub fn peer_ttl(&mut self, dur: u32) -> &mut Self {
        self.peer_ttl = dur;

        self
    }

    pub fn banner(&mut self, banner: String) -> &mut Self {
        self.banner = banner;

        self
    }

    /// Start the main event loop.
    ///
    /// Handles the connections as well as `Lrthrome`.rx events.
    pub async fn up(&mut self) -> LrthromeResult<()> {
        self.start_timers();
        self.temper_cache().await?;

        info!("Started processing connections");

        loop {
            select! {
                _ = tokio::signal::ctrl_c() => {
                    // Exit to main
                    return Ok(());
                }
                Ok((stream, addr)) = self.listener.accept() => {
                    let (tx_shutdown, rx_shutdown) = watch::channel(false);
                    let (tx_bytes, rx_bytes) = mpsc::unbounded_channel();

                    debug!("Peer has connected (addr = {})", addr);

                    let mut peer = PeerRegistry::new(tx_shutdown, tx_bytes);

                    let tree_size = {
                        let c = self.shared.cache.read().await;

                        c.len()
                    };

                    let payload = Established {
                        rate_limit: self.rate_limit.into(),
                        tree_size: tree_size as u32,
                        cache_ttl: self.cache_ttl,
                        peer_ttl: self.peer_ttl,
                        banner: &self.banner,
                    }.to_bytes();

                    Self::peer_send(&addr, &mut peer, payload);

                    self.peers.insert(addr, peer);
                    self.process_peer(Peer::new(addr, stream, rx_shutdown, rx_bytes));
                }
                Some(message) = self.rx.recv() => {
                    match message {
                        Message::CacheTick => self.temper_cache().await?,
                        Message::PeerTick => self.sweep_peers()?,
                        Message::PeerFrame(addr, buf) => {
                            debug!("Received peer frame (addr = {}) (length = {})", addr, buf.len());

                            if let Err(e) = self.process_frame(addr, buf.as_ref()).await {
                                if let Some(peer) = self.peers.get_mut(&addr) {
                                    Self::peer_error(&addr, peer, e);
                                    self.cleanup();
                                }
                            }
                        },
                        Message::PeerDisconnected(addr) => {
                            debug!("Peer has disconnected (addr = {})", addr);

                            self.peers.remove(&addr);
                        }
                    }
                }
            }
        }
    }

    #[inline]
    async fn process_frame(&mut self, addr: SocketAddr, frame: &[u8]) -> LrthromeResult<()> {
        let (frame, header) = Header::parse(frame).map_err(|_| LrthromeError::MalformedPayload)?;

        debug!(
            "Received peer frame (type = {}) (addr = {})",
            header.variant.to_string(),
            addr
        );

        match header.variant {
            Variant::Identify => {
                // Unused ATM
                // let (_, identify) = Identify::parse(frame).map_err(|_| LrthromeError::MalformedPayload)?;
            }
            Variant::Request => {
                let (_, request) =
                    Request::parse(frame).map_err(|_| LrthromeError::MalformedPayload)?;

                if let Some(peer) = self.peers.get_mut(&addr) {
                    if self.ratelimiter.check(addr.ip()).is_err() {
                        warn!("Peer exceeded ratelimit (addr = {})", addr);

                        return Err(LrthromeError::Ratelimited);
                    }

                    peer.last_request = Instant::now();

                    let longest_match = {
                        let c = self.shared.cache.read().await;

                        c.longest_match(request.ip_address)

                        // Read guard dropped here
                    };

                    let resp = match longest_match {
                        Some(m) => {
                            info!(
                                "{} found in range of {}/{} ({:?}) (addr = {})",
                                request.ip_address, m.0, m.1, request.meta, addr,
                            );

                            ResponseOkFound {
                                ip_address: request.ip_address,
                                prefix: m.0,
                                mask_len: m.1,
                            }
                        }
                        .to_bytes(),
                        None => ResponseOkNotFound {
                            ip_address: request.ip_address,
                        }
                        .to_bytes(),
                    };

                    Self::peer_send(&addr, peer, resp);
                }
            }
            _ => (),
        }

        Ok(())
    }

    fn peer_error(addr: &SocketAddr, peer: &mut PeerRegistry, error: LrthromeError) {
        let resp = ResponseError {
            code: error.code(),
            message: &error.to_string(),
        }
        .to_bytes();

        Self::peer_send(&addr, peer, resp);
        Self::shutdown_peer(peer, &addr);
    }

    fn peer_send(addr: &SocketAddr, peer: &mut PeerRegistry, payload: Bytes) {
        if let Err(e) = peer.tx_bytes.send(payload) {
            error!("Unable to send payload to peer (addr = {}): {}", addr, e);
        }
    }

    fn shutdown_peer(peer: &mut PeerRegistry, addr: &SocketAddr) {
        if let Err(e) = peer.tx_shutdown.send(true) {
            error!("Unable to shutdown peer (addr = {}): {}", addr, e);
        }
    }

    fn cleanup(&mut self) {
        self.ratelimiter.cleanup(Duration::from_secs(60));
    }

    async fn temper_cache(&mut self) -> LrthromeResult<()> {
        let mut c = self.shared.cache.write().await;

        c.temper(&self.sources).await?;

        Ok(())
    }

    fn sweep_peers(&mut self) -> LrthromeResult<()> {
        for c in self.peers.values() {
            if c.last_request.elapsed() > Duration::from_secs(self.peer_ttl as u64) {
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
                        break;
                    }
                    Some(bytes) = peer.rx_bytes.recv() => {
                        if let Err(e) = peer.frame.send(bytes).await {
                            error!("Unable to send bytes to {}: {}", peer.addr, e);
                        }
                    }
                    frame = peer.frame.next() => {
                        match frame {
                            Some(message) => {
                                match message {
                                    Ok(buf) => {
                                        let _ = shared.tx.send(Message::PeerFrame(peer.addr, buf));
                                    },
                                    Err(_) => {
                                        break;
                                    }
                                }
                            },
                            None => {
                                break;
                            }
                        }
                    }
                }
            }

            // Peer has no more frames, declare disconnect.
            let _ = shared.tx.send(Message::PeerDisconnected(peer.addr));

            // Exiting this future will drop peer, dropping the connection
        });
    }

    /// Starts background timers.
    ///
    /// Peer & Cache TTL timers will initialize here.
    fn start_timers(&mut self) {
        let shared = self.shared.clone();
        let cache_ttl = Duration::from_secs(self.cache_ttl as u64);

        tokio::spawn(async move {
            loop {
                sleep(cache_ttl).await;

                if let Err(e) = shared.tx.send(Message::CacheTick) {
                    error!("Unable to send cache tick: {0}", e);
                }
            }
        });

        let shared = self.shared.clone();
        let peer_ttl = Duration::from_secs(self.peer_ttl as u64);

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
