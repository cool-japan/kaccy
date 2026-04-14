//! BIP 350 bech32m encoding/decoding utilities for Taproot addresses and data.
//!
//! Provides helpers for working with segwit addresses (bech32 for v0, bech32m for v1+),
//! validating Taproot (P2TR) addresses, and inspecting witness programs.
//!
//! Uses the `bitcoin` crate's address types internally.
//!
//! # Examples
//!
//! ```
//! use kaccy_bitcoin::bech32m_utils::{Bech32mCodec, TaprootAddressInfo, AddressConverter};
//!
//! // Check if an address is bech32m (Taproot / segwit v1+)
//! let taproot = "bc1p5d7rjq7g6rdk2yhzks9smlaqtedr4dekq08ge8ztwac72sfr9rusxg3297";
//! assert!(Bech32mCodec::is_bech32m(taproot));
//!
//! // Normalize an address (lowercase, trim)
//! let normalized = AddressConverter::normalize("  BC1P5D7RJQ7G6RDK2YHZKS9SMLAQTEDR4DEKQ08GE8ZTWAC72SFR9RUSXG3297  ");
//! assert_eq!(normalized, taproot);
//! ```

use std::str::FromStr;
use thiserror::Error;

use bitcoin::address::NetworkUnchecked;
use bitcoin::{Address, Network, WitnessVersion};

/// Errors from bech32m encoding/decoding operations.
#[derive(Debug, Error)]
pub enum Bech32mError {
    /// The human-readable part (HRP) is not valid for the requested network.
    #[error("Invalid HRP '{0}' for requested network")]
    InvalidHrp(String),

    /// The encoded data payload is invalid.
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Decoding the address string failed.
    #[error("Decoding failed: {0}")]
    DecodingFailed(String),

    /// Unexpected witness version.
    #[error("Invalid witness version {0}: expected 0 (bech32) or 1-16 (bech32m)")]
    InvalidWitnessVersion(u8),

    /// The witness program length is outside the valid range.
    #[error("Invalid witness program length {0}: must be 2-40 bytes")]
    InvalidLength(usize),
}

/// Parse a segwit address string into `(hrp_str, witness_version, program_bytes)`.
///
/// Returns `Err` for non-segwit addresses (P2PKH, P2SH) and invalid strings.
fn parse_segwit(address: &str) -> Result<(String, u8, Vec<u8>), Bech32mError> {
    // Try each network; the first one that parses the address and has a witness program wins.
    for network in [
        Network::Bitcoin,
        Network::Testnet,
        Network::Regtest,
        Network::Signet,
    ] {
        if let Ok(checked) = Address::from_str(address)
            .map_err(|_| ())
            .and_then(|u: Address<NetworkUnchecked>| u.require_network(network).map_err(|_| ()))
        {
            if let Some(wp) = checked.witness_program() {
                let version: WitnessVersion = wp.version();
                let program: Vec<u8> = wp.program().as_bytes().to_vec();
                let hrp_str = match network {
                    Network::Bitcoin => "bc".to_string(),
                    Network::Regtest => "bcrt".to_string(),
                    // Testnet, Testnet4, Signet all use "tb"
                    _ => "tb".to_string(),
                };
                return Ok((hrp_str, version.to_num(), program));
            }
        }
    }

    Err(Bech32mError::DecodingFailed(format!(
        "could not parse '{}' as a segwit address",
        address
    )))
}

/// Stateless bech32 / bech32m codec utilities.
///
/// Wraps the `bitcoin` crate's address parsing to provide a convenient
/// high-level API for checking and decoding segwit addresses.
pub struct Bech32mCodec;

impl Bech32mCodec {
    /// Returns `true` if `address` is a valid bech32m address (segwit version 1+).
    ///
    /// Taproot (P2TR) addresses use bech32m encoding and witness version 1.
    pub fn is_bech32m(address: &str) -> bool {
        let lower = address.to_lowercase();
        // Quick prefix check: bech32m segwit v1+ uses version character 'p'
        // bc1p / tb1p / bcrt1p
        let has_taproot_prefix =
            lower.starts_with("bc1p") || lower.starts_with("tb1p") || lower.starts_with("bcrt1p");

        if !has_taproot_prefix {
            return false;
        }

        parse_segwit(&lower)
            .map(|(_hrp, version, _program)| version >= 1)
            .unwrap_or(false)
    }

    /// Returns `true` if `address` is a valid bech32 address (segwit version 0).
    ///
    /// P2WPKH and P2WSH addresses use bech32 encoding and witness version 0.
    pub fn is_bech32(address: &str) -> bool {
        let lower = address.to_lowercase();
        // bech32 v0 uses 'q' as the version character: bc1q / tb1q / bcrt1q
        let has_v0_prefix =
            lower.starts_with("bc1q") || lower.starts_with("tb1q") || lower.starts_with("bcrt1q");

        if !has_v0_prefix {
            return false;
        }

        parse_segwit(&lower)
            .map(|(_hrp, version, _program)| version == 0)
            .unwrap_or(false)
    }

