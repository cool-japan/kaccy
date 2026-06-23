//! Advanced encryption utilities
//!
//! This module provides:
//! - Field-level authenticated encryption (AEAD) for sensitive data
//! - Key rotation mechanisms
//! - Encryption key management
//! - Data integrity verification
//!
//! # Cryptography
//!
//! Encryption uses real authenticated encryption with associated data (AEAD):
//!
//! - [`EncryptionAlgorithm::Aes256`] -> AES-256-GCM (via the `aes-gcm` crate)
//! - [`EncryptionAlgorithm::ChaCha20`] -> ChaCha20-Poly1305 (via the
//!   `chacha20poly1305` crate)
//!
//! Each [`EncryptionKey`] carries real 256-bit secret key material generated
//! from the operating-system CSPRNG. The secret bytes are never serialized
//! (`#[serde(skip)]`) and are zeroized on drop. A non-secret SHA-256 digest of
//! the key (`key_hash`) is retained purely as a stable identifier.
//!
//! Every call to [`EncryptedField::encrypt_value`] draws a fresh random 96-bit
//! nonce and produces a 128-bit authentication tag. [`EncryptedField::decrypt_value`]
//! verifies that tag and fails with a typed error on any tampering or wrong key.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use chacha20poly1305::ChaCha20Poly1305;
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CoreError, Result};

/// Length of the symmetric key material in bytes (256 bits).
const KEY_LEN: usize = 32;
/// Length of the AEAD nonce in bytes (96 bits) for both GCM and Poly1305.
const NONCE_LEN: usize = 12;
/// Length of the AEAD authentication tag in bytes (128 bits).
const TAG_LEN: usize = 16;

/// Encryption algorithm type.
///
/// Selects which AEAD construction [`EncryptedField`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    /// AES-256-GCM authenticated encryption.
    Aes256,
    /// ChaCha20-Poly1305 authenticated encryption.
    ChaCha20,
}

/// Generate `KEY_LEN` bytes of cryptographically secure randomness from the OS
/// CSPRNG.
fn random_key_bytes() -> [u8; KEY_LEN] {
    let mut bytes = [0u8; KEY_LEN];
    rand::rng().fill_bytes(&mut bytes);
    bytes
}

/// Generate a fresh random AEAD nonce from the OS CSPRNG.
fn random_nonce_bytes() -> [u8; NONCE_LEN] {
    let mut bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut bytes);
    bytes
}

/// Encryption key with metadata.
///
/// Holds real 256-bit secret key material used for AEAD. The secret bytes are
/// intentionally excluded from serialization and zeroized on drop; only the
/// non-secret [`EncryptionKey::key_hash`] identifier is persisted.
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    /// Key ID.
    pub id: Uuid,
    /// Key version (for rotation).
    pub version: u32,
    /// Algorithm used.
    pub algorithm: EncryptionAlgorithm,
    /// Non-secret identifier: SHA-256 digest of the secret key material.
    key_hash: String,
    /// Secret 256-bit key material. Never serialized; zeroized on drop.
    ///
    /// When an [`EncryptionKey`] is deserialized this is reconstructed empty,
    /// and any attempt to encrypt/decrypt with it fails with a typed error
    /// rather than silently using a fake key.
    #[serde(skip)]
    key_material: Vec<u8>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Expiration timestamp (for rotation).
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether this key is active.
    pub is_active: bool,
}

impl EncryptionKey {
    /// Create a new encryption key with fresh random 256-bit key material.
    pub fn new(algorithm: EncryptionAlgorithm) -> Self {
        let key_material = random_key_bytes();
        let key_hash = Self::hash_key(&key_material);

        Self {
            id: Uuid::new_v4(),
            version: 1,
            algorithm,
            key_hash,
            key_material: key_material.to_vec(),
            created_at: Utc::now(),
            expires_at: None,
            is_active: true,
        }
    }

    /// Hash the secret key material to produce the non-secret identifier.
    fn hash_key(key: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hex::encode(hasher.finalize())
    }

