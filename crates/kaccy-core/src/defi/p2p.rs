//! Peer-to-Peer Trading Features
//!
//! This module provides P2P trading infrastructure including:
//! - P2P order matching
//! - Distributed order books
//! - Gossip protocol for price discovery
//! - Distributed Hash Table (DHT) for data storage

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// P2P peer node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: Uuid,
    pub address: String,
    pub port: u16,
    pub last_seen: DateTime<Utc>,
    pub reputation: f64,
    pub connected_peers: HashSet<Uuid>,
}

impl Peer {
    pub fn new(address: String, port: u16) -> Self {
        Self {
            id: Uuid::new_v4(),
            address,
            port,
            last_seen: Utc::now(),
            reputation: 1.0,
            connected_peers: HashSet::new(),
        }
    }

    pub fn connect_to(&mut self, peer_id: Uuid) {
        self.connected_peers.insert(peer_id);
        self.last_seen = Utc::now();
    }
}

/// P2P order for direct peer-to-peer trading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2POrder {
    pub id: Uuid,
    pub peer_id: Uuid,
    pub token_symbol: String,
    pub side: OrderSide,
    pub amount: Decimal,
    pub price: Decimal,
    pub created_at: DateTime<Utc>,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Open,
    Matched,
    Filled,
    Cancelled,
}

/// Distributed order book across P2P network
pub struct DistributedOrderBook {
    pub token_symbol: String,
    pub orders: HashMap<Uuid, P2POrder>,
    pub local_peer_id: Uuid,
    pub known_peers: Vec<Peer>,
}

impl DistributedOrderBook {
    pub fn new(token_symbol: String, local_peer_id: Uuid) -> Self {
        Self {
            token_symbol,
            orders: HashMap::new(),
            local_peer_id,
            known_peers: Vec::new(),
        }
    }

    /// Add order to distributed book
    pub fn add_order(&mut self, order: P2POrder) {
        self.orders.insert(order.id, order);
    }

    /// Match orders in P2P fashion
    pub fn match_orders(&mut self) -> Vec<(Uuid, Uuid)> {
        let mut matches = Vec::new();

        let buy_orders: Vec<_> = self
            .orders
            .values()
            .filter(|o| matches!(o.side, OrderSide::Buy) && matches!(o.status, OrderStatus::Open))
            .collect();

        let sell_orders: Vec<_> = self
            .orders
            .values()
            .filter(|o| matches!(o.side, OrderSide::Sell) && matches!(o.status, OrderStatus::Open))
            .collect();

        for buy_order in &buy_orders {
            for sell_order in &sell_orders {
                if buy_order.price >= sell_order.price && buy_order.amount >= sell_order.amount {
                    matches.push((buy_order.id, sell_order.id));
                }
            }
        }

        matches
    }

    /// Sync order book with peers
    pub fn sync_with_peers(&mut self, peer_orders: HashMap<Uuid, P2POrder>) {
        for (id, order) in peer_orders {
            self.orders.insert(id, order);
        }
    }
}

/// Gossip protocol for price discovery
pub struct GossipProtocol {
    pub peer_id: Uuid,
    pub known_peers: HashMap<Uuid, Peer>,
    pub gossip_interval_ms: u64,
    pub fanout: usize, // Number of peers to gossip to
}

impl GossipProtocol {
    pub fn new(peer_id: Uuid, fanout: usize) -> Self {
        Self {
            peer_id,
            known_peers: HashMap::new(),
            gossip_interval_ms: 1000,
            fanout,
        }
    }

    /// Add peer to known peers
    pub fn add_peer(&mut self, peer: Peer) {
        self.known_peers.insert(peer.id, peer);
    }

    /// Select random peers for gossip
    pub fn select_gossip_targets(&self) -> Vec<Uuid> {
        let peer_ids: Vec<_> = self.known_peers.keys().copied().collect();
        peer_ids.into_iter().take(self.fanout).collect()
    }

