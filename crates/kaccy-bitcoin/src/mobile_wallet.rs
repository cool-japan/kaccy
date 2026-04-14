//! Mobile wallet standard compatibility.
//!
//! Provides utilities for interoperating with common mobile Bitcoin wallets
//! including BRD (Bread Wallet), BlueWallet, Muun, Phoenix, and others.
//! Implements standard derivation paths, xpub export formats, and wallet
//! restore procedures commonly used in mobile contexts.

use serde::{Deserialize, Serialize};

// ============================================================================
// MobileWalletStandard
// ============================================================================

/// Identifies the derivation / script standard used by a mobile wallet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobileWalletStandard {
    /// BIP 44 P2PKH — legacy mobile wallets
    Bip44Legacy,
    /// BIP 49 P2SH-P2WPKH — segwit compatible (e.g. Trezor old style)
    Bip49WrappedSegwit,
    /// BIP 84 P2WPKH — native segwit (BlueWallet, etc.)
    Bip84NativeSegwit,
    /// BIP 86 P2TR — Taproot (Phoenix, etc.)
    Bip86Taproot,
    /// Muun's custom non-standard derivation
    MuunCustom,
    /// WatchTower-compatible (for Lightning)
    LightningCompatible,
}

impl MobileWalletStandard {
    /// Return the BIP-32 derivation purpose value, if this standard is BIP-defined.
    pub fn derivation_purpose(&self) -> Option<u32> {
        match self {
            Self::Bip44Legacy => Some(44),
            Self::Bip49WrappedSegwit => Some(49),
            Self::Bip84NativeSegwit => Some(84),
            Self::Bip86Taproot => Some(86),
            Self::MuunCustom | Self::LightningCompatible => None,
        }
    }

    /// Human-readable wallet name for this standard.
    pub fn wallet_name(&self) -> &'static str {
        match self {
            Self::Bip44Legacy => "BIP44 Legacy",
            Self::Bip49WrappedSegwit => "BIP49 Wrapped SegWit",
            Self::Bip84NativeSegwit => "BIP84 Native SegWit",
            Self::Bip86Taproot => "BIP86 Taproot",
            Self::MuunCustom => "Muun Custom",
            Self::LightningCompatible => "Lightning Compatible",
        }
    }

    /// Returns `true` if this standard follows BIP-32 derivation conventions.
    pub fn is_standard_bip32(&self) -> bool {
        !matches!(self, Self::MuunCustom | Self::LightningCompatible)
    }

    /// Recommended gap limit for address scanning.
    pub fn gap_limit(&self) -> u32 {
        match self {
            Self::LightningCompatible => 5,
            _ => 20,
        }
    }
}

// ============================================================================
// MobileWalletExport
// ============================================================================

/// An xpub export record for interoperability with mobile wallets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileWalletExport {
    /// The wallet standard this export targets
    pub standard: MobileWalletStandard,
    /// Extended public key (xpub / ypub / zpub)
    pub xpub: String,
    /// Derivation path of the account xpub (e.g. `m/84'/0'/0'`)
    pub derivation_path: String,
    /// Master key fingerprint (4 bytes, hex) — empty string if not provided
    pub master_fingerprint: String,
    /// Network identifier ("mainnet", "testnet", "regtest", "signet")
    pub network: String,
    /// Account index
    pub account: u32,
    /// Optional UNIX timestamp of wallet creation
    pub creation_timestamp: Option<u64>,
}

impl MobileWalletExport {
    /// Create a new `MobileWalletExport`.
    ///
    /// The derivation path is computed automatically from `standard` and `account`.
    /// `master_fingerprint` defaults to an empty string; use [`with_fingerprint`] to set it.
    ///
    /// [`with_fingerprint`]: MobileWalletExport::with_fingerprint
    pub fn new(
        standard: MobileWalletStandard,
        xpub: String,
        account: u32,
        network: bitcoin::Network,
    ) -> Self {
        let derivation_path = standard
            .derivation_purpose()
            .map(|p| {
                let coin = if network == bitcoin::Network::Bitcoin {
                    0
                } else {
                    1
                };
                format!("m/{}'/{}'/{}'", p, coin, account)
            })
            .unwrap_or_else(|| "m/custom".to_string());

        #[allow(unreachable_patterns)]
        let network_str = match network {
            bitcoin::Network::Bitcoin => "mainnet",
            bitcoin::Network::Testnet | bitcoin::Network::Testnet4 => "testnet",
            bitcoin::Network::Regtest => "regtest",
            bitcoin::Network::Signet => "signet",
            _ => "unknown",
        };

        Self {
            standard,
            xpub,
            derivation_path,
            master_fingerprint: String::new(),
            network: network_str.to_string(),
            account,
            creation_timestamp: None,
        }
    }

