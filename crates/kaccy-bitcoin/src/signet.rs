//! Signet network support
//!
//! Signet is a Bitcoin test network with centralized block signing,
//! providing more reliability than testnet for testing applications.

use crate::client::{BitcoinClient, BitcoinNetwork};
use crate::error::{BitcoinError, Result};
use serde::{Deserialize, Serialize};

/// Signet network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignetConfig {
    /// RPC URL for the signet node
    pub rpc_url: String,
    /// RPC username
    pub rpc_user: String,
    /// RPC password
    pub rpc_password: String,
    /// Whether this is a custom signet (vs. default signet)
    pub is_custom: bool,
    /// Challenge script for custom signet (hex encoded)
    pub custom_challenge: Option<String>,
    /// Custom signet network magic (optional)
    pub network_magic: Option<u32>,
}

impl Default for SignetConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://127.0.0.1:38332".to_string(),
            rpc_user: "signet".to_string(),
            rpc_password: "signet".to_string(),
            is_custom: false,
            custom_challenge: None,
            network_magic: None,
        }
    }
}

impl SignetConfig {
    /// Create a new configuration for the default signet network
    pub fn default_signet(rpc_url: String, rpc_user: String, rpc_password: String) -> Self {
        Self {
            rpc_url,
            rpc_user,
            rpc_password,
            is_custom: false,
            custom_challenge: None,
            network_magic: None,
        }
    }

    /// Create a new configuration for a custom signet network
    pub fn custom_signet(
        rpc_url: String,
        rpc_user: String,
        rpc_password: String,
        challenge_script: String,
        network_magic: Option<u32>,
    ) -> Result<Self> {
        // Validate challenge script is valid hex
        if !challenge_script.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(BitcoinError::InvalidAddress(
                "Challenge script must be valid hex".to_string(),
            ));
        }

        Ok(Self {
            rpc_url,
            rpc_user,
            rpc_password,
            is_custom: true,
            custom_challenge: Some(challenge_script),
            network_magic,
        })
    }

    /// Validate the signet configuration
    pub fn validate(&self) -> Result<()> {
        // Ensure URL is not empty
        if self.rpc_url.is_empty() {
            return Err(BitcoinError::InvalidAddress(
                "RPC URL cannot be empty".to_string(),
            ));
        }

        // For custom signet, ensure challenge is provided
        if self.is_custom && self.custom_challenge.is_none() {
            return Err(BitcoinError::InvalidAddress(
                "Custom signet requires a challenge script".to_string(),
            ));
        }

        Ok(())
    }
}

/// Signet-specific client wrapper
pub struct SignetClient {
    client: BitcoinClient,
    config: SignetConfig,
}

impl SignetClient {
    /// Create a new signet client with the given configuration
    pub fn new(config: SignetConfig) -> Result<Self> {
        config.validate()?;

        let client = BitcoinClient::new(
            &config.rpc_url,
            &config.rpc_user,
            &config.rpc_password,
            BitcoinNetwork::Signet,
        )?;

        tracing::info!(is_custom = config.is_custom, "Signet client initialized");

        Ok(Self { client, config })
    }

    /// Get the underlying Bitcoin client
    pub fn client(&self) -> &BitcoinClient {
        &self.client
    }

    /// Get the signet configuration
    pub fn config(&self) -> &SignetConfig {
        &self.config
    }

    /// Check if this is a custom signet
    pub fn is_custom_signet(&self) -> bool {
        self.config.is_custom
    }

    /// Get the custom challenge script if this is a custom signet
    pub fn custom_challenge(&self) -> Option<&str> {
        self.config.custom_challenge.as_deref()
    }

    /// Verify that the connected node is running on signet
    pub async fn verify_network(&self) -> Result<bool> {
        let network_info = self.client.get_network_info()?;

        // Check if the network name contains "signet"
        let is_signet = network_info.subversion.to_lowercase().contains("signet");

        if !is_signet {
            tracing::warn!("Node is not running on signet network");
        }

        Ok(is_signet)
    }
}

/// Signet faucet information for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignetFaucet {
    /// Faucet URL
    pub url: String,
    /// Description of the faucet
    pub description: String,
}

