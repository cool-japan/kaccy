use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use argon2::password_hash::{rand_core::RngCore, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher as Argon2Hasher, PasswordVerifier};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::collections::HashMap;
use std::sync::RwLock;

/// Encryption provider for column-level encryption
#[derive(Debug)]
pub struct EncryptionProvider {
    /// Active encryption keys (key_id -> key)
    keys: RwLock<HashMap<String, Vec<u8>>>,
    /// Current active key ID
    active_key_id: RwLock<String>,
}

impl EncryptionProvider {
    /// Create a new encryption provider with a master key
    pub fn new(master_key: &[u8]) -> Result<Self, EncryptionError> {
        if master_key.len() != 32 {
            return Err(EncryptionError::InvalidKeySize {
                expected: 32,
                actual: master_key.len(),
            });
        }

        let mut keys = HashMap::new();
        let key_id = "master".to_string();
        keys.insert(key_id.clone(), master_key.to_vec());

        Ok(Self {
            keys: RwLock::new(keys),
            active_key_id: RwLock::new(key_id),
        })
    }

    /// Generate a new random encryption key
    pub fn generate_key() -> Vec<u8> {
        let mut key = vec![0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Add a new encryption key for key rotation
    pub fn add_key(&self, key_id: String, key: Vec<u8>) -> Result<(), EncryptionError> {
        if key.len() != 32 {
            return Err(EncryptionError::InvalidKeySize {
                expected: 32,
                actual: key.len(),
            });
        }

        let mut keys = self.keys.write().map_err(|_| EncryptionError::LockError)?;
        keys.insert(key_id, key);
        Ok(())
    }

    /// Set the active key ID for new encryptions
    pub fn set_active_key(&self, key_id: String) -> Result<(), EncryptionError> {
        let keys = self.keys.read().map_err(|_| EncryptionError::LockError)?;
        if !keys.contains_key(&key_id) {
            return Err(EncryptionError::KeyNotFound(key_id));
        }

        let mut active = self
            .active_key_id
            .write()
            .map_err(|_| EncryptionError::LockError)?;
        *active = key_id;
        Ok(())
    }

    /// Encrypt data using the active key
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, EncryptionError> {
        let active_key_id = self
            .active_key_id
            .read()
            .map_err(|_| EncryptionError::LockError)?;
        self.encrypt_with_key(&active_key_id, plaintext)
    }

    /// Encrypt data with a specific key
    pub fn encrypt_with_key(
        &self,
        key_id: &str,
        plaintext: &[u8],
    ) -> Result<EncryptedData, EncryptionError> {
        let keys = self.keys.read().map_err(|_| EncryptionError::LockError)?;
        let key = keys
            .get(key_id)
            .ok_or_else(|| EncryptionError::KeyNotFound(key_id.to_string()))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let key_obj = Key::<Aes256Gcm>::from_slice(key);
        let cipher = Aes256Gcm::new(key_obj);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        Ok(EncryptedData {
            key_id: key_id.to_string(),
            nonce: nonce_bytes.to_vec(),
            ciphertext,
        })
    }

    /// Decrypt data
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, EncryptionError> {
        let keys = self.keys.read().map_err(|_| EncryptionError::LockError)?;
        let key = keys
            .get(&encrypted.key_id)
            .ok_or_else(|| EncryptionError::KeyNotFound(encrypted.key_id.clone()))?;

        if encrypted.nonce.len() != 12 {
            return Err(EncryptionError::InvalidNonceSize {
                expected: 12,
                actual: encrypted.nonce.len(),
            });
        }

        let nonce = Nonce::from_slice(&encrypted.nonce);
        let key_obj = Key::<Aes256Gcm>::from_slice(key);
        let cipher = Aes256Gcm::new(key_obj);

        cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))
    }

    /// Encrypt a string value
    pub fn encrypt_string(&self, plaintext: &str) -> Result<String, EncryptionError> {
        let encrypted = self.encrypt(plaintext.as_bytes())?;
        Ok(encrypted.to_base64())
    }

    /// Decrypt a string value
    pub fn decrypt_string(&self, ciphertext: &str) -> Result<String, EncryptionError> {
        let encrypted = EncryptedData::from_base64(ciphertext)?;
        let plaintext = self.decrypt(&encrypted)?;
        String::from_utf8(plaintext).map_err(|e| EncryptionError::InvalidUtf8(e.to_string()))
    }