    /// Attach a master key fingerprint (4-byte hex string) to this export.
    pub fn with_fingerprint(mut self, fingerprint: String) -> Self {
        self.master_fingerprint = fingerprint;
        self
    }

    /// Serialize this export to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize an export from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Returns `true` if the export is for a testnet, signet, or regtest network.
    pub fn is_testnet(&self) -> bool {
        self.network != "mainnet"
    }
}

// ============================================================================
// MobileWalletScanner
// ============================================================================

/// Drives scan-based wallet recovery across standard derivation paths.
#[derive(Debug, Clone)]
pub struct MobileWalletScanner {
    /// The derivation standard to use
    pub standard: MobileWalletStandard,
    /// Number of consecutive empty addresses before stopping scan
    pub gap_limit: u32,
    /// Current account index
    pub account: u32,
    /// Maximum number of accounts to scan
    pub max_accounts: u32,
}

impl Default for MobileWalletScanner {
    fn default() -> Self {
        Self {
            standard: MobileWalletStandard::Bip84NativeSegwit,
            gap_limit: 20,
            account: 0,
            max_accounts: 5,
        }
    }
}

impl MobileWalletScanner {
    /// Create a scanner for the given standard, inheriting its default gap limit.
    pub fn new(standard: MobileWalletStandard) -> Self {
        let gap_limit = standard.gap_limit();
        Self {
            standard,
            gap_limit,
            account: 0,
            max_accounts: 5,
        }
    }

    /// Full derivation path for receive address at `index` under the current account.
    pub fn receive_path(&self, index: u32) -> String {
        let purpose = self.standard.derivation_purpose().unwrap_or(84);
        format!("m/{}'/{}'/{}'/{}/{}", purpose, 0, self.account, 0, index)
    }

    /// Full derivation path for change address at `index` under the current account.
    pub fn change_path(&self, index: u32) -> String {
        let purpose = self.standard.derivation_purpose().unwrap_or(84);
        format!("m/{}'/{}'/{}'/{}/{}", purpose, 0, self.account, 1, index)
    }

    /// Return the account-level derivation paths for all accounts up to `max_accounts`.
    pub fn all_accounts_paths(&self) -> Vec<String> {
        (0..self.max_accounts)
            .map(|a| {
                let purpose = self.standard.derivation_purpose().unwrap_or(84);
                format!("m/{}'/{}'/{}'", purpose, 0, a)
            })
            .collect()
    }
}

// ============================================================================
// WalletRestoreInfo
// ============================================================================

/// Information gathered during a wallet restore scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletRestoreInfo {
    /// Wallet standard detected during restore, if identifiable
    pub detected_standard: Option<MobileWalletStandard>,
    /// Extended public key discovered
    pub xpub: String,
    /// All account-level derivation paths explored
    pub account_paths: Vec<String>,
    /// Estimated number of addresses scanned
    pub estimated_address_count: u32,
    /// Gap limit used during scanning
    pub gap_limit: u32,
}

// ============================================================================
// MobileWalletDetector
// ============================================================================

/// Heuristic detection of the wallet standard from an xpub string or derivation path.
pub struct MobileWalletDetector;

impl MobileWalletDetector {
    /// Guess the [`MobileWalletStandard`] from an xpub prefix.
    ///
    /// | Prefix        | Standard                 |
    /// |---------------|--------------------------|
    /// | `xpub`/`tpub` | BIP 44 legacy            |
    /// | `ypub`/`upub` | BIP 49 wrapped SegWit    |
    /// | `zpub`/`vpub` | BIP 84 native SegWit     |
    pub fn detect_from_xpub(xpub: &str) -> Option<MobileWalletStandard> {
        if xpub.starts_with("xpub") || xpub.starts_with("tpub") {
            Some(MobileWalletStandard::Bip44Legacy)
        } else if xpub.starts_with("ypub") || xpub.starts_with("upub") {
            Some(MobileWalletStandard::Bip49WrappedSegwit)
        } else if xpub.starts_with("zpub") || xpub.starts_with("vpub") {
            Some(MobileWalletStandard::Bip84NativeSegwit)
        } else {
            None
        }
    }