    /// Validate that `address` is a Taproot (P2TR) address for the given `network`.
    ///
    /// `network` must be one of `"main"`, `"mainnet"`, `"bitcoin"`, `"test"`,
    /// `"testnet"`, `"regtest"`, or `"signet"`.
    ///
    /// Returns `Ok(())` if valid, or a [`Bech32mError`] describing why it failed.
    pub fn validate_taproot_address(address: &str, network: &str) -> Result<(), Bech32mError> {
        let lower = address.to_lowercase();
        let (hrp, witness_version, program) = parse_segwit(&lower)?;

        let expected_hrp = match network {
            "main" | "mainnet" | "bitcoin" => "bc",
            "test" | "testnet" => "tb",
            "regtest" => "bcrt",
            "signet" => "tb",
            other => {
                return Err(Bech32mError::InvalidHrp(format!(
                    "unknown network '{}'",
                    other
                )));
            }
        };

        if hrp != expected_hrp {
            return Err(Bech32mError::InvalidHrp(format!(
                "address HRP '{}' does not match expected '{}' for network '{}'",
                hrp, expected_hrp, network
            )));
        }

        if witness_version != 1 {
            return Err(Bech32mError::InvalidWitnessVersion(witness_version));
        }

        // Taproot witness program must be exactly 32 bytes (x-only pubkey)
        if program.len() != 32 {
            return Err(Bech32mError::InvalidLength(program.len()));
        }

        Ok(())
    }

    /// Decode a segwit address into its components: `(hrp, witness_version, program)`.
    ///
    /// Works for both bech32 (v0) and bech32m (v1+) addresses.
    pub fn decode(address: &str) -> Result<(String, u8, Vec<u8>), Bech32mError> {
        parse_segwit(&address.to_lowercase())
    }

    /// Encode a witness program into a segwit address string.
    ///
    /// Uses bech32m for witness version >= 1 and bech32 for version 0.
    /// The `hrp` must be `"bc"`, `"tb"`, or `"bcrt"`.
    pub fn encode(hrp: &str, version: u8, data: &[u8]) -> Result<String, Bech32mError> {
        if version > 16 {
            return Err(Bech32mError::InvalidWitnessVersion(version));
        }
        if data.len() < 2 || data.len() > 40 {
            return Err(Bech32mError::InvalidLength(data.len()));
        }

        let network = match hrp {
            "bc" => Network::Bitcoin,
            "tb" => Network::Testnet,
            "bcrt" => Network::Regtest,
            other => {
                return Err(Bech32mError::InvalidHrp(format!(
                    "unknown HRP '{}'; expected 'bc', 'tb', or 'bcrt'",
                    other
                )));
            }
        };

        let witness_ver = WitnessVersion::try_from(version)
            .map_err(|_| Bech32mError::InvalidWitnessVersion(version))?;

        let program = bitcoin::WitnessProgram::new(witness_ver, data)
            .map_err(|e| Bech32mError::InvalidData(format!("invalid witness program: {}", e)))?;

        let address = Address::from_witness_program(program, network);
        Ok(address.to_string())
    }
}

/// Parsed information about a Taproot or segwit address.
#[derive(Debug, Clone)]
pub struct TaprootAddressInfo {
    /// Human-readable part (e.g. `"bc"`, `"tb"`, `"bcrt"`)
    pub hrp: String,
    /// Witness version (0 for P2WPKH/P2WSH, 1 for P2TR)
    pub witness_version: u8,
    /// Witness program bytes
    pub program: Vec<u8>,
    /// Whether this address is on Bitcoin mainnet
    pub is_mainnet: bool,
}

impl TaprootAddressInfo {
    /// Parse an address string into a `TaprootAddressInfo`.
    pub fn from_address(address: &str) -> Result<Self, Bech32mError> {
        let (hrp, witness_version, program) = Bech32mCodec::decode(address)?;
        let is_mainnet = hrp == "bc";
        Ok(Self {
            hrp,
            witness_version,
            program,
            is_mainnet,
        })
    }

    /// Returns `true` if this is a P2TR (Taproot) address.
    ///
    /// Taproot requires witness version 1 and a 32-byte witness program.
    pub fn is_taproot(&self) -> bool {
        self.witness_version == 1 && self.program.len() == 32
    }
}

/// Address conversion and validation helpers.
pub struct AddressConverter;

impl AddressConverter {
    /// Convert an address to lowercase.
    pub fn to_lowercase(address: &str) -> String {
        address.to_lowercase()
    }

    /// Normalize an address: lowercase and trim surrounding whitespace.
    pub fn normalize(address: &str) -> String {
        address.trim().to_lowercase()
    }

