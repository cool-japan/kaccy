//! Atomic Swap functionality for trustless cross-chain exchanges
//!
//! This module implements Hash Time-Locked Contracts (HTLCs) for atomic swaps,
//! enabling trustless exchange of Bitcoin with other cryptocurrencies.

use bitcoin::{Amount, Network};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::{BitcoinError, Result};
use crate::timelock::{HtlcContract, HtlcManager, TimeLockType};

/// Atomic swap role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapRole {
    /// Initiator of the swap
    Initiator,
    /// Participant (responder) of the swap
    Participant,
}

/// Atomic swap status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapStatus {
    /// Swap initiated, waiting for participant
    Initiated,
    /// Participant has locked funds
    Locked,
    /// Swap completed successfully
    Completed,
    /// Swap refunded (timeout)
    Refunded,
    /// Swap cancelled
    Cancelled,
}

/// Atomic swap configuration
#[derive(Debug, Clone)]
pub struct AtomicSwapConfig {
    /// Bitcoin network
    pub network: Network,
    /// Initiator timelock (in blocks)
    pub initiator_timelock_blocks: u32,
    /// Participant timelock (in blocks, must be less than initiator)
    pub participant_timelock_blocks: u32,
}

impl Default for AtomicSwapConfig {
    fn default() -> Self {
        Self {
            network: Network::Bitcoin,
            // Initiator has longer timeout to ensure they can claim after participant reveals preimage
            initiator_timelock_blocks: 288,   // ~48 hours
            participant_timelock_blocks: 144, // ~24 hours
        }
    }
}

/// Atomic swap details
#[derive(Debug, Clone)]
pub struct AtomicSwap {
    /// Unique swap ID
    pub swap_id: String,
    /// Role in the swap
    pub role: SwapRole,
    /// Payment hash
    pub payment_hash: [u8; 32],
    /// Payment preimage (only known to initiator initially)
    pub preimage: Option<Vec<u8>>,
    /// Initiator's public key
    pub initiator_pubkey: Vec<u8>,
    /// Participant's public key
    pub participant_pubkey: Vec<u8>,
    /// Amount to swap
    pub amount: Amount,
    /// HTLC contract
    pub htlc: HtlcContract,
    /// Current status
    pub status: SwapStatus,
    /// Creation time
    pub created_at: DateTime<Utc>,
    /// Funding transaction ID (when funded)
    pub funding_txid: Option<String>,
    /// Claim transaction ID (when claimed)
    pub claim_txid: Option<String>,
}

/// Atomic swap manager
///
/// Manages trustless cross-chain atomic swaps using Hash Time-Locked Contracts (HTLCs).
///
/// # Examples
///
/// ```no_run
/// use kaccy_bitcoin::{AtomicSwapConfig, AtomicSwapManager};
/// use bitcoin::{Amount, Network};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = AtomicSwapConfig {
///     network: Network::Testnet,
///     initiator_timelock_blocks: 288,
///     participant_timelock_blocks: 144,
/// };
///
/// let manager = AtomicSwapManager::new(config);
///
/// // Initiate a swap
/// let swap = manager.initiate_swap(
///     "swap123".to_string(),
///     Amount::from_sat(100_000),
///     vec![0u8; 33], // initiator pubkey
///     vec![0u8; 33], // participant pubkey
/// )?;
///
/// println!("Swap ID: {}", swap.swap_id);
/// # Ok(())
/// # }
/// ```
pub struct AtomicSwapManager {
    config: AtomicSwapConfig,
    htlc_manager: HtlcManager,
    active_swaps: Arc<RwLock<HashMap<String, AtomicSwap>>>,
}

