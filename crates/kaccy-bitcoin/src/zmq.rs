//! ZeroMQ Integration for Bitcoin Core Real-time Notifications
//!
//! This module provides ZMQ (ZeroMQ) client functionality for receiving real-time
//! notifications from Bitcoin Core about blocks, transactions, and mempool events.
//!
//! Bitcoin Core's ZMQ interface publishes four main topics:
//! - `hashblock`: New block hash
//! - `hashtx`: New transaction hash
//! - `rawblock`: Full block data
//! - `rawtx`: Full transaction data
//!
//! # Examples
//!
//! ```no_run
//! use kaccy_bitcoin::zmq::{ZmqConfig, ZmqClient, ZmqNotification};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = ZmqConfig::new()
//!         .with_block_endpoint("tcp://127.0.0.1:28332")
//!         .with_tx_endpoint("tcp://127.0.0.1:28333");
//!
//!     let mut client = ZmqClient::new(config).await.unwrap();
//!
//!     while let Some(notification) = client.recv().await {
//!         match notification {
//!             ZmqNotification::Block { hash, .. } => {
//!                 println!("New block: {}", hash);
//!             }
//!             ZmqNotification::Transaction { txid, .. } => {
//!                 println!("New transaction: {}", txid);
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//! ```

use crate::error::{BitcoinError, Result};
use bitcoin::consensus::Decodable;
use bitcoin::{Block, Transaction};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

/// ZMQ topic types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZmqTopic {
    /// Block hash notifications
    HashBlock,
    /// Transaction hash notifications
    HashTx,
    /// Raw block notifications
    RawBlock,
    /// Raw transaction notifications
    RawTx,
    /// Sequence notifications (for tracking block connections/disconnections)
    Sequence,
}

impl ZmqTopic {
    /// Get the ZMQ topic string
    pub fn as_str(&self) -> &'static str {
        match self {
            ZmqTopic::HashBlock => "hashblock",
            ZmqTopic::HashTx => "hashtx",
            ZmqTopic::RawBlock => "rawblock",
            ZmqTopic::RawTx => "rawtx",
            ZmqTopic::Sequence => "sequence",
        }
    }
}

/// ZMQ configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZmqConfig {
    /// Block hash endpoint (e.g., "tcp://127.0.0.1:28332")
    pub hashblock_endpoint: Option<String>,
    /// Transaction hash endpoint
    pub hashtx_endpoint: Option<String>,
    /// Raw block endpoint
    pub rawblock_endpoint: Option<String>,
    /// Raw transaction endpoint
    pub rawtx_endpoint: Option<String>,
    /// Sequence endpoint
    pub sequence_endpoint: Option<String>,
    /// Buffer size for notification queue
    pub buffer_size: usize,
    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u64,
}

impl ZmqConfig {
    pub fn new() -> Self {
        Self {
            hashblock_endpoint: None,
            hashtx_endpoint: None,
            rawblock_endpoint: None,
            rawtx_endpoint: None,
            sequence_endpoint: None,
            buffer_size: 1000,
            connection_timeout_ms: 5000,
        }
    }

    pub fn with_block_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.hashblock_endpoint = Some(endpoint.into());
        self
    }

    pub fn with_tx_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.hashtx_endpoint = Some(endpoint.into());
        self
    }

    pub fn with_raw_block_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.rawblock_endpoint = Some(endpoint.into());
        self
    }

    pub fn with_raw_tx_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.rawtx_endpoint = Some(endpoint.into());
        self
    }

    pub fn with_sequence_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.sequence_endpoint = Some(endpoint.into());
        self
    }

    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }
}

