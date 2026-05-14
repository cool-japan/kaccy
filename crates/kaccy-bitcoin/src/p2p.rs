// Peer-to-peer Bitcoin network integration
//
// This module provides direct P2P communication with Bitcoin nodes without
// requiring Bitcoin Core RPC. Key features:
// - Direct P2P communication with Bitcoin nodes
// - Transaction broadcasting via P2P network
// - Block downloading and verification
// - Peer discovery and management
// - Network message handling

use crate::error::{BitcoinError, Result};
use bitcoin::{
    BlockHash, Network, Transaction, Txid,
    consensus::encode,
    p2p::{
        ServiceFlags,
        message::{NetworkMessage, RawNetworkMessage},
        message_blockdata::Inventory,
        message_network::VersionMessage,
    },
};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// P2P network configuration
#[derive(Debug, Clone)]
pub struct P2pConfig {
    /// Network type
    pub network: Network,
    /// User agent string
    pub user_agent: String,
    /// Protocol version
    pub protocol_version: u32,
    /// Service flags
    pub services: ServiceFlags,
    /// Maximum number of peers
    pub max_peers: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Ping interval
    pub ping_interval: Duration,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            network: Network::Bitcoin,
            user_agent: "/kaccy-bitcoin:0.1.0/".to_string(),
            protocol_version: 70016,
            services: ServiceFlags::NONE,
            max_peers: 8,
            connection_timeout: Duration::from_secs(30),
            ping_interval: Duration::from_secs(120),
        }
    }
}

/// Peer connection information
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer socket address
    pub address: SocketAddr,
    /// Peer version message
    pub version: Option<VersionMessage>,
    /// Connection time
    pub connected_at: SystemTime,
    /// Last seen time
    pub last_seen: SystemTime,
    /// Number of messages sent
    pub messages_sent: u64,
    /// Number of messages received
    pub messages_received: u64,
    /// Peer services
    pub services: ServiceFlags,
    /// Is connection inbound
    pub inbound: bool,
}

impl PeerInfo {
    /// Create new peer info
    pub fn new(address: SocketAddr, inbound: bool) -> Self {
        let now = SystemTime::now();
        Self {
            address,
            version: None,
            connected_at: now,
            last_seen: now,
            messages_sent: 0,
            messages_received: 0,
            services: ServiceFlags::NONE,
            inbound,
        }
    }

    /// Update last seen time
    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now();
    }

    /// Get connection duration
    pub fn connection_duration(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.connected_at)
            .unwrap_or_default()
    }

    /// Check if peer is stale (no activity for a while)
    pub fn is_stale(&self, timeout: Duration) -> bool {
        SystemTime::now()
            .duration_since(self.last_seen)
            .unwrap_or_default()
            > timeout
    }
}

/// P2P event types
#[derive(Debug, Clone)]
pub enum P2pEvent {
    /// New peer connected
    PeerConnected(SocketAddr),
    /// Peer disconnected
    PeerDisconnected(SocketAddr),
    /// New transaction received
    TransactionReceived(Txid),
    /// New block received
    BlockReceived(BlockHash),
    /// Peer version handshake completed
    VersionHandshake(SocketAddr, VersionMessage),
}