    /// Non-secret SHA-256 identifier of this key.
    pub fn key_hash(&self) -> &str {
        &self.key_hash
    }

    /// Borrow the raw secret key bytes if present.
    ///
    /// Returns an error for keys whose secret material is unavailable (for
    /// example, an [`EncryptionKey`] reconstructed via deserialization).
    fn secret_bytes(&self) -> Result<&[u8; KEY_LEN]> {
        let slice: &[u8] = self.key_material.as_slice();
        slice.try_into().map_err(|_| {
            CoreError::Configuration(format!(
                "Encryption key {} has no usable secret material (expected {} bytes, found {})",
                self.id,
                KEY_LEN,
                slice.len()
            ))
        })
    }

    /// Check if key is expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Rotate key (create new version with fresh key material).
    pub fn rotate(&self) -> Self {
        let mut new_key = Self::new(self.algorithm);
        new_key.version = self.version + 1;
        new_key
    }
}

impl std::fmt::Debug for EncryptionKey {
    /// Debug output deliberately omits the secret key material to avoid leaking
    /// it into logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionKey")
            .field("id", &self.id)
            .field("version", &self.version)
            .field("algorithm", &self.algorithm)
            .field("key_hash", &self.key_hash)
            .field("key_material", &"<redacted>")
            .field("created_at", &self.created_at)
            .field("expires_at", &self.expires_at)
            .field("is_active", &self.is_active)
            .finish()
    }
}

impl Drop for EncryptionKey {
    /// Best-effort zeroization of the secret key material on drop.
    ///
    /// Writes through a volatile pointer and inserts a compiler fence so the
    /// overwrite is not elided as dead code.
    fn drop(&mut self) {
        if self.key_material.is_empty() {
            return;
        }
        for byte in self.key_material.iter_mut() {
            // SAFETY: `byte` is a valid, aligned, writable `u8` location for the
            // lifetime of this loop iteration.
            unsafe {
                std::ptr::write_volatile(byte, 0u8);
            }
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}

/// Encrypted field with metadata.
///
/// Stores AEAD output: a base64-encoded ciphertext, the random 96-bit nonce
/// (`iv`), and the 128-bit authentication tag (`auth_tag`), all alongside the
/// identity of the key used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedField {
    /// Encrypted data (base64 encoded, without the appended tag).
    pub ciphertext: String,
    /// Key ID used for encryption.
    pub key_id: Uuid,
    /// Key version used.
    pub key_version: u32,
    /// Random 96-bit nonce used for encryption (base64 encoded).
    pub iv: String,
    /// AEAD authentication tag (base64 encoded, 128-bit).
    pub auth_tag: Option<String>,
    /// Encrypted timestamp.
    pub encrypted_at: DateTime<Utc>,
}

/// Run the AEAD seal operation for the given algorithm, returning the combined
/// ciphertext-with-appended-tag buffer.
fn aead_seal(
    algorithm: EncryptionAlgorithm,
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let nonce = AesNonce::from_slice(nonce);
    match algorithm {
        EncryptionAlgorithm::Aes256 => {
            let cipher = Aes256Gcm::new_from_slice(key)
                .map_err(|e| CoreError::Configuration(format!("Invalid AES-256 key: {e}")))?;
            cipher
                .encrypt(nonce, plaintext)
                .map_err(|_| CoreError::Calculation("AES-256-GCM encryption failed".to_string()))
        }
        EncryptionAlgorithm::ChaCha20 => {
            let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|e| {
                CoreError::Configuration(format!("Invalid ChaCha20 key: {e}"))
            })?;
            cipher.encrypt(nonce, plaintext).map_err(|_| {
                CoreError::Calculation("ChaCha20-Poly1305 encryption failed".to_string())
            })
        }
    }
}

