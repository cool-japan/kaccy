//! Advanced encryption utilities
//!
//! This module provides:
//! - Field-level encryption for sensitive data
//! - Key rotation mechanisms
//! - Encryption key management
//! - Data integrity verification

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Encryption algorithm type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    /// AES-256 (simulated for now)
    Aes256,
    /// ChaCha20 (simulated for now)
    ChaCha20,
}

/// Encryption key with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    /// Key ID
    pub id: Uuid,
    /// Key version (for rotation)
    pub version: u32,
    /// Algorithm used
    pub algorithm: EncryptionAlgorithm,
    /// Key material (hashed for storage)
    key_hash: String,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Expiration timestamp (for rotation)
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether this key is active
    pub is_active: bool,
}

impl EncryptionKey {
    /// Create a new encryption key
    pub fn new(algorithm: EncryptionAlgorithm) -> Self {
        // In production, this would use a proper key generation mechanism
        let key_material = Uuid::new_v4().to_string();
        let key_hash = Self::hash_key(&key_material);

        Self {
            id: Uuid::new_v4(),
            version: 1,
            algorithm,
            key_hash,
            created_at: Utc::now(),
            expires_at: None,
            is_active: true,
        }
    }

    /// Hash key material for storage
    fn hash_key(key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Check if key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Rotate key (create new version)
    pub fn rotate(&self) -> Self {
        let mut new_key = Self::new(self.algorithm);
        new_key.version = self.version + 1;
        new_key
    }
}

/// Encrypted field with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedField {
    /// Encrypted data (base64 encoded)
    pub ciphertext: String,
    /// Key ID used for encryption
    pub key_id: Uuid,
    /// Key version used
    pub key_version: u32,
    /// Initialization vector (IV) for encryption
    pub iv: String,
    /// Authentication tag (for AEAD)
    pub auth_tag: Option<String>,
    /// Encrypted timestamp
    pub encrypted_at: DateTime<Utc>,
}

impl EncryptedField {
    /// Create a simulated encrypted field
    /// In production, this would use actual encryption
    pub fn encrypt_value(value: &str, key: &EncryptionKey) -> Self {
        // Simulated encryption: base64 encode + hash for demonstration
        let ciphertext = base64::encode(value);
        let iv = base64::encode(Uuid::new_v4().to_string());

        // Simulated HMAC for authentication
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        hasher.update(key.key_hash.as_bytes());
        let auth_tag = hex::encode(hasher.finalize());

        Self {
            ciphertext,
            key_id: key.id,
            key_version: key.version,
            iv,
            auth_tag: Some(auth_tag),
            encrypted_at: Utc::now(),
        }
    }

    /// Decrypt the field (simulated)
    /// In production, this would use actual decryption
    pub fn decrypt_value(&self, _key: &EncryptionKey) -> Result<String> {
        // Simulated decryption: base64 decode
        base64::decode(&self.ciphertext)
            .map_err(|e| CoreError::Serialization(format!("Decryption failed: {}", e)))
            .and_then(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|e| CoreError::Serialization(format!("Invalid UTF-8: {}", e)))
            })
    }

    /// Verify authentication tag
    pub fn verify_integrity(&self, value: &str, key: &EncryptionKey) -> bool {
        if let Some(auth_tag) = &self.auth_tag {
            let mut hasher = Sha256::new();
            hasher.update(value.as_bytes());
            hasher.update(key.key_hash.as_bytes());
            let computed_tag = hex::encode(hasher.finalize());

            computed_tag == *auth_tag
        } else {
            false
        }
    }
}

/// Key rotation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationPolicy {
    /// Rotation interval in days
    pub rotation_interval_days: u32,
    /// Grace period for old keys (days)
    pub grace_period_days: u32,
    /// Automatic rotation enabled
    pub auto_rotate: bool,
}

impl KeyRotationPolicy {
    /// Create a default rotation policy
    pub fn default_policy() -> Self {
        Self {
            rotation_interval_days: 90, // Rotate every 90 days
            grace_period_days: 30,      // 30-day grace period
            auto_rotate: true,
        }
    }

    /// Create a strict rotation policy
    pub fn strict_policy() -> Self {
        Self {
            rotation_interval_days: 30, // Monthly rotation
            grace_period_days: 7,       // 1-week grace period
            auto_rotate: true,
        }
    }