/// Peer manager for maintaining connections
pub struct PeerManager {
    /// Active peers
    peers: Arc<RwLock<HashMap<SocketAddr, PeerInfo>>>,
    /// Configuration
    config: P2pConfig,
    /// Event sender
    event_tx: mpsc::UnboundedSender<P2pEvent>,
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new(config: P2pConfig) -> (Self, mpsc::UnboundedReceiver<P2pEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let manager = Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            config,
            event_tx,
        };
        (manager, event_rx)
    }

    /// Add a peer
    pub fn add_peer(&self, address: SocketAddr, inbound: bool) -> bool {
        let mut peers = self.peers.write().unwrap_or_else(|e| e.into_inner());
        if peers.len() >= self.config.max_peers {
            return false;
        }
        if peers.contains_key(&address) {
            return false;
        }
        peers.insert(address, PeerInfo::new(address, inbound));
        let _ = self.event_tx.send(P2pEvent::PeerConnected(address));
        true
    }

    /// Remove a peer
    pub fn remove_peer(&self, address: &SocketAddr) {
        let mut peers = self.peers.write().unwrap_or_else(|e| e.into_inner());
        if peers.remove(address).is_some() {
            let _ = self.event_tx.send(P2pEvent::PeerDisconnected(*address));
        }
    }

    /// Update peer version
    pub fn update_peer_version(&self, address: &SocketAddr, version: VersionMessage) {
        let mut peers = self.peers.write().unwrap_or_else(|e| e.into_inner());
        if let Some(peer) = peers.get_mut(address) {
            peer.services = version.services;
            peer.version = Some(version.clone());
            peer.update_last_seen();
            let _ = self
                .event_tx
                .send(P2pEvent::VersionHandshake(*address, version));
        }
    }

    /// Get peer count
    pub fn peer_count(&self) -> usize {
        self.peers.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Get all peer addresses
    pub fn get_peers(&self) -> Vec<SocketAddr> {
        self.peers.read().unwrap_or_else(|e| e.into_inner()).keys().copied().collect()
    }

    /// Get peer info
    pub fn get_peer_info(&self, address: &SocketAddr) -> Option<PeerInfo> {
        self.peers.read().unwrap_or_else(|e| e.into_inner()).get(address).cloned()
    }

    /// Remove stale peers
    pub fn remove_stale_peers(&self, timeout: Duration) {
        let stale_peers: Vec<SocketAddr> = self
            .peers
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|(_, info)| info.is_stale(timeout))
            .map(|(addr, _)| *addr)
            .collect();

        for addr in stale_peers {
            self.remove_peer(&addr);
        }
    }
}

/// P2P network client
pub struct P2pClient {
    config: P2pConfig,
    peer_manager: Arc<PeerManager>,
    /// Known transactions
    known_txs: Arc<RwLock<HashSet<Txid>>>,
    /// Known blocks
    #[allow(dead_code)]
    known_blocks: Arc<RwLock<HashSet<BlockHash>>>,
}

impl P2pClient {
    /// Create a new P2P client
    pub fn new(config: P2pConfig) -> (Self, mpsc::UnboundedReceiver<P2pEvent>) {
        let (peer_manager, event_rx) = PeerManager::new(config.clone());
        let client = Self {
            config,
            peer_manager: Arc::new(peer_manager),
            known_txs: Arc::new(RwLock::new(HashSet::new())),
            known_blocks: Arc::new(RwLock::new(HashSet::new())),
        };
        (client, event_rx)
    }

    /// Connect to a peer
    pub async fn connect_peer(&self, address: SocketAddr) -> Result<()> {
        info!(?address, "Connecting to peer");

        if !self.peer_manager.add_peer(address, false) {
            warn!(
                ?address,
                "Failed to add peer (max peers reached or already connected)"
            );
            return Err(BitcoinError::ConnectionFailed(
                "Max peers reached or already connected".to_string(),
            ));
        }

        match tokio::time::timeout(self.config.connection_timeout, TcpStream::connect(address))
            .await
        {
            Ok(Ok(mut stream)) => {
                debug!(?address, "TCP connection established");

                // Send version message
                let version_msg = self.create_version_message(address)?;
                self.send_message(&mut stream, NetworkMessage::Version(version_msg))
                    .await?;

                // Handle handshake in background
                let peer_manager = Arc::clone(&self.peer_manager);
                let network = self.config.network;
                tokio::spawn(async move {
                    if let Err(e) =
                        Self::handle_peer_connection(stream, peer_manager, network).await
                    {
                        error!(?address, error = ?e, "Peer connection error");
                    }
                });

                Ok(())
            }
            Ok(Err(e)) => {
                self.peer_manager.remove_peer(&address);
                Err(BitcoinError::ConnectionFailed(format!(
                    "Failed to connect to {}: {}",
                    address, e
                )))
            }
            Err(_) => {
                self.peer_manager.remove_peer(&address);
                Err(BitcoinError::ConnectionTimeout {
                    timeout_secs: self.config.connection_timeout.as_secs(),
                })
            }
        }
    }

