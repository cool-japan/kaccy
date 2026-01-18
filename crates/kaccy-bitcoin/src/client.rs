//! Bitcoin Core RPC client

use bitcoin::{Address, Amount, Network, Txid};
use bitcoincore_rpc::json::{
    GetBlockchainInfoResult, GetNetworkInfoResult, GetRawTransactionResult, GetTransactionResult,
};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::Serialize;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::error::{BitcoinError, Result};

/// Bitcoin network configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitcoinNetwork {
    Mainnet,
    Testnet,
    /// Testnet4 (when available, currently maps to Testnet)
    Testnet4,
    Regtest,
    Signet,
}

impl From<BitcoinNetwork> for Network {
    fn from(network: BitcoinNetwork) -> Self {
        match network {
            BitcoinNetwork::Mainnet => Network::Bitcoin,
            BitcoinNetwork::Testnet => Network::Testnet,
            // Testnet4 not yet available in bitcoin crate, map to Testnet for now
            BitcoinNetwork::Testnet4 => Network::Testnet,
            BitcoinNetwork::Regtest => Network::Regtest,
            BitcoinNetwork::Signet => Network::Signet,
        }
    }
}

/// Configuration for automatic reconnection
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

/// Connection parameters for client recreation
#[derive(Clone)]
struct ConnectionParams {
    url: String,
    user: String,
    password: String,
}

/// Bitcoin Core RPC client wrapper with automatic reconnection
///
/// # Examples
///
/// ```no_run
/// use kaccy_bitcoin::{BitcoinClient, BitcoinNetwork};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let client = BitcoinClient::new(
///     "http://localhost:8332",
///     "rpcuser",
///     "rpcpassword",
///     BitcoinNetwork::Testnet,
/// )?;
///
/// // Check connection health
/// let is_healthy = client.health_check()?;
/// println!("Bitcoin node healthy: {}", is_healthy);
/// # Ok(())
/// # }
/// ```
pub struct BitcoinClient {
    client: Arc<RwLock<Client>>,
    network: BitcoinNetwork,
    connection_params: ConnectionParams,
    reconnect_config: ReconnectConfig,
}

impl BitcoinClient {
    /// Create a new Bitcoin RPC client
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use kaccy_bitcoin::{BitcoinClient, BitcoinNetwork};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = BitcoinClient::new(
    ///     "http://localhost:18443",
    ///     "user",
    ///     "pass",
    ///     BitcoinNetwork::Regtest,
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(url: &str, user: &str, password: &str, network: BitcoinNetwork) -> Result<Self> {
        Self::with_config(url, user, password, network, ReconnectConfig::default())
    }

    /// Create a new Bitcoin RPC client with custom reconnection config
    pub fn with_config(
        url: &str,
        user: &str,
        password: &str,
        network: BitcoinNetwork,
        reconnect_config: ReconnectConfig,
    ) -> Result<Self> {
        let client = Client::new(url, Auth::UserPass(user.to_string(), password.to_string()))?;

        tracing::info!(url = url, network = ?network, "Bitcoin RPC client connected");

        Ok(Self {
            client: Arc::new(RwLock::new(client)),
            network,
            connection_params: ConnectionParams {
                url: url.to_string(),
                user: user.to_string(),
                password: password.to_string(),
            },
            reconnect_config,
        })
    }

    /// Try to reconnect to Bitcoin Core
    fn reconnect(&self) -> Result<()> {
        let params = &self.connection_params;
        let new_client = Client::new(
            &params.url,
            Auth::UserPass(params.user.clone(), params.password.clone()),
        )?;

        let mut client = self.client.write().unwrap();
        *client = new_client;

        tracing::info!("Bitcoin RPC client reconnected");
        Ok(())
    }

    /// Execute an RPC operation with automatic retry on connection failure
    fn with_retry<T, F>(&self, operation: F) -> Result<T>
    where
        F: Fn(&Client) -> std::result::Result<T, bitcoincore_rpc::Error>,
    {
        let mut last_error = None;
        let mut delay = self.reconnect_config.initial_delay;

        for attempt in 0..=self.reconnect_config.max_retries {
            let client = self.client.read().unwrap();
            match operation(&client) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    drop(client); // Release the read lock

                    if attempt < self.reconnect_config.max_retries {
                        tracing::warn!(
                            attempt = attempt + 1,
                            max_retries = self.reconnect_config.max_retries,
                            delay_ms = delay.as_millis(),
                            "Bitcoin RPC failed, attempting reconnection"
                        );

                        std::thread::sleep(delay);

                        // Try to reconnect
                        if let Err(reconnect_err) = self.reconnect() {
                            tracing::warn!(error = %reconnect_err, "Reconnection failed");
                        }

                        // Exponential backoff
                        delay = std::cmp::min(
                            Duration::from_secs_f64(
                                delay.as_secs_f64() * self.reconnect_config.backoff_multiplier,
                            ),
                            self.reconnect_config.max_delay,
                        );
                    }
                }
            }
        }

