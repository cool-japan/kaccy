//! MEV Mitigation Strategies
//!
//! This module implements various strategies to mitigate MEV extraction,
//! protecting users from sandwich attacks, front-running, and other MEV exploits.

use crate::error::CoreError;
use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit, Nonce},
};
use chacha20poly1305::ChaCha20Poly1305;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};

/// Length of the AEAD nonce in bytes (96 bits, the standard for both AES-256-GCM
/// and ChaCha20-Poly1305).
const AEAD_NONCE_LEN: usize = 12;

/// Length of the AEAD symmetric key in bytes (256 bits).
const AEAD_KEY_LEN: usize = 32;

/// Length of the Poly1305 / GCM authentication tag in bytes (128 bits).
const AEAD_TAG_LEN: usize = 16;

/// Domain-separation label for deriving the AEAD encryption key from the reveal key.
///
/// Using a label distinct from the commitment-hash label guarantees that the publicly
/// stored `decryption_key_hash` (see [`commitment_hash`]) can never be used to recover the
/// actual encryption key, even though both are derived from the same reveal key.
const AEAD_KEY_DERIVATION_LABEL: &[u8] = b"kaccy-mev/commit-reveal/aead-key/v1";

/// Domain-separation label for the public reveal-key commitment hash.
const KEY_COMMITMENT_LABEL: &[u8] = b"kaccy-mev/commit-reveal/key-commitment/v1";

/// AEAD construction used to encrypt a committed transaction.
///
/// Both variants provide IND-CCA2 authenticated encryption with a 256-bit key, a 96-bit
/// nonce and a 128-bit authentication tag; they differ only in the underlying primitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AeadScheme {
    /// ChaCha20-Poly1305 (RFC 8439). Constant-time in software, the default.
    ChaCha20Poly1305,
    /// AES-256-GCM (NIST SP 800-38D). Hardware-accelerated where AES-NI is available.
    Aes256Gcm,
}

impl AeadScheme {
    /// Default AEAD scheme used by the convenience commit helpers.
    pub const DEFAULT: Self = Self::ChaCha20Poly1305;
}

/// Derive the 256-bit AEAD key from an arbitrary-length reveal key.
///
/// The reveal key supplied by callers may be any length; the AEAD ciphers require an
/// exactly 32-byte key, so we deterministically derive one as
/// `SHA-256(AEAD_KEY_DERIVATION_LABEL || reveal_key)`. The domain-separation label keeps
/// this key distinct from the public commitment hash.
fn derive_aead_key(reveal_key: &[u8]) -> [u8; AEAD_KEY_LEN] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(AEAD_KEY_DERIVATION_LABEL);
    hasher.update(reveal_key);
    let digest = hasher.finalize();
    let mut key = [0u8; AEAD_KEY_LEN];
    key.copy_from_slice(&digest);
    key
}

/// Compute the public commitment for a reveal key.
///
/// This is the value bound at commit time and stored as `decryption_key_hash`. It is
/// `hex(SHA-256(KEY_COMMITMENT_LABEL || reveal_key))`. Because it uses a label distinct from
/// [`derive_aead_key`], publishing the commitment leaks nothing about the AEAD key.
pub fn commitment_hash(reveal_key: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(KEY_COMMITMENT_LABEL);
    hasher.update(reveal_key);
    hex::encode(hasher.finalize())
}

/// Generate a fresh random 96-bit nonce from the operating-system CSPRNG.
fn random_nonce() -> [u8; AEAD_NONCE_LEN] {
    let mut nonce = [0u8; AEAD_NONCE_LEN];
    rand::fill(&mut nonce);
    nonce
}

/// Authenticated-encrypt `plaintext` under `reveal_key` using the requested AEAD scheme.
///
/// A fresh random nonce is generated and prepended to the ciphertext (which already carries
/// the authentication tag). The returned blob is laid out as `nonce || ciphertext || tag`
/// and is exactly what [`aead_decrypt`] expects.
fn aead_encrypt(
    scheme: AeadScheme,
    plaintext: &[u8],
    reveal_key: &[u8],
) -> Result<Vec<u8>, CoreError> {
    let key = derive_aead_key(reveal_key);
    let nonce_bytes = random_nonce();

    let ciphertext = match scheme {
        AeadScheme::ChaCha20Poly1305 => {
            let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| {
                CoreError::Validation(format!("Failed to initialize ChaCha20-Poly1305: {e}"))
            })?;
            cipher
                .encrypt(&Nonce::<ChaCha20Poly1305>::from(nonce_bytes), plaintext)
                .map_err(|_| {
                    CoreError::Validation("ChaCha20-Poly1305 encryption failed".to_string())
                })?
        }
        AeadScheme::Aes256Gcm => {
            let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| {
                CoreError::Validation(format!("Failed to initialize AES-256-GCM: {e}"))
            })?;
            cipher
                .encrypt(&Nonce::<Aes256Gcm>::from(nonce_bytes), plaintext)
                .map_err(|_| CoreError::Validation("AES-256-GCM encryption failed".to_string()))?
        }
    };

    let mut blob = Vec::with_capacity(AEAD_NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(blob)
}