impl Default for ZmqConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// ZMQ notification event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZmqNotification {
    /// New block hash
    Block {
        /// Block hash
        hash: String,
        /// Sequence number
        sequence: u32,
    },
    /// New transaction hash
    Transaction {
        /// Transaction ID
        txid: String,
        /// Sequence number
        sequence: u32,
    },
    /// Raw block data
    RawBlock {
        /// Block hash
        hash: String,
        /// Full block
        block: Vec<u8>,
        /// Sequence number
        sequence: u32,
    },
    /// Raw transaction data
    RawTransaction {
        /// Transaction ID
        txid: String,
        /// Full transaction
        tx: Vec<u8>,
        /// Sequence number
        sequence: u32,
    },
    /// Sequence event (block connected/disconnected)
    Sequence {
        /// Event type
        event_type: SequenceEvent,
        /// Block hash
        hash: String,
        /// Sequence number
        sequence: u32,
    },
}

/// Sequence event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceEvent {
    /// Block connected to main chain
    BlockConnected,
    /// Block disconnected (reorg)
    BlockDisconnected,
    /// Transaction added to mempool
    TransactionAdded,
    /// Transaction removed from mempool
    TransactionRemoved,
}

/// ZMQ client for receiving Bitcoin Core notifications
pub struct ZmqClient {
    config: Arc<ZmqConfig>,
    receiver: mpsc::Receiver<ZmqNotification>,
    #[allow(dead_code)]
    shutdown_tx: mpsc::Sender<()>,
}

impl ZmqClient {
    /// Create a new ZMQ client
    pub async fn new(config: ZmqConfig) -> Result<Self> {
        let (tx, rx) = mpsc::channel(config.buffer_size);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        let config_arc = Arc::new(config);
        let config_clone = config_arc.clone();

        // Spawn background task to handle ZMQ subscriptions
        tokio::spawn(async move {
            // In a real implementation, this would use zmq crate to subscribe
            // For now, this is a placeholder structure
            let _ = &config_clone;
            let _ = &tx;

            // Wait for shutdown signal
            let _ = shutdown_rx.recv().await;
        });

        Ok(Self {
            config: config_arc,
            receiver: rx,
            shutdown_tx,
        })
    }

    /// Receive next notification
    pub async fn recv(&mut self) -> Option<ZmqNotification> {
        self.receiver.recv().await
    }

    /// Try to receive notification without blocking
    pub fn try_recv(&mut self) -> Option<ZmqNotification> {
        self.receiver.try_recv().ok()
    }

    /// Get configuration
    pub fn config(&self) -> &ZmqConfig {
        &self.config
    }
}

/// ZMQ notification handler trait
#[async_trait::async_trait]
pub trait ZmqHandler: Send + Sync {
    /// Handle block notification
    async fn on_block(&self, hash: String, sequence: u32) -> Result<()>;

    /// Handle transaction notification
    async fn on_transaction(&self, txid: String, sequence: u32) -> Result<()>;

    /// Handle raw block notification
    async fn on_raw_block(&self, hash: String, block: Block, sequence: u32) -> Result<()>;

    /// Handle raw transaction notification
    async fn on_raw_transaction(&self, txid: String, tx: Transaction, sequence: u32) -> Result<()>;

    /// Handle sequence event
    async fn on_sequence(&self, event: SequenceEvent, hash: String, sequence: u32) -> Result<()>;
}

/// Multi-handler ZMQ processor
pub struct ZmqProcessor {
    client: ZmqClient,
    handlers: Vec<Arc<dyn ZmqHandler>>,
}

impl ZmqProcessor {
    /// Create a new processor
    pub fn new(client: ZmqClient) -> Self {
        Self {
            client,
            handlers: Vec::new(),
        }
    }

    /// Add a notification handler
    pub fn add_handler(&mut self, handler: Arc<dyn ZmqHandler>) {
        self.handlers.push(handler);
    }

    /// Process notifications
    pub async fn run(&mut self) -> Result<()> {
        while let Some(notification) = self.client.recv().await {
            self.handle_notification(notification).await?;
        }
        Ok(())
    }