        Err(BitcoinError::Rpc(last_error.unwrap()))
    }

    /// Get the configured network
    pub fn network(&self) -> BitcoinNetwork {
        self.network
    }

    /// Check if the connection is healthy
    pub fn health_check(&self) -> Result<bool> {
        match self.with_retry(|c| c.get_blockchain_info()) {
            Ok(_) => Ok(true),
            Err(e) => {
                tracing::warn!(error = %e, "Bitcoin RPC health check failed");
                Ok(false)
            }
        }
    }

    /// Get blockchain info
    pub fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResult> {
        self.with_retry(|c| c.get_blockchain_info())
    }

    /// Get network info
    pub fn get_network_info(&self) -> Result<GetNetworkInfoResult> {
        self.with_retry(|c| c.get_network_info())
    }

    /// Get current block height
    pub fn get_block_height(&self) -> Result<u64> {
        let info = self.with_retry(|c| c.get_blockchain_info())?;
        Ok(info.blocks)
    }

    /// Get best block hash
    pub fn get_best_block_hash(&self) -> Result<bitcoin::BlockHash> {
        self.with_retry(|c| c.get_best_block_hash())
    }

    /// Generate a new address
    pub fn get_new_address(
        &self,
        label: Option<&str>,
    ) -> Result<Address<bitcoin::address::NetworkUnchecked>> {
        self.with_retry(|c| c.get_new_address(label, None))
    }

    /// Get amount received by address
    pub fn get_received_by_address(
        &self,
        address: &Address,
        min_confirmations: Option<u32>,
    ) -> Result<Amount> {
        self.with_retry(|c| c.get_received_by_address(address, min_confirmations))
    }

    /// Get transaction by ID
    pub fn get_transaction(&self, txid: &Txid) -> Result<GetTransactionResult> {
        self.with_retry(|c| c.get_transaction(txid, None))
    }

    /// Get raw transaction
    pub fn get_raw_transaction(&self, txid: &Txid) -> Result<GetRawTransactionResult> {
        self.with_retry(|c| c.get_raw_transaction_info(txid, None))
    }

    /// Get unspent transactions for an address
    pub fn list_unspent(
        &self,
        min_conf: Option<usize>,
        max_conf: Option<usize>,
        addresses: Option<&[&Address<bitcoin::address::NetworkChecked>]>,
    ) -> Result<Vec<bitcoincore_rpc::json::ListUnspentResultEntry>> {
        self.with_retry(|c| c.list_unspent(min_conf, max_conf, addresses, None, None))
    }

    /// Get wallet balance
    pub fn get_balance(&self) -> Result<Amount> {
        self.with_retry(|c| c.get_balance(None, None))
    }

    /// Validate an address (basic validation without RPC)
    pub fn validate_address(&self, address: &str) -> Result<AddressValidation> {
        // Parse address to validate format
        let parsed = address
            .parse::<Address<bitcoin::address::NetworkUnchecked>>()
            .map_err(|e| BitcoinError::InvalidAddress(e.to_string()));

        match parsed {
            Ok(_addr) => Ok(AddressValidation {
                is_valid: true,
                is_mine: false, // Would need wallet check
                is_script: address.starts_with("3") || address.starts_with("bc1q"),
            }),
            Err(_) => Ok(AddressValidation {
                is_valid: false,
                is_mine: false,
                is_script: false,
            }),
        }
    }

    /// Get mempool info
    pub fn get_mempool_info(&self) -> Result<bitcoincore_rpc::json::GetMempoolInfoResult> {
        self.with_retry(|c| c.get_mempool_info())
    }

    /// Estimate smart fee (sats/vB)
    pub fn estimate_smart_fee(&self, conf_target: u16) -> Result<Option<f64>> {
        let result = self.with_retry(|c| c.estimate_smart_fee(conf_target, None))?;
        Ok(result.fee_rate.map(|amt| {
            // Convert BTC/kB to sat/vB
            amt.to_sat() as f64 / 1000.0
        }))
    }
}

/// Address validation result
#[derive(Debug, Clone, Serialize)]
pub struct AddressValidation {
    pub is_valid: bool,
    pub is_mine: bool,
    pub is_script: bool,
}

/// Summary of node status
#[derive(Debug, Clone, Serialize)]
pub struct NodeStatus {
    pub connected: bool,
    pub block_height: u64,
    pub network: String,
    pub version: u64,
    pub connections: usize,
    pub mempool_size: u64,
}

impl BitcoinClient {
    /// Get comprehensive node status
    pub fn get_node_status(&self) -> Result<NodeStatus> {
        let blockchain_info = self.with_retry(|c| c.get_blockchain_info())?;
        let network_info = self.with_retry(|c| c.get_network_info())?;
        let mempool_info = self.with_retry(|c| c.get_mempool_info())?;

        Ok(NodeStatus {
            connected: true,
            block_height: blockchain_info.blocks,
            network: blockchain_info.chain.to_string(),
            version: network_info.version as u64,
            connections: network_info.connections,
            mempool_size: mempool_info.size as u64,
        })
    }

