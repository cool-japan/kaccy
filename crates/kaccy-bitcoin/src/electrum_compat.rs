//! Electrum wallet compatibility utilities.
//!
//! Provides tools for interoperating with Electrum-format wallets including
//! derivation path compatibility, address format handling, and wallet file
//! format parsing.
//!
//! Electrum uses a specific set of derivation paths, extended-key version bytes
//! (`ypub`, `zpub`, …), and a default gap limit of 20 that differ slightly from
//! bare BIP 32/44/49/84/86 conventions.  This module bridges those differences.
//!
//! # Derivation paths by wallet type
//!
//! | Type | BIP | Mainnet path |
//! |------|-----|--------------|
//! | Standard (P2PKH) | BIP 44 | `m/44'/0'/0'` |
//! | P2SH-P2WPKH (nested SegWit) | BIP 49 | `m/49'/0'/0'` |
//! | Native P2WPKH | BIP 84 | `m/84'/0'/0'` |
//! | P2TR (Taproot) | BIP 86 | `m/86'/0'/0'` |
//!
//! # Example
//!
//! ```rust
//! use kaccy_bitcoin::electrum_compat::{ElectrumWalletType, ElectrumAddressDeriver};
//! use bitcoin::Network;
//!
//! let deriver = ElectrumAddressDeriver::new(
//!     ElectrumWalletType::NativeSegwit,
//!     0,
//!     Network::Bitcoin,
//! );
//! assert_eq!(deriver.receive_path(0), "m/84'/0'/0'/0/0");
//! assert_eq!(deriver.change_path(3), "m/84'/0'/0'/1/3");
//! ```

use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Errors that can arise when working with Electrum wallet data.
#[derive(Debug, thiserror::Error)]
pub enum ElectrumError {
    /// The provided master or extended key is invalid.
    #[error("Invalid master key: {0}")]
    InvalidMasterKey(String),
    /// The derivation path string is malformed.
    #[error("Invalid derivation path: {0}")]
    InvalidDerivationPath(String),
    /// Checksum mismatch when decoding an encoded key or address.
    #[error("Invalid checksum: {0}")]
    InvalidChecksum(String),
    /// The address string could not be parsed.
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    /// The wallet type is not supported by this operation.
    #[error("Unsupported wallet type: {0}")]
    UnsupportedWalletType(String),
    /// A generic parse error.
    #[error("Parse error: {0}")]
    ParseError(String),
}

// ─── Wallet type ─────────────────────────────────────────────────────────────

/// The script/address type used by an Electrum wallet.
///
/// Each variant maps to a well-known BIP derivation purpose and an Electrum
/// `script_type` string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElectrumWalletType {
    /// Standard P2PKH (legacy) — `m/44'/0'/account'`.
    Standard,
    /// P2SH-P2WPKH (SegWit nested in P2SH) — `m/49'/0'/account'`.
    P2ShSegwit,
    /// Native P2WPKH (bech32) — `m/84'/0'/account'`.
    NativeSegwit,
    /// P2TR Taproot — `m/86'/0'/account'`.
    Taproot,
    /// M-of-N multisig wallet.
    Multisig {
        /// Required signatures.
        m: u8,
        /// Total co-signers.
        n: u8,
    },
}

impl ElectrumWalletType {
    /// Return the BIP purpose level (44 / 49 / 84 / 86) for this wallet type,
    /// or `None` for multisig which uses BIP 45.
    pub fn purpose(&self) -> Option<u32> {
        match self {
            Self::Standard => Some(44),
            Self::P2ShSegwit => Some(49),
            Self::NativeSegwit => Some(84),
            Self::Taproot => Some(86),
            Self::Multisig { .. } => None,
        }
    }

