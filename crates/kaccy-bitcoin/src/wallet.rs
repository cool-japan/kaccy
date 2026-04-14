//! Wallet management

use bitcoin::Amount;
use std::sync::Arc;
use uuid::Uuid;

use crate::client::BitcoinClient;
use crate::error::Result;

/// Manages wallet operations for the platform
pub struct WalletManager {
    client: Arc<BitcoinClient>,
}

impl WalletManager {
    /// Create a new wallet manager using the given Bitcoin client
    pub fn new(client: Arc<BitcoinClient>) -> Self {
        Self { client }
    }

    /// Generate a unique deposit address for an order
    pub fn generate_deposit_address(&self, order_id: Uuid) -> Result<String> {
        let label = format!("order:{}", order_id);
        let address = self.client.get_new_address(Some(&label))?;
        Ok(address.assume_checked().to_string())
    }

    /// Generate a deposit address for a user (for general deposits)
    pub fn generate_user_deposit_address(&self, user_id: Uuid) -> Result<String> {
        let label = format!("user:{}", user_id);
        let address = self.client.get_new_address(Some(&label))?;
        Ok(address.assume_checked().to_string())
    }

    /// Get wallet balance
    pub fn get_wallet_balance(&self) -> Result<Amount> {
        self.client.get_balance()
    }

    /// Validate a BTC address
    pub fn validate_address(&self, address: &str) -> Result<bool> {
        let validation = self.client.validate_address(address)?;
        Ok(validation.is_valid)
    }

    /// Estimate fee for withdrawal (in satoshis per vbyte)
    pub fn estimate_withdrawal_fee(&self) -> Result<u64> {
        // Target 6 blocks for confirmation (~1 hour)
        let fee_rate = self.client.estimate_smart_fee(6)?;
        Ok(fee_rate.unwrap_or(10.0) as u64)
    }
}

/// Represents a deposit that was detected
#[derive(Debug, Clone)]
pub struct DetectedDeposit {
    /// Receiving address for this deposit
    pub address: String,
    /// Amount of the deposit
    pub amount: Amount,
    /// Transaction ID
    pub txid: String,
    /// Number of confirmations
    pub confirmations: u32,
}

/// Represents a pending withdrawal
#[derive(Debug, Clone)]
pub struct PendingWithdrawal {
    /// User requesting the withdrawal
    pub user_id: Uuid,
    /// Bitcoin address to send funds to
    pub destination_address: String,
    /// Withdrawal amount
    pub amount: Amount,
    /// Estimated network fee
    pub fee: Amount,
}