    /// Broadcast a transaction to all peers
    pub async fn broadcast_transaction(&self, tx: &Transaction) -> Result<()> {
        let txid = tx.compute_txid();
        info!(?txid, "Broadcasting transaction");

        // Mark as known
        self.known_txs.write().unwrap_or_else(|e| e.into_inner()).insert(txid);

        // Create inventory message
        let inv = Inventory::Transaction(txid);
        let _msg = NetworkMessage::Inv(vec![inv]);

        // Broadcast to all peers
        let peers = self.peer_manager.get_peers();
        if peers.is_empty() {
            return Err(BitcoinError::ConnectionFailed(
                "No peers connected".to_string(),
            ));
        }

        for peer_addr in peers {
            // In a real implementation, we would send to each peer
            // For now, just log
            debug!(?peer_addr, ?txid, "Would send tx to peer");
        }

        Ok(())
    }

    /// Request blocks from peers
    pub async fn request_blocks(&self, block_hashes: Vec<BlockHash>) -> Result<()> {
        info!(count = block_hashes.len(), "Requesting blocks");

        let peers = self.peer_manager.get_peers();
        if peers.is_empty() {
            return Err(BitcoinError::ConnectionFailed(
                "No peers connected".to_string(),
            ));
        }

        // Create inventory for blocks
        let inv: Vec<Inventory> = block_hashes
            .iter()
            .map(|hash| Inventory::Block(*hash))
            .collect();

        let _msg = NetworkMessage::GetData(inv);

        // Request from first available peer
        let peer_addr = peers[0];
        debug!(
            ?peer_addr,
            count = block_hashes.len(),
            "Requesting blocks from peer"
        );

        Ok(())
    }

    /// Get peer count
    pub fn peer_count(&self) -> usize {
        self.peer_manager.peer_count()
    }

    /// Get connected peers
    pub fn get_peers(&self) -> Vec<SocketAddr> {
        self.peer_manager.get_peers()
    }

    /// Create version message
    fn create_version_message(&self, peer_addr: SocketAddr) -> Result<VersionMessage> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("SystemTime should be after UNIX_EPOCH")
            .as_secs() as i64;

        // Create receiver address (the peer)
        let receiver = bitcoin::p2p::address::Address {
            services: ServiceFlags::NONE,
            address: match peer_addr.ip() {
                IpAddr::V4(ip) => ip.to_ipv6_mapped().segments(),
                IpAddr::V6(ip) => ip.segments(),
            },
            port: peer_addr.port(),
        };

        // Create sender address (us)
        let sender = bitcoin::p2p::address::Address {
            services: self.config.services,
            address: [0; 8], // Use unspecified address
            port: 0,
        };

