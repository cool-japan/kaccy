//! Submarine swaps between on-chain Bitcoin and Lightning Network
//!
//! Submarine swaps enable trustless atomic swaps between on-chain BTC
//! and Lightning Network payments using HTLCs.

use bitcoin::Network;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{BitcoinError, Result};

/// Submarine swap direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapDirection {
    /// On-chain to Lightning (submarine swap)
    OnchainToLightning,
    /// Lightning to on-chain (reverse submarine swap)
    LightningToOnchain,
}

/// Submarine swap status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubmarineSwapStatus {
    /// Swap initiated
    Initiated,
    /// Waiting for on-chain payment
    WaitingForOnchain,
    /// On-chain payment detected
    OnchainDetected,
    /// Waiting for Lightning invoice
    WaitingForInvoice,
    /// Lightning invoice created
    InvoiceCreated,
    /// Lightning payment sent
    LightningPaymentSent,
    /// Lightning payment confirmed
    LightningPaymentConfirmed,
    /// Swap completed successfully
    Completed,
    /// Swap failed
    Failed,
    /// Swap refunded
    Refunded,
    /// Swap expired
    Expired,
}

/// Submarine swap details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmarineSwap {
    /// Unique swap ID
    pub id: Uuid,
    /// Swap direction
    pub direction: SwapDirection,
    /// Current status
    pub status: SubmarineSwapStatus,
    /// Amount in satoshis
    pub amount_sats: u64,
    /// Service fee in satoshis
    pub fee_sats: u64,
    /// Payment hash (HTLC)
    pub payment_hash: String,
    /// Payment preimage (revealed when claimed)
    pub preimage: Option<String>,
    /// On-chain address for payment (as string)
    pub onchain_address: Option<String>,
    /// Lightning invoice (for Lightning -> On-chain)
    pub lightning_invoice: Option<String>,
    /// On-chain transaction ID (when detected)
    pub txid: Option<String>,
    /// Refund address (as string)
    pub refund_address: String,
    /// Locktime for refund (block height or timestamp)
    pub locktime: u32,
    /// Network
    pub network: Network,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Expiration timestamp
    pub expires_at: DateTime<Utc>,
    /// Completion timestamp
    pub completed_at: Option<DateTime<Utc>>,
}

impl SubmarineSwap {
    /// Create a new submarine swap (on-chain to Lightning)
    ///
    /// # Examples
    ///
    /// ```
    /// use kaccy_bitcoin::SubmarineSwap;
    /// use bitcoin::Network;
    ///
    /// let swap = SubmarineSwap::new_onchain_to_lightning(
    ///     100_000,  // amount in sats
    ///     1_000,    // fee in sats
    ///     "abc123".to_string(),  // payment hash
    ///     "bc1qaddr".to_string(), // on-chain address
    ///     "bc1qrefund".to_string(), // refund address
    ///     144,      // locktime blocks
    ///     Network::Testnet,
    ///     24,       // expiry hours
    /// );
    ///
    /// assert_eq!(swap.amount_sats, 100_000);
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn new_onchain_to_lightning(
        amount_sats: u64,
        fee_sats: u64,
        payment_hash: String,
        onchain_address: String,
        refund_address: String,
        locktime: u32,
        network: Network,
        expiry_hours: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            direction: SwapDirection::OnchainToLightning,
            status: SubmarineSwapStatus::Initiated,
            amount_sats,
            fee_sats,
            payment_hash,
            preimage: None,
            onchain_address: Some(onchain_address),
            lightning_invoice: None,
            txid: None,
            refund_address,
            locktime,
            network,
            created_at: now,
            expires_at: now + Duration::hours(expiry_hours),
            completed_at: None,
        }
    }

    /// Create a new reverse submarine swap (Lightning to on-chain)
    #[allow(clippy::too_many_arguments)]
    pub fn new_lightning_to_onchain(
        amount_sats: u64,
        fee_sats: u64,
        payment_hash: String,
        lightning_invoice: String,
        refund_address: String,
        locktime: u32,
        network: Network,
        expiry_hours: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            direction: SwapDirection::LightningToOnchain,
            status: SubmarineSwapStatus::Initiated,
            amount_sats,
            fee_sats,
            payment_hash,
            preimage: None,
            onchain_address: None,
            lightning_invoice: Some(lightning_invoice),
            txid: None,
            refund_address,
            locktime,
            network,
            created_at: now,
            expires_at: now + Duration::hours(expiry_hours),
            completed_at: None,
        }
    }

    /// Check if the swap has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Update swap status
    pub fn update_status(&mut self, status: SubmarineSwapStatus) {
        self.status = status;
        if matches!(
            status,
            SubmarineSwapStatus::Completed
                | SubmarineSwapStatus::Failed
                | SubmarineSwapStatus::Refunded
                | SubmarineSwapStatus::Expired
        ) {
            self.completed_at = Some(Utc::now());
        }
    }

    /// Set the preimage (when swap is claimed)
    pub fn set_preimage(&mut self, preimage: String) {
        self.preimage = Some(preimage);
    }

    /// Set the on-chain transaction ID
    pub fn set_txid(&mut self, txid: String) {
        self.txid = Some(txid);
    }

    /// Calculate total cost including fees
    pub fn total_cost(&self) -> u64 {
        self.amount_sats + self.fee_sats
    }

    /// Calculate the amount received (after fees)
    pub fn amount_received(&self) -> u64 {
        self.amount_sats.saturating_sub(self.fee_sats)
    }
}

