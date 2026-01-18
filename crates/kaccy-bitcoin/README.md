# kaccy-bitcoin

Comprehensive Bitcoin integration for Kaccy Protocol.

## Overview

This crate provides a complete Bitcoin integration solution including Bitcoin Core RPC client, HD wallet management, transaction monitoring, Lightning Network support, Layer 2 integrations, privacy features, and advanced Bitcoin functionalities.

## Core Modules

### `client`

Bitcoin Core RPC client with automatic reconnection:

- **`BitcoinClient`** - Connection to Bitcoin Core node with exponential backoff retry
- Network info and blockchain queries (`get_network_info`, `get_block_height`)
- Transaction broadcasting and mempool operations
- Fee estimation (`estimate_smart_fee`)
- Address validation and monitoring
- Health checking and node status

### `hd_wallet`

HD wallet and address generation (BIP32/BIP84):

- **`HdWallet`** - BIP84 native SegWit hierarchical deterministic wallet
- Unique deposit address generation per order
- Derivation path: `m/84'/0'/0'/0/{order_index}`
- External and internal chain support
- Address caching to avoid regeneration
- XPUB validation and address book management

### `monitor`

Payment monitoring and confirmation tracking:

- **`PaymentMonitor`** - Watches for incoming transactions via polling
- **`ConfirmationTracker`** - Multi-level confirmation tracking (1, 3, 6 confirmations)
- **`TransactionMatcher`** - Match incoming transactions to orders by address
- Payment status tracking (Pending, Seen, Confirming, Confirmed, Failed)
- Underpayment/overpayment detection and notifications
- RBF (Replace-By-Fee) transaction handling

### `lightning`

Lightning Network integration:

- **`LndClient`** - REST client for LND integration
- **`LightningProvider`** - Trait abstraction for multiple implementations
- **`LightningPaymentManager`** - Invoice creation and payment monitoring
- Channel management (open, close, list, balance monitoring)
- Invoice generation with customizable expiry
- Real-time payment status updates

## Advanced Features

### Transaction Management

#### `psbt`

PSBT (Partially Signed Bitcoin Transactions):

- **`PsbtBuilder`** - Generate unsigned transactions
- **`PsbtManager`** - Withdrawal and payout management
- Hardware wallet support via external signing
- PSBT combining and finalization
- Batch withdrawal support
- Dynamic fee estimation (fast/medium/slow)

#### `multisig`

Multi-signature wallet support:

- **`MultisigWallet`** - M-of-N multi-signature configurations
- **`CustodyManager`** - Tiered custody based on balance thresholds
- P2WSH native SegWit multisig addresses
- PSBT-based transaction signing
- **`SignatureCoordinator`** - Collect signatures from multiple parties
- Platform + User + Cold storage arrangements

#### `taproot`

Taproot integration (BIP 341/342):

- **`TaprootManager`** - Key pair generation and management
- Key-path spending for enhanced privacy
- Script-path spending with Taproot script tree
- P2TR address creation and validation
- Enhanced efficiency over legacy formats

#### `descriptor`

Output descriptor wallet support:

- **`DescriptorWallet`** - Modern Bitcoin Core descriptor integration
- Multiple descriptor types (PKH, WPKH, SH-WPKH, TR, Multi, SortedMulti)
- Descriptor import and range derivation
- Checksum validation
- WPKH, Taproot, and Multisig descriptor creation

### Privacy Features

#### `payjoin`

PayJoin support (BIP 78):

- **`PayJoinCoordinator`** - Sender and receiver coordination
- PSBT enhancement with receiver inputs
- PayJoin URI builder (BIP 21 extension)
- Proposal validation and response handling
- Enhanced transaction privacy

#### `coinjoin`

CoinJoin integration:

- **`CoinJoinCoordinator`** - Multi-participant session management
- Equal denomination outputs for maximum privacy
- Input/output collection and shuffling
- Signature coordination across participants
- Client participation interface

### Layer 2 Integrations

#### `stacks`