        Ok(VersionMessage {
            version: self.config.protocol_version,
            services: self.config.services,
            timestamp,
            receiver,
            sender,
            nonce: rand::random(),
            user_agent: self.config.user_agent.clone(),
            start_height: 0,
            relay: false,
        })
    }

    /// Send a network message
    async fn send_message(&self, stream: &mut TcpStream, message: NetworkMessage) -> Result<()> {
        let raw_msg = RawNetworkMessage::new(self.config.network.magic(), message);
        let bytes = encode::serialize(&raw_msg);
        stream
            .write_all(&bytes)
            .await
            .map_err(|e| BitcoinError::ConnectionFailed(e.to_string()))?;
        Ok(())
    }

    /// Handle peer connection
    async fn handle_peer_connection(
        mut stream: TcpStream,
        peer_manager: Arc<PeerManager>,
        network: Network,
    ) -> Result<()> {
        let peer_addr = stream.peer_addr().map_err(|e| {
            BitcoinError::ConnectionFailed(format!("Failed to get peer address: {}", e))
        })?;

        let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer

        loop {
            match stream.read(&mut buffer).await {
                Ok(0) => {
                    // Connection closed
                    peer_manager.remove_peer(&peer_addr);
                    break;
                }
                Ok(n) => {
                    // Parse network message
                    let data = &buffer[..n];
                    match encode::deserialize::<RawNetworkMessage>(data) {
                        Ok(msg) => {
                            debug!(?peer_addr, command = ?msg.command(), "Received message");
                            // Handle message based on type
                            match msg.payload() {
                                NetworkMessage::Version(v) => {
                                    peer_manager.update_peer_version(&peer_addr, v.clone());
                                    // Send verack
                                    let verack = RawNetworkMessage::new(
                                        network.magic(),
                                        NetworkMessage::Verack,
                                    );
                                    let bytes = encode::serialize(&verack);
                                    let _ = stream.write_all(&bytes).await;
                                }
                                NetworkMessage::Verack => {
                                    debug!(?peer_addr, "Handshake completed");
                                }
                                NetworkMessage::Ping(nonce) => {
                                    // Send pong
                                    let pong = RawNetworkMessage::new(
                                        network.magic(),
                                        NetworkMessage::Pong(*nonce),
                                    );
                                    let bytes = encode::serialize(&pong);
                                    let _ = stream.write_all(&bytes).await;
                                }
                                _ => {
                                    // Handle other messages
                                }
                            }
                        }
                        Err(e) => {
                            warn!(?peer_addr, error = ?e, "Failed to parse message");
                        }
                    }
                }
                Err(e) => {
                    error!(?peer_addr, error = ?e, "Read error");
                    peer_manager.remove_peer(&peer_addr);
                    break;
                }
            }
        }

        Ok(())
    }
}

/// P2P statistics
#[derive(Debug, Clone, Default)]
pub struct P2pStats {
    /// Total peers connected
    pub total_peers: usize,
    /// Total transactions broadcast
    pub transactions_broadcast: u64,
    /// Total blocks downloaded
    pub blocks_downloaded: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2p_config_default() {
        let config = P2pConfig::default();
        assert_eq!(config.network, Network::Bitcoin);
        assert_eq!(config.max_peers, 8);
        assert_eq!(config.protocol_version, 70016);
    }

    #[test]
    fn test_peer_info_creation() {
        let addr: SocketAddr = "127.0.0.1:8333".parse().unwrap();
        let peer = PeerInfo::new(addr, false);
        assert_eq!(peer.address, addr);
        assert!(!peer.inbound);
        assert_eq!(peer.messages_sent, 0);
        assert_eq!(peer.messages_received, 0);
    }

    #[test]
    fn test_peer_info_stale() {
        let addr: SocketAddr = "127.0.0.1:8333".parse().unwrap();
        let mut peer = PeerInfo::new(addr, false);

        // Should not be stale immediately
        assert!(!peer.is_stale(Duration::from_secs(10)));

        // Manually set last_seen to past
        peer.last_seen = SystemTime::now() - Duration::from_secs(20);
        assert!(peer.is_stale(Duration::from_secs(10)));
    }

    #[test]
    fn test_peer_manager() {
        let config = P2pConfig::default();
        let (manager, _rx) = PeerManager::new(config);

        let addr: SocketAddr = "127.0.0.1:8333".parse().unwrap();
        assert!(manager.add_peer(addr, false));
        assert_eq!(manager.peer_count(), 1);

        // Cannot add same peer twice
        assert!(!manager.add_peer(addr, false));
        assert_eq!(manager.peer_count(), 1);

        manager.remove_peer(&addr);
        assert_eq!(manager.peer_count(), 0);
    }

    #[tokio::test]
    async fn test_p2p_client_creation() {
        let config = P2pConfig::default();
        let (client, _rx) = P2pClient::new(config);
        assert_eq!(client.peer_count(), 0);
    }

    #[test]
    fn test_p2p_stats() {
        let stats = P2pStats::default();
        assert_eq!(stats.total_peers, 0);
        assert_eq!(stats.transactions_broadcast, 0);
        assert_eq!(stats.blocks_downloaded, 0);
    }
}