    /// Return the Electrum `script_type` identifier string for this wallet.
    pub fn script_type(&self) -> &'static str {
        match self {
            Self::Standard => "p2pkh",
            Self::P2ShSegwit => "p2wpkh-p2sh",
            Self::NativeSegwit => "p2wpkh",
            Self::Taproot => "p2tr",
            Self::Multisig { .. } => "p2sh",
        }
    }

    /// Build the standard account-level derivation path for mainnet.
    ///
    /// For multisig wallets this follows BIP 45 (`m/45'/account`).
    pub fn standard_path(&self, account: u32) -> String {
        match self {
            Self::Standard => format!("m/44'/0'/{}'", account),
            Self::P2ShSegwit => format!("m/49'/0'/{}'", account),
            Self::NativeSegwit => format!("m/84'/0'/{}'", account),
            Self::Taproot => format!("m/86'/0'/{}'", account),
            Self::Multisig { .. } => format!("m/45'/{}", account),
        }
    }

    /// Detect the wallet type from a known derivation path prefix.
    ///
    /// Returns `None` if the path doesn't match any known BIP purpose.
    pub fn from_path(path: &str) -> Option<Self> {
        if path.contains("m/44'") || path.contains("m/44/") {
            Some(Self::Standard)
        } else if path.contains("m/49'") || path.contains("m/49/") {
            Some(Self::P2ShSegwit)
        } else if path.contains("m/84'") || path.contains("m/84/") {
            Some(Self::NativeSegwit)
        } else if path.contains("m/86'") || path.contains("m/86/") {
            Some(Self::Taproot)
        } else {
            None
        }
    }
}

// ─── Master key ──────────────────────────────────────────────────────────────

/// An Electrum extended public key together with wallet type metadata.
///
/// Electrum stores extended keys using version-prefixed encodings (`xpub`,
/// `ypub`, `zpub`, `Xpub`, `Ypub`, `Zpub`, `Vpub`, …).  This struct captures
/// both the raw key string and the inferred wallet type.
#[derive(Debug, Clone)]
pub struct ElectrumMasterKey {
    /// Detected or provided wallet type.
    pub key_type: ElectrumWalletType,
    /// The extended public key string as provided (xpub / ypub / zpub …).
    pub xpub: String,
    /// 8-hex-character master fingerprint (derived from the key prefix or
    /// set explicitly).
    pub fingerprint: String,
}

impl ElectrumMasterKey {
    /// Construct an [`ElectrumMasterKey`] from an xpub string and explicit
    /// wallet type.
    ///
    /// The fingerprint is set to `"00000000"` as a placeholder — a full
    /// implementation would derive it from the actual key material.
    ///
    /// # Errors
    ///
    /// Returns [`ElectrumError::InvalidMasterKey`] if `xpub` is empty.
    pub fn from_xpub(xpub: &str, key_type: ElectrumWalletType) -> Result<Self, ElectrumError> {
        if xpub.is_empty() {
            return Err(ElectrumError::InvalidMasterKey("empty xpub string".into()));
        }
        // Placeholder fingerprint — computing the real fingerprint requires
        // full BIP 32 key derivation which is out of scope for compatibility
        // metadata storage.
        let fingerprint = "00000000".to_string();
        Ok(Self {
            key_type,
            xpub: xpub.to_string(),
            fingerprint,
        })
    }

    /// Returns `true` if this key is for a SegWit wallet (nested or native).
    pub fn is_segwit(&self) -> bool {
        matches!(
            self.key_type,
            ElectrumWalletType::P2ShSegwit | ElectrumWalletType::NativeSegwit
        )
    }

    /// Returns `true` if this key is for a Taproot wallet.
    pub fn is_taproot(&self) -> bool {
        matches!(self.key_type, ElectrumWalletType::Taproot)
    }

    /// Return the key string, noting that version-byte conversion (e.g.
    /// `ypub` → `xpub`) requires knowledge of the raw key bytes and is
    /// therefore not performed here.
    ///
    /// Callers requiring the actual `xpub` encoding must re-derive the key
    /// using a BIP 32 library such as `bitcoin::bip32`.
    pub fn to_normalized_xpub(&self) -> String {
        // Note: full re-encoding from ypub/zpub to xpub requires the raw key
        // bytes.  This method returns the key as-is with a comment for
        // diagnostic purposes.
        self.xpub.clone()
    }

    /// Build the address-level child path suffix (not the full derivation path)
    /// for the given chain and index.
    ///
    /// `change = false` → receive chain (`/0/index`).
    /// `change = true`  → change chain  (`/1/index`).
    pub fn derive_address_path(&self, change: bool, index: u32) -> String {
        let chain: u32 = if change { 1 } else { 0 };
        format!("/{}/{}", chain, index)
    }
}

