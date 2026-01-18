//! Time-locked transactions and Hash Time-Locked Contracts (HTLC)
//!
//! This module provides support for time-locked transactions, which are essential
//! for advanced payment flows like Lightning Network and atomic swaps.

use bitcoin::absolute::LockTime;
use bitcoin::blockdata::opcodes;
use bitcoin::hashes::{Hash, sha256};
use bitcoin::{Address, Amount, Network, ScriptBuf, Sequence, TxOut};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Time-lock type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeLockType {
    /// Absolute block height (nLockTime with block height)
    BlockHeight(u32),
    /// Absolute timestamp (nLockTime with UNIX timestamp)
    Timestamp(u32),
    /// Relative block height (nSequence with blocks)
    RelativeBlocks(u16),
    /// Relative time (nSequence with 512-second intervals)
    RelativeTime(u16),
}

impl TimeLockType {
    /// Check if the timelock has expired
    pub fn is_expired(&self, current_height: u32, current_time: u32) -> bool {
        match self {
            TimeLockType::BlockHeight(height) => current_height >= *height,
            TimeLockType::Timestamp(timestamp) => current_time >= *timestamp,
            TimeLockType::RelativeBlocks(_) | TimeLockType::RelativeTime(_) => {
                // Relative timelocks are evaluated differently
                false
            }
        }
    }

    /// Convert to nLockTime value
    pub fn to_lock_time(&self) -> Option<LockTime> {
        match self {
            TimeLockType::BlockHeight(height) => LockTime::from_height(*height).ok(),
            TimeLockType::Timestamp(timestamp) => LockTime::from_time(*timestamp).ok(),
            _ => None,
        }
    }

    /// Convert to nSequence value
    pub fn to_sequence(&self) -> Option<Sequence> {
        match self {
            TimeLockType::RelativeBlocks(blocks) => Some(Sequence::from_height(*blocks)),
            TimeLockType::RelativeTime(intervals) => {
                Some(Sequence::from_512_second_intervals(*intervals))
            }
            _ => None,
        }
    }
}

/// Hash Time-Locked Contract (HTLC) configuration
#[derive(Debug, Clone)]
pub struct HtlcConfig {
    /// Payment hash (SHA256)
    pub payment_hash: [u8; 32],
    /// Sender's public key
    pub sender_pubkey: Vec<u8>,
    /// Receiver's public key
    pub receiver_pubkey: Vec<u8>,
    /// Time lock
    pub timelock: TimeLockType,
    /// Network
    pub network: Network,
}

/// HTLC script builder
pub struct HtlcScriptBuilder {
    config: HtlcConfig,
}

impl HtlcScriptBuilder {
    /// Create a new HTLC script builder
    pub fn new(config: HtlcConfig) -> Self {
        Self { config }
    }

    /// Build the HTLC script
    ///
    /// Script format (simplified for compatibility):
    /// ```text
    /// OP_IF
    ///     OP_SHA256 <payment_hash> OP_EQUALVERIFY <receiver_pubkey> OP_CHECKSIG
    /// OP_ELSE
    ///     <timelock> OP_CHECKLOCKTIMEVERIFY OP_DROP <sender_pubkey> OP_CHECKSIG
    /// OP_ENDIF
    /// ```
    ///
    /// Note: This is a simplified implementation. For production use,
    /// consider using a proper script template library or manual script construction.
    pub fn build_script(&self) -> Result<ScriptBuf> {
        // Simplified implementation: create a basic script structure
        // In a production implementation, you would use proper script building with
        // the correct types and methods

        let mut script_bytes = Vec::new();

        // OP_IF
        script_bytes.push(opcodes::all::OP_IF.to_u8());

        // Receiver path: OP_SHA256 <hash> OP_EQUALVERIFY ...
        script_bytes.push(opcodes::all::OP_SHA256.to_u8());
        script_bytes.push(32); // Push 32 bytes
        script_bytes.extend_from_slice(&self.config.payment_hash);
        script_bytes.push(opcodes::all::OP_EQUALVERIFY.to_u8());

        // Receiver pubkey (simplified)
        if !self.config.receiver_pubkey.is_empty() {
            script_bytes.push(self.config.receiver_pubkey.len() as u8);
            script_bytes.extend_from_slice(&self.config.receiver_pubkey);
        }
        script_bytes.push(opcodes::all::OP_CHECKSIG.to_u8());

        // OP_ELSE
        script_bytes.push(opcodes::all::OP_ELSE.to_u8());

        // Sender path with timelock
        match self.config.timelock {
            TimeLockType::BlockHeight(height) | TimeLockType::Timestamp(height) => {
                let height_bytes = height.to_le_bytes();
                script_bytes.push(height_bytes.len() as u8);
                script_bytes.extend_from_slice(&height_bytes);
                script_bytes.push(opcodes::all::OP_CLTV.to_u8());
                script_bytes.push(opcodes::all::OP_DROP.to_u8());
            }
            TimeLockType::RelativeBlocks(_) | TimeLockType::RelativeTime(_) => {
                if let Some(sequence) = self.config.timelock.to_sequence() {
                    let seq_bytes = sequence.to_consensus_u32().to_le_bytes();
                    script_bytes.push(seq_bytes.len() as u8);
                    script_bytes.extend_from_slice(&seq_bytes);
                    script_bytes.push(opcodes::all::OP_CSV.to_u8());
                    script_bytes.push(opcodes::all::OP_DROP.to_u8());
                }
            }
        }

        // Sender pubkey
        if !self.config.sender_pubkey.is_empty() {
            script_bytes.push(self.config.sender_pubkey.len() as u8);
            script_bytes.extend_from_slice(&self.config.sender_pubkey);
        }
        script_bytes.push(opcodes::all::OP_CHECKSIG.to_u8());

        // OP_ENDIF
        script_bytes.push(opcodes::all::OP_ENDIF.to_u8());

        Ok(ScriptBuf::from_bytes(script_bytes))
    }

