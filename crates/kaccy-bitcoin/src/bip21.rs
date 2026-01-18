//! BIP 21: URI Scheme for Bitcoin Payments
//!
//! This module implements BIP 21, which defines a URI scheme for making Bitcoin
//! payment requests. These URIs are commonly used in QR codes and payment links.
//!
//! # Features
//!
//! - Parse Bitcoin payment URIs
//! - Generate payment URIs with amount, label, and message
//! - Support for additional parameters
//! - Lightning invoice fallback support
//! - Network-aware address validation
//!
//! # URI Format
//!
//! ```text
//! bitcoin:<address>[?amount=<amount>][&label=<label>][&message=<message>]
//! ```
//!
//! # Example
//!
//! ```rust
//! use kaccy_bitcoin::bip21::{BitcoinUri, BitcoinUriBuilder};
//! use bitcoin::Network;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a payment URI
//! let uri = BitcoinUriBuilder::new("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
//!     .amount(100_000) // 0.001 BTC in satoshis
//!     .label("Donation to Alice")
//!     .message("Thank you for your support")
//!     .build()?;
//!
//! let uri_string = uri.to_string();
//! // bitcoin:bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh?amount=0.00100000&label=Donation%20to%20Alice&message=Thank%20you%20for%20your%20support
//!
//! // Parse a URI
//! let parsed = BitcoinUri::parse(&uri_string)?;
//! assert_eq!(parsed.address, "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh");
//! assert_eq!(parsed.amount, Some(100_000));
//! # Ok(())
//! # }
//! ```

use crate::error::BitcoinError;
use bitcoin::{Address, Network};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// Bitcoin payment URI according to BIP 21
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BitcoinUri {
    /// Bitcoin address
    pub address: String,

    /// Amount in satoshis
    pub amount: Option<u64>,

    /// Human-readable label
    pub label: Option<String>,

    /// Human-readable message
    pub message: Option<String>,

    /// Additional parameters (e.g., for extensions)
    pub extras: HashMap<String, String>,
}

impl BitcoinUri {
    /// Create a new Bitcoin URI with just an address
    pub fn new(address: String) -> Self {
        Self {
            address,
            amount: None,
            label: None,
            message: None,
            extras: HashMap::new(),
        }
    }

    /// Parse a Bitcoin URI string
    ///
    /// # Example
    ///
    /// ```rust
    /// use kaccy_bitcoin::bip21::BitcoinUri;
    ///
    /// let uri = BitcoinUri::parse("bitcoin:bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh?amount=0.001")?;
    /// # Ok::<(), kaccy_bitcoin::BitcoinError>(())
    /// ```
    pub fn parse(uri: &str) -> Result<Self, BitcoinError> {
        // Remove "bitcoin:" prefix if present
        let uri = if let Some(stripped) = uri.strip_prefix("bitcoin:") {
            stripped
        } else {
            uri
        };

        // Split address and query parameters
        let parts: Vec<&str> = uri.splitn(2, '?').collect();
        let address = parts[0].to_string();

        if address.is_empty() {
            return Err(BitcoinError::InvalidAddress("Empty address in URI".into()));
        }

        let mut parsed = Self::new(address);

        // Parse query parameters if present
        if parts.len() > 1 {
            let query = parts[1];
            for param in query.split('&') {
                let kv: Vec<&str> = param.splitn(2, '=').collect();
                if kv.len() != 2 {
                    continue;
                }

                let key = urlencoding::decode(kv[0])
                    .map_err(|e| BitcoinError::InvalidInput(format!("URL decode error: {}", e)))?
                    .to_string();
                let value = urlencoding::decode(kv[1])
                    .map_err(|e| BitcoinError::InvalidInput(format!("URL decode error: {}", e)))?
                    .to_string();

                match key.as_str() {
                    "amount" => {
                        // Amount is in BTC, convert to satoshis
                        let btc: f64 = value.parse().map_err(|_| {
                            BitcoinError::InvalidInput(format!("Invalid amount: {}", value))
                        })?;
                        parsed.amount = Some((btc * 100_000_000.0) as u64);
                    }
                    "label" => {
                        parsed.label = Some(value);
                    }
                    "message" => {
                        parsed.message = Some(value);
                    }
                    _ => {
                        // Store as extra parameter
                        parsed.extras.insert(key, value);
                    }
                }
            }
        }

        Ok(parsed)
    }