// ─── Address deriver ─────────────────────────────────────────────────────────

/// Derives Electrum-compatible full derivation paths for receive and change
/// addresses.
///
/// Path structure: `m/<purpose>'/<coin_type>'/<account>'/<chain>/<index>`
///
/// where `<chain>` is 0 for receive and 1 for change.
#[derive(Debug, Clone)]
pub struct ElectrumAddressDeriver {
    /// Wallet type determines the purpose and script format.
    pub wallet_type: ElectrumWalletType,
    /// BIP 44 account index (0-based).
    pub account: u32,
    /// Target network.
    pub network: bitcoin::Network,
}

impl ElectrumAddressDeriver {
    /// Create a new deriver for the given wallet type, account, and network.
    pub fn new(wallet_type: ElectrumWalletType, account: u32, network: bitcoin::Network) -> Self {
        Self {
            wallet_type,
            account,
            network,
        }
    }

    /// Return the full BIP-44-style path for a receive address at `index`.
    ///
    /// Example: `m/84'/0'/0'/0/5` for native SegWit, account 0, index 5.
    pub fn receive_path(&self, index: u32) -> String {
        let base = self.account_path();
        format!("{}/0/{}", base, index)
    }

    /// Return the full BIP-44-style path for a change address at `index`.
    ///
    /// Example: `m/84'/0'/0'/1/2` for native SegWit, account 0, index 2.
    pub fn change_path(&self, index: u32) -> String {
        let base = self.account_path();
        format!("{}/1/{}", base, index)
    }

    /// Determine whether a full derivation path refers to the change chain.
    ///
    /// Returns `true` when the second-to-last path component is `1`.
    pub fn is_change_path(path: &str) -> bool {
        // Split on '/' and look at the second-to-last segment
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 2 {
            return false;
        }
        let chain_part = parts[parts.len() - 2];
        // Strip trailing hardening markers before comparing
        let chain_clean = chain_part.trim_end_matches(['\'', 'h']);
        chain_clean == "1"
    }

    /// Return a human-readable name for the address type produced by this deriver.
    pub fn address_type_name(&self) -> &'static str {
        self.wallet_type.script_type()
    }

    // ── private ──────────────────────────────────────────────────────────────

    /// Compute the account-level path (`m/purpose'/coin_type'/account'`).
    fn account_path(&self) -> String {
        let coin_type = match self.network {
            bitcoin::Network::Bitcoin => 0,
            _ => 1,
        };
        match &self.wallet_type {
            ElectrumWalletType::Standard => {
                format!("m/44'/{}'/{}'", coin_type, self.account)
            }
            ElectrumWalletType::P2ShSegwit => {
                format!("m/49'/{}'/{}'", coin_type, self.account)
            }
            ElectrumWalletType::NativeSegwit => {
                format!("m/84'/{}'/{}'", coin_type, self.account)
            }
            ElectrumWalletType::Taproot => {
                format!("m/86'/{}'/{}'", coin_type, self.account)
            }
            ElectrumWalletType::Multisig { .. } => {
                format!("m/45'/{}", self.account)
            }
        }
    }
}

// ─── Wallet info ─────────────────────────────────────────────────────────────

/// Metadata describing an Electrum-format wallet.
///
/// This struct aggregates the key configuration parameters that Electrum
/// stores in its wallet JSON files.
#[derive(Debug, Clone)]
pub struct ElectrumWalletInfo {
    /// Script/address type.
    pub wallet_type: ElectrumWalletType,
    /// The wallet's extended public key.
    pub xpub: String,
    /// BIP 44 account index.
    pub account: u32,
    /// Target Bitcoin network.
    pub network: bitcoin::Network,
    /// Whether this is a watch-only wallet (no private keys stored).
    pub is_watch_only: bool,
    /// Address gap limit (Electrum default: 20).
    pub gap_limit: u32,
}