impl Default for SignetFaucet {
    fn default() -> Self {
        Self {
            url: "https://signetfaucet.com".to_string(),
            description: "Default signet faucet for obtaining test coins".to_string(),
        }
    }
}

/// Well-known signet faucets
pub struct SignetFaucets;

impl SignetFaucets {
    /// Get a list of well-known signet faucets
    pub fn known_faucets() -> Vec<SignetFaucet> {
        vec![
            SignetFaucet {
                url: "https://signetfaucet.com".to_string(),
                description: "Main signet faucet".to_string(),
            },
            SignetFaucet {
                url: "https://alt.signetfaucet.com".to_string(),
                description: "Alternative signet faucet".to_string(),
            },
        ]
    }
}

/// Signet challenge script parser
pub struct ChallengeParser;

impl ChallengeParser {
    /// Parse a challenge script from hex string
    pub fn parse_hex(hex: &str) -> Result<Vec<u8>> {
        hex::decode(hex).map_err(|e| {
            BitcoinError::InvalidAddress(format!("Invalid challenge script hex: {}", e))
        })
    }

    /// Validate a challenge script
    pub fn validate_challenge(script: &[u8]) -> Result<()> {
        if script.is_empty() {
            return Err(BitcoinError::InvalidAddress(
                "Challenge script cannot be empty".to_string(),
            ));
        }

        // Basic validation - script should be reasonable length
        if script.len() > 10000 {
            return Err(BitcoinError::InvalidAddress(
                "Challenge script too large".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signet_config_default() {
        let config = SignetConfig::default();
        assert!(!config.is_custom);
        assert_eq!(config.rpc_url, "http://127.0.0.1:38332");
        assert!(config.custom_challenge.is_none());
        assert!(config.network_magic.is_none());
    }

    #[test]
    fn test_signet_config_default_signet() {
        let config = SignetConfig::default_signet(
            "http://localhost:38332".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );
        assert!(!config.is_custom);
        assert_eq!(config.rpc_url, "http://localhost:38332");
    }

    #[test]
    fn test_signet_config_custom_signet() {
        let challenge = "5121".to_string(); // Valid hex
        let config = SignetConfig::custom_signet(
            "http://localhost:38332".to_string(),
            "user".to_string(),
            "pass".to_string(),
            challenge.clone(),
            Some(0x0a03cf40),
        )
        .unwrap();

        assert!(config.is_custom);
        assert_eq!(config.custom_challenge, Some(challenge));
        assert_eq!(config.network_magic, Some(0x0a03cf40));
    }

    #[test]
    fn test_signet_config_invalid_challenge() {
        let result = SignetConfig::custom_signet(
            "http://localhost:38332".to_string(),
            "user".to_string(),
            "pass".to_string(),
            "invalid_hex!".to_string(), // Invalid hex
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_signet_config_validate() {
        let mut config = SignetConfig::default();
        assert!(config.validate().is_ok());

        config.rpc_url = "".to_string();
        assert!(config.validate().is_err());

        config.rpc_url = "http://localhost:38332".to_string();
        config.is_custom = true;
        config.custom_challenge = None;
        assert!(config.validate().is_err());

        config.custom_challenge = Some("5121".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_challenge_parser_parse_hex() {
        let hex = "5121";
        let result = ChallengeParser::parse_hex(hex).unwrap();
        assert_eq!(result, vec![0x51, 0x21]);
    }

    #[test]
    fn test_challenge_parser_invalid_hex() {
        let hex = "invalid";
        let result = ChallengeParser::parse_hex(hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_challenge_parser_validate() {
        let script = vec![0x51, 0x21];
        assert!(ChallengeParser::validate_challenge(&script).is_ok());

        let empty_script: Vec<u8> = vec![];
        assert!(ChallengeParser::validate_challenge(&empty_script).is_err());

        let large_script = vec![0u8; 10001];
        assert!(ChallengeParser::validate_challenge(&large_script).is_err());
    }

    #[test]
    fn test_signet_faucets() {
        let faucets = SignetFaucets::known_faucets();
        assert!(!faucets.is_empty());
        assert!(faucets.iter().any(|f| f.url.contains("signetfaucet")));
    }
}