/// Submarine swap configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmarineSwapConfig {
    /// Minimum swap amount in satoshis
    pub min_amount: u64,
    /// Maximum swap amount in satoshis
    pub max_amount: u64,
    /// Service fee percentage (basis points, 100 = 1%)
    pub fee_percentage: u64,
    /// Minimum service fee in satoshis
    pub min_fee: u64,
    /// Default swap expiry in hours
    pub default_expiry_hours: i64,
    /// Refund locktime in blocks (for on-chain to Lightning)
    pub onchain_to_ln_locktime_blocks: u32,
    /// Refund locktime in blocks (for Lightning to on-chain)
    pub ln_to_onchain_locktime_blocks: u32,
    /// Required on-chain confirmations
    pub required_confirmations: u32,
}

impl Default for SubmarineSwapConfig {
    fn default() -> Self {
        Self {
            min_amount: 10_000,                 // 10k sats
            max_amount: 100_000_000,            // 1 BTC
            fee_percentage: 100,                // 1%
            min_fee: 1_000,                     // 1k sats minimum fee
            default_expiry_hours: 24,           // 24 hours
            onchain_to_ln_locktime_blocks: 144, // ~24 hours
            ln_to_onchain_locktime_blocks: 144,
            required_confirmations: 3,
        }
    }
}

impl SubmarineSwapConfig {
    /// Calculate fee for a swap amount
    pub fn calculate_fee(&self, amount_sats: u64) -> u64 {
        let percentage_fee = (amount_sats * self.fee_percentage) / 10_000;
        percentage_fee.max(self.min_fee)
    }

    /// Validate swap amount
    pub fn validate_amount(&self, amount_sats: u64) -> Result<()> {
        if amount_sats < self.min_amount {
            return Err(BitcoinError::Validation(format!(
                "Amount {} is below minimum {}",
                amount_sats, self.min_amount
            )));
        }
        if amount_sats > self.max_amount {
            return Err(BitcoinError::Validation(format!(
                "Amount {} exceeds maximum {}",
                amount_sats, self.max_amount
            )));
        }
        Ok(())
    }
}

/// Submarine swap service manager
pub struct SubmarineSwapService {
    config: SubmarineSwapConfig,
    swaps: Arc<RwLock<HashMap<Uuid, SubmarineSwap>>>,
    network: Network,
}

impl SubmarineSwapService {
    /// Create a new submarine swap service
    pub fn new(config: SubmarineSwapConfig, network: Network) -> Self {
        Self {
            config,
            swaps: Arc::new(RwLock::new(HashMap::new())),
            network,
        }
    }

    /// Create a new on-chain to Lightning swap
    pub async fn create_onchain_to_lightning_swap(
        &self,
        amount_sats: u64,
        payment_hash: String,
        onchain_address: String,
        refund_address: String,
    ) -> Result<SubmarineSwap> {
        // Validate amount
        self.config.validate_amount(amount_sats)?;

        // Calculate fee
        let fee = self.config.calculate_fee(amount_sats);

        // Create swap
        let swap = SubmarineSwap::new_onchain_to_lightning(
            amount_sats,
            fee,
            payment_hash,
            onchain_address,
            refund_address,
            self.config.onchain_to_ln_locktime_blocks,
            self.network,
            self.config.default_expiry_hours,
        );

        // Store swap
        self.swaps.write().await.insert(swap.id, swap.clone());

        tracing::info!(
            swap_id = %swap.id,
            amount = amount_sats,
            fee = fee,
            "Created on-chain to Lightning swap"
        );

        Ok(swap)
    }