Stacks (STX) integration:

- **`StacksClient`** - Mainnet/testnet support
- Smart contract deployment (Clarity contracts)
- **`ContractDeploymentManager`** - Contract lifecycle management
- **`StacksTokenBridge`** - BTC <-> STX token bridging
- Bridge transaction monitoring
- Address validation and network checks

#### `rsk`

RSK integration:

- **`RskClient`** - Mainnet/testnet/regtest support
- **`PegManager`** - BTC-pegged token (rBTC) management
- Peg-in operations (BTC -> rBTC)
- Peg-out operations (rBTC -> BTC)
- Confirmation tracking for peg operations
- Ethereum-compatible address handling
- Smart contract deployment support

### Advanced Bitcoin Features

#### `timelock`

Time-locked transactions and HTLCs:

- **`HtlcManager`** - Hash Time-Locked Contracts
- Multiple timelock types (BlockHeight, Timestamp, RelativeBlocks, RelativeTime)
- HTLC script builder with IF/ELSE paths
- Payment hash generation and verification
- HTLC status tracking (Pending, Active, Claimed, Refunded, Expired)
- OP_CLTV and OP_CSV support

#### `atomic_swap`

Atomic swap protocol:

- **`AtomicSwapManager`** - Trustless cross-chain exchange
- Initiator and Participant role support
- Swap status tracking (Initiated, Locked, Completed, Refunded, Cancelled)
- Preimage generation and management
- Configurable timelock periods
- Full swap lifecycle management

### Monitoring & Security

#### `mempool`

Mempool and reorganization monitoring:

- **`MempoolMonitor`** - Track unconfirmed transactions
- **`AddressWatcher`** - Detect incoming unconfirmed payments
- **`ReorgTracker`** - Chain reorganization detection
- Immediate pending status notification
- Mempool statistics and analysis

#### `activity_monitor`

Suspicious activity detection:

- **`ActivityMonitor`** - Large transaction detection
- Rapid transaction succession monitoring
- Velocity limits (amount per hour)
- Risk scoring system (0-100)
- Alert broadcasting for suspicious patterns

#### `tx_limits`

Transaction limit enforcement:

- **`LimitEnforcer`** - Per-user and platform-wide limits
- Multiple limit periods (hourly, daily, weekly, monthly)
- Single transaction maximum
- Usage tracking and enforcement
- Limit violation notifications

#### `notifications`

Admin notification system:

- **`AdminNotificationService`** - Broadcast channel for alerts
- **`NotificationPriority`** - Levels (Low/Medium/High/Critical)
- Underpayment/overpayment notifications
- RBF replacement alerts
- Large transaction alerts

### Scaling & Optimization

#### `batch_optimizer`

Batched withdrawal optimization:

- **`BatchOptimizer`** - Aggregate multiple user withdrawals
- Multiple strategies (MinimizeTransactions, MinimizeFees, Balanced, Priority)
- Efficiency analysis and fee savings calculation
- Reduced on-chain fees through batching

#### `utxo`

UTXO management and optimization:

- **`UtxoManager`** - UTXO listing and filtering
- Consolidation during low-fee periods
- Optimal input selection strategies (LargestFirst, SmallestFirst, BranchAndBound, FirstFit)
- Transaction fee estimation
- Consolidation recommendations

### Utilities

#### `tx_parser`

Transaction parsing and analysis:

- **`TransactionParser`** - Parse incoming transactions
- **`ParsedTransaction`** - Full transaction details
- Sender address extraction for refunds
- Confidence levels for sender identification
- Transaction issue analysis

#### `rbf`

Replace-By-Fee tracking:

- **`RbfTracker`** - Monitor transaction replacements
- RBF event notifications
- Transaction conflict detection
- Status tracking for replaced transactions

#### `transaction_manager`

Integrated transaction management:

- **`TransactionManager`** - Combines limits, UTXO, and activity monitoring
- Transaction statistics and analytics
- User and platform usage tracking
- Consolidation recommendations
- Alert subscriptions

