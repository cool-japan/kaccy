//! Batch withdrawal processor example
//!
//! This example demonstrates how to process multiple user withdrawals efficiently
//! by batching them into fewer transactions to save on fees.

use kaccy_bitcoin::{
    BatchOptimizer, BatchStrategy, BatchWithdrawal, BitcoinClient, BitcoinMetrics, BitcoinNetwork,
    CoinControl, ConsolidationConfig, InMemoryMetricsBackend, MetricsTimer, SelectionStrategy,
    UtxoManager, WithdrawalRequest,
};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Batch Withdrawal Processor Example");
    info!("==================================");

    // Configuration
    let bitcoin_rpc_url = "http://localhost:8332";
    let bitcoin_rpc_user = "user";
    let bitcoin_rpc_pass = "pass";

    // Initialize Bitcoin client
    info!("\n1. Connecting to Bitcoin Core...");
    let bitcoin_client = Arc::new(
        BitcoinClient::new(
            bitcoin_rpc_url,
            bitcoin_rpc_user,
            bitcoin_rpc_pass,
            BitcoinNetwork::Regtest,
        )
        .expect("Failed to create Bitcoin client"),
    );

    // Initialize metrics
    let metrics_backend = Arc::new(InMemoryMetricsBackend::new());
    let metrics = Arc::new(BitcoinMetrics::new(metrics_backend));

    // Initialize UTXO manager
    info!("\n2. Initializing UTXO manager...");
    let consolidation_config = ConsolidationConfig {
        min_utxo_count: 100,
        max_fee_rate: 10.0,
        target_utxo_count: 50,
        min_utxo_value_sats: 10_000,
        consolidation_address: None,
    };
    let utxo_manager = UtxoManager::new(bitcoin_client.clone(), consolidation_config);

    // Initialize coin control for advanced UTXO selection
    let coin_control = Arc::new(tokio::sync::RwLock::new(CoinControl::new()));

    // Example: Simulate pending withdrawal requests
    info!("\n3. Collecting pending withdrawal requests...");
    let withdrawal_requests = [
        create_mock_withdrawal(
            "user1",
            "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
            100_000,
        ),
        create_mock_withdrawal(
            "user2",
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            50_000,
        ),
        create_mock_withdrawal(
            "user3",
            "bc1q9xwjvr8yx8kxwq3qtz5dzt9x9qw8zt6xqy6x5z",
            75_000,
        ),
        create_mock_withdrawal(
            "user4",
            "bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el",
            200_000,
        ),
        create_mock_withdrawal(
            "user5",
            "bc1qwqdg6squsna38e46795at95yu9atm8azzmyvckulcc7kytlcckxswvvzej",
            30_000,
        ),
        create_mock_withdrawal(
            "user6",
            "bc1q2vt8wde46jl6z7aqz07z5h9wkl2v8dxe8xm4jk",
            150_000,
        ),
    ];

    info!(
        "  Collected {} withdrawal requests",
        withdrawal_requests.len()
    );
    info!(
        "  Total amount: {} sats",
        withdrawal_requests
            .iter()
            .map(|w| w.amount_sats)
            .sum::<u64>()
    );

    // Convert to BatchWithdrawal format
    let batch_withdrawals: Vec<BatchWithdrawal> = withdrawal_requests
        .iter()
        .enumerate()
        .map(|(i, w)| BatchWithdrawal {
            user_id: format!("user{}", i + 1),
            address: w.destination_address.clone(),
            amount_sats: w.amount_sats,
            priority: if w.amount_sats > 100_000 { 10 } else { 5 },
        })
        .collect();

    // Example: Optimize batch using different strategies
    info!("\n4. Optimizing batch with different strategies...");

    // Strategy 1: Minimize number of transactions
    info!("\n  Strategy 1: Minimize Transactions");
    let batch_optimizer = BatchOptimizer::new(
        25,  // max_outputs_per_tx
        2,   // min_batch_size
        5.0, // fee_rate
    );
    let optimized_min_tx = batch_optimizer
        .optimize(
            batch_withdrawals.clone(),
            BatchStrategy::MinimizeTransactions,
        )
        .expect("Failed to optimize batch");

    info!("    Transactions: {}", optimized_min_tx.transaction_count);
    info!(
        "    Total fees: {} sats",
        optimized_min_tx.estimated_total_fees
    );
    info!(
        "    Fee savings: {} sats",
        optimized_min_tx.estimated_savings
    );

    // Strategy 2: Minimize fees
    info!("\n  Strategy 2: Minimize Fees");
    let optimized_min_fees = batch_optimizer
        .optimize(batch_withdrawals.clone(), BatchStrategy::MinimizeFees)
        .expect("Failed to optimize batch");

    info!("    Transactions: {}", optimized_min_fees.transaction_count);
    info!(
        "    Total fees: {} sats",
        optimized_min_fees.estimated_total_fees
    );
    info!(
        "    Fee savings: {} sats vs individual txs",
        optimized_min_fees.estimated_savings
    );

    // Strategy 3: Balanced approach
    info!("\n  Strategy 3: Balanced");
    let optimized_balanced = batch_optimizer
        .optimize(batch_withdrawals.clone(), BatchStrategy::Balanced)
        .expect("Failed to optimize batch");

    info!("    Transactions: {}", optimized_balanced.transaction_count);
    info!(
        "    Total fees: {} sats",
        optimized_balanced.estimated_total_fees
    );

    // Example: Get available UTXOs for withdrawal
    info!("\n5. Checking available UTXOs...");
    match utxo_manager.list_utxos() {
        Ok(utxos) => {
            info!("  Available UTXOs: {}", utxos.len());
            let total_available: u64 = utxos.iter().map(|u| u.amount_sats).sum();
            info!("  Total available: {} sats", total_available);

            // Apply coin control filters
            let coin_control_guard = coin_control.read().await;
            let filtered_utxos = coin_control_guard.filter_utxos(utxos.clone());
            info!("  After coin control: {} UTXOs", filtered_utxos.len());

            // Select UTXOs for withdrawal
            if !filtered_utxos.is_empty() {
                info!("\n6. Selecting UTXOs for batch withdrawal...");
                let timer = MetricsTimer::start();

                let total_needed: u64 =
                    batch_withdrawals.iter().map(|w| w.amount_sats).sum::<u64>() + 50_000; // Add buffer for fees

                match utxo_manager.select_utxos(total_needed, 5.0, SelectionStrategy::LargestFirst)
                {
                    Ok(selection) => {
                        let duration = timer.stop();

                        info!("  Selected {} UTXOs", selection.selected.len());
                        info!("  Total amount: {} sats", selection.total_input_sats);
                        info!("  Estimated fee: {} sats", selection.estimated_fee_sats);
                        info!("  Change: {} sats", selection.change_sats);

                        // Record metrics
                        metrics
                            .record_utxo_selection(
                                "largest_first",
                                selection.selected.len(),
                                selection.total_input_sats,
                                duration,
                            )
                            .await;
                    }
                    Err(e) => {
                        warn!("  Failed to select UTXOs: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            warn!("  Failed to list UTXOs: {}", e);
            warn!("  Note: This example requires a running Bitcoin Core node");
        }
    }

    // Example: Create batch withdrawal PSBTs
    info!("\n7. Creating batch withdrawal transactions...");
    info!("  (In production, these would be signed and broadcast)");

    for (i, batch) in optimized_balanced.batches.iter().enumerate() {
        info!("\n  Batch {}:", i + 1);
        info!("    Withdrawals: {}", batch.len());
        info!(
            "    Total amount: {} sats",
            batch.iter().map(|w| w.amount_sats).sum::<u64>()
        );
        info!("    Recipients:");
        for withdrawal in batch {
            info!(
                "      - {} -> {} sats to {}",
                withdrawal.user_id,
                withdrawal.amount_sats,
                &withdrawal.address[..20]
            );
        }
    }

    // Get metrics summary
    info!("\n8. Metrics Summary:");
    let stats = metrics.get_stats().await;
    info!("  Total UTXO selections: {}", stats.total_utxo_selections);

    info!("\nBatch Withdrawal Processor Example Complete!");
    info!("\nKey Benefits of Batch Processing:");
    info!(
        "  1. Significant fee savings ({:.2}% saved)",
        (optimized_balanced.estimated_savings as f64
            / (optimized_balanced.estimated_total_fees + optimized_balanced.estimated_savings)
                as f64)
            * 100.0
    );
    info!(
        "  2. Fewer transactions on-chain ({} vs {} individual)",
        optimized_balanced.transaction_count,
        batch_withdrawals.len()
    );
    info!("  3. Better UTXO management");
    info!("  4. Improved privacy (multiple payments in one tx)");

    info!("\nProduction Considerations:");
    info!("  1. Implement proper signing workflow (hardware wallet/HSM)");
    info!("  2. Add retry logic for failed broadcasts");
    info!("  3. Monitor transaction confirmations");
    info!("  4. Handle edge cases (RBF, fee bumping)");
    info!("  5. Implement withdrawal queue management");
    info!("  6. Add proper error handling and rollback");
    info!("  7. Set up monitoring and alerting");

    Ok(())
}

/// Helper function to create mock withdrawal requests
fn create_mock_withdrawal(_user_id: &str, address: &str, amount_sats: u64) -> WithdrawalRequest {
    WithdrawalRequest {
        user_id: Uuid::new_v4(), // In production, use actual user UUID
        destination_address: address.to_string(),
        amount_sats,
        fee_rate: Some(5), // 5 sat/vB
    }
}
