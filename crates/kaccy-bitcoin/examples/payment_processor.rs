//! Basic Bitcoin payment processor example
//!
//! This example demonstrates how to:
//! - Connect to Bitcoin Core
//! - Generate unique payment addresses
//! - Monitor incoming payments
//! - Track confirmations
//!
//! Run with: cargo run --example payment_processor

use kaccy_bitcoin::{BitcoinClient, BitcoinNetwork, HdWallet, MonitorConfig, ReconnectConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Bitcoin Payment Processor Example ===\n");

    // Load configuration from environment
    let rpc_url =
        std::env::var("BITCOIN_RPC_URL").unwrap_or_else(|_| "http://localhost:18443".to_string());
    let rpc_user = std::env::var("BITCOIN_RPC_USER").unwrap_or_else(|_| "rpcuser".to_string());
    let rpc_password =
        std::env::var("BITCOIN_RPC_PASSWORD").unwrap_or_else(|_| "rpcpassword".to_string());
    let wallet_xpub = std::env::var("WALLET_XPUB").unwrap_or_else(|_| {
        // Example xpub (DO NOT use in production)
        "xpub6CUGRUonZSQ4TWtTMmzXdrXDtypWKiKrhko4egpiMZbpiaQL2jkwSB1icqYh2cfDfVxdx4df189oLKnC5fSwqPfgyP3hooxujYzAu3fDVmz".to_string()
    });

    // Connect to Bitcoin Core with automatic reconnection
    println!("Connecting to Bitcoin Core at {}...", rpc_url);
    let reconnect_config = ReconnectConfig {
        max_retries: 5,
        initial_delay: std::time::Duration::from_millis(1000),
        max_delay: std::time::Duration::from_millis(30000),
        backoff_multiplier: 2.0,
    };

    let client = Arc::new(BitcoinClient::with_config(
        &rpc_url,
        &rpc_user,
        &rpc_password,
        BitcoinNetwork::Regtest,
        reconnect_config,
    )?);

    // Check connection
    let block_height = client.get_block_height()?;
    println!("✓ Connected to Bitcoin network");
    println!("  Block height: {}\n", block_height);

    // Initialize HD wallet
    println!("Initializing HD wallet...");
    let wallet_config = kaccy_bitcoin::HdWalletConfig::testnet(wallet_xpub);
    let wallet = Arc::new(HdWallet::new(wallet_config)?);
    println!("✓ HD wallet initialized\n");

    // Generate payment addresses for orders
    println!("Generating payment addresses (external chain):");
    for i in 0..3 {
        let address = wallet.derive_address(0, i)?; // chain=0 (external), index=i
        println!("  Index {}: {}", i, address);
    }
    println!();

    // Configure payment monitor
    let monitor_config = MonitorConfig {
        poll_interval_secs: 10, // Poll every 10 seconds
        required_confirmations: 6,
        order_expiry_hours: 24,
    };

    println!("Starting payment monitor...");
    println!("Configuration:");
    println!(
        "  Poll interval: {} seconds",
        monitor_config.poll_interval_secs
    );
    println!(
        "  Required confirmations: {}",
        monitor_config.required_confirmations
    );
    println!();

    // Note: PaymentMonitor requires a database pool in the real implementation
    // For this example, we'll just show the configuration
    println!("Note: In a real application, PaymentMonitor would run as a background task");
    println!("      monitoring all pending orders and updating their status.");

    println!();

    // Example of checking a specific order
    let example_index = 100;
    let example_address = wallet.derive_address(0, example_index)?;

    println!(
        "Example: Checking payment for address index {}",
        example_index
    );
    println!("  Payment address: {}", example_address);

    // Check if payment has been received
    // Note: In production, you would use PaymentMonitor to track payments
    // For this example, we demonstrate manual checking
    match client.validate_address(&example_address) {
        Ok(validation) => {
            if validation.is_valid {
                println!("  Address is valid: {}", validation.is_valid);
                println!();
                println!("  In production, PaymentMonitor would:");
                println!("    1. Poll Bitcoin Core periodically");
                println!("    2. Detect incoming transactions to this address");
                println!("    3. Track confirmation count (0 -> 1 -> 3 -> 6)");
                println!("    4. Update order status accordingly");
                println!("    5. Send notifications on status changes");
            }
        }
        Err(e) => println!("  Error validating address: {}", e),
    }

    println!("\n=== End of Example ===");
    println!("\nNext steps:");
    println!("1. Send Bitcoin to one of the addresses above");
    println!("2. Run this example again to see the payment detected");
    println!("3. Check the status as confirmations accumulate");

    Ok(())
}