/// Authenticated-decrypt a `nonce || ciphertext || tag` blob produced by [`aead_encrypt`].
///
/// The AEAD authentication tag is verified as part of decryption: a tampered ciphertext or a
/// wrong key yields a [`CoreError::Validation`] error instead of corrupted plaintext.
fn aead_decrypt(scheme: AeadScheme, blob: &[u8], reveal_key: &[u8]) -> Result<Vec<u8>, CoreError> {
    // A valid blob must contain at least a nonce and a full authentication tag.
    if blob.len() < AEAD_NONCE_LEN + AEAD_TAG_LEN {
        return Err(CoreError::Validation(
            "Ciphertext too short to contain nonce and authentication tag".to_string(),
        ));
    }
    let (nonce_bytes, ciphertext) = blob.split_at(AEAD_NONCE_LEN);

    let key = derive_aead_key(reveal_key);
    match scheme {
        AeadScheme::ChaCha20Poly1305 => {
            let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|e| {
                CoreError::Validation(format!("Failed to initialize ChaCha20-Poly1305: {e}"))
            })?;
            let nonce = Nonce::<ChaCha20Poly1305>::try_from(nonce_bytes)
                .map_err(|_| CoreError::Validation("Invalid nonce length".to_string()))?;
            cipher.decrypt(&nonce, ciphertext).map_err(|_| {
                CoreError::Validation(
                    "ChaCha20-Poly1305 authentication failed (tampered data or wrong key)"
                        .to_string(),
                )
            })
        }
        AeadScheme::Aes256Gcm => {
            let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| {
                CoreError::Validation(format!("Failed to initialize AES-256-GCM: {e}"))
            })?;
            let nonce = Nonce::<Aes256Gcm>::try_from(nonce_bytes)
                .map_err(|_| CoreError::Validation("Invalid nonce length".to_string()))?;
            cipher.decrypt(&nonce, ciphertext).map_err(|_| {
                CoreError::Validation(
                    "AES-256-GCM authentication failed (tampered data or wrong key)".to_string(),
                )
            })
        }
    }
}

/// Types of MEV mitigation strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MitigationStrategy {
    /// Bundle submission - transactions executed together atomically
    BundleSubmission,
    /// Transaction ordering protection - fair ordering rules
    FairOrdering,
    /// MEV rebate - share MEV profits with users
    MevRebate,
    /// Privacy-preserving execution - hide transaction details
    PrivateExecution,
    /// Time-delayed execution - randomized execution time
    TimeDelayed,
    /// Threshold encryption - decrypt only when threshold reached
    ThresholdEncryption,
}

/// Configuration for MEV rebate mechanism
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevRebateConfig {
    /// Percentage of detected MEV to rebate to victims (0-100)
    pub rebate_percentage: Decimal,
    /// Minimum MEV amount to trigger rebate
    pub min_rebate_amount: Decimal,
    /// Duration to wait before processing rebate
    pub processing_delay: Duration,
}

impl Default for MevRebateConfig {
    fn default() -> Self {
        Self {
            rebate_percentage: Decimal::new(50, 0),      // 50% rebate
            min_rebate_amount: Decimal::new(1, 3),       // 0.001 BTC
            processing_delay: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// MEV rebate record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevRebate {
    /// Unique identifier for this rebate
    pub rebate_id: String,
    /// User who was harmed by MEV extraction
    pub victim_user_id: String,
    /// User who performed MEV extraction
    pub attacker_user_id: String,
    /// Total MEV amount extracted
    pub mev_amount: Decimal,
    /// Amount to be rebated to the victim
    pub rebate_amount: Decimal,
    /// Transaction IDs involved in the MEV event
    pub transaction_ids: Vec<String>,
    /// When this rebate record was created
    pub created_at: SystemTime,
    /// When this rebate was processed
    pub processed_at: Option<SystemTime>,
    /// Current processing status
    pub status: RebateStatus,
}

/// Status of an MEV rebate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebateStatus {
    /// Rebate is queued and awaiting processing
    Pending,
    /// Rebate is currently being processed
    Processing,
    /// Rebate has been successfully paid out
    Completed,
    /// Rebate processing failed
    Failed,
}

/// MEV rebate manager
///
/// Manages the distribution of MEV rebates to victims of MEV extraction
pub struct MevRebateManager {
    /// Rebate configuration
    config: MevRebateConfig,
    /// All rebates indexed by rebate ID
    pending_rebates: HashMap<String, MevRebate>,
    /// Monotonic counter used to generate unique rebate IDs
    rebate_counter: u64,
}

impl MevRebateManager {
    /// Create a new MEV rebate manager with the given configuration
    pub fn new(config: MevRebateConfig) -> Self {
        Self {
            config,
            pending_rebates: HashMap::new(),
            rebate_counter: 0,
        }
    }