    /// Check if key needs rotation
    pub fn needs_rotation(&self, key: &EncryptionKey) -> bool {
        let age_days = (Utc::now() - key.created_at).num_days();
        age_days >= self.rotation_interval_days as i64
    }
}

/// Encryption key manager
pub struct KeyManager {
    /// All encryption keys
    keys: HashMap<Uuid, EncryptionKey>,
    /// Current active key ID
    active_key_id: Option<Uuid>,
    /// Rotation policy
    rotation_policy: KeyRotationPolicy,
}

impl KeyManager {
    /// Create a new key manager
    pub fn new(rotation_policy: KeyRotationPolicy) -> Self {
        Self {
            keys: HashMap::new(),
            active_key_id: None,
            rotation_policy,
        }
    }

    /// Generate a new key
    pub fn generate_key(&mut self, algorithm: EncryptionAlgorithm) -> Uuid {
        let key = EncryptionKey::new(algorithm);
        let key_id = key.id;

        // Deactivate old active key if exists
        if let Some(old_active_id) = self.active_key_id {
            if let Some(old_key) = self.keys.get_mut(&old_active_id) {
                old_key.is_active = false;
            }
        }

        self.active_key_id = Some(key_id);
        self.keys.insert(key_id, key);

        key_id
    }

    /// Get active key
    pub fn get_active_key(&self) -> Option<&EncryptionKey> {
        self.active_key_id.and_then(|id| self.keys.get(&id))
    }

    /// Get key by ID
    pub fn get_key(&self, key_id: &Uuid) -> Option<&EncryptionKey> {
        self.keys.get(key_id)
    }

    /// Rotate active key
    pub fn rotate_active_key(&mut self) -> Result<Uuid> {
        let active_key_id = self
            .active_key_id
            .ok_or_else(|| CoreError::Configuration("No active key to rotate".to_string()))?;

        let active_key = self
            .keys
            .get(&active_key_id)
            .ok_or_else(|| CoreError::Configuration("Active key not found".to_string()))?;

        let new_key = active_key.rotate();
        let new_key_id = new_key.id;

        // Mark old key as inactive
        if let Some(old_key) = self.keys.get_mut(&active_key_id) {
            old_key.is_active = false;
        }

        self.active_key_id = Some(new_key_id);
        self.keys.insert(new_key_id, new_key);

        Ok(new_key_id)
    }

    /// Check all keys for rotation needs
    pub fn check_rotation_needs(&self) -> Vec<Uuid> {
        self.keys
            .values()
            .filter(|key| key.is_active && self.rotation_policy.needs_rotation(key))
            .map(|key| key.id)
            .collect()
    }

    /// Re-encrypt field with new key
    pub fn re_encrypt_field(&self, field: &EncryptedField) -> Result<EncryptedField> {
        // Get old key
        let old_key = self
            .get_key(&field.key_id)
            .ok_or_else(|| CoreError::NotFound(format!("Key {} not found", field.key_id)))?;

        // Decrypt with old key
        let plaintext = field.decrypt_value(old_key)?;

        // Encrypt with new active key
        let new_key = self
            .get_active_key()
            .ok_or_else(|| CoreError::Configuration("No active key available".to_string()))?;

        Ok(EncryptedField::encrypt_value(&plaintext, new_key))
    }

    /// Get all keys
    pub fn get_all_keys(&self) -> Vec<&EncryptionKey> {
        self.keys.values().collect()
    }

    /// Cleanup expired keys (beyond grace period)
    pub fn cleanup_expired_keys(&mut self) -> Vec<Uuid> {
        let grace_period = chrono::Duration::days(self.rotation_policy.grace_period_days as i64);
        let cutoff = Utc::now() - grace_period;

        let expired_keys: Vec<Uuid> = self
            .keys
            .iter()
            .filter(|(_, key)| !key.is_active && key.created_at < cutoff)
            .map(|(id, _)| *id)
            .collect();

        for key_id in &expired_keys {
            self.keys.remove(key_id);
        }

        expired_keys
    }
}

impl Default for KeyManager {
    fn default() -> Self {
        Self::new(KeyRotationPolicy::default_policy())
    }
}

/// Hash utilities for one-way encryption
pub struct HashUtil;