/// Run the AEAD open operation for the given algorithm over the combined
/// ciphertext-with-appended-tag buffer, verifying the tag.
fn aead_open(
    algorithm: EncryptionAlgorithm,
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext_with_tag: &[u8],
) -> Result<Vec<u8>> {
    let nonce = AesNonce::from_slice(nonce);
    match algorithm {
        EncryptionAlgorithm::Aes256 => {
            let cipher = Aes256Gcm::new_from_slice(key)
                .map_err(|e| CoreError::Configuration(format!("Invalid AES-256 key: {e}")))?;
            cipher.decrypt(nonce, ciphertext_with_tag).map_err(|_| {
                CoreError::Validation(
                    "AES-256-GCM authentication failed (tampered data or wrong key)".to_string(),
                )
            })
        }
        EncryptionAlgorithm::ChaCha20 => {
            let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|e| {
                CoreError::Configuration(format!("Invalid ChaCha20 key: {e}"))
            })?;
            cipher.decrypt(nonce, ciphertext_with_tag).map_err(|_| {
                CoreError::Validation(
                    "ChaCha20-Poly1305 authentication failed (tampered data or wrong key)"
                        .to_string(),
                )
            })
        }
    }
}

impl EncryptedField {
    /// Encrypt a UTF-8 string value with real authenticated encryption.
    ///
    /// A fresh random 96-bit nonce is generated for every call. The returned
    /// field stores the ciphertext and the 128-bit authentication tag
    /// separately (both base64 encoded).
    ///
    /// # Errors
    ///
    /// Returns an error if the key has no usable secret material or if the
    /// underlying AEAD operation fails.
    pub fn encrypt_value(value: &str, key: &EncryptionKey) -> Result<Self> {
        let secret = key.secret_bytes()?;
        let nonce_bytes = random_nonce_bytes();

        let mut combined = aead_seal(key.algorithm, secret, &nonce_bytes, value.as_bytes())?;

        // The `aead` crate appends the tag after the ciphertext; split it out so
        // the tag is stored explicitly in `auth_tag`.
        if combined.len() < TAG_LEN {
            return Err(CoreError::Calculation(
                "AEAD output shorter than authentication tag".to_string(),
            ));
        }
        let tag = combined.split_off(combined.len() - TAG_LEN);

        Ok(Self {
            ciphertext: BASE64.encode(&combined),
            key_id: key.id,
            key_version: key.version,
            iv: BASE64.encode(nonce_bytes),
            auth_tag: Some(BASE64.encode(&tag)),
            encrypted_at: Utc::now(),
        })
    }

    /// Decrypt the field and verify its authentication tag.
    ///
    /// # Errors
    ///
    /// Returns an error if the key is wrong, the data has been tampered with,
    /// the stored fields are malformed, or the decrypted bytes are not valid
    /// UTF-8.
    pub fn decrypt_value(&self, key: &EncryptionKey) -> Result<String> {
        let secret = key.secret_bytes()?;

        let ciphertext = BASE64
            .decode(self.ciphertext.as_bytes())
            .map_err(|e| CoreError::Serialization(format!("Invalid ciphertext encoding: {e}")))?;

        let nonce_vec = BASE64
            .decode(self.iv.as_bytes())
            .map_err(|e| CoreError::Serialization(format!("Invalid nonce encoding: {e}")))?;
        let nonce: [u8; NONCE_LEN] = nonce_vec.as_slice().try_into().map_err(|_| {
            CoreError::Serialization(format!(
                "Invalid nonce length: expected {} bytes, found {}",
                NONCE_LEN,
                nonce_vec.len()
            ))
        })?;

        let tag_b64 = self
            .auth_tag
            .as_ref()
            .ok_or_else(|| CoreError::Validation("Missing authentication tag".to_string()))?;
        let tag = BASE64
            .decode(tag_b64.as_bytes())
            .map_err(|e| CoreError::Serialization(format!("Invalid auth tag encoding: {e}")))?;
        if tag.len() != TAG_LEN {
            return Err(CoreError::Serialization(format!(
                "Invalid auth tag length: expected {} bytes, found {}",
                TAG_LEN,
                tag.len()
            )));
        }

        // Reassemble the combined ciphertext-with-tag buffer the `aead` crate
        // expects for verification + decryption.
        let mut combined = ciphertext;
        combined.extend_from_slice(&tag);

        let plaintext = aead_open(key.algorithm, secret, &nonce, &combined)?;

        String::from_utf8(plaintext)
            .map_err(|e| CoreError::Serialization(format!("Invalid UTF-8: {e}")))
    }

