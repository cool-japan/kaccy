// Testnet4 support
//
// Testnet4 is the next generation Bitcoin testnet that will replace Testnet3.
// This module provides configuration and utilities for Testnet4 when it becomes available.
//
// Key improvements over Testnet3:
// - More stable difficulty adjustment
// - Better protection against mining monopolization
// - Improved reset mechanisms

use crate::client::{BitcoinClient, BitcoinNetwork, ReconnectConfig};
use crate::error::Result;
use bitcoin::Network;

/// Testnet4 configuration
#[derive(Debug, Clone)]
pub struct Testnet4Config {
    /// RPC endpoint URL
    pub rpc_url: String,
    /// RPC username
    pub rpc_user: String,
    /// RPC password
    pub rpc_password: String,
    /// Reconnection configuration
    pub reconnect_config: ReconnectConfig,
}

impl Testnet4Config {
    /// Create a new Testnet4 configuration
    pub fn new(rpc_url: String, rpc_user: String, rpc_password: String) -> Self {
        Self {
            rpc_url,
            rpc_user,
            rpc_password,
            reconnect_config: ReconnectConfig::default(),
        }
    }

    /// Create with custom reconnection configuration
    pub fn with_reconnect_config(mut self, reconnect_config: ReconnectConfig) -> Self {
        self.reconnect_config = reconnect_config;
        self
    }
}

impl Default for Testnet4Config {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:48332".to_string(), // Default Testnet4 RPC port (proposed)
            rpc_user: "bitcoin".to_string(),
            rpc_password: "".to_string(),
            reconnect_config: ReconnectConfig::default(),
        }
    }
}

/// Testnet4 client wrapper
pub struct Testnet4Client {
    client: BitcoinClient,
}

impl Testnet4Client {
    /// Create a new Testnet4 client
    pub fn new(config: Testnet4Config) -> Result<Self> {
        let client = BitcoinClient::with_config(
            &config.rpc_url,
            &config.rpc_user,
            &config.rpc_password,
            BitcoinNetwork::Testnet4,
            config.reconnect_config,
        )?;

        Ok(Self { client })
    }

    /// Get the underlying Bitcoin client
    pub fn client(&self) -> &BitcoinClient {
        &self.client
    }

    /// Get network type
    pub fn network(&self) -> Network {
        BitcoinNetwork::Testnet4.into()
    }

    /// Check if Testnet4 is available
    pub fn is_available(&self) -> Result<bool> {
        // Try to get network info to check connectivity
        match self.client.get_network_info() {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get Testnet4 faucet information
    pub fn get_faucet_info(&self) -> FaucetInfo {
        FaucetInfo::default()
    }
}

/// Testnet4 faucet information
#[derive(Debug, Clone)]
pub struct FaucetInfo {
    /// Faucet URLs
    pub urls: Vec<String>,
    /// Maximum amount per request (in satoshis)
    pub max_amount: u64,
    /// Rate limit information
    pub rate_limit: String,
}

impl Default for FaucetInfo {
    fn default() -> Self {
        Self {
            urls: vec![
                // Placeholder URLs - will be updated when Testnet4 launches
                "https://testnet4-faucet.mempool.space".to_string(),
                "https://testnet4.coinfaucet.eu".to_string(),
            ],
            max_amount: 100_000_000, // 1 BTC
            rate_limit: "Once per 24 hours per IP".to_string(),
        }
    }
}

/// Testnet4 network parameters
#[derive(Debug, Clone)]
pub struct Testnet4Params {
    /// Network magic bytes
    pub magic: [u8; 4],
    /// Default port
    pub port: u16,
    /// Default RPC port
    pub rpc_port: u16,
    /// Genesis block hash
    pub genesis_hash: String,
    /// BIP44 coin type
    pub bip44_coin_type: u32,
}

impl Default for Testnet4Params {
    fn default() -> Self {
        Self {
            // Placeholder values - will be updated when Testnet4 launches
            magic: [0x0b, 0x11, 0x09, 0x07], // Placeholder magic bytes
            port: 48333,                     // Placeholder port
            rpc_port: 48332,                 // Placeholder RPC port
            genesis_hash: "TBD".to_string(), // To be determined
            bip44_coin_type: 1,              // Same as Testnet3
        }
    }
}

impl Testnet4Params {
    /// Get network parameters
    pub fn get() -> Self {
        Self::default()
    }

    /// Check if these are the correct parameters
    pub fn verify(&self) -> bool {
        // When Testnet4 launches, add verification logic here
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_testnet4_config_default() {
        let config = Testnet4Config::default();
        assert_eq!(config.rpc_url, "http://localhost:48332");
        assert_eq!(config.rpc_user, "bitcoin");
    }

    #[test]
    fn test_testnet4_config_new() {
        let config = Testnet4Config::new(
            "http://example.com:48332".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );
        assert_eq!(config.rpc_url, "http://example.com:48332");
        assert_eq!(config.rpc_user, "user");
        assert_eq!(config.rpc_password, "pass");
    }

    #[test]
    fn test_testnet4_params() {
        let params = Testnet4Params::get();
        assert_eq!(params.port, 48333);
        assert_eq!(params.rpc_port, 48332);
        assert_eq!(params.bip44_coin_type, 1);
        assert!(params.verify());
    }

    #[test]
    fn test_faucet_info() {
        let info = FaucetInfo::default();
        assert!(!info.urls.is_empty());
        assert!(info.max_amount > 0);
        assert!(!info.rate_limit.is_empty());
    }

    #[test]
    fn test_network_conversion() {
        let network: Network = BitcoinNetwork::Testnet4.into();
        // Currently maps to Testnet until Testnet4 is officially supported
        assert_eq!(network, Network::Testnet);
    }
}