impl HashUtil {
    /// SHA-256 hash
    pub fn sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// SHA-512 hash
    pub fn sha512(data: &[u8]) -> String {
        let mut hasher = Sha512::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// HMAC-SHA256
    pub fn hmac_sha256(key: &[u8], data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Verify HMAC
    pub fn verify_hmac(key: &[u8], data: &[u8], expected_hmac: &str) -> bool {
        let computed = Self::hmac_sha256(key, data);
        computed == expected_hmac
    }
}

/// Base64 utilities (re-export for convenience)
mod base64 {
    pub fn encode(data: impl AsRef<[u8]>) -> String {
        data.as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }

    pub fn decode(s: &str) -> std::result::Result<Vec<u8>, String> {
        if s.len() % 2 != 0 {
            return Err("Invalid hex string length".to_string());
        }

        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_key_creation() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);

        assert_eq!(key.version, 1);
        assert!(key.is_active);
        assert!(!key.is_expired());
    }

    #[test]
    fn test_key_rotation() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let rotated = key.rotate();

        assert_eq!(rotated.version, 2);
        assert_eq!(rotated.algorithm, key.algorithm);
    }

    #[test]
    fn test_encrypted_field() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let value = "sensitive data";

        let encrypted = EncryptedField::encrypt_value(value, &key);

        assert_eq!(encrypted.key_id, key.id);
        assert_eq!(encrypted.key_version, key.version);
        assert!(encrypted.auth_tag.is_some());

        let decrypted = encrypted.decrypt_value(&key).unwrap();
        assert_eq!(decrypted, value);
    }

    #[test]
    fn test_key_manager() {
        let mut manager = KeyManager::new(KeyRotationPolicy::default_policy());

        let key_id = manager.generate_key(EncryptionAlgorithm::Aes256);
        assert!(manager.get_active_key().is_some());
        assert_eq!(manager.get_active_key().unwrap().id, key_id);
    }

    #[test]
    fn test_key_manager_rotation() {
        let mut manager = KeyManager::new(KeyRotationPolicy::default_policy());

        let first_key_id = manager.generate_key(EncryptionAlgorithm::Aes256);
        let second_key_id = manager.rotate_active_key().unwrap();

        assert_ne!(first_key_id, second_key_id);
        assert_eq!(manager.get_active_key().unwrap().id, second_key_id);

        // First key should be inactive
        let first_key = manager.get_key(&first_key_id).unwrap();
        assert!(!first_key.is_active);
    }

    #[test]
    fn test_rotation_policy() {
        let policy = KeyRotationPolicy::default_policy();
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);

        // New key shouldn't need rotation
        assert!(!policy.needs_rotation(&key));
    }

    #[test]
    fn test_hash_utilities() {
        let data = b"test data";

        let hash = HashUtil::sha256(data);
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 produces 32 bytes = 64 hex chars

        let hash512 = HashUtil::sha512(data);
        assert_eq!(hash512.len(), 128); // SHA-512 produces 64 bytes = 128 hex chars
    }

    #[test]
    fn test_hmac() {
        let key = b"secret key";
        let data = b"message";

        let hmac = HashUtil::hmac_sha256(key, data);
        assert!(HashUtil::verify_hmac(key, data, &hmac));

        // Wrong HMAC should fail
        assert!(!HashUtil::verify_hmac(key, data, "wrong_hmac"));
    }

    #[test]
    fn test_field_integrity() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let value = "secret";

        let encrypted = EncryptedField::encrypt_value(value, &key);
        assert!(encrypted.verify_integrity(value, &key));

        // Tampered data should fail verification
        assert!(!encrypted.verify_integrity("tampered", &key));
    }

    #[test]
    fn test_re_encryption() {
        let mut manager = KeyManager::new(KeyRotationPolicy::default_policy());

        // Create first key and encrypt data
        manager.generate_key(EncryptionAlgorithm::Aes256);
        let first_key = manager.get_active_key().unwrap();
        let encrypted = EncryptedField::encrypt_value("test data", first_key);

        // Rotate key
        manager.rotate_active_key().unwrap();

        // Re-encrypt with new key
        let re_encrypted = manager.re_encrypt_field(&encrypted).unwrap();

        assert_ne!(encrypted.key_id, re_encrypted.key_id);
        assert_eq!(encrypted.key_version + 1, re_encrypted.key_version);

        // Should decrypt to same value
        let decrypted = re_encrypted
            .decrypt_value(manager.get_active_key().unwrap())
            .unwrap();
        assert_eq!(decrypted, "test data");
    }
}