    /// Create a new MEV rebate manager with default configuration
    pub fn with_defaults() -> Self {
        Self::new(MevRebateConfig::default())
    }

    /// Creates a new rebate for a victim of MEV extraction
    pub fn create_rebate(
        &mut self,
        victim_user_id: String,
        attacker_user_id: String,
        mev_amount: Decimal,
        transaction_ids: Vec<String>,
    ) -> Result<MevRebate, CoreError> {
        if mev_amount < self.config.min_rebate_amount {
            return Err(CoreError::Validation(
                "MEV amount below minimum threshold".to_string(),
            ));
        }

        let rebate_amount = mev_amount * self.config.rebate_percentage / Decimal::new(100, 0);

        self.rebate_counter += 1;
        let rebate_id = format!("rebate_{}", self.rebate_counter);

        let rebate = MevRebate {
            rebate_id: rebate_id.clone(),
            victim_user_id,
            attacker_user_id,
            mev_amount,
            rebate_amount,
            transaction_ids,
            created_at: SystemTime::now(),
            processed_at: None,
            status: RebateStatus::Pending,
        };

        self.pending_rebates.insert(rebate_id, rebate.clone());
        Ok(rebate)
    }

    /// Processes pending rebates that have passed the processing delay
    pub fn process_pending_rebates(&mut self) -> Vec<MevRebate> {
        let now = SystemTime::now();
        let mut processed = Vec::new();

        for rebate in self.pending_rebates.values_mut() {
            if rebate.status != RebateStatus::Pending {
                continue;
            }

            let elapsed = now
                .duration_since(rebate.created_at)
                .unwrap_or(Duration::ZERO);

            if elapsed >= self.config.processing_delay {
                rebate.status = RebateStatus::Processing;
                processed.push(rebate.clone());
            }
        }

        processed
    }

    /// Marks a rebate as completed
    pub fn complete_rebate(&mut self, rebate_id: &str) -> Result<(), CoreError> {
        let rebate = self
            .pending_rebates
            .get_mut(rebate_id)
            .ok_or_else(|| CoreError::NotFound(format!("Rebate {} not found", rebate_id)))?;

        rebate.status = RebateStatus::Completed;
        rebate.processed_at = Some(SystemTime::now());
        Ok(())
    }

    /// Marks a rebate as failed
    pub fn fail_rebate(&mut self, rebate_id: &str) -> Result<(), CoreError> {
        let rebate = self
            .pending_rebates
            .get_mut(rebate_id)
            .ok_or_else(|| CoreError::NotFound(format!("Rebate {} not found", rebate_id)))?;

        rebate.status = RebateStatus::Failed;
        Ok(())
    }

    /// Gets all rebates for a specific victim
    pub fn get_victim_rebates(&self, user_id: &str) -> Vec<&MevRebate> {
        self.pending_rebates
            .values()
            .filter(|r| r.victim_user_id == user_id)
            .collect()
    }

    /// Gets total rebate amount for a victim
    pub fn get_total_rebate_amount(&self, user_id: &str) -> Decimal {
        self.pending_rebates
            .values()
            .filter(|r| r.victim_user_id == user_id && r.status == RebateStatus::Completed)
            .map(|r| r.rebate_amount)
            .sum()
    }
}

/// Transaction bundle for atomic execution
///
/// Bundles multiple transactions together to prevent sandwich attacks
/// by ensuring they execute atomically in a specific order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionBundle {
    /// Unique identifier for this bundle
    pub bundle_id: String,
    /// Ordered list of transactions in this bundle
    pub transactions: Vec<BundledTransaction>,
    /// Earliest acceptable execution time
    pub min_timestamp: Option<SystemTime>,
    /// Latest acceptable execution time
    pub max_timestamp: Option<SystemTime>,
    /// Whether to revert all transactions if any one fails
    pub revert_on_failure: bool,
}

/// A single transaction inside a bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundledTransaction {
    /// Transaction identifier
    pub tx_id: String,
    /// User who submitted this transaction
    pub user_id: String,
    /// Position of this transaction within the bundle (zero-indexed)
    pub order_index: usize,
    /// Raw encoded transaction data
    pub data: Vec<u8>,
}

impl TransactionBundle {
    /// Create an empty transaction bundle with the given ID
    pub fn new(bundle_id: String) -> Self {
        Self {
            bundle_id,
            transactions: Vec::new(),
            min_timestamp: None,
            max_timestamp: None,
            revert_on_failure: true,
        }
    }

    /// Adds a transaction to the bundle
    pub fn add_transaction(&mut self, tx: BundledTransaction) -> Result<(), CoreError> {
        // Ensure order indices are sequential
        let expected_index = self.transactions.len();
        if tx.order_index != expected_index {
            return Err(CoreError::Validation(format!(
                "Invalid order index. Expected {}, got {}",
                expected_index, tx.order_index
            )));
        }

        self.transactions.push(tx);
        Ok(())
    }

