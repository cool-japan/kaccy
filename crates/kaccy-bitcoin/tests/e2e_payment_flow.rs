//! End-to-end payment flow integration tests.
//!
//! These tests verify complete payment workflows using mock components.
//! No real network connection is required.

use kaccy_bitcoin::{
    BlockchainExplorer, ExplorerAddress, ExplorerTransaction, ExplorerUtxo, MockBlockchainExplorer,
    RotatingExplorerClient, RotatingExplorerConfig,
};

#[tokio::test]
async fn test_mock_payment_flow() {
    let explorer = MockBlockchainExplorer::new();

    // Simulate a confirmed payment transaction
    explorer.add_transaction(ExplorerTransaction {
        txid: "payment_tx_abc123def456".to_string(),
        confirmed: true,
        block_height: Some(820_000),
        fee_satoshis: Some(1_500),
        value_satoshis: 100_000_000, // 1 BTC
        timestamp: Some(1_700_200_000),
    });

    explorer.set_block_height(820_010);

    // Retrieve and validate the transaction
    let tx = explorer
        .get_transaction("payment_tx_abc123def456")
        .await
        .expect("should find payment transaction");

    assert!(tx.confirmed, "payment must be confirmed");
    assert_eq!(tx.value_satoshis, 100_000_000);
    assert_eq!(tx.block_height, Some(820_000));

    // Verify current block height
    let height = explorer.get_block_height().await.expect("get height");
    assert_eq!(height, 820_010);

    // Confirm sufficient confirmations (820_010 - 820_000 = 10 confirmations)
    let confirmations = height - tx.block_height.unwrap();
    assert!(confirmations >= 6, "payment should have >= 6 confirmations");
}

#[tokio::test]
async fn test_payment_address_lookup() {
    let explorer = MockBlockchainExplorer::new();

    let utxo1 = ExplorerUtxo {
        txid: "funding_tx_1".to_string(),
        vout: 0,
        value_satoshis: 50_000_000,
        confirmed: true,
        block_height: Some(819_990),
    };

    let utxo2 = ExplorerUtxo {
        txid: "funding_tx_2".to_string(),
        vout: 1,
        value_satoshis: 25_000_000,
        confirmed: true,
        block_height: Some(819_995),
    };

    explorer.add_address(ExplorerAddress {
        address: "bc1qtest_payment_address".to_string(),
        balance_satoshis: 75_000_000,
        tx_count: 2,
        unspent_outputs: vec![utxo1, utxo2],
    });

    let info = explorer
        .get_address_info("bc1qtest_payment_address")
        .await
        .expect("should find address");

    assert_eq!(info.balance_satoshis, 75_000_000);
    assert_eq!(info.tx_count, 2);

    let utxos = explorer
        .get_utxos("bc1qtest_payment_address")
        .await
        .expect("should get UTXOs");

    assert_eq!(utxos.len(), 2);
    let total: u64 = utxos.iter().map(|u| u.value_satoshis).sum();
    assert_eq!(total, 75_000_000);
}

#[tokio::test]
async fn test_explorer_rotation() {
    let config = RotatingExplorerConfig {
        endpoints: vec![
            "http://explorer1.example.com/api".to_string(),
            "http://explorer2.example.com/api".to_string(),
            "http://explorer3.example.com/api".to_string(),
        ],
        timeout_secs: 5,
        max_retries: 2,
        rotate_on_error: true,
    };

    let client = RotatingExplorerClient::new(config);

    // Verify initial state
    let initial_endpoint = client.current_endpoint().expect("should have endpoint");
    assert!(initial_endpoint.contains("explorer1"));

    // Rotate once
    client.rotate();
    let second_endpoint = client.current_endpoint().expect("should have endpoint");
    assert!(second_endpoint.contains("explorer2"));

    // Rotate again
    client.rotate();
    let third_endpoint = client.current_endpoint().expect("should have endpoint");
    assert!(third_endpoint.contains("explorer3"));

    // Rotate wraps back to first
    client.rotate();
    let wrapped_endpoint = client.current_endpoint().expect("should have endpoint");
    assert!(wrapped_endpoint.contains("explorer1"));
}

#[tokio::test]
async fn test_mock_broadcast() {
    let explorer = MockBlockchainExplorer::new();

    let raw_tx_hex = "02000000000101abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890000000006a47304402207b610b5a5b88e8cd7b1ca9b47a3ccd4f8f91de03a7f5e9b847bef4e84e51e21022043e1d2dcbd57e08b96b1a47dfe07f4b2bf0b6ec5b45f1fb8e3b87d0ea46c1e5012103b2e547e2b28e0a78bc5c5e98dd0e3e2cad5d73aa1db74ea69c4e1e76e1d5b7f2ffffffff01a0860100000000001976a914d7f9a6e0d4b5c3a2f1e8b0c9d4a5f6e7c8b9a0a488ac00000000";

    let txid = explorer
        .broadcast_transaction(raw_tx_hex)
        .await
        .expect("broadcast should succeed");

    // MockBlockchainExplorer returns "mock_{hex_len}"
    assert_eq!(txid, format!("mock_{}", raw_tx_hex.len()));
    assert!(txid.starts_with("mock_"));
}