    /// Gossip price information
    pub fn gossip_price(&self, token: &str, price: Decimal, targets: &[Uuid]) -> Vec<PriceGossip> {
        targets
            .iter()
            .map(|&peer_id| PriceGossip {
                id: Uuid::new_v4(),
                from_peer: self.peer_id,
                to_peer: peer_id,
                token: token.to_string(),
                price,
                timestamp: Utc::now(),
                hop_count: 0,
            })
            .collect()
    }

    /// Process received gossip
    pub fn process_gossip(&mut self, gossip: &PriceGossip) -> bool {
        // Verify hop count to prevent infinite propagation
        if gossip.hop_count > 10 {
            return false;
        }

        // Update peer last_seen
        if let Some(peer) = self.known_peers.get_mut(&gossip.from_peer) {
            peer.last_seen = Utc::now();
        }

        true
    }
}

/// Price gossip message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceGossip {
    pub id: Uuid,
    pub from_peer: Uuid,
    pub to_peer: Uuid,
    pub token: String,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
    pub hop_count: u8,
}

/// Distributed Hash Table for decentralized data storage
pub struct DHT {
    pub node_id: Uuid,
    pub storage: HashMap<String, Vec<u8>>,
    pub routing_table: HashMap<Uuid, Peer>,
    pub k_bucket_size: usize, // Kademlia k-bucket size
}

impl DHT {
    pub fn new(node_id: Uuid) -> Self {
        Self {
            node_id,
            storage: HashMap::new(),
            routing_table: HashMap::new(),
            k_bucket_size: 20,
        }
    }

    /// Store data in DHT
    pub fn store(&mut self, key: String, value: Vec<u8>) {
        self.storage.insert(key, value);
    }

    /// Retrieve data from DHT
    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.storage.get(key)
    }

    /// Find closest nodes to a key (Kademlia-style)
    pub fn find_closest_nodes(&self, _target_key: &str, count: usize) -> Vec<Uuid> {
        // Simplified: return first 'count' peers
        // In production, would calculate XOR distance
        self.routing_table.keys().take(count).copied().collect()
    }

    /// Add peer to routing table
    pub fn add_to_routing_table(&mut self, peer: Peer) {
        if self.routing_table.len() < self.k_bucket_size {
            self.routing_table.insert(peer.id, peer);
        }
    }
}

/// P2P order matching engine
pub struct P2PMatchingEngine {
    pub order_books: HashMap<String, DistributedOrderBook>,
    pub local_peer: Peer,
}

impl P2PMatchingEngine {
    pub fn new(local_peer: Peer) -> Self {
        Self {
            order_books: HashMap::new(),
            local_peer,
        }
    }

    /// Create or get order book for token
    pub fn get_or_create_order_book(&mut self, token: &str) -> &mut DistributedOrderBook {
        self.order_books
            .entry(token.to_string())
            .or_insert_with(|| DistributedOrderBook::new(token.to_string(), self.local_peer.id))
    }

    /// Submit P2P order
    pub fn submit_order(&mut self, order: P2POrder) -> Uuid {
        let order_id = order.id;
        let order_book = self.get_or_create_order_book(&order.token_symbol);
        order_book.add_order(order);
        order_id
    }

