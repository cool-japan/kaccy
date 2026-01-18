//! Atomic swap example
//!
//! This example demonstrates how to:
//! - Initiate a trustless atomic swap
//! - Track swap status
//! - Claim or refund swaps
//!
//! Run with: cargo run --example atomic_swap

use kaccy_bitcoin::{AtomicSwapConfig, AtomicSwapManager, SwapStatus};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("=== Atomic Swap Example ===\n");
    println!("Atomic swaps enable trustless cross-chain or peer-to-peer exchanges");
    println!("using Hash Time-Locked Contracts (HTLCs).\n");

    // Create swap configuration
    let config = AtomicSwapConfig {
        initiator_timelock_blocks: 48,   // 8 hours
        participant_timelock_blocks: 24, // 4 hours
        network: bitcoin::Network::Regtest,
    };

    println!("Swap Configuration:");
    println!(
        "  Initiator timelock: {} blocks (~{} hours)",
        config.initiator_timelock_blocks,
        config.initiator_timelock_blocks / 6
    );
    println!(
        "  Participant timelock: {} blocks (~{} hours)",
        config.participant_timelock_blocks,
        config.participant_timelock_blocks / 6
    );
    println!("  Network: {:?}", config.network);
    println!();

    // Create swap manager
    let swap_mgr = AtomicSwapManager::new(config);

    // Scenario 1: Initiator creates a swap
    println!("=== Scenario 1: Initiating a Swap ===\n");

    let counterparty_pubkey = "03fff97bd5755eeea420453a14355235d382f6472f8568a18b2f057a1460297556";
    let amount = bitcoin::Amount::from_sat(100_000); // 0.001 BTC
    let refund_address = "bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";

    println!("Swap parameters:");
    println!(
        "  Amount: {} sats ({:.8} BTC)",
        amount.to_sat(),
        amount.to_btc()
    );
    println!("  Counterparty: {}...", &counterparty_pubkey[..20]);
    println!("  Refund address: {}", refund_address);
    println!();

    // Convert pubkey from hex
    let counterparty_bytes = hex::decode(counterparty_pubkey.trim_start_matches("0x"))?;
    let initiator_bytes =
        hex::decode("02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")?;

    let swap = swap_mgr.initiate_swap(
        uuid::Uuid::new_v4().to_string(),
        amount,
        initiator_bytes.clone(),
        counterparty_bytes.clone(),
    )?;

    println!("✓ Swap initiated successfully!");
    println!("  Swap ID: {}", swap.swap_id);
    println!("  Payment hash: {}", hex::encode(swap.payment_hash));
    println!("  Status: {:?}", swap.status);
    println!("  Role: {:?}", swap.role);
    println!();

    println!("Next steps for initiator:");
    println!("1. Fund the HTLC address with {} sats", amount.to_sat());
    println!("2. Share payment hash with counterparty");
    println!("3. Wait for counterparty to lock their funds");
    println!("4. Claim counterparty's funds using the preimage");
    println!("5. Counterparty learns preimage and claims your funds");
    println!();

    // Scenario 2: Participant responds to swap
    println!("=== Scenario 2: Participating in a Swap ===\n");

    let received_payment_hash = swap.payment_hash;
    println!("Received swap proposal:");
    println!("  Payment hash: {}", hex::encode(received_payment_hash));
    println!();

    let participant_swap = swap_mgr.participate_swap(
        uuid::Uuid::new_v4().to_string(),
        received_payment_hash,
        amount,
        counterparty_bytes.clone(),
        initiator_bytes.clone(),
    )?;

    println!("✓ Joined swap as participant!");
    println!("  Swap ID: {}", participant_swap.swap_id);
    println!("  Status: {:?}", participant_swap.status);
    println!();

    println!("Next steps for participant:");
    println!("1. Verify initiator has funded their HTLC");
    println!("2. Fund your HTLC with {} sats", amount.to_sat());
    println!("3. Wait for initiator to claim (reveals preimage)");
    println!("4. Use revealed preimage to claim initiator's funds");
    println!();

    // Swap lifecycle states
    println!("=== Swap Lifecycle ===\n");

    let states = vec![
        (SwapStatus::Initiated, "Swap created, awaiting funding"),
        (SwapStatus::Locked, "Funds locked in HTLC, awaiting claim"),
        (
            SwapStatus::Completed,
            "Successfully claimed by counterparty",
        ),
        (SwapStatus::Refunded, "Timelock expired, funds refunded"),
        (SwapStatus::Cancelled, "Swap cancelled before funding"),
    ];

    for (status, description) in states {
        println!("{:?}", status);
        println!("  └─> {}", description);
        println!();
    }

    // Security features
    println!("=== Security Features ===\n");

    println!("✓ Trustless execution");
    println!("  - No trusted third party required");
    println!("  - Atomic: either both parties get funds or both get refunds");
    println!();

    println!("✓ Hash Time-Locked Contracts (HTLC)");
    println!("  - Hash lock: only holder of preimage can claim");
    println!("  - Time lock: automatic refund after expiry");
    println!();

    println!("✓ Asymmetric timelocks");
    println!("  - Initiator has longer timelock (48 blocks)");
    println!("  - Participant has shorter timelock (24 blocks)");
    println!("  - Prevents participant from claiming then timing out");
    println!();

    println!("✓ Refund protection");
    println!("  - If counterparty doesn't claim, get automatic refund");
    println!("  - Refund address specified upfront");
    println!();

    // Common use cases
    println!("=== Use Cases ===\n");

    println!("1. Cross-chain swaps");
    println!("   - BTC <-> LTC atomic swap");
    println!("   - BTC <-> BCH atomic swap");
    println!("   - Any chain with HTLC support");
    println!();

    println!("2. Peer-to-peer exchanges");
    println!("   - Direct BTC trading without exchange");
    println!("   - No KYC required");
    println!("   - No counterparty risk");
    println!();

    println!("3. Lightning channel opening");
    println!("   - Open channels trustlessly");
    println!("   - No need to trust channel peer");
    println!();

    println!("4. Submarine swaps");
    println!("   - On-chain to Lightning");
    println!("   - Lightning to on-chain");
    println!();

    // List active swaps
    println!("=== Active Swaps ===\n");
    let active_swaps = swap_mgr.list_swaps();
    println!("Total active swaps: {}", active_swaps.len());

    for (i, s) in active_swaps.iter().enumerate() {
        println!("\n{}. Swap {}", i + 1, s.swap_id);
        println!("   Role: {:?}", s.role);
        println!("   Status: {:?}", s.status);
        println!("   Amount: {} sats", s.amount.to_sat());
    }

    println!("\n=== End of Example ===");

    Ok(())
}