    async fn handle_notification(&self, notification: ZmqNotification) -> Result<()> {
        for handler in &self.handlers {
            match &notification {
                ZmqNotification::Block { hash, sequence } => {
                    handler.on_block(hash.clone(), *sequence).await?;
                }
                ZmqNotification::Transaction { txid, sequence } => {
                    handler.on_transaction(txid.clone(), *sequence).await?;
                }
                ZmqNotification::RawBlock {
                    hash,
                    block,
                    sequence,
                } => {
                    let parsed_block = Block::consensus_decode(&mut &block[..]).map_err(|e| {
                        BitcoinError::InvalidTransaction(format!("Failed to parse block: {}", e))
                    })?;
                    handler
                        .on_raw_block(hash.clone(), parsed_block, *sequence)
                        .await?;
                }
                ZmqNotification::RawTransaction { txid, tx, sequence } => {
                    let parsed_tx = Transaction::consensus_decode(&mut &tx[..]).map_err(|e| {
                        BitcoinError::InvalidTransaction(format!("Failed to parse tx: {}", e))
                    })?;
                    handler
                        .on_raw_transaction(txid.clone(), parsed_tx, *sequence)
                        .await?;
                }
                ZmqNotification::Sequence {
                    event_type,
                    hash,
                    sequence,
                } => {
                    handler
                        .on_sequence(*event_type, hash.clone(), *sequence)
                        .await?;
                }
            }
        }
        Ok(())
    }
}

/// Block notification monitor
pub struct BlockMonitor {
    on_block: Option<Box<dyn Fn(String, u32) + Send + Sync>>,
}

impl BlockMonitor {
    pub fn new() -> Self {
        Self { on_block: None }
    }

    pub fn on_block<F>(mut self, callback: F) -> Self
    where
        F: Fn(String, u32) + Send + Sync + 'static,
    {
        self.on_block = Some(Box::new(callback));
        self
    }
}

impl Default for BlockMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ZmqHandler for BlockMonitor {
    async fn on_block(&self, hash: String, sequence: u32) -> Result<()> {
        if let Some(callback) = &self.on_block {
            callback(hash, sequence);
        }
        Ok(())
    }

    async fn on_transaction(&self, _txid: String, _sequence: u32) -> Result<()> {
        Ok(())
    }

    async fn on_raw_block(&self, _hash: String, _block: Block, _sequence: u32) -> Result<()> {
        Ok(())
    }

    async fn on_raw_transaction(
        &self,
        _txid: String,
        _tx: Transaction,
        _sequence: u32,
    ) -> Result<()> {
        Ok(())
    }

    async fn on_sequence(
        &self,
        _event: SequenceEvent,
        _hash: String,
        _sequence: u32,
    ) -> Result<()> {
        Ok(())
    }
}

/// Transaction notification monitor
pub struct TransactionMonitor {
    on_tx: Option<Box<dyn Fn(String, u32) + Send + Sync>>,
}

impl TransactionMonitor {
    pub fn new() -> Self {
        Self { on_tx: None }
    }

    pub fn on_transaction<F>(mut self, callback: F) -> Self
    where
        F: Fn(String, u32) + Send + Sync + 'static,
    {
        self.on_tx = Some(Box::new(callback));
        self
    }
}

impl Default for TransactionMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ZmqHandler for TransactionMonitor {
    async fn on_block(&self, _hash: String, _sequence: u32) -> Result<()> {
        Ok(())
    }

    async fn on_transaction(&self, txid: String, sequence: u32) -> Result<()> {
        if let Some(callback) = &self.on_tx {
            callback(txid, sequence);
        }
        Ok(())
    }

    async fn on_raw_block(&self, _hash: String, _block: Block, _sequence: u32) -> Result<()> {
        Ok(())
    }

    async fn on_raw_transaction(
        &self,
        _txid: String,
        _tx: Transaction,
        _sequence: u32,
    ) -> Result<()> {
        Ok(())
    }