    /// Validates the bundle
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.transactions.is_empty() {
            return Err(CoreError::Validation("Bundle cannot be empty".to_string()));
        }

        // Check timestamp constraints
        if let (Some(min), Some(max)) = (self.min_timestamp, self.max_timestamp) {
            if min > max {
                return Err(CoreError::Validation(
                    "min_timestamp cannot be greater than max_timestamp".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Sets time constraints for bundle execution
    pub fn with_time_constraints(
        mut self,
        min_timestamp: SystemTime,
        max_timestamp: SystemTime,
    ) -> Self {
        self.min_timestamp = Some(min_timestamp);
        self.max_timestamp = Some(max_timestamp);
        self
    }
}

/// Fair ordering manager
///
/// Implements fair transaction ordering to prevent front-running
#[derive(Debug)]
pub struct FairOrderingManager {
    /// Transactions waiting to be included in the next batch
    pending_queue: VecDeque<PendingTransaction>,
    /// How long to accumulate transactions before ordering and dispatching them
    batch_interval: Duration,
    /// Timestamp of the most recently dispatched batch
    last_batch_time: SystemTime,
}

#[derive(Debug, Clone)]
struct PendingTransaction {
    tx_id: String,
    #[allow(dead_code)]
    user_id: String,
    received_at: SystemTime,
    #[allow(dead_code)]
    priority_score: Decimal,
}

impl FairOrderingManager {
    /// Create a new fair ordering manager with the specified batch interval
    pub fn new(batch_interval: Duration) -> Self {
        Self {
            pending_queue: VecDeque::new(),
            batch_interval,
            last_batch_time: SystemTime::now(),
        }
    }

    /// Create a fair ordering manager with a 10-second batch interval
    pub fn with_defaults() -> Self {
        Self::new(Duration::from_secs(10)) // 10-second batches
    }

    /// Adds a transaction to the pending queue
    pub fn submit_transaction(&mut self, tx_id: String, user_id: String) -> Result<(), CoreError> {
        let tx = PendingTransaction {
            tx_id,
            user_id,
            received_at: SystemTime::now(),
            priority_score: Decimal::ZERO,
        };

        self.pending_queue.push_back(tx);
        Ok(())
    }

    /// Processes a batch of transactions using fair ordering
    pub fn process_batch(&mut self) -> Result<Vec<String>, CoreError> {
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(self.last_batch_time)
            .unwrap_or(Duration::ZERO);

        if elapsed < self.batch_interval {
            return Ok(Vec::new()); // Not time for next batch yet
        }

        // Collect all transactions in the current batch window
        let mut batch = Vec::new();
        while let Some(tx) = self.pending_queue.pop_front() {
            batch.push(tx);
        }

        if batch.is_empty() {
            return Ok(Vec::new());
        }

        // Sort by received time (FIFO) - this is the fair ordering
        batch.sort_by_key(|a| a.received_at);

        // Extract transaction IDs in fair order
        let ordered_tx_ids: Vec<String> = batch.iter().map(|tx| tx.tx_id.clone()).collect();

        self.last_batch_time = now;
        Ok(ordered_tx_ids)
    }

    /// Gets the number of pending transactions
    pub fn pending_count(&self) -> usize {
        self.pending_queue.len()
    }

    /// Gets time until next batch
    pub fn time_until_next_batch(&self) -> Duration {
        let elapsed = SystemTime::now()
            .duration_since(self.last_batch_time)
            .unwrap_or(Duration::ZERO);

        self.batch_interval.saturating_sub(elapsed)
    }
}

/// Privacy-preserving execution engine
///
/// Provides mechanisms for executing transactions without revealing details
/// until after execution, preventing MEV extraction.
#[derive(Debug)]
pub struct PrivateExecutionEngine {
    /// Encrypted transactions awaiting decryption and execution
    encrypted_pool: HashMap<String, EncryptedTransaction>,
}

#[derive(Debug, Clone)]
struct EncryptedTransaction {
    #[allow(dead_code)]
    tx_id: String,
    /// AEAD ciphertext laid out as `nonce || ciphertext || tag` (see [`aead_encrypt`]).
    encrypted_data: Vec<u8>,
    /// AEAD construction used to encrypt `encrypted_data`.
    scheme: AeadScheme,
    #[allow(dead_code)]
    submitted_at: SystemTime,
    /// Public commitment to the reveal key, `hex(SHA-256(label || key))`.
    ///
    /// Bound at commit time; the supplied reveal key is checked against this commitment
    /// before any decryption is attempted, so the revealed key is provably the one the
    /// committer intended.
    decryption_key_hash: String,
    /// Optional time-lock: the reveal key must not be accepted before this instant.
    ///
    /// When set, [`PrivateExecutionEngine::decrypt_and_execute`] rejects decryption attempts
    /// made before the unlock time even if the correct key is presented, modelling a
    /// commit-reveal scheme where the key only becomes valid at/after the unlock time.
    unlock_time: Option<SystemTime>,
}

impl PrivateExecutionEngine {
    /// Create an empty private execution engine
    pub fn new() -> Self {
        Self {
            encrypted_pool: HashMap::new(),
        }
    }

    /// Submits an already-encrypted transaction (commit phase).
    ///
    /// `encrypted_data` must be a `nonce || ciphertext || tag` blob produced by
    /// [`Self::encrypt_transaction`] using the default AEAD scheme, and
    /// `decryption_key_hash` must be the [`commitment_hash`] of the reveal key. The reveal
    /// key itself is never supplied here, so the transaction stays confidential until it is
    /// revealed via [`Self::decrypt_and_execute`].
    pub fn submit_encrypted_transaction(
        &mut self,
        tx_id: String,
        encrypted_data: Vec<u8>,
        decryption_key_hash: String,
    ) -> Result<(), CoreError> {
        self.submit_encrypted_transaction_with_scheme(
            tx_id,
            encrypted_data,
            decryption_key_hash,
            AeadScheme::DEFAULT,
            None,
        )
    }

    /// Submits an already-encrypted transaction, specifying the AEAD scheme and an optional
    /// time-lock (commit phase).
    ///
    /// `encrypted_data` must be a `nonce || ciphertext || tag` blob produced by
    /// [`Self::encrypt_transaction_with_scheme`] under the same `scheme`. When `unlock_time`
    /// is `Some`, the revealed key is only accepted at/after that instant.
    pub fn submit_encrypted_transaction_with_scheme(
        &mut self,
        tx_id: String,
        encrypted_data: Vec<u8>,
        decryption_key_hash: String,
        scheme: AeadScheme,
        unlock_time: Option<SystemTime>,
    ) -> Result<(), CoreError> {
        if encrypted_data.len() < AEAD_NONCE_LEN + AEAD_TAG_LEN {
            return Err(CoreError::Validation(
                "Encrypted transaction too short to contain nonce and authentication tag"
                    .to_string(),
            ));
        }
        if self.encrypted_pool.contains_key(&tx_id) {
            return Err(CoreError::AlreadyExists(format!(
                "Encrypted transaction {tx_id} already committed"
            )));
        }

        let encrypted_tx = EncryptedTransaction {
            tx_id: tx_id.clone(),
            encrypted_data,
            scheme,
            submitted_at: SystemTime::now(),
            decryption_key_hash,
            unlock_time,
        };

        self.encrypted_pool.insert(tx_id, encrypted_tx);
        Ok(())
    }

    /// Authenticated-encrypt raw transaction bytes under `reveal_key` (default AEAD scheme).
    ///
    /// Returns a `nonce || ciphertext || tag` blob suitable for
    /// [`Self::submit_encrypted_transaction`]. Each call uses a fresh random 96-bit nonce, so
    /// encrypting identical plaintexts yields distinct ciphertexts.
    pub fn encrypt_transaction(tx_bytes: &[u8], reveal_key: &[u8]) -> Result<Vec<u8>, CoreError> {
        aead_encrypt(AeadScheme::DEFAULT, tx_bytes, reveal_key)
    }

    /// Authenticated-encrypt raw transaction bytes under `reveal_key` with an explicit scheme.
    pub fn encrypt_transaction_with_scheme(
        tx_bytes: &[u8],
        reveal_key: &[u8],
        scheme: AeadScheme,
    ) -> Result<Vec<u8>, CoreError> {
        aead_encrypt(scheme, tx_bytes, reveal_key)
    }

    /// Encrypt and commit a transaction in one step (commit phase, default scheme).
    ///
    /// Derives the reveal-key commitment, encrypts `tx_bytes` and stores the ciphertext. The
    /// returned value is the ciphertext blob that was committed. The reveal key must be kept
    /// secret by the caller until the reveal phase.
    pub fn commit_transaction(
        &mut self,
        tx_id: String,
        tx_bytes: &[u8],
        reveal_key: &[u8],
    ) -> Result<Vec<u8>, CoreError> {
        self.commit_transaction_with_unlock(tx_id, tx_bytes, reveal_key, AeadScheme::DEFAULT, None)
    }

    /// Encrypt and commit a transaction with an explicit scheme and optional time-lock.
    ///
    /// When `unlock_time` is `Some`, [`Self::decrypt_and_execute`] refuses to reveal the
    /// transaction before that instant, even when the correct key is supplied.
    pub fn commit_transaction_with_unlock(
        &mut self,
        tx_id: String,
        tx_bytes: &[u8],
        reveal_key: &[u8],
        scheme: AeadScheme,
        unlock_time: Option<SystemTime>,
    ) -> Result<Vec<u8>, CoreError> {
        let encrypted_data = aead_encrypt(scheme, tx_bytes, reveal_key)?;
        let key_hash = commitment_hash(reveal_key);
        self.submit_encrypted_transaction_with_scheme(
            tx_id,
            encrypted_data.clone(),
            key_hash,
            scheme,
            unlock_time,
        )?;
        Ok(encrypted_data)
    }

    /// Reveal, decrypt and verify a committed transaction (reveal phase).
    ///
    /// The supplied `decryption_key` is the revealed reveal key. Decryption proceeds only
    /// when:
    ///
    /// 1. a transaction with `tx_id` exists in the pool;
    /// 2. the transaction is not time-locked, or the unlock time has passed;
    /// 3. the key matches the commitment bound at commit time; and
    /// 4. the AEAD authentication tag verifies.
    ///
    /// On success the decrypted plaintext transaction bytes are returned and the entry is
    /// removed from the pool. On any failure an honest [`CoreError`] is returned and the
    /// committed ciphertext is left untouched, so a wrong or early key cannot discard a
    /// pending transaction. Before the correct key is revealed (and, for time-locked entries,
    /// before the unlock time) the plaintext cannot be recovered — there is no passthrough.
    pub fn decrypt_and_execute(
        &mut self,
        tx_id: &str,
        decryption_key: &[u8],
    ) -> Result<Vec<u8>, CoreError> {
        // Look up without removing: a failed reveal must not consume the commitment.
        let encrypted_tx = self
            .encrypted_pool
            .get(tx_id)
            .ok_or_else(|| CoreError::NotFound(format!("Transaction {tx_id} not found")))?;

        // Enforce the time-lock: the key is only valid at/after the unlock time.
        if let Some(unlock_time) = encrypted_tx.unlock_time {
            if SystemTime::now() < unlock_time {
                return Err(CoreError::InvalidState(format!(
                    "Transaction {tx_id} is time-locked and cannot be decrypted before its unlock time"
                )));
            }
        }

        // Verify the revealed key against the commitment bound at commit time. This is a
        // constant-time-irrelevant equality on hashes (not the key itself); the real
        // confidentiality guarantee is the AEAD tag check below.
        let provided_commitment = commitment_hash(decryption_key);
        if provided_commitment != encrypted_tx.decryption_key_hash {
            return Err(CoreError::Validation(format!(
                "Revealed key does not match the commitment for transaction {tx_id}"
            )));
        }

        // Authenticated decryption: rejects tampered ciphertext or a wrong key via the tag.
        let plaintext = aead_decrypt(
            encrypted_tx.scheme,
            &encrypted_tx.encrypted_data,
            decryption_key,
        )?;

        // Only consume the commitment once the reveal has fully succeeded.
        self.encrypted_pool.remove(tx_id);
        Ok(plaintext)
    }

    /// Gets the number of encrypted transactions in the pool
    pub fn pool_size(&self) -> usize {
        self.encrypted_pool.len()
    }
}

impl Default for PrivateExecutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// MEV protection coordinator
///
/// Coordinates multiple MEV mitigation strategies
pub struct MevProtectionCoordinator {
    /// Manager for MEV rebates to victims
    rebate_manager: MevRebateManager,
    /// Fair ordering component
    fair_ordering: FairOrderingManager,
    /// Private execution component
    private_execution: PrivateExecutionEngine,
    /// Which mitigation strategies are active
    enabled_strategies: Vec<MitigationStrategy>,
}

impl MevProtectionCoordinator {
    /// Create a coordinator with the specified set of active strategies
    pub fn new(enabled_strategies: Vec<MitigationStrategy>) -> Self {
        Self {
            rebate_manager: MevRebateManager::with_defaults(),
            fair_ordering: FairOrderingManager::with_defaults(),
            private_execution: PrivateExecutionEngine::new(),
            enabled_strategies,
        }
    }

    /// Create a coordinator with all available strategies enabled
    pub fn with_all_strategies() -> Self {
        Self::new(vec![
            MitigationStrategy::FairOrdering,
            MitigationStrategy::MevRebate,
            MitigationStrategy::PrivateExecution,
        ])
    }

    /// Checks if a specific strategy is enabled
    pub fn is_strategy_enabled(&self, strategy: MitigationStrategy) -> bool {
        self.enabled_strategies.contains(&strategy)
    }

    /// Gets the rebate manager
    pub fn rebate_manager(&mut self) -> &mut MevRebateManager {
        &mut self.rebate_manager
    }

    /// Gets the fair ordering manager
    pub fn fair_ordering(&mut self) -> &mut FairOrderingManager {
        &mut self.fair_ordering
    }

    /// Gets the private execution engine
    pub fn private_execution(&mut self) -> &mut PrivateExecutionEngine {
        &mut self.private_execution
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mev_rebate_creation() {
        let mut manager = MevRebateManager::with_defaults();

        let rebate = manager
            .create_rebate(
                "victim".to_string(),
                "attacker".to_string(),
                Decimal::new(100, 2),
                vec!["tx1".to_string(), "tx2".to_string()],
            )
            .unwrap();

        assert_eq!(rebate.victim_user_id, "victim");
        assert_eq!(rebate.rebate_amount, Decimal::new(50, 2)); // 50% of 1.00
        assert_eq!(rebate.status, RebateStatus::Pending);
    }

    #[test]
    fn test_rebate_below_minimum() {
        let mut manager = MevRebateManager::with_defaults();

        let result = manager.create_rebate(
            "victim".to_string(),
            "attacker".to_string(),
            Decimal::new(1, 4), // 0.0001 BTC, below minimum
            vec!["tx1".to_string()],
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_bundle() {
        let mut bundle = TransactionBundle::new("bundle1".to_string());

        let tx1 = BundledTransaction {
            tx_id: "tx1".to_string(),
            user_id: "user1".to_string(),
            order_index: 0,
            data: vec![1, 2, 3],
        };

        let tx2 = BundledTransaction {
            tx_id: "tx2".to_string(),
            user_id: "user2".to_string(),
            order_index: 1,
            data: vec![4, 5, 6],
        };

        assert!(bundle.add_transaction(tx1).is_ok());
        assert!(bundle.add_transaction(tx2).is_ok());
        assert!(bundle.validate().is_ok());
        assert_eq!(bundle.transactions.len(), 2);
    }

    #[test]
    fn test_fair_ordering() {
        let mut manager = FairOrderingManager::new(Duration::from_millis(100));

        manager
            .submit_transaction("tx1".to_string(), "user1".to_string())
            .unwrap();
        std::thread::sleep(Duration::from_millis(50));
        manager
            .submit_transaction("tx2".to_string(), "user2".to_string())
            .unwrap();

        std::thread::sleep(Duration::from_millis(100));

        let ordered = manager.process_batch().unwrap();
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0], "tx1"); // First in, first out
        assert_eq!(ordered[1], "tx2");
    }

    #[test]
    fn test_private_execution_commit_reveal_roundtrip() {
        let mut engine = PrivateExecutionEngine::new();

        // Commit phase: encrypt the real transaction bytes under a reveal key.
        let tx_plaintext = b"swap 1.5 BTC -> 30000 USDC @ market".to_vec();
        let reveal_key = b"super-secret-reveal-key-0xdeadbeef";

        let ciphertext = engine
            .commit_transaction("tx1".to_string(), &tx_plaintext, reveal_key)
            .unwrap();

        // The committed blob must be genuine ciphertext, not the plaintext passthrough.
        assert_ne!(ciphertext, tx_plaintext);
        assert!(ciphertext.len() >= tx_plaintext.len() + AEAD_NONCE_LEN + AEAD_TAG_LEN);
        // The plaintext must not appear verbatim inside the ciphertext.
        assert!(
            !ciphertext
                .windows(tx_plaintext.len())
                .any(|w| w == tx_plaintext.as_slice()),
            "plaintext leaked into ciphertext"
        );
        assert_eq!(engine.pool_size(), 1);

        // Reveal phase: the correct key recovers the original transaction.
        let decrypted = engine.decrypt_and_execute("tx1", reveal_key).unwrap();
        assert_eq!(decrypted, tx_plaintext);
        assert_eq!(engine.pool_size(), 0);
    }

    #[test]
    fn test_private_execution_separately_encrypted_then_submitted() {
        // Exercise the lower-level submit/encrypt API directly (client-side encryption).
        let mut engine = PrivateExecutionEngine::new();
        let tx_plaintext = b"limit sell 100 SOL @ 240.50".to_vec();
        let reveal_key = b"another-reveal-key";

        let ciphertext = PrivateExecutionEngine::encrypt_transaction(&tx_plaintext, reveal_key)
            .expect("encryption should succeed");
        engine
            .submit_encrypted_transaction(
                "tx-sol".to_string(),
                ciphertext,
                commitment_hash(reveal_key),
            )
            .unwrap();

        let decrypted = engine.decrypt_and_execute("tx-sol", reveal_key).unwrap();
        assert_eq!(decrypted, tx_plaintext);
    }

    #[test]
    fn test_private_execution_tampered_ciphertext_fails() {
        let mut engine = PrivateExecutionEngine::new();
        let tx_plaintext = b"transfer 5 ETH to 0xabc".to_vec();
        let reveal_key = b"tamper-test-key";

        let mut ciphertext = PrivateExecutionEngine::encrypt_transaction(&tx_plaintext, reveal_key)
            .expect("encryption should succeed");

        // Flip a byte inside the ciphertext body (past the nonce) to simulate tampering.
        let tamper_index = ciphertext.len() - 1;
        ciphertext[tamper_index] ^= 0xff;

        engine
            .submit_encrypted_transaction(
                "tx-tampered".to_string(),
                ciphertext,
                commitment_hash(reveal_key),
            )
            .unwrap();

        // The correct key is presented, but the authentication tag must reject the data.
        let result = engine.decrypt_and_execute("tx-tampered", reveal_key);
        assert!(result.is_err(), "tampered ciphertext must fail to decrypt");
        // The commitment is not consumed on a failed reveal.
        assert_eq!(engine.pool_size(), 1);
    }

    #[test]
    fn test_private_execution_wrong_key_fails() {
        let mut engine = PrivateExecutionEngine::new();
        let tx_plaintext = b"buy 0.25 BTC @ market".to_vec();
        let reveal_key = b"the-real-reveal-key";
        let wrong_key = b"a-different-wrong-key";

        engine
            .commit_transaction("tx-wrongkey".to_string(), &tx_plaintext, reveal_key)
            .unwrap();

        let result = engine.decrypt_and_execute("tx-wrongkey", wrong_key);
        assert!(result.is_err(), "wrong key must fail to decrypt");
        // A wrong key must not discard the pending transaction.
        assert_eq!(engine.pool_size(), 1);

        // The correct key still works afterwards.
        let decrypted = engine
            .decrypt_and_execute("tx-wrongkey", reveal_key)
            .unwrap();
        assert_eq!(decrypted, tx_plaintext);
        assert_eq!(engine.pool_size(), 0);
    }

    #[test]
    fn test_private_execution_timelock_rejects_early_reveal() {
        let mut engine = PrivateExecutionEngine::new();
        let tx_plaintext = b"time-locked order payload".to_vec();
        let reveal_key = b"timelock-reveal-key";

        // Lock far enough in the future that an early reveal is deterministically rejected.
        let unlock_time = SystemTime::now() + Duration::from_secs(3600);
        engine
            .commit_transaction_with_unlock(
                "tx-locked".to_string(),
                &tx_plaintext,
                reveal_key,
                AeadScheme::DEFAULT,
                Some(unlock_time),
            )
            .unwrap();

        // Even with the correct key, decryption before the unlock time is rejected.
        let early = engine.decrypt_and_execute("tx-locked", reveal_key);
        assert!(early.is_err(), "early reveal of a time-locked tx must fail");
        assert_eq!(engine.pool_size(), 1);
    }

    #[test]
    fn test_private_execution_timelock_allows_reveal_after_unlock() {
        let mut engine = PrivateExecutionEngine::new();
        let tx_plaintext = b"unlocked order payload".to_vec();
        let reveal_key = b"timelock-reveal-key-2";

        // Lock in the past so the unlock time has already elapsed.
        let unlock_time = SystemTime::now() - Duration::from_secs(1);
        engine
            .commit_transaction_with_unlock(
                "tx-unlocked".to_string(),
                &tx_plaintext,
                reveal_key,
                AeadScheme::Aes256Gcm,
                Some(unlock_time),
            )
            .unwrap();

        let decrypted = engine
            .decrypt_and_execute("tx-unlocked", reveal_key)
            .unwrap();
        assert_eq!(decrypted, tx_plaintext);
    }

    #[test]
    fn test_private_execution_aes_gcm_roundtrip() {
        let mut engine = PrivateExecutionEngine::new();
        let tx_plaintext = b"aes-gcm transaction body".to_vec();
        let reveal_key = b"aes-gcm-reveal-key";

        let ciphertext = PrivateExecutionEngine::encrypt_transaction_with_scheme(
            &tx_plaintext,
            reveal_key,
            AeadScheme::Aes256Gcm,
        )
        .unwrap();
        assert_ne!(ciphertext, tx_plaintext);

        engine
            .submit_encrypted_transaction_with_scheme(
                "tx-aes".to_string(),
                ciphertext,
                commitment_hash(reveal_key),
                AeadScheme::Aes256Gcm,
                None,
            )
            .unwrap();

        let decrypted = engine.decrypt_and_execute("tx-aes", reveal_key).unwrap();
        assert_eq!(decrypted, tx_plaintext);
    }

    #[test]
    fn test_private_execution_nonce_is_randomized() {
        let tx_plaintext = b"same plaintext".to_vec();
        let reveal_key = b"same-key";

        let c1 = PrivateExecutionEngine::encrypt_transaction(&tx_plaintext, reveal_key).unwrap();
        let c2 = PrivateExecutionEngine::encrypt_transaction(&tx_plaintext, reveal_key).unwrap();

        // Fresh random nonce per encryption => identical plaintext yields different ciphertext.
        assert_ne!(c1, c2, "nonce reuse detected: ciphertexts must differ");
    }

    #[test]
    fn test_mev_protection_coordinator() {
        let coordinator = MevProtectionCoordinator::with_all_strategies();

        assert!(coordinator.is_strategy_enabled(MitigationStrategy::FairOrdering));
        assert!(coordinator.is_strategy_enabled(MitigationStrategy::MevRebate));
        assert!(coordinator.is_strategy_enabled(MitigationStrategy::PrivateExecution));
    }

    #[test]
    fn test_rebate_completion() {
        let mut manager = MevRebateManager::with_defaults();

        let rebate = manager
            .create_rebate(
                "victim".to_string(),
                "attacker".to_string(),
                Decimal::new(100, 2),
                vec!["tx1".to_string()],
            )
            .unwrap();

        manager.complete_rebate(&rebate.rebate_id).unwrap();

        let victim_rebates = manager.get_victim_rebates("victim");
        assert_eq!(victim_rebates[0].status, RebateStatus::Completed);
    }
}