    /// List transactions since a specific block
    pub fn list_since_block(
        &self,
        block_hash: Option<&bitcoin::BlockHash>,
        target_confirmations: Option<usize>,
    ) -> Result<ListSinceBlockResult> {
        let result =
            self.with_retry(|c| c.list_since_block(block_hash, target_confirmations, None, None))?;

        Ok(ListSinceBlockResult {
            transactions: result
                .transactions
                .into_iter()
                .map(|tx| TransactionInfo {
                    txid: tx.info.txid,
                    address: tx.detail.address.map(|a| a.assume_checked().to_string()),
                    category: format!("{:?}", tx.detail.category),
                    amount: tx.detail.amount.to_sat(),
                    confirmations: tx.info.confirmations,
                    block_hash: tx.info.blockhash,
                    block_time: tx.info.blocktime,
                    time: tx.info.time,
                })
                .collect(),
            last_block: result.lastblock,
        })
    }

    /// Get detailed address info including balance
    pub fn get_address_info(&self, address: &str) -> Result<AddressInfo> {
        // Parse and validate address
        let parsed: Address<bitcoin::address::NetworkUnchecked> =
            address.parse().map_err(|e: bitcoin::address::ParseError| {
                BitcoinError::InvalidAddress(e.to_string())
            })?;

        let checked_addr = parsed.assume_checked();

        // Get received amount
        let received = self.with_retry(|c| c.get_received_by_address(&checked_addr, Some(0)))?;
        let received_confirmed =
            self.with_retry(|c| c.get_received_by_address(&checked_addr, Some(1)))?;

        Ok(AddressInfo {
            address: address.to_string(),
            is_valid: true,
            total_received_sats: received.to_sat(),
            confirmed_received_sats: received_confirmed.to_sat(),
            unconfirmed_sats: received
                .to_sat()
                .saturating_sub(received_confirmed.to_sat()),
        })
    }

    /// Send raw transaction to the network
    pub fn send_raw_transaction(&self, tx_hex: &str) -> Result<Txid> {
        let tx_hex_owned = tx_hex.to_string();
        self.with_retry(|c| c.send_raw_transaction(tx_hex_owned.clone()))
    }

    /// Get block by height
    pub fn get_block_hash(&self, height: u64) -> Result<bitcoin::BlockHash> {
        self.with_retry(|c| c.get_block_hash(height))
    }

    /// Test mempool accept for a transaction
    pub fn test_mempool_accept(&self, tx_hex: &str) -> Result<bool> {
        let rawtxs = vec![tx_hex.to_string()];
        let results = self.with_retry(|c| c.test_mempool_accept(&rawtxs))?;
        Ok(results.first().map(|r| r.allowed).unwrap_or(false))
    }

    /// Generate blocks to an address (regtest only)
    pub fn generate_to_address(
        &self,
        blocks: u64,
        address: &bitcoin::Address,
    ) -> Result<Vec<bitcoin::BlockHash>> {
        self.with_retry(|c| c.generate_to_address(blocks, address))
    }

    /// Send to an address
    pub fn send_to_address(
        &self,
        address: &bitcoin::Address,
        amount: bitcoin::Amount,
    ) -> Result<Txid> {
        self.with_retry(|c| c.send_to_address(address, amount, None, None, None, None, None, None))
    }

    /// Invalidate a block (regtest only)
    pub fn invalidate_block(&self, block_hash: &bitcoin::BlockHash) -> Result<()> {
        self.with_retry(|c| c.invalidate_block(block_hash))
    }

    /// Reconsider a block (regtest only)
    pub fn reconsider_block(&self, block_hash: &bitcoin::BlockHash) -> Result<()> {
        self.with_retry(|c| c.reconsider_block(block_hash))
    }
}

/// Result from list_since_block
#[derive(Debug, Clone, Serialize)]
pub struct ListSinceBlockResult {
    pub transactions: Vec<TransactionInfo>,
    pub last_block: bitcoin::BlockHash,
}

/// Transaction info from list_since_block
#[derive(Debug, Clone, Serialize)]
pub struct TransactionInfo {
    pub txid: Txid,
    pub address: Option<String>,
    pub category: String,
    pub amount: i64,
    pub confirmations: i32,
    pub block_hash: Option<bitcoin::BlockHash>,
    pub block_time: Option<u64>,
    pub time: u64,
}

/// Detailed address information
#[derive(Debug, Clone, Serialize)]
pub struct AddressInfo {
    pub address: String,
    pub is_valid: bool,
    pub total_received_sats: u64,
    pub confirmed_received_sats: u64,
    pub unconfirmed_sats: u64,
}