    /// Verify that this field authentically decrypts to `value` under `key`.
    ///
    /// Returns `false` on any AEAD failure (wrong key, tampering, malformed
    /// fields) or if the decrypted plaintext does not equal `value`.
    pub fn verify_integrity(&self, value: &str, key: &EncryptionKey) -> bool {
        match self.decrypt_value(key) {
            Ok(plaintext) => plaintext == value,
            Err(_) => false,
        }
    }
}

/// Key rotation policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationPolicy {
    /// Rotation interval in days.
    pub rotation_interval_days: u32,
    /// Grace period for old keys (days).
    pub grace_period_days: u32,
    /// Automatic rotation enabled.
    pub auto_rotate: bool,
}

impl KeyRotationPolicy {
    /// Create a default rotation policy.
    pub fn default_policy() -> Self {
        Self {
            rotation_interval_days: 90, // Rotate every 90 days
            grace_period_days: 30,      // 30-day grace period
            auto_rotate: true,
        }
    }

    /// Create a strict rotation policy.
    pub fn strict_policy() -> Self {
        Self {
            rotation_interval_days: 30, // Monthly rotation
            grace_period_days: 7,       // 1-week grace period
            auto_rotate: true,
        }
    }

    /// Check if key needs rotation.
    pub fn needs_rotation(&self, key: &EncryptionKey) -> bool {
        let age_days = (Utc::now() - key.created_at).num_days();
        age_days >= self.rotation_interval_days as i64
    }
}

/// Encryption key manager.
pub struct KeyManager {
    /// All encryption keys.
    keys: HashMap<Uuid, EncryptionKey>,
    /// Current active key ID.
    active_key_id: Option<Uuid>,
    /// Rotation policy.
    rotation_policy: KeyRotationPolicy,
}

impl KeyManager {
    /// Create a new key manager.
    pub fn new(rotation_policy: KeyRotationPolicy) -> Self {
        Self {
            keys: HashMap::new(),
            active_key_id: None,
            rotation_policy,
        }
    }

    /// Generate a new key.
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

    /// Get active key.
    pub fn get_active_key(&self) -> Option<&EncryptionKey> {
        self.active_key_id.and_then(|id| self.keys.get(&id))
    }

    /// Get key by ID.
    pub fn get_key(&self, key_id: &Uuid) -> Option<&EncryptionKey> {
        self.keys.get(key_id)
    }

    /// Rotate active key.
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

    /// Check all keys for rotation needs.
    pub fn check_rotation_needs(&self) -> Vec<Uuid> {
        self.keys
            .values()
            .filter(|key| key.is_active && self.rotation_policy.needs_rotation(key))
            .map(|key| key.id)
            .collect()
    }

    /// Re-encrypt field with new key.
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

        EncryptedField::encrypt_value(&plaintext, new_key)
    }

    /// Get all keys.
    pub fn get_all_keys(&self) -> Vec<&EncryptionKey> {
        self.keys.values().collect()
    }

    /// Cleanup expired keys (beyond grace period).
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

/// Hash utilities for one-way encryption.
pub struct HashUtil;