    /// Match orders across all order books
    pub fn match_all_orders(&mut self) -> HashMap<String, Vec<(Uuid, Uuid)>> {
        let mut all_matches = HashMap::new();

        for (token, order_book) in &mut self.order_books {
            let matches = order_book.match_orders();
            if !matches.is_empty() {
                all_matches.insert(token.clone(), matches);
            }
        }

        all_matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_peer_creation() {
        let peer = Peer::new("192.168.1.1".to_string(), 8080);
        assert_eq!(peer.address, "192.168.1.1");
        assert_eq!(peer.port, 8080);
        assert_eq!(peer.reputation, 1.0);
    }

    #[test]
    fn test_peer_connection() {
        let mut peer1 = Peer::new("node1".to_string(), 8080);
        let peer2 = Peer::new("node2".to_string(), 8081);

        peer1.connect_to(peer2.id);
        assert!(peer1.connected_peers.contains(&peer2.id));
    }

    #[test]
    fn test_distributed_order_book() {
        let peer_id = Uuid::new_v4();
        let mut order_book = DistributedOrderBook::new("BTC".to_string(), peer_id);

        let order = P2POrder {
            id: Uuid::new_v4(),
            peer_id,
            token_symbol: "BTC".to_string(),
            side: OrderSide::Buy,
            amount: dec!(1.0),
            price: dec!(50000),
            created_at: Utc::now(),
            status: OrderStatus::Open,
        };

        order_book.add_order(order);
        assert_eq!(order_book.orders.len(), 1);
    }

    #[test]
    fn test_order_matching() {
        let peer_id = Uuid::new_v4();
        let mut order_book = DistributedOrderBook::new("BTC".to_string(), peer_id);

        let buy_order = P2POrder {
            id: Uuid::new_v4(),
            peer_id,
            token_symbol: "BTC".to_string(),
            side: OrderSide::Buy,
            amount: dec!(1.0),
            price: dec!(50000),
            created_at: Utc::now(),
            status: OrderStatus::Open,
        };

        let sell_order = P2POrder {
            id: Uuid::new_v4(),
            peer_id,
            token_symbol: "BTC".to_string(),
            side: OrderSide::Sell,
            amount: dec!(1.0),
            price: dec!(49000),
            created_at: Utc::now(),
            status: OrderStatus::Open,
        };

        order_book.add_order(buy_order);
        order_book.add_order(sell_order);

        let matches = order_book.match_orders();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_gossip_protocol() {
        let peer_id = Uuid::new_v4();
        let mut gossip = GossipProtocol::new(peer_id, 3);

        let peer = Peer::new("node1".to_string(), 8080);
        gossip.add_peer(peer);

        assert_eq!(gossip.known_peers.len(), 1);

        let targets = gossip.select_gossip_targets();
        assert!(!targets.is_empty());
    }

    #[test]
    fn test_price_gossip() {
        let peer_id = Uuid::new_v4();
        let gossip_protocol = GossipProtocol::new(peer_id, 2);

        let peer1 = Peer::new("node1".to_string(), 8080);
        let peer2 = Peer::new("node2".to_string(), 8081);

        let targets = vec![peer1.id, peer2.id];
        let gossip_messages = gossip_protocol.gossip_price("BTC", dec!(50000), &targets);

        assert_eq!(gossip_messages.len(), 2);
        assert_eq!(gossip_messages[0].token, "BTC");
    }

    #[test]
    fn test_dht() {
        let node_id = Uuid::new_v4();
        let mut dht = DHT::new(node_id);

        dht.store("key1".to_string(), vec![1, 2, 3]);
        let value = dht.get("key1").unwrap();
        assert_eq!(value, &vec![1, 2, 3]);
    }

    #[test]
    fn test_dht_routing_table() {
        let node_id = Uuid::new_v4();
        let mut dht = DHT::new(node_id);

        let peer = Peer::new("node1".to_string(), 8080);
        dht.add_to_routing_table(peer);

        assert_eq!(dht.routing_table.len(), 1);
    }

    #[test]
    fn test_p2p_matching_engine() {
        let peer = Peer::new("localhost".to_string(), 8080);
        let mut engine = P2PMatchingEngine::new(peer);

        let order = P2POrder {
            id: Uuid::new_v4(),
            peer_id: Uuid::new_v4(),
            token_symbol: "BTC".to_string(),
            side: OrderSide::Buy,
            amount: dec!(1.0),
            price: dec!(50000),
            created_at: Utc::now(),
            status: OrderStatus::Open,
        };

        let _order_id = engine.submit_order(order);
        assert!(engine.order_books.contains_key("BTC"));
    }
}