### `error`

Comprehensive Bitcoin-specific error types with detailed error handling.

## Usage Examples

### Basic Payment Monitoring

```rust
use kaccy_bitcoin::{BitcoinClient, HdWallet, PaymentMonitor, MonitorConfig};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to Bitcoin Core with automatic reconnection
    let client = Arc::new(BitcoinClient::new(
        "http://localhost:8332",
        "rpcuser",
        "rpcpassword",
    )?);

    // Initialize HD wallet (BIP84 native SegWit)
    let wallet = HdWallet::new(&std::env::var("WALLET_XPUB")?)?;

    // Generate unique address for order
    let order_id = uuid::Uuid::new_v4();
    let address = wallet.derive_address(order_id)?;
    println!("Deposit to: {}", address);

    // Start payment monitoring with confirmation tracking
    let config = MonitorConfig::default();
    let monitor = PaymentMonitor::new(client, config);
    monitor.start().await?;

    Ok(())
}
```

### Lightning Network Invoice

```rust
use kaccy_bitcoin::{LndClient, InvoiceRequest, LightningPaymentManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lnd = LndClient::new("https://localhost:8080", "macaroon_hex").await?;

    // Create invoice for order
    let invoice_req = InvoiceRequest {
        amount_sats: 50_000,
        description: "Order #12345".to_string(),
        expiry_secs: Some(3600),
    };

    let invoice = lnd.create_invoice(invoice_req).await?;
    println!("Pay invoice: {}", invoice.payment_request);

    // Monitor payment status
    let payment_mgr = LightningPaymentManager::new(lnd);
    let status = payment_mgr.check_payment(&invoice.payment_hash).await?;

    Ok(())
}
```

### Multi-Signature Wallet

```rust
use kaccy_bitcoin::{MultisigWallet, MultisigConfig, CustodyManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create 2-of-3 multisig configuration
    let config = MultisigConfig {
        required_signatures: 2,
        total_keys: 3,
        network: bitcoin::Network::Bitcoin,
    };

    let pubkeys = vec![
        "platform_pubkey".to_string(),
        "user_pubkey".to_string(),
        "cold_storage_pubkey".to_string(),
    ];

    let multisig = MultisigWallet::new(config, pubkeys)?;
    let address = multisig.get_address()?;

    // Tiered custody management
    let custody_mgr = CustodyManager::new(config);
    let level = custody_mgr.determine_custody_level(1_000_000)?;

    Ok(())
}
```

### PSBT Withdrawal

```rust
use kaccy_bitcoin::{PsbtManager, WithdrawalRequest, FeeEstimation};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let psbt_mgr = PsbtManager::new(client, wallet);

    // Create withdrawal transaction
    let request = WithdrawalRequest {
        user_id: uuid::Uuid::new_v4(),
        destination: "bc1q...".to_string(),
        amount: 100_000,
        fee_rate: FeeEstimation::Medium,
    };

    let psbt = psbt_mgr.create_withdrawal(request).await?;

    // Sign with hardware wallet and broadcast
    let signed = psbt_mgr.finalize_and_extract(psbt).await?;

    Ok(())
}
```

### Taproot Address

```rust
use kaccy_bitcoin::{TaprootManager, TaprootConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = TaprootConfig::default();
    let taproot_mgr = TaprootManager::new(config);

    // Generate Taproot key pair
    let keypair = taproot_mgr.generate_keypair()?;

    // Create key-path address (most efficient and private)
    let address = taproot_mgr.create_key_path_address(&keypair)?;
    println!("Taproot address: {}", address);

    Ok(())
}
```

### Atomic Swap

```rust
use kaccy_bitcoin::{AtomicSwapManager, AtomicSwapConfig, SwapRole};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AtomicSwapConfig::default();
    let swap_mgr = AtomicSwapManager::new(config);

    // Initiate atomic swap
    let swap = swap_mgr.initiate_swap(
        "counterparty_pubkey",
        100_000, // amount in sats
        "refund_address",
    ).await?;

    println!("Swap ID: {}", swap.swap_id);
    println!("Payment hash: {}", hex::encode(&swap.payment_hash));

    Ok(())
}
```