impl AtomicSwapManager {
    /// Create a new atomic swap manager
    ///
    /// # Examples
    ///
    /// ```
    /// use kaccy_bitcoin::{AtomicSwapConfig, AtomicSwapManager};
    /// use bitcoin::Network;
    ///
    /// let config = AtomicSwapConfig::default();
    /// let manager = AtomicSwapManager::new(config);
    /// ```
    pub fn new(config: AtomicSwapConfig) -> Self {
        let htlc_manager = HtlcManager::new(config.network);
        Self {
            config,
            htlc_manager,
            active_swaps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initiate a new atomic swap
    pub fn initiate_swap(
        &self,
        swap_id: String,
        amount: Amount,
        initiator_pubkey: Vec<u8>,
        participant_pubkey: Vec<u8>,
    ) -> Result<AtomicSwap> {
        // Generate a random preimage
        let preimage = self.generate_preimage();
        let payment_hash = HtlcManager::generate_payment_hash(&preimage);

        // Create HTLC with initiator's timelock
        let timelock = TimeLockType::RelativeBlocks(self.config.initiator_timelock_blocks as u16);
        let htlc = self.htlc_manager.create_htlc(
            payment_hash,
            initiator_pubkey.clone(),
            participant_pubkey.clone(),
            timelock,
        )?;

        let swap = AtomicSwap {
            swap_id: swap_id.clone(),
            role: SwapRole::Initiator,
            payment_hash,
            preimage: Some(preimage),
            initiator_pubkey,
            participant_pubkey,
            amount,
            htlc,
            status: SwapStatus::Initiated,
            created_at: Utc::now(),
            funding_txid: None,
            claim_txid: None,
        };

        // Store in active swaps
        let mut swaps = self.active_swaps.write().unwrap_or_else(|e| e.into_inner());
        swaps.insert(swap_id, swap.clone());

        Ok(swap)
    }

    /// Participate in an existing atomic swap
    pub fn participate_swap(
        &self,
        swap_id: String,
        payment_hash: [u8; 32],
        amount: Amount,
        initiator_pubkey: Vec<u8>,
        participant_pubkey: Vec<u8>,
    ) -> Result<AtomicSwap> {
        // Create HTLC with participant's timelock (shorter than initiator's)
        let timelock = TimeLockType::RelativeBlocks(self.config.participant_timelock_blocks as u16);
        let htlc = self.htlc_manager.create_htlc(
            payment_hash,
            participant_pubkey.clone(),
            initiator_pubkey.clone(),
            timelock,
        )?;

        let swap = AtomicSwap {
            swap_id: swap_id.clone(),
            role: SwapRole::Participant,
            payment_hash,
            preimage: None, // Participant doesn't know preimage initially
            initiator_pubkey,
            participant_pubkey,
            amount,
            htlc,
            status: SwapStatus::Initiated,
            created_at: Utc::now(),
            funding_txid: None,
            claim_txid: None,
        };

        // Store in active swaps
        let mut swaps = self.active_swaps.write().unwrap_or_else(|e| e.into_inner());
        swaps.insert(swap_id, swap.clone());

        Ok(swap)
    }

    /// Mark swap as funded
    pub fn mark_funded(&self, swap_id: &str, funding_txid: String) -> Result<()> {
        let mut swaps = self.active_swaps.write().unwrap_or_else(|e| e.into_inner());
        let swap = swaps
            .get_mut(swap_id)
            .ok_or_else(|| BitcoinError::Validation(format!("Swap {} not found", swap_id)))?;

        swap.funding_txid = Some(funding_txid);
        swap.status = SwapStatus::Locked;
        swap.htlc.mark_active();

        Ok(())
    }

    /// Claim funds from the swap (revealing the preimage)
    pub fn claim_swap(&self, swap_id: &str, preimage: Vec<u8>) -> Result<Vec<u8>> {
        let mut swaps = self.active_swaps.write().unwrap_or_else(|e| e.into_inner());
        let swap = swaps
            .get_mut(swap_id)
            .ok_or_else(|| BitcoinError::Validation(format!("Swap {} not found", swap_id)))?;

        // Verify preimage
        if !HtlcManager::verify_preimage(&preimage, &swap.payment_hash) {
            return Err(BitcoinError::Validation("Invalid preimage".to_string()));
        }

        // Update swap status
        swap.preimage = Some(preimage.clone());
        swap.status = SwapStatus::Completed;
        swap.htlc.mark_claimed();

        Ok(preimage)
    }

    /// Refund swap after timeout
    pub fn refund_swap(&self, swap_id: &str) -> Result<()> {
        let mut swaps = self.active_swaps.write().unwrap_or_else(|e| e.into_inner());
        let swap = swaps
            .get_mut(swap_id)
            .ok_or_else(|| BitcoinError::Validation(format!("Swap {} not found", swap_id)))?;

        // Update swap status
        swap.status = SwapStatus::Refunded;
        swap.htlc.mark_refunded();

        Ok(())
    }

    /// Get swap details
    pub fn get_swap(&self, swap_id: &str) -> Option<AtomicSwap> {
        let swaps = self.active_swaps.read().unwrap_or_else(|e| e.into_inner());
        swaps.get(swap_id).cloned()
    }

    /// List all active swaps
    pub fn list_swaps(&self) -> Vec<AtomicSwap> {
        let swaps = self.active_swaps.read().unwrap_or_else(|e| e.into_inner());
        swaps.values().cloned().collect()
    }

    /// Generate a random preimage
    fn generate_preimage(&self) -> Vec<u8> {
        use bitcoin::secp256k1::rand::RngCore;
        use bitcoin::secp256k1::rand::rngs::OsRng;

        let mut preimage = vec![0u8; 32];
        OsRng.fill_bytes(&mut preimage);
        preimage
    }

    /// Cancel a swap
    pub fn cancel_swap(&self, swap_id: &str) -> Result<()> {
        let mut swaps = self.active_swaps.write().unwrap_or_else(|e| e.into_inner());
        let swap = swaps
            .get_mut(swap_id)
            .ok_or_else(|| BitcoinError::Validation(format!("Swap {} not found", swap_id)))?;

        if swap.status != SwapStatus::Initiated {
            return Err(BitcoinError::Validation(
                "Can only cancel initiated swaps".to_string(),
            ));
        }

        swap.status = SwapStatus::Cancelled;
        Ok(())
    }
}

/// Atomic swap protocol steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwapStep {
    /// 1. Initiator creates swap and generates preimage
    InitiatorCreate,
    /// 2. Initiator locks funds in HTLC
    InitiatorLock,
    /// 3. Participant creates matching swap (same hash)
    ParticipantCreate,
    /// 4. Participant locks funds in HTLC
    ParticipantLock,
    /// 5. Initiator claims participant's funds (reveals preimage)
    InitiatorClaim,
    /// 6. Participant claims initiator's funds (using revealed preimage)
    ParticipantClaim,
    /// Alternative: Refund paths if timeouts expire
    Refund,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_swap_config_defaults() {
        let config = AtomicSwapConfig::default();
        assert_eq!(config.network, Network::Bitcoin);
        assert_eq!(config.initiator_timelock_blocks, 288);
        assert_eq!(config.participant_timelock_blocks, 144);
        // Initiator timeout should be longer
        assert!(config.initiator_timelock_blocks > config.participant_timelock_blocks);
    }

    #[test]
    fn test_initiate_swap() {
        let config = AtomicSwapConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = AtomicSwapManager::new(config);

        let swap = manager
            .initiate_swap(
                "swap1".to_string(),
                Amount::from_sat(100000),
                vec![0x02; 33],
                vec![0x03; 33],
            )
            .unwrap();

        assert_eq!(swap.swap_id, "swap1");
        assert_eq!(swap.role, SwapRole::Initiator);
        assert_eq!(swap.status, SwapStatus::Initiated);
        assert!(swap.preimage.is_some());
    }

    #[test]
    fn test_participate_swap() {
        let config = AtomicSwapConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = AtomicSwapManager::new(config);

        let payment_hash = HtlcManager::generate_payment_hash(b"secret");

        let swap = manager
            .participate_swap(
                "swap1".to_string(),
                payment_hash,
                Amount::from_sat(100000),
                vec![0x02; 33],
                vec![0x03; 33],
            )
            .unwrap();

        assert_eq!(swap.swap_id, "swap1");
        assert_eq!(swap.role, SwapRole::Participant);
        assert_eq!(swap.status, SwapStatus::Initiated);
        assert!(swap.preimage.is_none());
    }

    #[test]
    fn test_swap_lifecycle() {
        let config = AtomicSwapConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = AtomicSwapManager::new(config);

        // Initiate swap
        let swap = manager
            .initiate_swap(
                "swap1".to_string(),
                Amount::from_sat(100000),
                vec![0x02; 33],
                vec![0x03; 33],
            )
            .unwrap();

        // Fund swap
        manager.mark_funded("swap1", "txid123".to_string()).unwrap();
        let funded_swap = manager.get_swap("swap1").unwrap();
        assert_eq!(funded_swap.status, SwapStatus::Locked);

        // Claim swap with preimage
        let preimage = swap.preimage.clone().unwrap();
        manager.claim_swap("swap1", preimage).unwrap();
        let completed_swap = manager.get_swap("swap1").unwrap();
        assert_eq!(completed_swap.status, SwapStatus::Completed);
    }

    #[test]
    fn test_claim_with_invalid_preimage() {
        let config = AtomicSwapConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = AtomicSwapManager::new(config);

        manager
            .initiate_swap(
                "swap1".to_string(),
                Amount::from_sat(100000),
                vec![0x02; 33],
                vec![0x03; 33],
            )
            .unwrap();

        manager.mark_funded("swap1", "txid123".to_string()).unwrap();

        // Try to claim with wrong preimage
        let wrong_preimage = b"wrong_preimage".to_vec();
        let result = manager.claim_swap("swap1", wrong_preimage);
        assert!(result.is_err());
    }

    #[test]
    fn test_refund_swap() {
        let config = AtomicSwapConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = AtomicSwapManager::new(config);

        manager
            .initiate_swap(
                "swap1".to_string(),
                Amount::from_sat(100000),
                vec![0x02; 33],
                vec![0x03; 33],
            )
            .unwrap();

        manager.mark_funded("swap1", "txid123".to_string()).unwrap();

        manager.refund_swap("swap1").unwrap();
        let swap = manager.get_swap("swap1").unwrap();
        assert_eq!(swap.status, SwapStatus::Refunded);
    }

    #[test]
    fn test_cancel_swap() {
        let config = AtomicSwapConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = AtomicSwapManager::new(config);

        manager
            .initiate_swap(
                "swap1".to_string(),
                Amount::from_sat(100000),
                vec![0x02; 33],
                vec![0x03; 33],
            )
            .unwrap();

        manager.cancel_swap("swap1").unwrap();
        let swap = manager.get_swap("swap1").unwrap();
        assert_eq!(swap.status, SwapStatus::Cancelled);
    }

    #[test]
    fn test_list_swaps() {
        let config = AtomicSwapConfig {
            network: Network::Testnet,
            ..Default::default()
        };
        let manager = AtomicSwapManager::new(config);

        manager
            .initiate_swap(
                "swap1".to_string(),
                Amount::from_sat(100000),
                vec![0x02; 33],
                vec![0x03; 33],
            )
            .unwrap();

        manager
            .initiate_swap(
                "swap2".to_string(),
                Amount::from_sat(200000),
                vec![0x02; 33],
                vec![0x03; 33],
            )
            .unwrap();

        let swaps = manager.list_swaps();
        assert_eq!(swaps.len(), 2);
    }
}