    /// Re-encrypt data with a new key (for key rotation)
    pub fn reencrypt(
        &self,
        encrypted: &EncryptedData,
        new_key_id: &str,
    ) -> Result<EncryptedData, EncryptionError> {
        let plaintext = self.decrypt(encrypted)?;
        self.encrypt_with_key(new_key_id, &plaintext)
    }
}

/// Encrypted data structure
#[derive(Debug, Clone)]
pub struct EncryptedData {
    /// Key ID used for encryption
    pub key_id: String,
    /// Nonce used for encryption
    pub nonce: Vec<u8>,
    /// Encrypted ciphertext
    pub ciphertext: Vec<u8>,
}

impl EncryptedData {
    /// Serialize to base64-encoded string format: key_id:nonce:ciphertext
    pub fn to_base64(&self) -> String {
        format!(
            "{}:{}:{}",
            self.key_id,
            BASE64.encode(&self.nonce),
            BASE64.encode(&self.ciphertext)
        )
    }

    /// Deserialize from base64-encoded string
    pub fn from_base64(encoded: &str) -> Result<Self, EncryptionError> {
        let parts: Vec<&str> = encoded.split(':').collect();
        if parts.len() != 3 {
            return Err(EncryptionError::InvalidFormat);
        }

        let key_id = parts[0].to_string();
        let nonce = BASE64
            .decode(parts[1])
            .map_err(|e| EncryptionError::Base64Error(e.to_string()))?;
        let ciphertext = BASE64
            .decode(parts[2])
            .map_err(|e| EncryptionError::Base64Error(e.to_string()))?;

        Ok(Self {
            key_id,
            nonce,
            ciphertext,
        })
    }
}

/// Key rotation manager
#[derive(Debug)]
pub struct KeyRotationManager {
    provider: EncryptionProvider,
}

impl KeyRotationManager {
    /// Create a new key rotation manager
    pub fn new(provider: EncryptionProvider) -> Self {
        Self { provider }
    }

    /// Rotate to a new key
    pub fn rotate_key(&self, new_key_id: String, new_key: Vec<u8>) -> Result<(), EncryptionError> {
        // Add the new key
        self.provider.add_key(new_key_id.clone(), new_key)?;

        // Set it as active
        self.provider.set_active_key(new_key_id)?;

        Ok(())
    }

    /// Re-encrypt data with the current active key
    pub fn reencrypt_data(
        &self,
        old_encrypted: &EncryptedData,
    ) -> Result<EncryptedData, EncryptionError> {
        let active_key_id = self
            .provider
            .active_key_id
            .read()
            .map_err(|_| EncryptionError::LockError)?;
        self.provider.reencrypt(old_encrypted, &active_key_id)
    }
}

/// Password hashing service using Argon2
#[derive(Debug, Clone)]
pub struct PasswordHashingService {
    argon2: Argon2<'static>,
}

impl Default for PasswordHashingService {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordHashingService {
    /// Create a new password hasher
    pub fn new() -> Self {
        Self {
            argon2: Argon2::default(),
        }
    }

    /// Hash a password
    pub fn hash_password(&self, password: &str) -> Result<String, EncryptionError> {
        let salt = SaltString::generate(&mut OsRng);

        let password_hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| EncryptionError::HashingFailed(e.to_string()))?;

        Ok(password_hash.to_string())
    }

    /// Verify a password against a hash
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, EncryptionError> {
        let parsed_hash =
            PasswordHash::new(hash).map_err(|e| EncryptionError::InvalidHash(e.to_string()))?;

        match self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
        {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(EncryptionError::VerificationFailed(e.to_string())),
        }
    }
}