### Batch Withdrawals

```rust
use kaccy_bitcoin::{BatchOptimizer, BatchStrategy, BatchWithdrawal};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let optimizer = BatchOptimizer::new();

    let withdrawals = vec![
        BatchWithdrawal {
            user_id: uuid::Uuid::new_v4(),
            address: "bc1q...".to_string(),
            amount: 50_000,
            priority: false,
        },
        // ... more withdrawals
    ];

    // Optimize batch for minimum fees
    let batch = optimizer.optimize(withdrawals, BatchStrategy::MinimizeFees)?;
    println!("Fee savings: {} sats", batch.efficiency.fee_savings);

    Ok(())
}
```

## Configuration

### Environment Variables

Core configuration:

- `BITCOIN_RPC_URL` - Bitcoin Core RPC endpoint (default: `http://localhost:8332`)
- `BITCOIN_RPC_USER` - RPC username
- `BITCOIN_RPC_PASSWORD` - RPC password
- `BITCOIN_NETWORK` - Network type (`mainnet`, `testnet`, `regtest`, `signet`)
- `WALLET_XPUB` - Extended public key for address derivation (BIP84)

Lightning Network:

- `LND_REST_URL` - LND REST API endpoint
- `LND_MACAROON` - LND macaroon hex string
- `LND_TLS_CERT_PATH` - Path to LND TLS certificate

Layer 2:

- `STACKS_API_URL` - Stacks API endpoint
- `RSK_RPC_URL` - RSK node RPC endpoint

### Configuration Structs

Most modules provide configuration structs with sensible defaults:

```rust
// Monitor configuration
let config = MonitorConfig {
    poll_interval_secs: 30,
    min_confirmations: 6,
    ..Default::default()
};

// Reconnection configuration
let reconnect_config = ReconnectConfig {
    max_retries: 5,
    initial_delay_ms: 1000,
    max_delay_ms: 60000,
};

// HTLC configuration
let htlc_config = HtlcConfig {
    default_timelock_blocks: 144,
    min_timelock_blocks: 6,
    max_timelock_blocks: 2016,
};
```

## Architecture

```
kaccy-bitcoin/
├── src/
│   ├── lib.rs                    # Crate root with public exports
│   │
│   ├── Core Modules
│   ├── client.rs                 # Bitcoin Core RPC client
│   ├── hd_wallet.rs              # BIP84 HD wallet
│   ├── wallet.rs                 # Wallet manager
│   ├── monitor.rs                # Payment monitoring
│   ├── confirmation.rs           # Confirmation tracking
│   ├── matcher.rs                # Transaction matching
│   ├── connection_pool.rs        # RPC connection pooling
│   ├── cache.rs                  # Transaction/UTXO/block caching
│   │
│   ├── Transaction Management
│   ├── psbt.rs                   # PSBT building and signing
│   ├── multisig.rs               # Multi-signature wallets
│   ├── taproot.rs                # Taproot addresses
│   ├── descriptor.rs             # Output descriptors
│   ├── tx_parser.rs              # Transaction parsing
│   ├── rbf.rs                    # Replace-By-Fee tracking
│   ├── advanced_rbf.rs           # Advanced RBF with batching
│   ├── fee_bumping.rs            # CPFP and RBF fee bumping
│   │
│   ├── Advanced Features
│   ├── lightning.rs              # Lightning Network
│   ├── timelock.rs               # HTLCs and timelocks
│   ├── atomic_swap.rs            # Atomic swaps
│   ├── submarine_swaps.rs        # Lightning-onchain swaps
│   ├── payjoin.rs                # PayJoin privacy
│   ├── coinjoin.rs               # CoinJoin privacy
│   ├── dlc.rs                    # Discreet Log Contracts
│   ├── miniscript_support.rs    # Miniscript policies
│   │
│   ├── Wallet Features
│   ├── coin_control.rs           # Manual UTXO selection
│   ├── gap_limit.rs              # BIP44 gap limit tracking
│   ├── transaction_history.rs    # Transaction history & export
│   ├── hwi.rs                    # Hardware wallet integration
│   ├── key_management.rs         # Key rotation & recovery
│   │
│   ├── Layer 2
│   ├── stacks.rs                 # Stacks integration
│   ├── rsk.rs                    # RSK integration
│   │
│   ├── Network Support
│   ├── p2p.rs                    # P2P protocol integration
│   ├── compact_filters.rs        # BIP 157/158 filters
│   ├── signet.rs                 # Signet support
│   ├── testnet4.rs               # Testnet4 support
│   │
│   ├── Monitoring & Security
│   ├── mempool.rs                # Mempool monitoring
│   ├── activity_monitor.rs       # Suspicious activity
│   ├── tx_limits.rs              # Transaction limits
│   ├── notifications.rs          # Admin notifications
│   ├── health.rs                 # Health check system
│   ├── audit.rs                  # Audit logging
│   │
│   ├── Optimization
│   ├── utxo.rs                   # UTXO management
│   ├── batch_optimizer.rs        # Batch withdrawals
│   ├── transaction_manager.rs    # Integrated management
│   │
│   ├── Observability
│   ├── metrics.rs                # Prometheus metrics
│   ├── structured_logging.rs     # Request/response logging
│   │
│   ├── Testing
│   ├── testing.rs                # Test helpers & utilities
│   │
│   └── error.rs                  # Error types
│
└── Cargo.toml
```