    /// Validate the address for a specific network
    pub fn validate_address(&self, network: Network) -> Result<Address, BitcoinError> {
        let address = Address::from_str(&self.address)
            .map_err(|e| BitcoinError::InvalidAddress(format!("Invalid address: {}", e)))?;

        let validated = address.require_network(network).map_err(|_| {
            BitcoinError::InvalidAddress(format!(
                "Address network does not match expected network: {:?}",
                network
            ))
        })?;

        Ok(validated)
    }

    /// Get the amount in BTC
    pub fn amount_btc(&self) -> Option<f64> {
        self.amount.map(|sats| sats as f64 / 100_000_000.0)
    }

    /// Get an extra parameter value
    pub fn get_extra(&self, key: &str) -> Option<&str> {
        self.extras.get(key).map(|s| s.as_str())
    }

    /// Check if this URI has a Lightning invoice fallback
    pub fn has_lightning_fallback(&self) -> bool {
        self.extras.contains_key("lightning")
    }

    /// Get the Lightning invoice if present
    pub fn lightning_invoice(&self) -> Option<&str> {
        self.get_extra("lightning")
    }
}

impl fmt::Display for BitcoinUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bitcoin:{}", self.address)?;

        let mut params = Vec::new();

        if let Some(amount) = self.amount {
            let btc = amount as f64 / 100_000_000.0;
            params.push(format!("amount={:.8}", btc));
        }

        if let Some(ref label) = self.label {
            params.push(format!("label={}", urlencoding::encode(label)));
        }

        if let Some(ref message) = self.message {
            params.push(format!("message={}", urlencoding::encode(message)));
        }

        // Add extra parameters in sorted order for consistency
        let mut extra_keys: Vec<_> = self.extras.keys().collect();
        extra_keys.sort();
        for key in extra_keys {
            if let Some(value) = self.extras.get(key) {
                params.push(format!(
                    "{}={}",
                    urlencoding::encode(key),
                    urlencoding::encode(value)
                ));
            }
        }

        if !params.is_empty() {
            write!(f, "?{}", params.join("&"))?;
        }

        Ok(())
    }
}

/// Builder for creating Bitcoin URIs
pub struct BitcoinUriBuilder {
    uri: BitcoinUri,
}

impl BitcoinUriBuilder {
    /// Create a new URI builder with an address
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            uri: BitcoinUri::new(address.into()),
        }
    }

    /// Set the payment amount in satoshis
    pub fn amount(mut self, satoshis: u64) -> Self {
        self.uri.amount = Some(satoshis);
        self
    }

    /// Set the payment amount in BTC
    pub fn amount_btc(mut self, btc: f64) -> Self {
        self.uri.amount = Some((btc * 100_000_000.0) as u64);
        self
    }

    /// Set the label
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.uri.label = Some(label.into());
        self
    }

    /// Set the message
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.uri.message = Some(message.into());
        self
    }

    /// Add a custom parameter
    pub fn extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.uri.extras.insert(key.into(), value.into());
        self
    }

    /// Add a Lightning invoice fallback
    pub fn lightning(mut self, invoice: impl Into<String>) -> Self {
        self.uri
            .extras
            .insert("lightning".to_string(), invoice.into());
        self
    }

    /// Build the URI
    pub fn build(self) -> Result<BitcoinUri, BitcoinError> {
        // Validate that the address is not empty
        if self.uri.address.is_empty() {
            return Err(BitcoinError::InvalidAddress(
                "Address cannot be empty".into(),
            ));
        }

        Ok(self.uri)
    }
}

/// QR code generation helper for Bitcoin URIs
pub struct QrCodeHelper;