    /// Create a new Lightning to on-chain swap
    pub async fn create_lightning_to_onchain_swap(
        &self,
        amount_sats: u64,
        payment_hash: String,
        lightning_invoice: String,
        refund_address: String,
    ) -> Result<SubmarineSwap> {
        // Validate amount
        self.config.validate_amount(amount_sats)?;

        // Calculate fee
        let fee = self.config.calculate_fee(amount_sats);

        // Create swap
        let swap = SubmarineSwap::new_lightning_to_onchain(
            amount_sats,
            fee,
            payment_hash,
            lightning_invoice,
            refund_address,
            self.config.ln_to_onchain_locktime_blocks,
            self.network,
            self.config.default_expiry_hours,
        );

        // Store swap
        self.swaps.write().await.insert(swap.id, swap.clone());

        tracing::info!(
            swap_id = %swap.id,
            amount = amount_sats,
            fee = fee,
            "Created Lightning to on-chain swap"
        );

        Ok(swap)
    }

    /// Get a swap by ID
    pub async fn get_swap(&self, swap_id: Uuid) -> Option<SubmarineSwap> {
        self.swaps.read().await.get(&swap_id).cloned()
    }

    /// Update swap status
    pub async fn update_swap_status(
        &self,
        swap_id: Uuid,
        status: SubmarineSwapStatus,
    ) -> Result<()> {
        let mut swaps = self.swaps.write().await;
        if let Some(swap) = swaps.get_mut(&swap_id) {
            swap.update_status(status);
            tracing::info!(
                swap_id = %swap_id,
                status = ?status,
                "Updated swap status"
            );
            Ok(())
        } else {
            Err(BitcoinError::Validation(format!(
                "Swap {} not found",
                swap_id
            )))
        }
    }

    /// Set swap preimage (when claimed)
    pub async fn set_swap_preimage(&self, swap_id: Uuid, preimage: String) -> Result<()> {
        let mut swaps = self.swaps.write().await;
        if let Some(swap) = swaps.get_mut(&swap_id) {
            swap.set_preimage(preimage);
            swap.update_status(SubmarineSwapStatus::Completed);
            tracing::info!(swap_id = %swap_id, "Swap claimed with preimage");
            Ok(())
        } else {
            Err(BitcoinError::Validation(format!(
                "Swap {} not found",
                swap_id
            )))
        }
    }

    /// List all active swaps
    pub async fn list_active_swaps(&self) -> Vec<SubmarineSwap> {
        self.swaps
            .read()
            .await
            .values()
            .filter(|swap| {
                !matches!(
                    swap.status,
                    SubmarineSwapStatus::Completed
                        | SubmarineSwapStatus::Failed
                        | SubmarineSwapStatus::Refunded
                        | SubmarineSwapStatus::Expired
                )
            })
            .cloned()
            .collect()
    }

    /// Clean up expired swaps
    pub async fn cleanup_expired_swaps(&self) -> usize {
        let mut swaps = self.swaps.write().await;
        let mut expired_count = 0;

        for swap in swaps.values_mut() {
            if swap.is_expired()
                && matches!(
                    swap.status,
                    SubmarineSwapStatus::Initiated
                        | SubmarineSwapStatus::WaitingForOnchain
                        | SubmarineSwapStatus::WaitingForInvoice
                )
            {
                swap.update_status(SubmarineSwapStatus::Expired);
                expired_count += 1;
            }
        }

        if expired_count > 0 {
            tracing::info!(count = expired_count, "Cleaned up expired swaps");
        }

        expired_count
    }

    /// Get swap statistics
    pub async fn get_statistics(&self) -> SubmarineSwapStatistics {
        let swaps = self.swaps.read().await;

        let total_swaps = swaps.len();
        let completed_swaps = swaps
            .values()
            .filter(|s| s.status == SubmarineSwapStatus::Completed)
            .count();
        let failed_swaps = swaps
            .values()
            .filter(|s| {
                matches!(
                    s.status,
                    SubmarineSwapStatus::Failed
                        | SubmarineSwapStatus::Refunded
                        | SubmarineSwapStatus::Expired
                )
            })
            .count();
        let active_swaps = swaps
            .values()
            .filter(|s| {
                !matches!(
                    s.status,
                    SubmarineSwapStatus::Completed
                        | SubmarineSwapStatus::Failed
                        | SubmarineSwapStatus::Refunded
                        | SubmarineSwapStatus::Expired
                )
            })
            .count();

        let total_volume_sats = swaps
            .values()
            .filter(|s| s.status == SubmarineSwapStatus::Completed)
            .map(|s| s.amount_sats)
            .sum();

        let total_fees_sats = swaps
            .values()
            .filter(|s| s.status == SubmarineSwapStatus::Completed)
            .map(|s| s.fee_sats)
            .sum();

        SubmarineSwapStatistics {
            total_swaps,
            completed_swaps,
            failed_swaps,
            active_swaps,
            total_volume_sats,
            total_fees_sats,
        }
    }
}