## Payment Flows

### On-Chain Payment Flow

```
1. User initiates purchase
   └─> Generate unique BTC address (HD wallet BIP84 derivation)
   └─> Display address to user
   └─> Cache address for future lookups

2. User sends BTC to address
   └─> MempoolMonitor detects unconfirmed transaction
   └─> Mark order as "pending" immediately
   └─> TransactionMatcher validates amount
   └─> Detect underpayment/overpayment
   └─> Send admin notification if discrepancy

3. Wait for confirmations (ConfirmationTracker)
   └─> 1 confirmation: Mark order as "confirming"
   └─> 3 confirmations: Update confidence level
   └─> 6 confirmations: Mark order as "confirmed"
   └─> Broadcast confirmation events

4. Execute trade
   └─> Update token supply
   └─> Credit user balance
   └─> Create trade record
   └─> Store transaction ID
```

### Lightning Network Payment Flow

```
1. User requests Lightning payment
   └─> Generate invoice with amount and expiry
   └─> Return payment request to user

2. User pays invoice
   └─> LightningPaymentManager monitors invoice status
   └─> Detect payment immediately upon receipt
   └─> No confirmations needed (instant finality)

3. Execute trade
   └─> Credit user balance instantly
   └─> Create trade record
   └─> Store payment hash
```

### Multi-Signature Withdrawal Flow

```
1. User requests withdrawal
   └─> CustodyManager determines required custody level
   └─> Check withdrawal limits (LimitEnforcer)
   └─> ActivityMonitor checks for suspicious patterns

2. Create PSBT
   └─> PsbtBuilder creates unsigned transaction
   └─> Select optimal UTXOs (UtxoManager)
   └─> Estimate fees based on target confirmation

3. Collect signatures
   └─> SignatureCoordinator manages signature collection
   └─> Platform signs first
   └─> User/hardware wallet signs
   └─> Cold storage signs if required (high-value)

4. Broadcast transaction
   └─> Finalize PSBT
   └─> Extract signed transaction
   └─> Broadcast to Bitcoin network
   └─> Monitor confirmation status
```

## Dependencies

Core Bitcoin libraries:

- `bitcoincore-rpc` - Bitcoin Core RPC client
- `bitcoin` - Bitcoin primitives and transaction handling
- `tokio` - Async runtime
- `serde` / `serde_json` - Serialization