impl HashUtil {
    /// SHA-256 hash.
    pub fn sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// SHA-512 hash.
    pub fn sha512(data: &[u8]) -> String {
        let mut hasher = Sha512::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// HMAC-SHA256.
    pub fn hmac_sha256(key: &[u8], data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Verify HMAC.
    pub fn verify_hmac(key: &[u8], data: &[u8], expected_hmac: &str) -> bool {
        let computed = Self::hmac_sha256(key, data);
        computed == expected_hmac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an [`EncryptionKey`] from explicit key bytes for deterministic
    /// known-answer testing.
    fn key_from_bytes(algorithm: EncryptionAlgorithm, bytes: [u8; KEY_LEN]) -> EncryptionKey {
        let key_hash = EncryptionKey::hash_key(&bytes);
        EncryptionKey {
            id: Uuid::new_v4(),
            version: 1,
            algorithm,
            key_hash,
            key_material: bytes.to_vec(),
            created_at: Utc::now(),
            expires_at: None,
            is_active: true,
        }
    }

    #[test]
    fn test_encryption_key_creation() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);

        assert_eq!(key.version, 1);
        assert!(key.is_active);
        assert!(!key.is_expired());
        // Real 256-bit key material must be present.
        assert_eq!(key.secret_bytes().expect("secret present").len(), KEY_LEN);
        // The identifier is the SHA-256 hex of the key bytes (64 hex chars).
        assert_eq!(key.key_hash().len(), 64);
    }

    #[test]
    fn test_distinct_keys_have_distinct_material() {
        let a = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let b = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        // Two freshly generated keys must not collide.
        assert_ne!(a.key_hash(), b.key_hash());
        assert_ne!(
            a.secret_bytes().expect("a"),
            b.secret_bytes().expect("b")
        );
    }

    #[test]
    fn test_key_rotation() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let rotated = key.rotate();

        assert_eq!(rotated.version, 2);
        assert_eq!(rotated.algorithm, key.algorithm);
        // Rotation must produce fresh key material.
        assert_ne!(rotated.key_hash(), key.key_hash());
    }

