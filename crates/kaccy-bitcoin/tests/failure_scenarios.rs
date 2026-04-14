//! Failure scenario tests: error handling, edge cases, resilience.
//!
//! Verifies that the new modules produce appropriate error types when given
//! invalid input, and that defaults are sane.

use kaccy_bitcoin::{
    AdvancedRpcConfig, Bech32mCodec, BitcoinError, BlockchainExplorer, MockBlockchainExplorer,
    RotatingExplorerClient, RotatingExplorerConfig, SchnorrError, SchnorrKeyPair,
};

#[tokio::test]
async fn test_mock_explorer_missing_tx_error() {
    let explorer = MockBlockchainExplorer::new();

    let result = explorer.get_transaction("nonexistent_txid").await;

    assert!(result.is_err(), "should error on missing transaction");

    match result.unwrap_err() {
        BitcoinError::NotFound(msg) => {
            assert!(
                msg.contains("nonexistent_txid"),
                "error message should include the txid"
            );
        }
        other => panic!("expected NotFound, got {:?}", other),
    }
}

#[test]
fn test_rotating_client_no_endpoints() {
    let config = RotatingExplorerConfig {
        endpoints: vec![],
        timeout_secs: 5,
        max_retries: 1,
        rotate_on_error: false,
    };

    let client = RotatingExplorerClient::new(config);

    // Should handle empty endpoint list gracefully
    assert!(
        client.current_endpoint().is_none(),
        "no endpoints => current_endpoint() must return None"
    );

    // Rotate on empty list should not panic
    client.rotate();
    assert!(client.current_endpoint().is_none());
}

#[test]
fn test_schnorr_invalid_key_error() {
    // Too short
    let bad_key = &[0u8; 16];
    let result = SchnorrKeyPair::from_secret_bytes(bad_key);
    assert!(result.is_err(), "short key should fail");

    // All zeros (not a valid private key in secp256k1)
    let zero_key = [0u8; 32];
    let result2 = SchnorrKeyPair::from_secret_bytes(&zero_key);
    assert!(
        result2.is_err(),
        "all-zero key must fail (not a valid secp256k1 secret key)"
    );

    // Verify the error variant
    match result.unwrap_err() {
        SchnorrError::InvalidSecretKey(_) => {} // expected
        other => panic!("expected InvalidSecretKey, got {:?}", other),
    }
}

#[test]
fn test_bech32m_invalid_address() {
    // Garbage string
    let result = Bech32mCodec::decode("this_is_not_an_address");
    assert!(result.is_err());

    // Valid-looking prefix but invalid checksum
    let result2 = Bech32mCodec::decode("bc1pinvalidchecksum0000000000000000000000000000000");
    assert!(result2.is_err());

    // Legacy address should fail segwit decode
    let result3 = Bech32mCodec::decode("1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2");
    assert!(result3.is_err());
}

#[test]
fn test_rpc_advanced_config_default() {
    let config = AdvancedRpcConfig::default();

    assert!(!config.rpc_url.is_empty(), "default URL must not be empty");
    assert!(
        !config.rpc_user.is_empty(),
        "default user must not be empty"
    );
    assert!(
        !config.rpc_pass.is_empty(),
        "default pass must not be empty"
    );
    assert!(config.timeout_secs > 0, "default timeout must be positive");

    // URL should be a localhost HTTP endpoint by default
    assert!(
        config.rpc_url.starts_with("http://"),
        "default URL should be http://"
    );
}