    /// Create P2WSH address for the HTLC
    pub fn create_address(&self) -> Result<Address> {
        let script = self.build_script()?;
        let address = Address::p2wsh(&script, self.config.network);
        Ok(address)
    }
}

/// HTLC manager
pub struct HtlcManager {
    network: Network,
}

impl HtlcManager {
    /// Create a new HTLC manager
    pub fn new(network: Network) -> Self {
        Self { network }
    }

    /// Create a new HTLC
    pub fn create_htlc(
        &self,
        payment_hash: [u8; 32],
        sender_pubkey: Vec<u8>,
        receiver_pubkey: Vec<u8>,
        timelock: TimeLockType,
    ) -> Result<HtlcContract> {
        let config = HtlcConfig {
            payment_hash,
            sender_pubkey,
            receiver_pubkey,
            timelock,
            network: self.network,
        };

        let builder = HtlcScriptBuilder::new(config.clone());
        let script = builder.build_script()?;
        let address = builder.create_address()?;

        Ok(HtlcContract {
            config,
            script,
            address,
            status: HtlcStatus::Pending,
            created_at: Utc::now(),
        })
    }

    /// Generate payment hash from preimage
    pub fn generate_payment_hash(preimage: &[u8]) -> [u8; 32] {
        let hash = sha256::Hash::hash(preimage);
        hash.to_byte_array()
    }

    /// Verify payment preimage
    pub fn verify_preimage(preimage: &[u8], payment_hash: &[u8; 32]) -> bool {
        let computed_hash = Self::generate_payment_hash(preimage);
        &computed_hash == payment_hash
    }
}

/// HTLC contract
#[derive(Debug, Clone)]
pub struct HtlcContract {
    /// Configuration
    pub config: HtlcConfig,
    /// The HTLC script
    pub script: ScriptBuf,
    /// The P2WSH address
    pub address: Address,
    /// Current status
    pub status: HtlcStatus,
    /// Creation time
    pub created_at: DateTime<Utc>,
}

/// HTLC status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtlcStatus {
    /// Waiting for funding
    Pending,
    /// Funded and active
    Active,
    /// Claimed by receiver (preimage revealed)
    Claimed,
    /// Refunded to sender (timelock expired)
    Refunded,
    /// Expired without claim or refund
    Expired,
}

impl HtlcContract {
    /// Check if the HTLC can be claimed
    pub fn can_claim(&self, current_height: u32, current_time: u32) -> bool {
        matches!(self.status, HtlcStatus::Active)
            && !self
                .config
                .timelock
                .is_expired(current_height, current_time)
    }

    /// Check if the HTLC can be refunded
    pub fn can_refund(&self, current_height: u32, current_time: u32) -> bool {
        matches!(self.status, HtlcStatus::Active)
            && self
                .config
                .timelock
                .is_expired(current_height, current_time)
    }

    /// Update status to claimed
    pub fn mark_claimed(&mut self) {
        self.status = HtlcStatus::Claimed;
    }

    /// Update status to refunded
    pub fn mark_refunded(&mut self) {
        self.status = HtlcStatus::Refunded;
    }

    /// Update status to active
    pub fn mark_active(&mut self) {
        self.status = HtlcStatus::Active;
    }
}

/// Simple timelock transaction builder
pub struct TimelockTxBuilder {
    #[allow(dead_code)]
    network: Network,
}

impl TimelockTxBuilder {
    /// Create a new timelock transaction builder
    pub fn new(network: Network) -> Self {
        Self { network }
    }

    /// Create a time-locked output
    pub fn create_timelock_output(
        &self,
        recipient: &Address,
        amount: Amount,
        _timelock: TimeLockType,
    ) -> Result<TxOut> {
        // For simple timelocks, we use a standard output
        // The timelock is enforced by the transaction's nLockTime or input's nSequence
        Ok(TxOut {
            value: amount,
            script_pubkey: recipient.script_pubkey(),
        })
    }