/// Statistics for submarine swaps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmarineSwapStatistics {
    /// Total number of swaps
    pub total_swaps: usize,
    /// Number of completed swaps
    pub completed_swaps: usize,
    /// Number of failed swaps
    pub failed_swaps: usize,
    /// Number of active swaps
    pub active_swaps: usize,
    /// Total volume in satoshis
    pub total_volume_sats: u64,
    /// Total fees collected in satoshis
    pub total_fees_sats: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_config_defaults() {
        let config = SubmarineSwapConfig::default();
        assert_eq!(config.min_amount, 10_000);
        assert_eq!(config.max_amount, 100_000_000);
        assert_eq!(config.fee_percentage, 100); // 1%
    }

    #[test]
    fn test_fee_calculation() {
        let config = SubmarineSwapConfig::default();

        // Test percentage fee
        let fee = config.calculate_fee(100_000); // 100k sats
        assert_eq!(fee, 1_000); // 1% = 1k sats

        // Test minimum fee
        let fee = config.calculate_fee(50_000); // 50k sats
        assert!(fee >= config.min_fee);
    }

    #[test]
    fn test_amount_validation() {
        let config = SubmarineSwapConfig::default();

        // Valid amount
        assert!(config.validate_amount(50_000).is_ok());

        // Below minimum
        assert!(config.validate_amount(5_000).is_err());

        // Above maximum
        assert!(config.validate_amount(200_000_000).is_err());
    }

    #[test]
    fn test_swap_creation() {
        let address = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string();
        let refund_address = address.clone();

        let swap = SubmarineSwap::new_onchain_to_lightning(
            100_000,
            1_000,
            "payment_hash_123".to_string(),
            address,
            refund_address,
            144,
            Network::Bitcoin,
            24,
        );

        assert_eq!(swap.direction, SwapDirection::OnchainToLightning);
        assert_eq!(swap.status, SubmarineSwapStatus::Initiated);
        assert_eq!(swap.amount_sats, 100_000);
        assert_eq!(swap.fee_sats, 1_000);
        assert_eq!(swap.total_cost(), 101_000);
        assert_eq!(swap.amount_received(), 99_000);
    }

    #[test]
    fn test_swap_expiration() {
        let address = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string();
        let refund_address = address.clone();

        let mut swap = SubmarineSwap::new_onchain_to_lightning(
            100_000,
            1_000,
            "payment_hash_123".to_string(),
            address,
            refund_address,
            144,
            Network::Bitcoin,
            24,
        );

        // Should not be expired initially
        assert!(!swap.is_expired());

        // Set expiration to past
        swap.expires_at = Utc::now() - Duration::hours(1);
        assert!(swap.is_expired());
    }

    #[tokio::test]
    async fn test_submarine_swap_service() {
        let config = SubmarineSwapConfig::default();
        let service = SubmarineSwapService::new(config, Network::Bitcoin);

        let address = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string();
        let refund_address = address.clone();

        let swap = service
            .create_onchain_to_lightning_swap(
                100_000,
                "payment_hash_123".to_string(),
                address,
                refund_address,
            )
            .await
            .unwrap();

        assert_eq!(swap.amount_sats, 100_000);
        assert_eq!(swap.status, SubmarineSwapStatus::Initiated);

        // Retrieve swap
        let retrieved = service.get_swap(swap.id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, swap.id);
    }

    #[tokio::test]
    async fn test_swap_status_update() {
        let config = SubmarineSwapConfig::default();
        let service = SubmarineSwapService::new(config, Network::Bitcoin);

        let address = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".to_string();

        let swap = service
            .create_onchain_to_lightning_swap(
                100_000,
                "payment_hash_123".to_string(),
                address.clone(),
                address,
            )
            .await
            .unwrap();

        // Update status
        service
            .update_swap_status(swap.id, SubmarineSwapStatus::OnchainDetected)
            .await
            .unwrap();

        let updated = service.get_swap(swap.id).await.unwrap();
        assert_eq!(updated.status, SubmarineSwapStatus::OnchainDetected);
    }
}