Additional dependencies:

- `reqwest` - HTTP client for Lightning/L2 APIs
- `uuid` - Unique identifiers
- `chrono` - Timestamp handling
- `thiserror` - Error type derivation
- `tracing` - Structured logging
- `async-trait` - Async trait support
- `base64` / `hex` - Encoding utilities
- `rand` - Random number generation

## Security Considerations

### General Security

- **Never store private keys** - Use watch-only wallet (xpub only)
- **Hardware wallet support** - PSBT enables external signing
- **Multi-signature** - Distributed key management for high-value funds
- **Cold storage** - Automatic sweep to cold wallet above threshold
- **Network isolation** - Keep Bitcoin Core behind firewall
- **TLS encryption** - Use TLS for RPC connections in production

### Transaction Security

- **Transaction limits** - Per-user and platform-wide limits enforced
- **Activity monitoring** - Detect suspicious patterns and high-risk transactions
- **Confirmation tracking** - Wait for sufficient confirmations (6 for high-value)
- **RBF protection** - Track transaction replacements and detect conflicts
- **Mempool monitoring** - Detect reorg attacks and double-spends early

### Privacy Features

- **Address reuse prevention** - Unique address per order via HD derivation
- **PayJoin support** - Break common input ownership heuristic
- **CoinJoin support** - Enhanced privacy for withdrawals
- **Taproot** - Improved privacy and efficiency

### Operational Security

- **Automatic reconnection** - Exponential backoff prevents denial of service
- **Health checks** - Regular node status monitoring
- **Admin notifications** - Alert on anomalies (underpayment, large transactions, RBF)
- **Audit trail** - All transactions logged with full details
- **Rate limiting** - Prevent abuse via velocity limits

## Testing

### Unit Tests

```bash
# Run all tests
cargo test -p kaccy-bitcoin

# Run specific module tests
cargo test -p kaccy-bitcoin --test taproot
cargo test -p kaccy-bitcoin --test atomic_swap

# Run with output
cargo test -p kaccy-bitcoin -- --nocapture
```

### Integration Tests

Requires running Bitcoin Core in regtest mode:

```bash
# Start Bitcoin Core regtest
bitcoind -regtest -daemon \
  -rpcuser=rpcuser \
  -rpcpassword=rpcpassword \
  -fallbackfee=0.00001

# Create wallet
bitcoin-cli -regtest -rpcuser=rpcuser -rpcpassword=rpcpassword createwallet "test"

# Run tests
BITCOIN_NETWORK=regtest \
BITCOIN_RPC_URL=http://localhost:18443 \
BITCOIN_RPC_USER=rpcuser \
BITCOIN_RPC_PASSWORD=rpcpassword \
cargo test -p kaccy-bitcoin
```

### Coverage

Current test coverage: **254 tests** across all modules

- Core modules: Client, HD Wallet, Monitor, Connection Pool, Cache
- Transaction management: PSBT, Multisig, Taproot, Descriptors, Fee Bumping, Advanced RBF
- Advanced features: Lightning, Timelock, Atomic Swap, Submarine Swaps, DLC, Miniscript
- Wallet features: Coin Control, Gap Limit, Transaction History, Hardware Wallets, Key Management
- Privacy: PayJoin, CoinJoin
- L2: Stacks, RSK
- Network: P2P, Compact Filters, Signet, Testnet4
- Monitoring: Mempool, Activity, Limits, Health Checks, Audit Logging
- Optimization: UTXO, Batch optimizer, Transaction Manager
- Observability: Metrics, Structured Logging
- Testing utilities: Fuzz helpers, Property tests, Integration helpers, Load tests

## Performance Considerations

- **Address caching** - Avoid repeated derivation
- **UTXO management** - Consolidate during low-fee periods
- **Batch withdrawals** - Reduce transaction count and fees
- **Connection pooling** - Reuse RPC connections
- **Efficient polling** - Configurable poll intervals (default 30s)

## License

Licensed under the same terms as the Kaccy Protocol project.
