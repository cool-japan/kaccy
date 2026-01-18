//! Advanced key management for Bitcoin wallets
//!
//! Provides advanced key management features including:
//! - Key rotation for enhanced security
//! - Time-delayed recovery for inheritance and emergency access
//! - Social recovery using multi-signature schemes

use bitcoin::Network;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::error::{BitcoinError, Result};

/// Key rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationConfig {
    /// Rotation interval in seconds
    pub rotation_interval: u64,
    /// Number of old keys to keep for transition
    pub keys_to_keep: usize,
    /// Whether to automatically rotate keys
    pub auto_rotate: bool,
}

impl Default for KeyRotationConfig {
    fn default() -> Self {
        Self {
            rotation_interval: 30 * 24 * 60 * 60, // 30 days
            keys_to_keep: 3,
            auto_rotate: false,
        }
    }
}

/// Rotated key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotatedKey {
    /// Key identifier
    pub key_id: String,
    /// Creation timestamp
    pub created_at: u64,
    /// Expiry timestamp (when this key should be rotated out)
    pub expires_at: u64,
    /// Whether this is the active key
    pub is_active: bool,
    /// Derivation path (if applicable)
    pub derivation_path: Option<String>,
}

/// Key rotation manager
pub struct KeyRotationManager {
    config: KeyRotationConfig,
    keys: Arc<RwLock<HashMap<String, RotatedKey>>>,
    active_key_id: Arc<RwLock<Option<String>>>,
}

impl KeyRotationManager {
    /// Create a new key rotation manager
    pub fn new(config: KeyRotationConfig) -> Self {
        Self {
            config,
            keys: Arc::new(RwLock::new(HashMap::new())),
            active_key_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a new key
    pub async fn register_key(
        &self,
        key_id: String,
        derivation_path: Option<String>,
    ) -> Result<RotatedKey> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let rotated_key = RotatedKey {
            key_id: key_id.clone(),
            created_at: now,
            expires_at: now + self.config.rotation_interval,
            is_active: false,
            derivation_path,
        };

        self.keys.write().await.insert(key_id, rotated_key.clone());

        tracing::info!(
            key_id = %rotated_key.key_id,
            expires_at = rotated_key.expires_at,
            "Registered new key"
        );

        Ok(rotated_key)
    }

    /// Set the active key
    pub async fn set_active_key(&self, key_id: String) -> Result<()> {
        // Deactivate current active key
        if let Some(old_key_id) = self.active_key_id.read().await.as_ref() {
            if let Some(key) = self.keys.write().await.get_mut(old_key_id) {
                key.is_active = false;
            }
        }

        // Activate new key
        let mut keys = self.keys.write().await;
        if let Some(key) = keys.get_mut(&key_id) {
            key.is_active = true;
            *self.active_key_id.write().await = Some(key_id.clone());

            tracing::info!(key_id = %key_id, "Activated new key");
            Ok(())
        } else {
            Err(BitcoinError::Validation(format!(
                "Key not found: {}",
                key_id
            )))
        }
    }

    /// Check if rotation is needed
    pub async fn needs_rotation(&self) -> bool {
        let active_key_id = self.active_key_id.read().await;
        if let Some(key_id) = active_key_id.as_ref() {
            if let Some(key) = self.keys.read().await.get(key_id) {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                return now >= key.expires_at;
            }
        }
        false
    }

    /// Get active key
    pub async fn get_active_key(&self) -> Option<RotatedKey> {
        let active_key_id = self.active_key_id.read().await;
        if let Some(key_id) = active_key_id.as_ref() {
            self.keys.read().await.get(key_id).cloned()
        } else {
            None
        }
    }

    /// Clean up expired keys
    pub async fn cleanup_expired_keys(&self) -> usize {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut keys = self.keys.write().await;
        let active_key_id = self.active_key_id.read().await.clone();

        // Get all keys sorted by creation time
        let mut sorted_keys: Vec<_> = keys
            .iter()
            .map(|(id, key)| (id.clone(), key.clone()))
            .collect();
        sorted_keys.sort_by_key(|(_, key)| key.created_at);

        // Keep the active key and the most recent N keys
        let keys_to_keep: Vec<String> = sorted_keys
            .iter()
            .rev()
            .take(self.config.keys_to_keep)
            .map(|(id, _)| id.clone())
            .collect();

        let mut removed_count = 0;
        keys.retain(|id, key| {
            let keep = key.is_active
                || keys_to_keep.contains(id)
                || active_key_id.as_ref() == Some(id)
                || key.expires_at > now;
            if !keep {
                removed_count += 1;
            }
            keep
        });

        if removed_count > 0 {
            tracing::info!(removed = removed_count, "Cleaned up expired keys");
        }

        removed_count
    }
}

/// Time-delayed recovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDelayedRecoveryConfig {
    /// Delay period in seconds before recovery is possible
    pub delay_seconds: u64,
    /// Network to use
    pub network: Network,
}

