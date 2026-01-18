//! Multi-signature treasury example
//!
//! This example demonstrates how to:
//! - Create a 2-of-3 multisig wallet
//! - Generate multisig addresses
//! - Manage custody levels based on balance
//! - Coordinate signature collection
//!
//! Run with: cargo run --example multisig_treasury

use kaccy_bitcoin::{CustodyManager, MultisigConfig, MultisigWallet};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("=== Multi-Signature Treasury Example ===\n");

    // Example public keys (DO NOT use in production)
    let pubkeys = vec![
        "02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9".to_string(), // Platform key
        "03fff97bd5755eeea420453a14355235d382f6472f8568a18b2f057a1460297556".to_string(), // User key
        "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798".to_string(), // Cold storage
    ];

    let key_labels = vec![
        "Platform".to_string(),
        "User".to_string(),
        "ColdStorage".to_string(),
    ];

    // Create 2-of-3 multisig configuration
    // In production, these would be real public keys from:
    // - Platform hot wallet
    // - User's hardware wallet
    // - Cold storage vault
    let config = MultisigConfig {
        required_signatures: 2,
        total_keys: 3,
        xpubs: pubkeys.clone(),
        key_labels: key_labels.clone(),
        derivation_path: "m/48'/0'/0'/2'".to_string(),
        network: bitcoin::Network::Bitcoin,
    };

    println!("Multisig Configuration:");
    println!(
        "  Type: {}-of-{}",
        config.required_signatures, config.total_keys
    );
    println!("  Network: {:?}", config.network);
    println!();

    println!("Participants:");
    println!("  1. Platform (hot wallet)    : {}...", &pubkeys[0][..20]);
    println!("  2. User (hardware wallet)   : {}...", &pubkeys[1][..20]);
    println!("  3. Cold Storage (vault)     : {}...", &pubkeys[2][..20]);
    println!();

    // Create multisig wallet
    let mut multisig = MultisigWallet::new(config.clone())?;
    println!("✓ Multisig wallet created");

    // Generate multisig address
    let address = multisig.get_address(0)?;
    println!("  Deposit address: {}", address.address);
    println!();

    // Custody management based on balance
    println!("=== Custody Level Management ===\n");
    let _custody_mgr = CustodyManager::new()
        .auto_sweep_threshold(10_000_000) // 0.1 BTC
        .min_hot_balance(1_000_000); // 0.01 BTC

    let test_balances = vec![
        (50_000, "Small withdrawal"),          // 0.0005 BTC
        (500_000, "Medium withdrawal"),        // 0.005 BTC
        (5_000_000, "Large withdrawal"),       // 0.05 BTC
        (50_000_000, "Very large withdrawal"), // 0.5 BTC
    ];

    for (amount, description) in test_balances {
        println!(
            "{} ({} sats / {:.8} BTC):",
            description,
            amount,
            amount as f64 / 100_000_000.0
        );

        println!("  Required signatures:");

        if amount < 1_000_000 {
            println!("    - Platform + User (2 signatures)");
            println!("    ✓ Standard processing");
        } else if amount < 10_000_000 {
            println!("    - Platform + User (2 signatures)");
            println!("    ⚠️  User approval required");
        } else {
            println!("    - All 3 keys (Platform + User + Cold Storage)");
            println!("    🔒 Maximum security");
            println!("    ⏰ Auto-sweep threshold exceeded");
        }
        println!();
    }

    println!("=== Signature Workflow ===\n");
    println!("For a 2-of-3 multisig transaction:");
    println!();
    println!("1. Create unsigned transaction (PSBT)");
    println!("   └─> PsbtBuilder generates transaction");
    println!();
    println!("2. Platform signs first (automatic)");
    println!("   └─> Hot wallet signs immediately");
    println!();
    println!("3. User signs second (manual)");
    println!("   └─> Hardware wallet signs via USB/QR");
    println!("   └─> Or user approves via app");
    println!();
    println!("4. Optional: Cold storage signs (for high-value)");
    println!("   └─> Air-gapped signing ceremony");
    println!("   └─> Requires physical access to vault");
    println!();
    println!("5. Broadcast transaction");
    println!("   └─> Once threshold (2) signatures collected");
    println!("   └─> Transaction is complete and valid");
    println!();

    println!("=== Security Features ===\n");
    println!("✓ Distributed key management");
    println!("  - No single point of failure");
    println!("  - Keys stored in separate locations");
    println!();
    println!("✓ Tiered security");
    println!("  - Small amounts: fast, 1 signature");
    println!("  - Medium amounts: user approval, 2 signatures");
    println!("  - Large amounts: cold storage, 3 signatures");
    println!();
    println!("✓ Hardware wallet support");
    println!("  - PSBT format compatible with Ledger, Trezor");
    println!("  - Air-gapped signing for cold storage");
    println!();
    println!("✓ Time delays for large withdrawals");
    println!("  - Optional waiting period");
    println!("  - Cancel suspicious transactions");
    println!();

    println!("=== Best Practices ===\n");
    println!("1. Store keys in different locations");
    println!("2. Use hardware wallets for user and cold storage keys");
    println!("3. Regularly test recovery procedures");
    println!("4. Document signature authority");
    println!("5. Implement time delays for large withdrawals");
    println!("6. Monitor all multisig transactions");
    println!("7. Keep backup of redeem scripts and descriptors");

    Ok(())
}