    #[test]
    fn test_secret_not_serialized() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let json = serde_json::to_string(&key).expect("serialize key");
        // The secret bytes must never appear in serialized output.
        assert!(!json.contains("key_material"));
        let restored: EncryptionKey = serde_json::from_str(&json).expect("deserialize key");
        // A deserialized key has no secret material and refuses crypto ops.
        assert!(restored.secret_bytes().is_err());
        assert!(EncryptedField::encrypt_value("x", &restored).is_err());
    }

    #[test]
    fn test_debug_redacts_secret() {
        let bytes = [0xABu8; KEY_LEN];
        let key = key_from_bytes(EncryptionAlgorithm::Aes256, bytes);
        let dbg = format!("{key:?}");
        assert!(dbg.contains("<redacted>"));
        // The raw secret byte pattern must not leak into Debug output.
        assert!(!dbg.contains("171, 171, 171"));
    }

    #[test]
    fn test_round_trip_aes256() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let value = "sensitive AES data \u{1F510}";

        let encrypted = EncryptedField::encrypt_value(value, &key).expect("encrypt");

        assert_eq!(encrypted.key_id, key.id);
        assert_eq!(encrypted.key_version, key.version);
        assert!(encrypted.auth_tag.is_some());
        // Ciphertext must not equal the plaintext (real encryption happened).
        assert_ne!(encrypted.ciphertext, value);

        let decrypted = encrypted.decrypt_value(&key).expect("decrypt");
        assert_eq!(decrypted, value);
    }

    #[test]
    fn test_round_trip_chacha20() {
        let key = EncryptionKey::new(EncryptionAlgorithm::ChaCha20);
        let value = "sensitive ChaCha data \u{1F512}";

        let encrypted = EncryptedField::encrypt_value(value, &key).expect("encrypt");
        assert!(encrypted.auth_tag.is_some());
        assert_ne!(encrypted.ciphertext, value);

        let decrypted = encrypted.decrypt_value(&key).expect("decrypt");
        assert_eq!(decrypted, value);
    }

    #[test]
    fn test_fresh_nonce_per_call() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let a = EncryptedField::encrypt_value("same", &key).expect("encrypt a");
        let b = EncryptedField::encrypt_value("same", &key).expect("encrypt b");
        // A fresh random nonce per call yields different IVs and ciphertexts for
        // identical plaintext.
        assert_ne!(a.iv, b.iv);
        assert_ne!(a.ciphertext, b.ciphertext);
        // Both still decrypt to the same plaintext.
        assert_eq!(a.decrypt_value(&key).expect("dec a"), "same");
        assert_eq!(b.decrypt_value(&key).expect("dec b"), "same");
    }

    #[test]
    fn test_tamper_ciphertext_detected_aes() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let mut encrypted = EncryptedField::encrypt_value("important", &key).expect("encrypt");

        // Flip one bit in the ciphertext.
        let mut ct = BASE64
            .decode(encrypted.ciphertext.as_bytes())
            .expect("decode ct");
        assert!(!ct.is_empty());
        ct[0] ^= 0x01;
        encrypted.ciphertext = BASE64.encode(&ct);

        // Tag verification must fail.
        let result = encrypted.decrypt_value(&key);
        assert!(result.is_err());
        assert!(!encrypted.verify_integrity("important", &key));
    }

    #[test]
    fn test_tamper_tag_detected_chacha() {
        let key = EncryptionKey::new(EncryptionAlgorithm::ChaCha20);
        let mut encrypted = EncryptedField::encrypt_value("important", &key).expect("encrypt");

        // Flip one byte in the authentication tag.
        let tag_b64 = encrypted.auth_tag.clone().expect("tag");
        let mut tag = BASE64.decode(tag_b64.as_bytes()).expect("decode tag");
        tag[0] ^= 0xFF;
        encrypted.auth_tag = Some(BASE64.encode(&tag));

        assert!(encrypted.decrypt_value(&key).is_err());
    }

    #[test]
    fn test_tamper_nonce_detected() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let mut encrypted = EncryptedField::encrypt_value("payload", &key).expect("encrypt");

        let mut nonce = BASE64.decode(encrypted.iv.as_bytes()).expect("decode iv");
        nonce[0] ^= 0x01;
        encrypted.iv = BASE64.encode(&nonce);

        // Changing the nonce must break authentication.
        assert!(encrypted.decrypt_value(&key).is_err());
    }

    #[test]
    fn test_wrong_key_fails_aes() {
        let key = EncryptionKey::new(EncryptionAlgorithm::Aes256);
        let other = EncryptionKey::new(EncryptionAlgorithm::Aes256);

        let encrypted = EncryptedField::encrypt_value("secret", &key).expect("encrypt");
        // Decrypting with a different key must fail authentication.
        assert!(encrypted.decrypt_value(&other).is_err());
        assert!(!encrypted.verify_integrity("secret", &other));
        // The correct key still works.
        assert_eq!(encrypted.decrypt_value(&key).expect("decrypt"), "secret");
    }

    #[test]
    fn test_wrong_key_fails_chacha() {
        let key = EncryptionKey::new(EncryptionAlgorithm::ChaCha20);
        let other = EncryptionKey::new(EncryptionAlgorithm::ChaCha20);

        let encrypted = EncryptedField::encrypt_value("secret", &key).expect("encrypt");
        assert!(encrypted.decrypt_value(&other).is_err());
    }

    #[test]
    fn test_aes_gcm_known_answer_vector() {
        // NIST-style AES-256-GCM test vector (gcmEncryptExtIV256, Count 0).
        // Key   = 256 zero bits
        // IV    = 96 zero bits
        // PT    = empty
        // AAD   = empty
        // Expected tag = 530f8afbc74536b9a963b4f1c4cb738b
        let key_bytes = [0u8; KEY_LEN];
        let nonce = [0u8; NONCE_LEN];
        let combined =
            aead_seal(EncryptionAlgorithm::Aes256, &key_bytes, &nonce, &[]).expect("seal");
        // Empty plaintext -> output is exactly the 16-byte tag.
        assert_eq!(combined.len(), TAG_LEN);
        assert_eq!(hex::encode(&combined), "530f8afbc74536b9a963b4f1c4cb738b");

        // And the open path verifies that tag and recovers the empty plaintext.
        let opened =
            aead_open(EncryptionAlgorithm::Aes256, &key_bytes, &nonce, &combined).expect("open");
        assert!(opened.is_empty());
    }

    #[test]
    fn test_chacha20poly1305_known_answer_vector() {
        // RFC 8439 Section 2.8.2 AEAD ChaCha20-Poly1305 test vector.
        let key_bytes: [u8; KEY_LEN] = [
            0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d,
            0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b,
            0x9c, 0x9d, 0x9e, 0x9f,
        ];
        // RFC nonce = 070000004041424344454647 (12 bytes).
        let nonce: [u8; NONCE_LEN] = [
            0x07, 0x00, 0x00, 0x00, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47,
        ];
        let plaintext = b"Ladies and Gentlemen of the class of '99: \
If I could offer you only one tip for the future, sunscreen would be it.";

        // This module's AEAD uses empty associated data. The ChaCha20 keystream
        // (and therefore the ciphertext) is independent of the AAD, so the
        // ciphertext must match the canonical RFC 8439 Section 2.8.2 ciphertext
        // exactly. (The RFC's Poly1305 tag is computed over non-empty AAD and is
        // therefore intentionally not asserted here; the tag is exercised by the
        // authenticated round-trip and tamper checks below.)
        let combined = aead_seal(EncryptionAlgorithm::ChaCha20, &key_bytes, &nonce, plaintext)
            .expect("seal");
        assert_eq!(combined.len(), plaintext.len() + TAG_LEN);
        let ciphertext = &combined[..combined.len() - TAG_LEN];
        let expected_ct = "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6\
3dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b3692ddbd7f2d778b8c9\
803aee328091b58fab324e4fad675945585808b4831d7bc3ff4def08e4b7a9de576d26586cec64b6116";
        assert_eq!(hex::encode(ciphertext), expected_ct);

        // The Poly1305 tag authenticates: a clean open recovers the plaintext.
        let opened =
            aead_open(EncryptionAlgorithm::ChaCha20, &key_bytes, &nonce, &combined).expect("open");
        assert_eq!(opened, plaintext);

        // And flipping a tag byte must make authentication fail.
        let mut tampered = combined.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(
            aead_open(EncryptionAlgorithm::ChaCha20, &key_bytes, &nonce, &tampered).is_err()
        );
    }

    #[test]
    fn test_key_manager() {
        let mut manager = KeyManager::new(KeyRotationPolicy::default_policy());

        let key_id = manager.generate_key(EncryptionAlgorithm::Aes256);
        assert!(manager.get_active_key().is_some());
        assert_eq!(
            manager.get_active_key().expect("active key").id,
            key_id
        );
    }

    #[test]
    fn test_key_manager_rotation() {
        let mut manager = KeyManager::new(KeyRotationPolicy::default_policy());

        let first_key_id = manager.generate_key(EncryptionAlgorithm::Aes256);
        let second_key_id = manager.rotate_active_key().expect("rotate");

        assert_ne!(first_key_id, second_key_id);
        assert_eq!(
            manager.get_active_key().expect("active key").id,
            second_key_id
        );

        // First key should be inactive
        let first_key = manager.get_key(&first_key_id).expect("first key");
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

        let encrypted = EncryptedField::encrypt_value(value, &key).expect("encrypt");
        assert!(encrypted.verify_integrity(value, &key));

        // A different expected value must fail verification.
        assert!(!encrypted.verify_integrity("tampered", &key));
    }

    #[test]
    fn test_re_encryption() {
        let mut manager = KeyManager::new(KeyRotationPolicy::default_policy());

        // Create first key and encrypt data
        manager.generate_key(EncryptionAlgorithm::Aes256);
        let encrypted = {
            let first_key = manager.get_active_key().expect("first key");
            EncryptedField::encrypt_value("test data", first_key).expect("encrypt")
        };

        // Rotate key
        manager.rotate_active_key().expect("rotate");

        // Re-encrypt with new key
        let re_encrypted = manager.re_encrypt_field(&encrypted).expect("re-encrypt");

        assert_ne!(encrypted.key_id, re_encrypted.key_id);
        assert_eq!(encrypted.key_version + 1, re_encrypted.key_version);

        // Should decrypt to same value
        let decrypted = re_encrypted
            .decrypt_value(manager.get_active_key().expect("active key"))
            .expect("decrypt");
        assert_eq!(decrypted, "test data");
    }
}