    async fn on_sequence(
        &self,
        _event: SequenceEvent,
        _hash: String,
        _sequence: u32,
    ) -> Result<()> {
        Ok(())
    }
}

/// Reorg (reorganization) detector using sequence events
pub struct ReorgDetector {
    on_reorg: Option<Box<dyn Fn(String, u32) + Send + Sync>>,
}

impl ReorgDetector {
    pub fn new() -> Self {
        Self { on_reorg: None }
    }

    pub fn on_reorg<F>(mut self, callback: F) -> Self
    where
        F: Fn(String, u32) + Send + Sync + 'static,
    {
        self.on_reorg = Some(Box::new(callback));
        self
    }
}

impl Default for ReorgDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ZmqHandler for ReorgDetector {
    async fn on_block(&self, _hash: String, _sequence: u32) -> Result<()> {
        Ok(())
    }

    async fn on_transaction(&self, _txid: String, _sequence: u32) -> Result<()> {
        Ok(())
    }

    async fn on_raw_block(&self, _hash: String, _block: Block, _sequence: u32) -> Result<()> {
        Ok(())
    }

    async fn on_raw_transaction(
        &self,
        _txid: String,
        _tx: Transaction,
        _sequence: u32,
    ) -> Result<()> {
        Ok(())
    }

    async fn on_sequence(&self, event: SequenceEvent, hash: String, sequence: u32) -> Result<()> {
        if event == SequenceEvent::BlockDisconnected {
            if let Some(callback) = &self.on_reorg {
                callback(hash, sequence);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zmq_topic_string() {
        assert_eq!(ZmqTopic::HashBlock.as_str(), "hashblock");
        assert_eq!(ZmqTopic::HashTx.as_str(), "hashtx");
        assert_eq!(ZmqTopic::RawBlock.as_str(), "rawblock");
        assert_eq!(ZmqTopic::RawTx.as_str(), "rawtx");
        assert_eq!(ZmqTopic::Sequence.as_str(), "sequence");
    }

    #[test]
    fn test_zmq_config_builder() {
        let config = ZmqConfig::new()
            .with_block_endpoint("tcp://127.0.0.1:28332")
            .with_tx_endpoint("tcp://127.0.0.1:28333")
            .with_buffer_size(500);

        assert_eq!(
            config.hashblock_endpoint,
            Some("tcp://127.0.0.1:28332".to_string())
        );
        assert_eq!(
            config.hashtx_endpoint,
            Some("tcp://127.0.0.1:28333".to_string())
        );
        assert_eq!(config.buffer_size, 500);
    }

    #[test]
    fn test_sequence_event() {
        assert_eq!(SequenceEvent::BlockConnected, SequenceEvent::BlockConnected);
        assert_ne!(
            SequenceEvent::BlockConnected,
            SequenceEvent::BlockDisconnected
        );
    }

    #[test]
    fn test_zmq_notification() {
        let notification = ZmqNotification::Block {
            hash: "abc123".to_string(),
            sequence: 100,
        };

        match notification {
            ZmqNotification::Block { hash, sequence } => {
                assert_eq!(hash, "abc123");
                assert_eq!(sequence, 100);
            }
            _ => panic!("Wrong notification type"),
        }
    }

    #[test]
    fn test_block_monitor_creation() {
        let _monitor = BlockMonitor::new().on_block(|hash, seq| {
            println!("Block: {} at seq {}", hash, seq);
        });

        // Monitor created successfully
    }

    #[test]
    fn test_transaction_monitor_creation() {
        let _monitor = TransactionMonitor::new().on_transaction(|txid, seq| {
            println!("Transaction: {} at seq {}", txid, seq);
        });

        // Monitor created successfully
    }

    #[test]
    fn test_reorg_detector_creation() {
        let _detector = ReorgDetector::new().on_reorg(|hash, seq| {
            println!("Reorg detected: {} at seq {}", hash, seq);
        });

        // Detector created successfully
    }
}