/// Errors that can occur during encryption operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncryptionError {
    #[error("Invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize { expected: usize, actual: usize },

    #[error("Invalid nonce size: expected {expected}, got {actual}")]
    InvalidNonceSize { expected: usize, actual: usize },

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid UTF-8: {0}")]
    InvalidUtf8(String),

    #[error("Invalid format")]
    InvalidFormat,

    #[error("Base64 error: {0}")]
    Base64Error(String),

    #[error("Lock error")]
    LockError,

    #[error("Hashing failed: {0}")]
    HashingFailed(String),

    #[error("Invalid hash: {0}")]
    InvalidHash(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key() {
        let key = EncryptionProvider::generate_key();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = EncryptionProvider::generate_key();
        let provider = EncryptionProvider::new(&key).unwrap();

        let plaintext = b"Hello, World!";
        let encrypted = provider.encrypt(plaintext).unwrap();
        let decrypted = provider.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_decrypt_string() {
        let key = EncryptionProvider::generate_key();
        let provider = EncryptionProvider::new(&key).unwrap();

        let plaintext = "Hello, World!";
        let encrypted = provider.encrypt_string(plaintext).unwrap();
        let decrypted = provider.decrypt_string(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_encrypted_data_base64() {
        let data = EncryptedData {
            key_id: "test".to_string(),
            nonce: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            ciphertext: vec![13, 14, 15, 16],
        };

        let encoded = data.to_base64();
        let decoded = EncryptedData::from_base64(&encoded).unwrap();

        assert_eq!(data.key_id, decoded.key_id);
        assert_eq!(data.nonce, decoded.nonce);
        assert_eq!(data.ciphertext, decoded.ciphertext);
    }

    #[test]
    fn test_key_rotation() {
        let key1 = EncryptionProvider::generate_key();
        let provider = EncryptionProvider::new(&key1).unwrap();

        let plaintext = b"Secret data";
        let encrypted1 = provider.encrypt(plaintext).unwrap();

        // Add a new key and rotate
        let key2 = EncryptionProvider::generate_key();
        provider.add_key("key2".to_string(), key2).unwrap();
        provider.set_active_key("key2".to_string()).unwrap();

        // Re-encrypt with new key
        let encrypted2 = provider.reencrypt(&encrypted1, "key2").unwrap();

        // Verify both can be decrypted
        let decrypted1 = provider.decrypt(&encrypted1).unwrap();
        let decrypted2 = provider.decrypt(&encrypted2).unwrap();

        assert_eq!(plaintext, decrypted1.as_slice());
        assert_eq!(plaintext, decrypted2.as_slice());
        assert_eq!(encrypted2.key_id, "key2");
    }

    #[test]
    fn test_invalid_key_size() {
        let short_key = vec![0u8; 16]; // Too short
        let result = EncryptionProvider::new(&short_key);
        assert!(result.is_err());
    }

    #[test]
    fn test_key_not_found() {
        let key = EncryptionProvider::generate_key();
        let provider = EncryptionProvider::new(&key).unwrap();

        let result = provider.encrypt_with_key("nonexistent", b"data");
        assert!(result.is_err());
    }

    #[test]
    fn test_password_hasher() {
        let hasher = PasswordHashingService::new();
        let password = "my_secure_password";

        let hash = hasher.hash_password(password).unwrap();
        assert!(hasher.verify_password(password, &hash).unwrap());
        assert!(!hasher.verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_password_hashing_produces_different_hashes() {
        let hasher = PasswordHashingService::new();
        let password = "same_password";

        let hash1 = hasher.hash_password(password).unwrap();
        let hash2 = hasher.hash_password(password).unwrap();

        // Different salts should produce different hashes
        assert_ne!(hash1, hash2);

        // But both should verify correctly
        assert!(hasher.verify_password(password, &hash1).unwrap());
        assert!(hasher.verify_password(password, &hash2).unwrap());
    }

    #[test]
    fn test_key_rotation_manager() {
        let key1 = EncryptionProvider::generate_key();
        let provider = EncryptionProvider::new(&key1).unwrap();
        let manager = KeyRotationManager::new(provider);

        let plaintext = b"Test data";
        let encrypted = manager.provider.encrypt(plaintext).unwrap();

        // Rotate key
        let key2 = EncryptionProvider::generate_key();
        manager.rotate_key("new_key".to_string(), key2).unwrap();

        // Re-encrypt with new key
        let reencrypted = manager.reencrypt_data(&encrypted).unwrap();
        let decrypted = manager.provider.decrypt(&reencrypted).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
        assert_eq!(reencrypted.key_id, "new_key");
    }
}