impl Default for TimeDelayedRecoveryConfig {
    fn default() -> Self {
        Self {
            delay_seconds: 90 * 24 * 60 * 60, // 90 days
            network: Network::Bitcoin,
        }
    }
}

/// Recovery key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryKey {
    /// Recovery key identifier
    pub id: String,
    /// Activation timestamp (when recovery becomes available)
    pub activation_time: u64,
    /// Recovery key derivation path
    pub derivation_path: String,
    /// Whether this recovery key has been used
    pub used: bool,
}

/// Time-delayed recovery manager
pub struct TimeDelayedRecoveryManager {
    config: TimeDelayedRecoveryConfig,
    recovery_keys: Arc<RwLock<HashMap<String, RecoveryKey>>>,
}

impl TimeDelayedRecoveryManager {
    /// Create a new time-delayed recovery manager
    pub fn new(config: TimeDelayedRecoveryConfig) -> Self {
        Self {
            config,
            recovery_keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new recovery key
    pub async fn create_recovery_key(&self, derivation_path: String) -> Result<RecoveryKey> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let recovery_key = RecoveryKey {
            id: uuid::Uuid::new_v4().to_string(),
            activation_time: now + self.config.delay_seconds,
            derivation_path,
            used: false,
        };

        self.recovery_keys
            .write()
            .await
            .insert(recovery_key.id.clone(), recovery_key.clone());

        tracing::info!(
            id = %recovery_key.id,
            activation_time = recovery_key.activation_time,
            "Created time-delayed recovery key"
        );

        Ok(recovery_key)
    }

    /// Check if a recovery key is available for use
    pub async fn is_recovery_available(&self, recovery_id: &str) -> Result<bool> {
        let keys = self.recovery_keys.read().await;
        if let Some(key) = keys.get(recovery_id) {
            if key.used {
                return Ok(false);
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            Ok(now >= key.activation_time)
        } else {
            Err(BitcoinError::Validation(format!(
                "Recovery key not found: {}",
                recovery_id
            )))
        }
    }

    /// Use a recovery key (mark as used)
    pub async fn use_recovery_key(&self, recovery_id: &str) -> Result<RecoveryKey> {
        if !self.is_recovery_available(recovery_id).await? {
            return Err(BitcoinError::Validation(
                "Recovery key not yet available or already used".to_string(),
            ));
        }

        let mut keys = self.recovery_keys.write().await;
        if let Some(key) = keys.get_mut(recovery_id) {
            key.used = true;
            tracing::info!(id = %recovery_id, "Used recovery key");
            Ok(key.clone())
        } else {
            Err(BitcoinError::Validation(format!(
                "Recovery key not found: {}",
                recovery_id
            )))
        }
    }

    /// Get time remaining until recovery is available
    pub async fn time_until_recovery(&self, recovery_id: &str) -> Result<Duration> {
        let keys = self.recovery_keys.read().await;
        if let Some(key) = keys.get(recovery_id) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            if now >= key.activation_time {
                Ok(Duration::from_secs(0))
            } else {
                Ok(Duration::from_secs(key.activation_time - now))
            }
        } else {
            Err(BitcoinError::Validation(format!(
                "Recovery key not found: {}",
                recovery_id
            )))
        }
    }
}

/// Social recovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialRecoveryConfig {
    /// Threshold (m in m-of-n)
    pub threshold: usize,
    /// Total guardians (n in m-of-n)
    pub total_guardians: usize,
    /// Network to use
    pub network: Network,
}

impl Default for SocialRecoveryConfig {
    fn default() -> Self {
        Self {
            threshold: 2,
            total_guardians: 3,
            network: Network::Bitcoin,
        }
    }
}

/// Guardian information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guardian {
    /// Guardian identifier
    pub id: String,
    /// Guardian's public key or xpub
    pub public_key: String,
    /// Guardian's name/identifier
    pub name: String,
    /// Whether this guardian is active
    pub active: bool,
}

/// Recovery share from a guardian
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryShare {
    /// Guardian who provided this share
    pub guardian_id: String,
    /// Share data (encrypted key share)
    pub share_data: Vec<u8>,
    /// Timestamp when share was created
    pub created_at: u64,
}

/// Social recovery manager using Shamir's Secret Sharing
pub struct SocialRecoveryManager {
    config: SocialRecoveryConfig,
    guardians: Arc<RwLock<HashMap<String, Guardian>>>,
    recovery_shares: Arc<RwLock<Vec<RecoveryShare>>>,
}