    /// Return `true` if `address` is a valid Bitcoin address (any type).
    ///
    /// Accepts bech32 (segwit v0), bech32m (segwit v1+), and base58check
    /// (legacy P2PKH / P2SH) addresses.
    pub fn is_valid_bitcoin_address(address: &str) -> bool {
        let normalized = Self::normalize(address);

        // Try segwit addresses first (bech32 / bech32m)
        if Bech32mCodec::is_bech32m(&normalized) || Bech32mCodec::is_bech32(&normalized) {
            return true;
        }

        // Try base58check (legacy P2PKH / P2SH): attempt parsing and
        // network-checking against all known networks.
        for network in [
            Network::Bitcoin,
            Network::Testnet,
            Network::Regtest,
            Network::Signet,
        ] {
            if Address::from_str(address)
                .and_then(|a: Address<NetworkUnchecked>| a.require_network(network))
                .is_ok()
            {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BIP 341 / mainnet Taproot test vector.
    const MAINNET_TAPROOT: &str = "bc1p5d7rjq7g6rdk2yhzks9smlaqtedr4dekq08ge8ztwac72sfr9rusxg3297";

    /// A known mainnet P2WPKH (bech32 v0) address.
    const MAINNET_SEGWIT_V0: &str = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";

    /// A legacy P2PKH mainnet address.
    const MAINNET_LEGACY: &str = "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2";

    #[test]
    fn test_taproot_address_info_mainnet() {
        let info =
            TaprootAddressInfo::from_address(MAINNET_TAPROOT).expect("parse taproot address");
        assert_eq!(info.hrp, "bc");
        assert_eq!(info.witness_version, 1);
        assert_eq!(info.program.len(), 32);
        assert!(info.is_mainnet);
        assert!(info.is_taproot());
    }

    #[test]
    fn test_is_bech32m_taproot() {
        assert!(Bech32mCodec::is_bech32m(MAINNET_TAPROOT));
        // Uppercase should also work after internal normalization
        assert!(Bech32mCodec::is_bech32m(&MAINNET_TAPROOT.to_uppercase()));
    }

    #[test]
    fn test_is_bech32_segwit_v0() {
        assert!(Bech32mCodec::is_bech32(MAINNET_SEGWIT_V0));
        assert!(!Bech32mCodec::is_bech32m(MAINNET_SEGWIT_V0));
    }

    #[test]
    fn test_is_not_bech32_for_taproot() {
        // Taproot must not be classified as bech32 (v0)
        assert!(!Bech32mCodec::is_bech32(MAINNET_TAPROOT));
    }

    #[test]
    fn test_validate_taproot_address_valid() {
        Bech32mCodec::validate_taproot_address(MAINNET_TAPROOT, "main")
            .expect("valid taproot address");
    }

    #[test]
    fn test_validate_taproot_address_invalid_hrp() {
        // Mainnet Taproot address — valid for mainnet, must fail for testnet validation
        let result = Bech32mCodec::validate_taproot_address(MAINNET_TAPROOT, "test");
        assert!(
            matches!(result, Err(Bech32mError::InvalidHrp(_))),
            "mainnet address must fail testnet HRP check, got: {:?}",
            result
        );

        // Validate the same address succeeds for mainnet
        Bech32mCodec::validate_taproot_address(MAINNET_TAPROOT, "main")
            .expect("must succeed for mainnet");
    }

    #[test]
    fn test_address_normalize() {
        let mixed = "  BC1P5D7RJQ7G6RDK2YHZKS9SMLAQTEDR4DEKQ08GE8ZTWAC72SFR9RUSXG3297  ";
        let normalized = AddressConverter::normalize(mixed);
        assert_eq!(normalized, MAINNET_TAPROOT);
    }

    #[test]
    fn test_is_valid_bitcoin_address_various() {
        // bech32m Taproot
        assert!(AddressConverter::is_valid_bitcoin_address(MAINNET_TAPROOT));
        // bech32 SegWit v0
        assert!(AddressConverter::is_valid_bitcoin_address(
            MAINNET_SEGWIT_V0
        ));
        // legacy P2PKH
        assert!(AddressConverter::is_valid_bitcoin_address(MAINNET_LEGACY));
        // garbage should fail
        assert!(!AddressConverter::is_valid_bitcoin_address(
            "not_an_address"
        ));
    }

    #[test]
    fn test_decode_taproot_address() {
        let (hrp, version, program) =
            Bech32mCodec::decode(MAINNET_TAPROOT).expect("decode taproot");
        assert_eq!(hrp, "bc");
        assert_eq!(version, 1);
        assert_eq!(program.len(), 32);
    }

    #[test]
    fn test_decode_segwit_v0_address() {
        let (hrp, version, program) =
            Bech32mCodec::decode(MAINNET_SEGWIT_V0).expect("decode segwit v0");
        assert_eq!(hrp, "bc");
        assert_eq!(version, 0);
        assert_eq!(program.len(), 20); // P2WPKH = 20-byte pubkey hash
    }

    #[test]
    fn test_bech32m_not_legacy() {
        assert!(!Bech32mCodec::is_bech32m(MAINNET_LEGACY));
        assert!(!Bech32mCodec::is_bech32(MAINNET_LEGACY));
    }
}