impl Default for ElectrumWalletInfo {
    fn default() -> Self {
        Self {
            wallet_type: ElectrumWalletType::NativeSegwit,
            xpub: String::new(),
            account: 0,
            network: bitcoin::Network::Bitcoin,
            is_watch_only: true,
            gap_limit: 20,
        }
    }
}

// ─── Compatibility utilities ──────────────────────────────────────────────────

/// A collection of stateless utility functions for Electrum wallet
/// interoperability.
pub struct ElectrumCompatibility;

impl ElectrumCompatibility {
    /// Detect the Electrum wallet type from an extended key version prefix.
    ///
    /// Returns `None` for unrecognised prefixes.
    ///
    /// | Prefix | Type | Network |
    /// |--------|------|---------|
    /// | `xpub` | Standard | Mainnet |
    /// | `ypub` | P2SH-SegWit | Mainnet |
    /// | `zpub` | Native SegWit | Mainnet |
    /// | `Xpub` | Standard | Testnet |
    /// | `Ypub` | P2SH-SegWit | Testnet |
    /// | `Zpub` | Native SegWit | Testnet |
    /// | `Vpub` | Taproot | Testnet |
    /// | `Upub` | P2SH-SegWit | Testnet (alt) |
    pub fn detect_key_type(key: &str) -> Option<ElectrumWalletType> {
        if key.starts_with("xpub") || key.starts_with("Xpub") {
            Some(ElectrumWalletType::Standard)
        } else if key.starts_with("ypub") || key.starts_with("Ypub") || key.starts_with("Upub") {
            Some(ElectrumWalletType::P2ShSegwit)
        } else if key.starts_with("zpub") || key.starts_with("Zpub") {
            Some(ElectrumWalletType::NativeSegwit)
        } else if key.starts_with("Vpub") {
            Some(ElectrumWalletType::Taproot)
        } else {
            None
        }
    }

    /// Attempt to normalise a version-prefixed key to an `xpub`-prefixed key.
    ///
    /// **Important:** A proper conversion requires re-encoding the raw key
    /// bytes with the standard `xpub` version bytes.  Because we only have the
    /// base58-encoded string here, this function returns the key unchanged
    /// together with a note explaining that full conversion requires the raw
    /// key material.
    ///
    /// # Errors
    ///
    /// Returns [`ElectrumError::InvalidMasterKey`] if the key string is empty.
    pub fn normalize_to_xpub(versioned_key: &str) -> Result<String, ElectrumError> {
        if versioned_key.is_empty() {
            return Err(ElectrumError::InvalidMasterKey(
                "empty key string".to_string(),
            ));
        }
        // Full re-encoding from ypub/zpub → xpub requires deserialising the
        // base58-check payload and changing the 4-byte version prefix before
        // re-encoding.  Return the key as-is; callers should use a BIP 32
        // library for the actual conversion.
        Ok(versioned_key.to_string())
    }

    /// Return Electrum's default address gap limit (20).
    pub fn gap_limit_default() -> u32 {
        20
    }

    /// Validate a BIP 39 mnemonic seed phrase using the `bip39` crate.
    ///
    /// Returns `true` when the mnemonic has a valid checksum and wordlist
    /// membership.  Note that Electrum's native seed format uses a different
    /// versioning scheme; this function only validates BIP 39 mnemonics.
    pub fn is_valid_electrum_seed(words: &str) -> bool {
        bip39::Mnemonic::from_str(words).is_ok()
    }

    /// Build the full receive derivation path for a given wallet type,
    /// account, and address index.
    ///
    /// Coin type is always 0 (mainnet).
    pub fn standard_receive_path(
        wallet_type: &ElectrumWalletType,
        account: u32,
        index: u32,
    ) -> String {
        let deriver =
            ElectrumAddressDeriver::new(wallet_type.clone(), account, bitcoin::Network::Bitcoin);
        deriver.receive_path(index)
    }

