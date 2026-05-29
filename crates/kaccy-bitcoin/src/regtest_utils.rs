//! Regtest automation utilities for testing
//!
//! Provides helper functions for automated testing with Bitcoin Core in regtest mode.

use crate::client::BitcoinClient;
use crate::error::BitcoinError;
use bitcoin::{Address, Amount};
use std::str::FromStr;

/// Configuration for regtest automation
#[derive(Debug, Clone)]
pub struct RegtestConfig {
    /// Number of blocks to mine for initial maturity
    pub initial_blocks: u32,
    /// Number of blocks to mine for transaction confirmations
    pub confirmation_blocks: u32,
    /// Auto-mine on transaction submission
    pub auto_mine: bool,
    /// Default amount to fund test wallets (in satoshis)
    pub default_funding_amount: u64,
}

impl Default for RegtestConfig {
    fn default() -> Self {
        Self {
            initial_blocks: 101, // Coinbase maturity is 100 blocks
            confirmation_blocks: 6,
            auto_mine: true,
            default_funding_amount: 100_000_000, // 1 BTC
        }
    }
}

/// Regtest automation helper
pub struct RegtestHelper {
    client: BitcoinClient,
    config: RegtestConfig,
    mining_address: Option<Address>,
}

impl RegtestHelper {
    /// Create a new regtest helper
    pub fn new(client: BitcoinClient, config: RegtestConfig) -> Self {
        Self {
            client,
            config,
            mining_address: None,
        }
    }

    /// Initialize regtest environment
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use kaccy_bitcoin::{BitcoinClient, BitcoinNetwork, regtest_utils::{RegtestHelper, RegtestConfig}};
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = BitcoinClient::new("http://localhost:18443", "user", "pass", BitcoinNetwork::Regtest)?;
    /// let config = RegtestConfig::default();
    /// let mut helper = RegtestHelper::new(client, config);
    /// helper.initialize()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn initialize(&mut self) -> Result<(), BitcoinError> {
        // Generate a mining address if not set
        if self.mining_address.is_none() {
            let address = self.client.get_new_address(None)?.assume_checked();
            self.mining_address = Some(address);
        }

        // Mine initial blocks for coinbase maturity
        self.mine_blocks(self.config.initial_blocks)?;