impl QrCodeHelper {
    /// Get the recommended QR code error correction level
    ///
    /// BIP 21 recommends using Medium (M) level
    pub fn recommended_error_correction() -> &'static str {
        "M" // Medium: ~15% error correction
    }

    /// Estimate QR code size needed for a URI
    ///
    /// Returns the recommended QR code version (1-40)
    pub fn estimate_qr_version(uri: &BitcoinUri) -> u8 {
        let uri_string = uri.to_string();
        let len = uri_string.len();

        // Rough estimation based on URI length
        // QR version 1 can hold ~25 alphanumeric chars
        // Each version adds ~4 chars capacity
        ((len as f64 / 25.0).ceil() as u8).clamp(1, 40)
    }

    /// Check if a URI is suitable for QR code
    ///
    /// Returns true if the URI can fit in a reasonable QR code size
    pub fn is_qr_friendly(uri: &BitcoinUri) -> bool {
        let uri_string = uri.to_string();
        // QR codes become hard to scan above version 20 (~470 chars)
        uri_string.len() <= 470
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_uri() {
        let uri = BitcoinUri::new("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string());
        let uri_string = uri.to_string();
        assert_eq!(
            uri_string,
            "bitcoin:bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"
        );
    }

    #[test]
    fn test_uri_with_amount() {
        let uri = BitcoinUriBuilder::new("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
            .amount(100_000)
            .build()
            .unwrap();

        let uri_string = uri.to_string();
        assert!(uri_string.contains("amount=0.00100000"));
    }

    #[test]
    fn test_uri_with_all_fields() {
        let uri = BitcoinUriBuilder::new("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
            .amount(100_000)
            .label("Donation")
            .message("Thank you")
            .build()
            .unwrap();

        let uri_string = uri.to_string();
        assert!(uri_string.contains("amount=0.00100000"));
        assert!(uri_string.contains("label=Donation"));
        assert!(uri_string.contains("message=Thank%20you"));
    }

    #[test]
    fn test_parse_simple_uri() {
        let uri = BitcoinUri::parse("bitcoin:bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh").unwrap();
        assert_eq!(uri.address, "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh");
        assert_eq!(uri.amount, None);
        assert_eq!(uri.label, None);
    }

    #[test]
    fn test_parse_uri_with_amount() {
        let uri =
            BitcoinUri::parse("bitcoin:bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh?amount=0.001")
                .unwrap();
        assert_eq!(uri.amount, Some(100_000));
        assert_eq!(uri.amount_btc(), Some(0.001));
    }

    #[test]
    fn test_parse_uri_with_all_fields() {
        let uri = BitcoinUri::parse(
            "bitcoin:bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh?amount=0.001&label=Donation&message=Thank%20you",
        )
        .unwrap();
        assert_eq!(uri.address, "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh");
        assert_eq!(uri.amount, Some(100_000));
        assert_eq!(uri.label, Some("Donation".to_string()));
        assert_eq!(uri.message, Some("Thank you".to_string()));
    }

    #[test]
    fn test_parse_without_prefix() {
        let uri =
            BitcoinUri::parse("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh?amount=0.001").unwrap();
        assert_eq!(uri.address, "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh");
        assert_eq!(uri.amount, Some(100_000));
    }

    #[test]
    fn test_extra_parameters() {
        let uri = BitcoinUriBuilder::new("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
            .extra("req-payment", "xyz123")
            .build()
            .unwrap();

        assert_eq!(uri.get_extra("req-payment"), Some("xyz123"));
    }

    #[test]
    fn test_lightning_fallback() {
        let uri = BitcoinUriBuilder::new("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
            .lightning("lnbc1...")
            .build()
            .unwrap();

        assert!(uri.has_lightning_fallback());
        assert_eq!(uri.lightning_invoice(), Some("lnbc1..."));
    }

    #[test]
    fn test_roundtrip() {
        let original = BitcoinUriBuilder::new("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
            .amount(100_000)
            .label("Test Label")
            .message("Test Message")
            .build()
            .unwrap();

        let uri_string = original.to_string();
        let parsed = BitcoinUri::parse(&uri_string).unwrap();

        assert_eq!(parsed.address, original.address);
        assert_eq!(parsed.amount, original.amount);
        assert_eq!(parsed.label, original.label);
        assert_eq!(parsed.message, original.message);
    }

    #[test]
    fn test_qr_helper() {
        let uri = BitcoinUri::new("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh".to_string());
        assert!(QrCodeHelper::is_qr_friendly(&uri));

        let version = QrCodeHelper::estimate_qr_version(&uri);
        assert!(version > 0 && version <= 40);
    }

    #[test]
    fn test_url_encoding() {
        let uri = BitcoinUriBuilder::new("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
            .label("Donation & Support")
            .message("Thank you! 🎉")
            .build()
            .unwrap();

        let uri_string = uri.to_string();
        assert!(uri_string.contains("Donation%20%26%20Support"));

        let parsed = BitcoinUri::parse(&uri_string).unwrap();
        assert_eq!(parsed.label, Some("Donation & Support".to_string()));
        assert_eq!(parsed.message, Some("Thank you! 🎉".to_string()));
    }

    #[test]
    fn test_empty_address_error() {
        let result = BitcoinUriBuilder::new("").build();
        assert!(result.is_err());
    }
}