    /// Build the full change derivation path for a given wallet type,
    /// account, and address index.
    ///
    /// Coin type is always 0 (mainnet).
    pub fn standard_change_path(
        wallet_type: &ElectrumWalletType,
        account: u32,
        index: u32,
    ) -> String {
        let deriver =
            ElectrumAddressDeriver::new(wallet_type.clone(), account, bitcoin::Network::Bitcoin);
        deriver.change_path(index)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_type_purpose() {
        assert_eq!(ElectrumWalletType::Standard.purpose(), Some(44));
        assert_eq!(ElectrumWalletType::P2ShSegwit.purpose(), Some(49));
        assert_eq!(ElectrumWalletType::NativeSegwit.purpose(), Some(84));
        assert_eq!(ElectrumWalletType::Taproot.purpose(), Some(86));
        assert_eq!(ElectrumWalletType::Multisig { m: 2, n: 3 }.purpose(), None);
    }

    #[test]
    fn test_wallet_type_standard_path() {
        assert_eq!(ElectrumWalletType::Standard.standard_path(0), "m/44'/0'/0'");
        assert_eq!(
            ElectrumWalletType::P2ShSegwit.standard_path(1),
            "m/49'/0'/1'"
        );
        assert_eq!(
            ElectrumWalletType::NativeSegwit.standard_path(0),
            "m/84'/0'/0'"
        );
        assert_eq!(ElectrumWalletType::Taproot.standard_path(2), "m/86'/0'/2'");
    }

    #[test]
    fn test_wallet_type_from_path_native_segwit() {
        let path = "m/84'/0'/0'/0/5";
        let wt = ElectrumWalletType::from_path(path);
        assert_eq!(wt, Some(ElectrumWalletType::NativeSegwit));
    }

    #[test]
    fn test_wallet_type_from_path_taproot() {
        let path = "m/86'/0'/0'/1/0";
        let wt = ElectrumWalletType::from_path(path);
        assert_eq!(wt, Some(ElectrumWalletType::Taproot));
    }

    #[test]
    fn test_wallet_type_from_path_unknown() {
        let path = "m/0'/0'/0'";
        let wt = ElectrumWalletType::from_path(path);
        assert!(wt.is_none());
    }

    #[test]
    fn test_address_deriver_receive_path() {
        let d = ElectrumAddressDeriver::new(
            ElectrumWalletType::NativeSegwit,
            0,
            bitcoin::Network::Bitcoin,
        );
        assert_eq!(d.receive_path(0), "m/84'/0'/0'/0/0");
        assert_eq!(d.receive_path(5), "m/84'/0'/0'/0/5");
    }

    #[test]
    fn test_address_deriver_change_path() {
        let d = ElectrumAddressDeriver::new(
            ElectrumWalletType::NativeSegwit,
            0,
            bitcoin::Network::Bitcoin,
        );
        assert_eq!(d.change_path(0), "m/84'/0'/0'/1/0");
        assert_eq!(d.change_path(2), "m/84'/0'/0'/1/2");
    }

    #[test]
    fn test_is_change_path() {
        assert!(ElectrumAddressDeriver::is_change_path("m/84'/0'/0'/1/5"));
        assert!(!ElectrumAddressDeriver::is_change_path("m/84'/0'/0'/0/5"));
    }

    #[test]
    fn test_detect_key_type_xpub() {
        let kt = ElectrumCompatibility::detect_key_type("xpub661MyMwAqRbcG...");
        assert_eq!(kt, Some(ElectrumWalletType::Standard));
    }

    #[test]
    fn test_detect_key_type_zpub() {
        let kt = ElectrumCompatibility::detect_key_type("zpub6r...");
        assert_eq!(kt, Some(ElectrumWalletType::NativeSegwit));
    }

    #[test]
    fn test_detect_key_type_ypub() {
        let kt = ElectrumCompatibility::detect_key_type("ypub6Qqz3g...");
        assert_eq!(kt, Some(ElectrumWalletType::P2ShSegwit));
    }

    #[test]
    fn test_detect_key_type_vpub_taproot() {
        let kt = ElectrumCompatibility::detect_key_type("Vpub5eCh...");
        assert_eq!(kt, Some(ElectrumWalletType::Taproot));
    }

    #[test]
    fn test_detect_key_type_unknown() {
        let kt = ElectrumCompatibility::detect_key_type("1BpEi6DfDAUFd153wN...");
        assert!(kt.is_none());
    }

    #[test]
    fn test_electrum_compat_receive_path() {
        let path = ElectrumCompatibility::standard_receive_path(&ElectrumWalletType::Taproot, 0, 7);
        assert_eq!(path, "m/86'/0'/0'/0/7");
    }

    #[test]
    fn test_electrum_compat_change_path() {
        let path = ElectrumCompatibility::standard_change_path(&ElectrumWalletType::Standard, 1, 3);
        assert_eq!(path, "m/44'/0'/1'/1/3");
    }

    #[test]
    fn test_wallet_info_default_gap_limit() {
        let info = ElectrumWalletInfo::default();
        assert_eq!(info.gap_limit, 20);
        assert!(info.is_watch_only);
        assert_eq!(info.account, 0);
        assert_eq!(info.network, bitcoin::Network::Bitcoin);
    }

    #[test]
    fn test_script_type_names() {
        assert_eq!(ElectrumWalletType::Standard.script_type(), "p2pkh");
        assert_eq!(ElectrumWalletType::P2ShSegwit.script_type(), "p2wpkh-p2sh");
        assert_eq!(ElectrumWalletType::NativeSegwit.script_type(), "p2wpkh");
        assert_eq!(ElectrumWalletType::Taproot.script_type(), "p2tr");
        assert_eq!(
            ElectrumWalletType::Multisig { m: 2, n: 3 }.script_type(),
            "p2sh"
        );
    }

    #[test]
    fn test_master_key_from_xpub() {
        let key =
            ElectrumMasterKey::from_xpub("xpub661MyMwAqRbcG...", ElectrumWalletType::Standard)
                .expect("should construct key");
        assert!(!key.is_segwit());
        assert!(!key.is_taproot());
        assert_eq!(key.fingerprint, "00000000");
    }

    #[test]
    fn test_master_key_empty_fails() {
        let result = ElectrumMasterKey::from_xpub("", ElectrumWalletType::Standard);
        assert!(matches!(result, Err(ElectrumError::InvalidMasterKey(_))));
    }

    #[test]
    fn test_master_key_is_segwit() {
        let key = ElectrumMasterKey::from_xpub("zpub6r...", ElectrumWalletType::NativeSegwit)
            .expect("ok");
        assert!(key.is_segwit());
        assert!(!key.is_taproot());
    }

    #[test]
    fn test_master_key_is_taproot() {
        let key =
            ElectrumMasterKey::from_xpub("Vpub5eCh...", ElectrumWalletType::Taproot).expect("ok");
        assert!(!key.is_segwit());
        assert!(key.is_taproot());
    }

    #[test]
    fn test_derive_address_path() {
        let key =
            ElectrumMasterKey::from_xpub("xpub...", ElectrumWalletType::Standard).expect("ok");
        assert_eq!(key.derive_address_path(false, 0), "/0/0");
        assert_eq!(key.derive_address_path(true, 5), "/1/5");
    }

    #[test]
    fn test_gap_limit_default() {
        assert_eq!(ElectrumCompatibility::gap_limit_default(), 20);
    }

    #[test]
    fn test_normalize_to_xpub_empty_fails() {
        let result = ElectrumCompatibility::normalize_to_xpub("");
        assert!(matches!(result, Err(ElectrumError::InvalidMasterKey(_))));
    }

    #[test]
    fn test_normalize_to_xpub_returns_input() {
        let key = "ypub6Qqz3gSFE8bN...";
        let result = ElectrumCompatibility::normalize_to_xpub(key).expect("ok");
        assert_eq!(result, key);
    }

    #[test]
    fn test_address_deriver_testnet() {
        let d = ElectrumAddressDeriver::new(
            ElectrumWalletType::NativeSegwit,
            0,
            bitcoin::Network::Testnet,
        );
        // Coin type for testnet is 1
        assert_eq!(d.receive_path(0), "m/84'/1'/0'/0/0");
    }

    #[test]
    fn test_multisig_path() {
        let d = ElectrumAddressDeriver::new(
            ElectrumWalletType::Multisig { m: 2, n: 3 },
            0,
            bitcoin::Network::Bitcoin,
        );
        assert_eq!(d.receive_path(0), "m/45'/0/0/0");
    }
}