    /// Guess the [`MobileWalletStandard`] from a derivation path string.
    pub fn suggest_standard(path: &str) -> Option<MobileWalletStandard> {
        if path.contains("44'") {
            Some(MobileWalletStandard::Bip44Legacy)
        } else if path.contains("49'") {
            Some(MobileWalletStandard::Bip49WrappedSegwit)
        } else if path.contains("84'") {
            Some(MobileWalletStandard::Bip84NativeSegwit)
        } else if path.contains("86'") {
            Some(MobileWalletStandard::Bip86Taproot)
        } else {
            None
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobile_wallet_standard_name() {
        assert_eq!(
            MobileWalletStandard::Bip44Legacy.wallet_name(),
            "BIP44 Legacy"
        );
        assert_eq!(
            MobileWalletStandard::Bip49WrappedSegwit.wallet_name(),
            "BIP49 Wrapped SegWit"
        );
        assert_eq!(
            MobileWalletStandard::Bip84NativeSegwit.wallet_name(),
            "BIP84 Native SegWit"
        );
        assert_eq!(
            MobileWalletStandard::Bip86Taproot.wallet_name(),
            "BIP86 Taproot"
        );
        assert_eq!(
            MobileWalletStandard::MuunCustom.wallet_name(),
            "Muun Custom"
        );
        assert_eq!(
            MobileWalletStandard::LightningCompatible.wallet_name(),
            "Lightning Compatible"
        );
    }

    #[test]
    fn test_mobile_wallet_standard_purpose() {
        assert_eq!(
            MobileWalletStandard::Bip44Legacy.derivation_purpose(),
            Some(44)
        );
        assert_eq!(
            MobileWalletStandard::Bip49WrappedSegwit.derivation_purpose(),
            Some(49)
        );
        assert_eq!(
            MobileWalletStandard::Bip84NativeSegwit.derivation_purpose(),
            Some(84)
        );
        assert_eq!(
            MobileWalletStandard::Bip86Taproot.derivation_purpose(),
            Some(86)
        );
        assert_eq!(MobileWalletStandard::MuunCustom.derivation_purpose(), None);
        assert_eq!(
            MobileWalletStandard::LightningCompatible.derivation_purpose(),
            None
        );
    }

    #[test]
    fn test_mobile_wallet_gap_limit() {
        assert_eq!(MobileWalletStandard::Bip84NativeSegwit.gap_limit(), 20);
        assert_eq!(MobileWalletStandard::Bip44Legacy.gap_limit(), 20);
        assert_eq!(MobileWalletStandard::LightningCompatible.gap_limit(), 5);
        assert_eq!(MobileWalletStandard::MuunCustom.gap_limit(), 20);
    }

    #[test]
    fn test_mobile_wallet_export_new() {
        let export = MobileWalletExport::new(
            MobileWalletStandard::Bip84NativeSegwit,
            "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYe2Bgb44XGX78".to_string(),
            0,
            bitcoin::Network::Bitcoin,
        );
        assert_eq!(export.derivation_path, "m/84'/0'/0'");
        assert_eq!(export.network, "mainnet");
        assert_eq!(export.account, 0);
        assert!(export.master_fingerprint.is_empty());
        assert!(!export.is_testnet());
    }

    #[test]
    fn test_mobile_wallet_export_json_roundtrip() {
        let export = MobileWalletExport::new(
            MobileWalletStandard::Bip44Legacy,
            "xpubDEAD".to_string(),
            2,
            bitcoin::Network::Bitcoin,
        )
        .with_fingerprint("aabbccdd".to_string());

        let json = export.to_json().expect("serialize");
        let restored = MobileWalletExport::from_json(&json).expect("deserialize");

        assert_eq!(restored.standard, MobileWalletStandard::Bip44Legacy);
        assert_eq!(restored.xpub, "xpubDEAD");
        assert_eq!(restored.account, 2);
        assert_eq!(restored.master_fingerprint, "aabbccdd");
    }

    #[test]
    fn test_mobile_wallet_export_testnet() {
        let export = MobileWalletExport::new(
            MobileWalletStandard::Bip84NativeSegwit,
            "vpub...".to_string(),
            0,
            bitcoin::Network::Testnet,
        );
        assert!(export.is_testnet());
        // Testnet coin type is 1.
        assert_eq!(export.derivation_path, "m/84'/1'/0'");
    }

    #[test]
    fn test_mobile_wallet_scanner_paths() {
        let scanner = MobileWalletScanner::new(MobileWalletStandard::Bip84NativeSegwit);
        assert_eq!(scanner.receive_path(0), "m/84'/0'/0'/0/0");
        assert_eq!(scanner.receive_path(5), "m/84'/0'/0'/0/5");
        assert_eq!(scanner.change_path(0), "m/84'/0'/0'/1/0");
        assert_eq!(scanner.change_path(3), "m/84'/0'/0'/1/3");
    }

    #[test]
    fn test_mobile_wallet_scanner_all_accounts() {
        let scanner = MobileWalletScanner {
            standard: MobileWalletStandard::Bip84NativeSegwit,
            gap_limit: 20,
            account: 0,
            max_accounts: 3,
        };
        let paths = scanner.all_accounts_paths();
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], "m/84'/0'/0'");
        assert_eq!(paths[1], "m/84'/0'/1'");
        assert_eq!(paths[2], "m/84'/0'/2'");
    }