    /// Check if a transaction's timelock has expired
    pub fn is_timelock_expired(
        &self,
        timelock: &TimeLockType,
        current_height: u32,
        current_time: u32,
    ) -> bool {
        timelock.is_expired(current_height, current_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timelock_block_height() {
        let timelock = TimeLockType::BlockHeight(100);
        assert!(!timelock.is_expired(99, 0));
        assert!(timelock.is_expired(100, 0));
        assert!(timelock.is_expired(101, 0));
    }

    #[test]
    fn test_timelock_timestamp() {
        let timelock = TimeLockType::Timestamp(1000000);
        assert!(!timelock.is_expired(0, 999999));
        assert!(timelock.is_expired(0, 1000000));
        assert!(timelock.is_expired(0, 1000001));
    }

    #[test]
    fn test_payment_hash_generation() {
        let preimage = b"test_preimage";
        let hash = HtlcManager::generate_payment_hash(preimage);
        assert_eq!(hash.len(), 32);
        assert!(HtlcManager::verify_preimage(preimage, &hash));
    }

    #[test]
    fn test_payment_hash_verification() {
        let preimage = b"test_preimage";
        let hash = HtlcManager::generate_payment_hash(preimage);
        let wrong_preimage = b"wrong_preimage";
        assert!(!HtlcManager::verify_preimage(wrong_preimage, &hash));
    }

    #[test]
    fn test_htlc_creation() {
        let manager = HtlcManager::new(Network::Testnet);
        let payment_hash = HtlcManager::generate_payment_hash(b"secret");
        let sender_pubkey = vec![0x02; 33];
        let receiver_pubkey = vec![0x03; 33];
        let timelock = TimeLockType::BlockHeight(100);

        let htlc = manager
            .create_htlc(payment_hash, sender_pubkey, receiver_pubkey, timelock)
            .unwrap();

        assert_eq!(htlc.status, HtlcStatus::Pending);
        assert!(!htlc.script.is_empty());
    }

    #[test]
    fn test_htlc_can_claim() {
        let manager = HtlcManager::new(Network::Testnet);
        let payment_hash = HtlcManager::generate_payment_hash(b"secret");
        let timelock = TimeLockType::BlockHeight(100);

        let mut htlc = manager
            .create_htlc(payment_hash, vec![0x02; 33], vec![0x03; 33], timelock)
            .unwrap();

        htlc.mark_active();

        // Can claim before timelock expiry
        assert!(htlc.can_claim(99, 0));
        // Cannot claim after timelock expiry
        assert!(!htlc.can_claim(100, 0));
    }

    #[test]
    fn test_htlc_can_refund() {
        let manager = HtlcManager::new(Network::Testnet);
        let payment_hash = HtlcManager::generate_payment_hash(b"secret");
        let timelock = TimeLockType::BlockHeight(100);

        let mut htlc = manager
            .create_htlc(payment_hash, vec![0x02; 33], vec![0x03; 33], timelock)
            .unwrap();

        htlc.mark_active();

        // Cannot refund before timelock expiry
        assert!(!htlc.can_refund(99, 0));
        // Can refund after timelock expiry
        assert!(htlc.can_refund(100, 0));
    }

    #[test]
    fn test_htlc_status_transitions() {
        let manager = HtlcManager::new(Network::Testnet);
        let payment_hash = HtlcManager::generate_payment_hash(b"secret");
        let timelock = TimeLockType::BlockHeight(100);

        let mut htlc = manager
            .create_htlc(payment_hash, vec![0x02; 33], vec![0x03; 33], timelock)
            .unwrap();

        assert_eq!(htlc.status, HtlcStatus::Pending);

        htlc.mark_active();
        assert_eq!(htlc.status, HtlcStatus::Active);

        htlc.mark_claimed();
        assert_eq!(htlc.status, HtlcStatus::Claimed);
    }

    #[test]
    fn test_timelock_to_lock_time() {
        let block_height = TimeLockType::BlockHeight(100);
        assert!(block_height.to_lock_time().is_some());

        // nLockTime timestamps must be >= 500000000 (LOCKTIME_THRESHOLD)
        let timestamp = TimeLockType::Timestamp(1600000000); // Valid Unix timestamp
        assert!(timestamp.to_lock_time().is_some());

        // Invalid timestamp (too low)
        let invalid_timestamp = TimeLockType::Timestamp(1000);
        assert!(invalid_timestamp.to_lock_time().is_none());

        let relative = TimeLockType::RelativeBlocks(10);
        assert!(relative.to_lock_time().is_none());
    }

    #[test]
    fn test_timelock_to_sequence() {
        let relative_blocks = TimeLockType::RelativeBlocks(10);
        assert!(relative_blocks.to_sequence().is_some());

        let relative_time = TimeLockType::RelativeTime(100);
        assert!(relative_time.to_sequence().is_some());

        let absolute = TimeLockType::BlockHeight(100);
        assert!(absolute.to_sequence().is_none());
    }
}