        Ok(())
    }

    /// Set custom mining address
    pub fn set_mining_address(&mut self, address: Address) {
        self.mining_address = Some(address);
    }

    /// Mine a specified number of blocks
    pub fn mine_blocks(&self, count: u32) -> Result<Vec<String>, BitcoinError> {
        let address = self
            .mining_address
            .clone()
            .ok_or_else(|| BitcoinError::InvalidAddress("Mining address not set".to_string()))?;

        let block_hashes = self
            .client
            .generate_to_address(count as u64, &address)
            .map_err(|e| BitcoinError::RpcError(e.to_string()))?;

        Ok(block_hashes.iter().map(|h| h.to_string()).collect())
    }

    /// Mine a single block
    pub fn mine_block(&self) -> Result<String, BitcoinError> {
        let blocks = self.mine_blocks(1)?;
        Ok(blocks
            .into_iter()
            .next()
            .expect("mine_blocks(1) always returns exactly one block hash"))
    }

    /// Fund a wallet address with test coins
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use kaccy_bitcoin::{BitcoinClient, BitcoinNetwork, regtest_utils::{RegtestHelper, RegtestConfig}};
    /// # use bitcoin::Address;
    /// # use std::str::FromStr;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = BitcoinClient::new("http://localhost:18443", "user", "pass", BitcoinNetwork::Regtest)?;
    /// # let helper = RegtestHelper::new(client, RegtestConfig::default());
    /// let address = Address::from_str("bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")?
    ///     .require_network(bitcoin::Network::Regtest)?;
    /// let txid = helper.fund_address(&address, None)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn fund_address(
        &self,
        address: &Address,
        amount: Option<u64>,
    ) -> Result<String, BitcoinError> {
        let amount_satoshis = amount.unwrap_or(self.config.default_funding_amount);
        let amount_btc = Amount::from_sat(amount_satoshis);

        let txid = self
            .client
            .send_to_address(address, amount_btc)
            .map_err(|e| BitcoinError::RpcError(e.to_string()))?;

        // Auto-mine if enabled
        if self.config.auto_mine {
            self.mine_blocks(self.config.confirmation_blocks)?;
        }

        Ok(txid.to_string())
    }

    /// Wait for a transaction to be confirmed
    pub fn wait_for_confirmation(&self, txid: &str) -> Result<u32, BitcoinError> {
        let txid_parsed = bitcoin::Txid::from_str(txid)
            .map_err(|e| BitcoinError::InvalidTransaction(e.to_string()))?;

        // Mine blocks to confirm the transaction
        self.mine_blocks(self.config.confirmation_blocks)?;

        // Get confirmation count
        let tx_info = self
            .client
            .get_transaction(&txid_parsed)
            .map_err(|e| BitcoinError::RpcError(e.to_string()))?;

        Ok(tx_info.info.confirmations as u32)
    }

    /// Advance blockchain time by mining empty blocks
    pub fn advance_time(&self, blocks: u32) -> Result<(), BitcoinError> {
        self.mine_blocks(blocks)?;
        Ok(())
    }

    /// Get current block height
    pub fn get_height(&self) -> Result<u64, BitcoinError> {
        self.client.get_block_height()
    }

    /// Invalidate a block (simulate reorg)
    pub fn invalidate_block(&self, block_hash: &str) -> Result<(), BitcoinError> {
        let hash = bitcoin::BlockHash::from_str(block_hash)
            .map_err(|e| BitcoinError::InvalidTransaction(e.to_string()))?;

        self.client
            .invalidate_block(&hash)
            .map_err(|e| BitcoinError::RpcError(e.to_string()))?;

        Ok(())
    }

    /// Reconsider a block (undo invalidation)
    pub fn reconsider_block(&self, block_hash: &str) -> Result<(), BitcoinError> {
        let hash = bitcoin::BlockHash::from_str(block_hash)
            .map_err(|e| BitcoinError::InvalidTransaction(e.to_string()))?;

        self.client
            .reconsider_block(&hash)
            .map_err(|e| BitcoinError::RpcError(e.to_string()))?;

        Ok(())
    }

    /// Simulate a blockchain reorganization
    ///
    /// Creates a fork by invalidating recent blocks and mining a longer chain
    pub fn simulate_reorg(&self, depth: u32) -> Result<ReorgInfo, BitcoinError> {
        let start_height = self.get_height()?;

        // Get the hash of the block at the fork point
        let fork_height = start_height.saturating_sub(depth as u64);
        let fork_hash = self.client.get_block_hash(fork_height)?.to_string();

        // Invalidate blocks from the fork point
        for i in 0..depth {
            let height = start_height - i as u64;
            let hash = self.client.get_block_hash(height)?.to_string();
            self.invalidate_block(&hash)?;
        }

        let current_height = self.get_height()?;

        // Mine a longer chain (depth + 1 blocks)
        self.mine_blocks(depth + 1)?;

        let new_height = self.get_height()?;

        Ok(ReorgInfo {
            fork_height,
            fork_hash,
            old_height: start_height,
            new_height,
            depth,
            blocks_invalidated: depth,
            blocks_added: (new_height - current_height) as u32,
        })
    }

    /// Generate a test wallet with funded addresses
    pub fn generate_test_wallet(
        &self,
        num_addresses: usize,
    ) -> Result<Vec<(Address, String)>, BitcoinError> {
        let mut wallet = Vec::new();

        for _ in 0..num_addresses {
            let address = self.client.get_new_address(None)?.assume_checked();
            let txid = self.fund_address(&address, None)?;
            wallet.push((address, txid));
        }

        Ok(wallet)
    }

    /// Reset regtest chain to genesis
    pub fn reset_chain(&mut self) -> Result<(), BitcoinError> {
        // Get genesis block hash
        let genesis_hash = self.client.get_block_hash(0)?.to_string();

        // Invalidate all blocks after genesis
        let current_height = self.get_height()?;
        for height in (1..=current_height).rev() {
            let hash = self.client.get_block_hash(height)?.to_string();
            self.invalidate_block(&hash)?;
        }

        // Reconsider genesis
        self.reconsider_block(&genesis_hash)?;

        // Re-initialize
        self.initialize()?;

        Ok(())
    }
}

/// Information about a blockchain reorganization
#[derive(Debug, Clone)]
pub struct ReorgInfo {
    /// Height of the fork point
    pub fork_height: u64,
    /// Hash of the block at the fork point
    pub fork_hash: String,
    /// Old chain height before reorg
    pub old_height: u64,
    /// New chain height after reorg
    pub new_height: u64,
    /// Depth of the reorg
    pub depth: u32,
    /// Number of blocks invalidated
    pub blocks_invalidated: u32,
    /// Number of new blocks added
    pub blocks_added: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regtest_config_default() {
        let config = RegtestConfig::default();
        assert_eq!(config.initial_blocks, 101);
        assert_eq!(config.confirmation_blocks, 6);
        assert!(config.auto_mine);
        assert_eq!(config.default_funding_amount, 100_000_000);
    }

    #[test]
    fn test_reorg_info_fields() {
        let info = ReorgInfo {
            fork_height: 100,
            fork_hash: "abc123".to_string(),
            old_height: 110,
            new_height: 112,
            depth: 10,
            blocks_invalidated: 10,
            blocks_added: 12,
        };

        assert_eq!(info.fork_height, 100);
        assert_eq!(info.depth, 10);
    }
}