    #[test]
    fn test_mobile_wallet_detector_from_xpub() {
        assert_eq!(
            MobileWalletDetector::detect_from_xpub("xpubABC"),
            Some(MobileWalletStandard::Bip44Legacy)
        );
        assert_eq!(
            MobileWalletDetector::detect_from_xpub("tpubXYZ"),
            Some(MobileWalletStandard::Bip44Legacy)
        );
        assert_eq!(
            MobileWalletDetector::detect_from_xpub("ypub123"),
            Some(MobileWalletStandard::Bip49WrappedSegwit)
        );
        assert_eq!(
            MobileWalletDetector::detect_from_xpub("upub456"),
            Some(MobileWalletStandard::Bip49WrappedSegwit)
        );
        assert_eq!(
            MobileWalletDetector::detect_from_xpub("zpubAAA"),
            Some(MobileWalletStandard::Bip84NativeSegwit)
        );
        assert_eq!(
            MobileWalletDetector::detect_from_xpub("vpubBBB"),
            Some(MobileWalletStandard::Bip84NativeSegwit)
        );
        assert_eq!(
            MobileWalletDetector::detect_from_xpub("unknown_prefix"),
            None
        );
    }

    #[test]
    fn test_mobile_wallet_detector_suggest_standard() {
        assert_eq!(
            MobileWalletDetector::suggest_standard("m/44'/0'/0'"),
            Some(MobileWalletStandard::Bip44Legacy)
        );
        assert_eq!(
            MobileWalletDetector::suggest_standard("m/49'/0'/0'"),
            Some(MobileWalletStandard::Bip49WrappedSegwit)
        );
        assert_eq!(
            MobileWalletDetector::suggest_standard("m/84'/0'/0'"),
            Some(MobileWalletStandard::Bip84NativeSegwit)
        );
        assert_eq!(
            MobileWalletDetector::suggest_standard("m/86'/0'/0'"),
            Some(MobileWalletStandard::Bip86Taproot)
        );
        assert_eq!(MobileWalletDetector::suggest_standard("m/custom"), None);
    }

    #[test]
    fn test_mobile_wallet_standard_is_standard_bip32() {
        assert!(MobileWalletStandard::Bip44Legacy.is_standard_bip32());
        assert!(MobileWalletStandard::Bip49WrappedSegwit.is_standard_bip32());
        assert!(MobileWalletStandard::Bip84NativeSegwit.is_standard_bip32());
        assert!(MobileWalletStandard::Bip86Taproot.is_standard_bip32());
        assert!(!MobileWalletStandard::MuunCustom.is_standard_bip32());
        assert!(!MobileWalletStandard::LightningCompatible.is_standard_bip32());
    }

    #[test]
    fn test_wallet_restore_info_serde() {
        let info = WalletRestoreInfo {
            detected_standard: Some(MobileWalletStandard::Bip84NativeSegwit),
            xpub: "zpubABC".to_string(),
            account_paths: vec!["m/84'/0'/0'".to_string()],
            estimated_address_count: 42,
            gap_limit: 20,
        };
        let json = serde_json::to_string(&info).expect("serialize");
        let back: WalletRestoreInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.detected_standard,
            Some(MobileWalletStandard::Bip84NativeSegwit)
        );
        assert_eq!(back.estimated_address_count, 42);
        assert_eq!(back.gap_limit, 20);
    }

    #[test]
    fn test_mobile_wallet_export_muun_path() {
        // MuunCustom has no BIP derivation purpose — falls back to "m/custom".
        let export = MobileWalletExport::new(
            MobileWalletStandard::MuunCustom,
            "xpubMUUN".to_string(),
            0,
            bitcoin::Network::Bitcoin,
        );
        assert_eq!(export.derivation_path, "m/custom");
    }
}