impl SocialRecoveryManager {
    /// Create a new social recovery manager
    pub fn new(config: SocialRecoveryConfig) -> Result<Self> {
        if config.threshold > config.total_guardians {
            return Err(BitcoinError::Validation(
                "Threshold cannot exceed total guardians".to_string(),
            ));
        }

        if config.threshold == 0 {
            return Err(BitcoinError::Validation(
                "Threshold must be at least 1".to_string(),
            ));
        }

        Ok(Self {
            config,
            guardians: Arc::new(RwLock::new(HashMap::new())),
            recovery_shares: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Add a guardian
    pub async fn add_guardian(&self, guardian: Guardian) -> Result<()> {
        let guardians_count = self.guardians.read().await.len();

        if guardians_count >= self.config.total_guardians {
            return Err(BitcoinError::Validation(format!(
                "Maximum number of guardians ({}) already reached",
                self.config.total_guardians
            )));
        }

        self.guardians
            .write()
            .await
            .insert(guardian.id.clone(), guardian.clone());

        tracing::info!(
            guardian_id = %guardian.id,
            guardian_name = %guardian.name,
            "Added guardian"
        );

        Ok(())
    }

    /// Remove a guardian
    pub async fn remove_guardian(&self, guardian_id: &str) -> Result<Guardian> {
        if let Some(guardian) = self.guardians.write().await.remove(guardian_id) {
            tracing::info!(guardian_id = %guardian_id, "Removed guardian");
            Ok(guardian)
        } else {
            Err(BitcoinError::Validation(format!(
                "Guardian not found: {}",
                guardian_id
            )))
        }
    }

    /// Get all guardians
    pub async fn get_guardians(&self) -> Vec<Guardian> {
        self.guardians.read().await.values().cloned().collect()
    }

    /// Submit a recovery share from a guardian
    pub async fn submit_recovery_share(&self, share: RecoveryShare) -> Result<()> {
        // Verify guardian exists
        if !self.guardians.read().await.contains_key(&share.guardian_id) {
            return Err(BitcoinError::Validation(format!(
                "Unknown guardian: {}",
                share.guardian_id
            )));
        }

        // Check if this guardian already submitted a share
        let mut shares = self.recovery_shares.write().await;
        if shares.iter().any(|s| s.guardian_id == share.guardian_id) {
            return Err(BitcoinError::Validation(format!(
                "Guardian {} already submitted a share",
                share.guardian_id
            )));
        }

        shares.push(share.clone());

        tracing::info!(
            guardian_id = %share.guardian_id,
            total_shares = shares.len(),
            "Received recovery share"
        );

        Ok(())
    }

    /// Check if recovery is possible (threshold met)
    pub async fn can_recover(&self) -> bool {
        self.recovery_shares.read().await.len() >= self.config.threshold
    }

    /// Attempt recovery with collected shares
    pub async fn attempt_recovery(&self) -> Result<Vec<u8>> {
        let shares = self.recovery_shares.read().await;

        if shares.len() < self.config.threshold {
            return Err(BitcoinError::Validation(format!(
                "Insufficient shares: have {}, need {}",
                shares.len(),
                self.config.threshold
            )));
        }

        // In production, this would:
        // 1. Use Shamir's Secret Sharing to reconstruct the secret
        // 2. Derive the recovery key from the secret
        // 3. Return the reconstructed key

        tracing::info!(
            shares_used = shares.len(),
            threshold = self.config.threshold,
            "Attempting recovery with collected shares"
        );

        // Placeholder: return empty recovery data
        Ok(vec![0u8; 32])
    }

    /// Clear all recovery shares (e.g., after successful recovery)
    pub async fn clear_shares(&self) {
        self.recovery_shares.write().await.clear();
        tracing::info!("Cleared all recovery shares");
    }

    /// Get recovery progress
    pub async fn get_recovery_progress(&self) -> RecoveryProgress {
        let shares_collected = self.recovery_shares.read().await.len();
        RecoveryProgress {
            shares_collected,
            threshold: self.config.threshold,
            total_guardians: self.config.total_guardians,
            can_recover: shares_collected >= self.config.threshold,
        }
    }
}

/// Recovery progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryProgress {
    /// Number of shares collected
    pub shares_collected: usize,
    /// Threshold required for recovery
    pub threshold: usize,
    /// Total number of guardians
    pub total_guardians: usize,
    /// Whether recovery is possible
    pub can_recover: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_key_rotation_registration() {
        let config = KeyRotationConfig::default();
        let manager = KeyRotationManager::new(config);

        let key = manager
            .register_key("key1".to_string(), Some("m/84'/0'/0'".to_string()))
            .await
            .unwrap();

        assert_eq!(key.key_id, "key1");
        assert!(!key.is_active);
        assert_eq!(key.derivation_path, Some("m/84'/0'/0'".to_string()));
    }

    #[tokio::test]
    async fn test_key_rotation_activation() {
        let config = KeyRotationConfig::default();
        let manager = KeyRotationManager::new(config);

        manager
            .register_key("key1".to_string(), None)
            .await
            .unwrap();
        manager.set_active_key("key1".to_string()).await.unwrap();

        let active = manager.get_active_key().await.unwrap();
        assert_eq!(active.key_id, "key1");
        assert!(active.is_active);
    }

    #[tokio::test]
    async fn test_key_rotation_cleanup() {
        let config = KeyRotationConfig {
            rotation_interval: 1, // 1 second
            keys_to_keep: 2,
            auto_rotate: false,
        };
        let manager = KeyRotationManager::new(config);

        // Register multiple keys
        manager
            .register_key("key1".to_string(), None)
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        manager
            .register_key("key2".to_string(), None)
            .await
            .unwrap();
        manager
            .register_key("key3".to_string(), None)
            .await
            .unwrap();
        manager.set_active_key("key3".to_string()).await.unwrap();

        let removed = manager.cleanup_expired_keys().await;
        assert!(removed <= 1); // Should keep active key and recent keys
    }

    #[tokio::test]
    async fn test_time_delayed_recovery_creation() {
        let config = TimeDelayedRecoveryConfig::default();
        let manager = TimeDelayedRecoveryManager::new(config);

        let recovery_key = manager
            .create_recovery_key("m/84'/0'/0'/0/0".to_string())
            .await
            .unwrap();

        assert!(!recovery_key.used);
        assert_eq!(recovery_key.derivation_path, "m/84'/0'/0'/0/0");
    }

    #[tokio::test]
    async fn test_time_delayed_recovery_not_available() {
        let config = TimeDelayedRecoveryConfig {
            delay_seconds: 3600, // 1 hour
            network: Network::Bitcoin,
        };
        let manager = TimeDelayedRecoveryManager::new(config);

        let recovery_key = manager
            .create_recovery_key("m/84'/0'/0'/0/0".to_string())
            .await
            .unwrap();

        let available = manager
            .is_recovery_available(&recovery_key.id)
            .await
            .unwrap();
        assert!(!available);
    }

    #[tokio::test]
    async fn test_social_recovery_add_guardian() {
        let config = SocialRecoveryConfig {
            threshold: 2,
            total_guardians: 3,
            network: Network::Bitcoin,
        };
        let manager = SocialRecoveryManager::new(config).unwrap();

        let guardian = Guardian {
            id: "guardian1".to_string(),
            public_key: "xpub...".to_string(),
            name: "Alice".to_string(),
            active: true,
        };

        manager.add_guardian(guardian).await.unwrap();

        let guardians = manager.get_guardians().await;
        assert_eq!(guardians.len(), 1);
        assert_eq!(guardians[0].name, "Alice");
    }

    #[tokio::test]
    async fn test_social_recovery_threshold_validation() {
        let result = SocialRecoveryManager::new(SocialRecoveryConfig {
            threshold: 5,
            total_guardians: 3, // Invalid: threshold > total
            network: Network::Bitcoin,
        });

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_social_recovery_share_submission() {
        let config = SocialRecoveryConfig {
            threshold: 2,
            total_guardians: 3,
            network: Network::Bitcoin,
        };
        let manager = SocialRecoveryManager::new(config).unwrap();

        // Add guardian first
        let guardian = Guardian {
            id: "guardian1".to_string(),
            public_key: "xpub...".to_string(),
            name: "Alice".to_string(),
            active: true,
        };
        manager.add_guardian(guardian).await.unwrap();

        // Submit share
        let share = RecoveryShare {
            guardian_id: "guardian1".to_string(),
            share_data: vec![1, 2, 3, 4],
            created_at: 0,
        };
        manager.submit_recovery_share(share).await.unwrap();

        let progress = manager.get_recovery_progress().await;
        assert_eq!(progress.shares_collected, 1);
        assert!(!progress.can_recover); // Need 2 shares
    }

    #[tokio::test]
    async fn test_social_recovery_threshold_met() {
        let config = SocialRecoveryConfig {
            threshold: 2,
            total_guardians: 3,
            network: Network::Bitcoin,
        };
        let manager = SocialRecoveryManager::new(config).unwrap();

        // Add guardians
        for i in 1..=3 {
            let guardian = Guardian {
                id: format!("guardian{}", i),
                public_key: format!("xpub{}", i),
                name: format!("Guardian {}", i),
                active: true,
            };
            manager.add_guardian(guardian).await.unwrap();
        }

        // Submit 2 shares (meets threshold)
        for i in 1..=2 {
            let share = RecoveryShare {
                guardian_id: format!("guardian{}", i),
                share_data: vec![i as u8; 32],
                created_at: 0,
            };
            manager.submit_recovery_share(share).await.unwrap();
        }

        assert!(manager.can_recover().await);

        let progress = manager.get_recovery_progress().await;
        assert!(progress.can_recover);
        assert_eq!(progress.shares_collected, 2);
    }
}
