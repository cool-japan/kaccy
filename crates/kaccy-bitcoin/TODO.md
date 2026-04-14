# kaccy-bitcoin TODO

## Phase 0 (MVP)

### Bitcoin Client

- [x] RPC connection setup
- [x] Basic connection methods
- [x] Network info query (`get_network_info`)
- [x] Block height query (`get_block_height`, `get_block_hash`)
- [x] Transaction query by txid (`get_transaction`, `get_raw_transaction`)
- [x] Address balance query (`get_received_by_address`, `get_address_info`)
- [x] Connection health check (`health_check`)
- [x] Node status (`get_node_status`)
- [x] Mempool info (`get_mempool_info`)
- [x] Fee estimation (`estimate_smart_fee`)
- [x] Address validation (`validate_address`)
- [x] List since block (`list_since_block`)
- [x] Automatic reconnection (ReconnectConfig, with_retry, exponential backoff)

### HD Wallet

- [x] Basic wallet structure
- [x] Address generation via RPC (`get_new_address`)
- [x] BIP84 (native SegWit) address derivation (hd_wallet.rs)
  - [x] HdWallet struct with Xpub parsing
  - [x] Derivation path: `m/84'/0'/0'/0/{order_index}` (derivation module)
  - [x] External chain (receiving) and internal chain (change) support
- [x] Address caching to avoid regeneration (CachedAddress, address_cache)
- [x] XPUB validation (validate_xpub, parse_xpub)
- [x] Address validation (is_wallet_address)
- [x] Address book for labels and order mapping (AddressBook)

### Payment Monitor

- [x] Basic monitor structure
- [x] Payment status enum (5 variants)
- [x] Check payment status with confirmation tracking
- [x] `list_since_block` for polling transactions
- [x] Poll Bitcoin Core for new transactions
  - [x] Polling interval: 30 seconds (MonitorConfig.poll_interval_secs)
  - [x] Background task runner (PaymentMonitorTask)
- [x] Match incoming transactions to orders (TransactionMatcher)
  - [x] By address (match_transactions, check_order_payment)
  - [x] Validate amount matches expected (MatchStatus: Exact, Underpaid, Overpaid)
- [x] Confirmation tracking (ConfirmationTracker, ConfirmationLevel)
  - [x] Update order status at 1, 3, 6 confirmations (ConfirmationLevel enum)
  - [x] Notify on full confirmation (ConfirmationEvent, NotificationType)
- [x] Handle underpayment/overpayment
  - [x] Log warning for discrepancies (PaymentEvent::Underpaid, PaymentEvent::Overpaid)
  - [x] Admin notification (notifications.rs)
    - [x] AdminNotificationService with broadcast channel
    - [x] NotificationPriority levels (Low/Medium/High/Critical)
    - [x] Underpayment/overpayment specific notifications
    - [x] RBF replacement notifications
    - [x] Large transaction alerts

### Transaction Handling

- [x] Get raw transaction
- [x] Send raw transaction
- [x] Test mempool accept
- [x] Parse incoming transactions (tx_parser.rs)
  - [x] ParsedTransaction with full transaction details
  - [x] Transaction analysis for issues
- [x] Extract sender address (for refunds)
  - [x] SenderInfo with confidence levels
  - [x] get_refund_address method
- [x] Store txid in order record (via OrderRepository::update_btc_txid in kaccy-db)
- [x] Handle transaction replacement (RBF) (rbf.rs)
  - [x] RbfTracker for monitoring replacements
  - [x] RbfEvent notifications
  - [x] Transaction conflict detection

### Error Handling

- [x] Basic error types
- [x] RPC connection errors
- [x] Invalid address errors
- [x] Insufficient confirmation errors (InsufficientConfirmations)
- [x] Network mismatch errors (NetworkMismatch)

## Phase 1 (Post-MVP)

### Lightning Network

- [x] LND/CLN integration (lightning.rs)
  - [x] REST client for LND (LndClient)
  - [x] LightningProvider trait abstraction
- [x] Invoice generation (InvoiceRequest, create_invoice)
  - [x] Amount in satoshis
  - [x] Expiry time (configurable, default 1 hour)
  - [x] Description with order ID
- [x] Payment monitoring (LightningPaymentManager)
  - [x] Subscribe to invoice updates (lookup_invoice)
  - [x] Handle expired invoices (is_expired check)
- [x] Channel management (list_channels, open_channel, close_channel)
  - [x] Monitor inbound capacity (ChannelBalance)
  - [x] Channel opening/closing support

### Transaction Signing

- [x] PSBT (Partially Signed Bitcoin Transactions) (psbt.rs)
  - [x] Generate unsigned transactions (PsbtBuilder)
  - [x] Support external signing (hardware wallet)
  - [x] Combine multiple PSBTs (combine_psbts)
  - [x] Finalize and extract signed transactions (finalize_and_extract)
- [x] Withdrawal transactions (PsbtManager)
  - [x] User BTC withdrawals (create_withdrawal)
  - [x] Issuer revenue payouts (create_payout)
  - [x] Batch withdrawals (create_batch_withdrawal)
- [x] Fee estimation (FeeEstimation)
  - [x] Target confirmation time (fast/medium/slow)
  - [x] Dynamic fee adjustment (from Bitcoin Core rates)

### Multi-Sig Support

- [x] 2-of-3 multi-signature (multisig.rs)
  - [x] MultisigConfig for M-of-N configurations
  - [x] Platform + User + Cold storage (MultisigWallet)
  - [x] P2WSH native SegWit multisig addresses
  - [x] PSBT-based transaction signing (MultisigTransaction)
  - [x] SignatureCoordinator for collecting signatures
- [x] Custody arrangement for large balances (CustodyManager)
  - [x] Tiered custody based on balance thresholds
  - [x] Automatic custody level determination

### Advanced Monitoring

- [x] WebSocket notifications (via kaccy-api WsBroadcaster)
  - [x] Real-time payment updates
- [x] Mempool monitoring (mempool.rs)
  - [x] MempoolMonitor for tracking transactions
  - [x] AddressWatcher for unconfirmed payments
  - [x] Detect incoming unconfirmed transactions
  - [x] Show pending status immediately
- [x] Block reorganization handling (mempool.rs)
  - [x] ReorgTracker for detecting chain reorgs
  - [x] MempoolEvent::Reorganization for notifications
  - [x] Affected transaction tracking

## Phase 2 (Future)

### L2 Integration

- [x] Stacks (STX) integration (stacks.rs)
  - [x] Stacks client with mainnet/testnet support
  - [x] Smart contract deployment (Clarity contracts)
  - [x] Contract deployment manager
  - [x] Token bridging (BTC <-> STX)
  - [x] Bridge transaction monitoring
  - [x] Address validation and network checks
- [x] RSK integration (rsk.rs)
  - [x] RSK client with mainnet/testnet/regtest support
  - [x] BTC-pegged token (rBTC) management
  - [x] Peg-in operations (BTC -> rBTC)
  - [x] Peg-out operations (rBTC -> BTC)
  - [x] Peg manager with confirmation tracking
  - [x] Ethereum-compatible address handling
  - [x] Smart contract deployment support

### Privacy Features

- [x] PayJoin support (payjoin.rs)
  - [x] PayJoin coordinator (BIP 78)
  - [x] Sender and receiver roles
  - [x] PSBT enhancement with receiver inputs
  - [x] PayJoin URI builder (BIP 21 extension)
  - [x] Proposal validation and response handling
- [x] CoinJoin integration (coinjoin.rs)
  - [x] CoinJoin session coordinator
  - [x] Multi-participant session management
  - [x] Equal denomination outputs for privacy
  - [x] Input/output collection and shuffling
  - [x] Signature coordination
  - [x] Client participation interface

### Scaling

- [x] Batched withdrawals
  - [x] Aggregate multiple user withdrawals (psbt.rs: create_batch_withdrawal)
  - [x] Reduce on-chain fees
  - [x] Batch optimizer (batch_optimizer.rs: BatchOptimizer)
  - [x] Multiple optimization strategies (MinimizeTransactions, MinimizeFees, Balanced, Priority)
  - [x] Efficiency analysis and fee savings calculation
- [x] UTXO management (utxo.rs)
  - [x] Consolidation during low-fee periods (ConsolidationConfig, get_consolidation_plan)
  - [x] Optimal input selection (SelectionStrategy: LargestFirst, SmallestFirst, BranchAndBound, FirstFit)
  - [x] UTXO listing and filtering
  - [x] Transaction fee estimation

### Security Enhancements

- [x] Transaction limits (tx_limits.rs)
  - [x] Per-user daily/monthly limits (LimitEnforcer with hourly/daily/weekly/monthly periods)
  - [x] Platform-wide limits
  - [x] Single transaction maximum
  - [x] Usage tracking and enforcement
- [x] Suspicious activity detection (activity_monitor.rs)
  - [x] Large transactions
  - [x] Rapid succession of transactions
  - [x] Velocity limits (amount per hour)
  - [x] Risk scoring system (0-100)
  - [x] Alert broadcasting
- [x] Cold storage automation (multisig.rs: CustodyManager)
  - [x] Auto-sweep to cold wallet above threshold (sweep_amount method)

### Integration & Utilities

- [x] Transaction Manager (transaction_manager.rs)
  - [x] Integrated transaction validation (combines limits, UTXO, activity monitoring)
  - [x] Transaction statistics and analytics
  - [x] User and platform usage tracking
  - [x] Consolidation recommendations
  - [x] Alert subscriptions

## Phase 3 (Advanced Features)

### Taproot Support

- [x] Taproot integration (taproot.rs)
  - [x] BIP 341/342 implementation
  - [x] TaprootManager for key pair generation
  - [x] Key-path spending support
  - [x] Script-path spending support with Taproot script tree
  - [x] P2TR address creation and validation
  - [x] Enhanced privacy and efficiency over legacy formats

### Time-Locked Transactions

- [x] HTLC support (timelock.rs)
  - [x] Hash Time-Locked Contracts (HTLC)
  - [x] Multiple timelock types (BlockHeight, Timestamp, RelativeBlocks, RelativeTime)
  - [x] HTLC script builder with IF/ELSE paths
  - [x] P2WSH address creation for HTLCs
  - [x] Payment hash generation and verification
  - [x] HTLC status tracking (Pending, Active, Claimed, Refunded, Expired)
  - [x] Support for both OP_CLTV (absolute) and OP_CSV (relative) timelocks

### Atomic Swaps

- [x] Atomic swap protocol (atomic_swap.rs)
  - [x] Trustless cross-chain exchange support
  - [x] Initiator and Participant roles
  - [x] Swap status tracking (Initiated, Locked, Completed, Refunded, Cancelled)
  - [x] Preimage generation and management
  - [x] Configurable timelock periods for safety
  - [x] Full swap lifecycle management
  - [x] Active swap tracking and listing

### Descriptor Wallets

- [x] Output descriptor support (descriptor.rs)
  - [x] Modern Bitcoin Core descriptor integration
  - [x] Multiple descriptor types (PKH, WPKH, SH-WPKH, TR, Multi, SortedMulti)
  - [x] Descriptor import and management
  - [x] WPKH descriptor creation
  - [x] Taproot descriptor creation
  - [x] Multisig descriptor creation
  - [x] Descriptor range support for derivation
  - [x] Checksum validation

## Phase 4 (Production Enhancements)

### Performance & Optimization

- [x] Benchmarking suite (criterion)
  - [x] Address derivation benchmarks
  - [x] Transaction parsing benchmarks
  - [x] UTXO selection benchmarks
  - [x] Batch optimization benchmarks
- [x] Connection pooling
  - [x] RPC connection pool for high concurrency
  - [x] Connection health monitoring
  - [x] Automatic failover
- [x] Caching improvements
  - [x] Transaction cache with TTL
  - [x] UTXO cache invalidation
  - [x] Block header cache

### Observability & Monitoring

- [x] Metrics collection
  - [x] Prometheus-compatible metrics interface
  - [x] Transaction volume metrics
  - [x] Fee estimation accuracy metrics
  - [x] RPC call latency metrics
  - [x] Confirmation time tracking
  - [x] In-memory and NoOp backends
- [x] Structured logging enhancements
  - [x] Request/response logging
  - [x] Correlation IDs for tracing
  - [x] Performance tracing spans
- [x] Health check enhancements
  - [x] Detailed component health status
  - [x] Dependency health (Bitcoin Core, LND)
  - [x] Resource utilization monitoring
  - [x] Background health checking

### Advanced Wallet Features

- [x] Gap limit tracking (BIP44)
  - [x] Automatic gap limit enforcement
  - [x] Address discovery on wallet import
  - [x] Gap limit configuration
- [x] Coin control
  - [x] Manual UTXO selection
  - [x] UTXO labeling and tagging
  - [x] Privacy-preserving coin selection
  - [x] Dust threshold management
- [x] Transaction history
  - [x] Paginated transaction listing
  - [x] Transaction filtering (by date, amount, type)
  - [x] Transaction export (CSV, JSON)
  - [x] Address reuse detection

### Network & Protocol Enhancements

- [x] Signet support
  - [x] Signet network configuration
  - [x] Custom signet setup
  - [x] Challenge script parsing
  - [x] Signet faucet information
- [x] Testnet4 support
  - [x] Testnet4 network configuration
  - [x] Testnet4 client wrapper
  - [x] Faucet information
  - [x] Network parameters (placeholder for when Testnet4 launches)
- [x] Compact block filters (BIP 157/158)
  - [x] Filter download and verification
  - [x] Privacy-preserving transaction detection
- [x] Peer-to-peer integration
  - [x] Direct P2P communication
  - [x] Transaction broadcasting via P2P
  - [x] Block downloading
  - [x] Peer discovery and management
  - [x] Network message handling

### Security Enhancements

- [x] Hardware wallet integration
  - [x] Ledger support
  - [x] Trezor support
  - [x] Generic HWI integration
  - [x] Air-gapped signing workflows
  - [x] PSBT signing with hardware wallets
  - [x] QR code support for air-gapped devices
- [x] Advanced key management
  - [x] Key rotation policies
  - [x] Time-delayed recovery
  - [x] Social recovery schemes
- [x] Audit logging
  - [x] Immutable audit trail
  - [x] Compliance reporting
  - [x] In-memory and file-based storage
  - [x] Query and filtering capabilities
  - [x] Event statistics

### Testing & Quality

- [x] Integration test suite
  - [x] Integration test helpers and utilities
  - [x] Wait-for-condition helpers
  - [x] Mock RPC responses
  - [x] Test configuration management
- [x] Fuzz testing
  - [x] Transaction parser fuzzing
  - [x] Address parsing fuzzing
  - [x] Random byte generation
  - [x] Fuzz test runners
- [x] Property-based testing
  - [x] Transaction invariant checking
  - [x] Amount validation properties
  - [x] Dust detection properties
  - [x] Address network consistency
- [x] Load testing
  - [x] Load test runner with configurable concurrency
  - [x] Performance statistics collection
  - [x] Latency tracking
  - [x] Success/failure rate monitoring

### Documentation & Examples

- [x] Example programs
  - [x] Basic payment processor example (examples/payment_processor.rs)
  - [x] Lightning payment gateway example (examples/lightning_gateway.rs)
  - [x] Multi-sig treasury example (examples/multisig_treasury.rs)
  - [x] Atomic swap example (examples/atomic_swap.rs)
  - [x] Batch withdrawal processor example (examples/batch_withdrawal_processor.rs)
  - [x] Production deployment example (examples/production_deployment.rs)
- [x] Architecture documentation
  - [x] Sequence diagrams for key flows
  - [x] Component interaction diagrams
  - [x] State machine diagrams
- [x] Deployment guide
  - [x] Production deployment checklist
  - [x] Security hardening guide
  - [x] Performance tuning guide
  - [x] Monitoring and alerting setup

### Additional Features

- [x] Fee bumping strategies
  - [x] Child-pays-for-parent (CPFP)
  - [x] RBF with optimal fee calculation
  - [x] Automated fee bumping policies
- [x] Advanced RBF
  - [x] Batch RBF (bump multiple transactions)
  - [x] RBF with payment consolidation
- [x] Submarine swaps
  - [x] On-chain to Lightning swaps
  - [x] Lightning to on-chain swaps
  - [x] HTLC-based trustless swaps
  - [x] Swap status tracking
  - [x] Automatic expiration handling
  - [x] Fee calculation and validation
- [x] DLC (Discreet Log Contracts) support
  - [x] Oracle integration
  - [x] Contract creation and execution
  - [x] Multi-oracle support
  - [x] Oracle announcements and attestations
  - [x] Contract lifecycle management
  - [x] CET (Contract Execution Transaction) building
  - [x] Refund transaction support
  - [x] Adaptor signature framework (placeholder)
- [x] Miniscript support
  - [x] Miniscript policy compilation
  - [x] Miniscript analysis and optimization
  - [x] Policy templates (single-key, multi-sig, timelock, etc.)
  - [x] Policy builder with fluent API
  - [x] Script type support (P2WSH, P2SH, etc.)

## Phase 5 (Modern Protocol Features)

### Silent Payments (BIP 352)

- [x] Silent payment address generation
  - [x] SP address format (sp1q...)
  - [x] Public key derivation for recipients
  - [x] Label support for address organization
- [x] Sending to silent payment addresses
  - [x] ECDH shared secret computation
  - [x] Output public key generation
  - [x] Transaction scanning optimization (framework)
- [x] Receiving silent payments
  - [x] Transaction scanning for owned outputs (framework)
  - [x] Label recovery and organization
  - [x] Efficient light client support (framework)
- [x] Silent payment wallet integration
  - [x] Balance calculation
  - [x] Transaction history (framework)
  - [x] Backup and recovery (via private keys)

### MuSig2 (Schnorr Multi-Signatures)

- [x] MuSig2 implementation
  - [x] Key aggregation (BIP 327)
  - [x] Nonce generation and aggregation
  - [x] Partial signature creation (framework)
  - [x] Signature aggregation (framework)
- [x] Taproot integration (compatible with existing taproot module)
  - [x] MuSig2 key-path spending support
  - [x] Better privacy than script-path multisig
- [x] Adaptor signatures
  - [x] Atomic swap support
  - [x] Payment channel applications

### BIP 324 (v2 P2P Transport)

- [x] Encrypted P2P connections
  - [x] ECDH key exchange
  - [x] ChaCha20-Poly1305 encryption (framework)
  - [x] Opportunistic encryption
- [x] Version negotiation
  - [x] Backward compatibility with v1 (fallback support)
  - [x] Feature detection
- [x] Traffic analysis resistance
  - [x] Packet length hiding (padding)
  - [x] Timing obfuscation (through encryption)

### FROST (Threshold Signatures)

- [x] FROST protocol implementation
  - [x] Distributed key generation (DKG framework)
  - [x] Signing with threshold participants
  - [x] Dynamic signing groups (via participant selection)
- [x] Advanced custody solutions
  - [x] T-of-N threshold wallets
  - [x] Better than traditional multisig (single signature on-chain)
  - [x] No blockchain footprint difference

### Advanced Fee Management

- [x] Mempool.space API integration
  - [x] Real-time fee recommendations
  - [x] Better fee estimation accuracy
  - [x] Historical fee data analysis
- [x] Package relay support
  - [x] Transaction package construction
  - [x] CPFP carve-out usage
  - [x] Anchor outputs optimization
- [x] Fee market analysis
  - [x] Mempool monitoring
  - [x] Optimal fee timing
  - [x] Fee spike detection

### Enhanced Privacy

- [x] Coin selection privacy
  - [x] Anti-address-clustering algorithms
  - [x] Amount fingerprinting resistance
  - [x] Optimal change output strategies
- [x] Network-level privacy
  - [x] Tor integration improvements
  - [x] Dandelion++ transaction broadcasting
  - [x] Connection graph optimization
- [x] UTXO management privacy
  - [x] Consolidation privacy analysis
  - [x] Toxic change detection
  - [x] Privacy score calculation

## Phase 6 (Additional Modern Features)

### BIP 47 (Reusable Payment Codes)

- [x] Payment code generation and parsing
  - [x] PaymentCode struct with version support (V1, V2, V3)
  - [x] Base58 encoding/decoding for payment codes
  - [x] Public key and chain code management
  - [x] Network-specific payment code formats
- [x] Payment channel management
  - [x] PaymentChannel for tracking sender/receiver pairs
  - [x] Notification transaction framework
  - [x] Address derivation (receive and send)
  - [x] ECDH shared secret computation
- [x] PaymentCodeManager for operations
  - [x] Payment code generation from xpub
  - [x] Address derivation interface
  - [x] Notification transaction creation (framework)

### BIP 329 (Wallet Label Export)

- [x] Label management
  - [x] LabelRecord with multiple types (tx, addr, pubkey, input, output, xpub)
  - [x] Spendable flag for address privacy
  - [x] Origin field for label attribution
  - [x] Reference field for linking related labels
- [x] LabelManager operations
  - [x] Add labels for transactions, addresses, pubkeys, inputs, outputs, xpubs
  - [x] Get labels by type and reference
  - [x] Remove labels
  - [x] Label filtering by type
- [x] Import/Export functionality
  - [x] JSON export (BIP 329 compliant)
  - [x] JSON import (BIP 329 compliant)
  - [x] Merge labels from import
  - [x] Sorted export for consistency

### BIP 21 (URI Scheme)

- [x] Bitcoin payment URI support
  - [x] BitcoinUri struct with parsing and generation
  - [x] Support for amount, label, message parameters
  - [x] Extra parameters for extensions (req-*, Lightning fallback)
  - [x] URL encoding/decoding for special characters
- [x] BitcoinUriBuilder
  - [x] Fluent API for building URIs
  - [x] Amount in satoshis or BTC
  - [x] Lightning invoice fallback support
  - [x] Custom parameter support
- [x] QR code helpers
  - [x] QR version estimation
  - [x] QR-friendly URI validation
  - [x] Recommended error correction level

### Address Utilities

- [x] Address type detection and analysis
  - [x] AddressType enum (P2PKH, P2SH, P2WPKH, P2WSH, P2TR)
  - [x] Network detection from address string
  - [x] Script type analysis
  - [x] SegWit/Legacy/Taproot classification
- [x] AddressInfo metadata
  - [x] Address type and network information
  - [x] Estimated input size for spending
  - [x] Script public key extraction
  - [x] Multisig detection (P2SH/P2WSH)
  - [x] RBF support indicator
- [x] Privacy and efficiency analysis
  - [x] Privacy score calculation (0-100)
  - [x] Spending fee cost estimation
  - [x] Address comparison utilities
  - [x] Privacy level comparison
- [x] Batch address analysis
  - [x] AddressBatchAnalyzer for multiple addresses
  - [x] Statistics aggregation (type counts, privacy scores)
  - [x] Upgradeable address detection (legacy to SegWit/Taproot)
  - [x] Most private type identification

## Phase 7 (Advanced Bitcoin Protocol & Utilities)

### Script Analysis & Templates

- [x] Bitcoin script analyzer (script_analyzer.rs)
  - [x] Script disassembler (opcodes to human-readable)
  - [x] Script complexity analysis
  - [x] Witness cost estimation
  - [x] Risky opcode detection
  - [x] Script type identification
  - [x] Full analysis reports
- [x] Script template library (script_templates.rs)
  - [x] P2PKH templates
  - [x] OP_RETURN templates
  - [x] Timelock (CLTV) templates
- [x] Script optimizer (script_optimizer.rs)
  - [x] Redundant opcode elimination
  - [x] Witness size optimization
  - [x] Script size reduction strategies

### Lightning Network v2

- [x] Channel management enhancements (lightning_v2.rs)
  - [x] Dual-funded channels (Lightning spec update)
  - [x] Channel splicing support (add/remove funds)
  - [x] Zero-conf channels
  - [x] Liquidity management automation
- [x] Advanced routing
  - [x] Multi-path payments (MPP)
  - [x] Adaptive routing algorithms
  - [x] Fee optimization strategies
  - [x] Route hints and privacy
- [x] BOLT 12 Offers
  - [x] Reusable payment offers
  - [x] Offer creation and parsing
  - [x] Offer-based invoicing

### Wallet Recovery & Backup

- [x] Seed phrase utilities (seed_recovery.rs)
  - [x] BIP 39 mnemonic generation (12/15/18/21/24 words)
  - [x] Mnemonic validation and checksum
  - [x] Passphrase support (BIP 39 extension)
  - [x] English wordlist support
  - [x] Seed XOR for split backup
  - [x] Entropy generation and recovery
  - [x] Word autocomplete functionality
- [x] Wallet recovery tools (wallet_recovery.rs)
  - [x] Derive all standard address types from seed (BIP 44/49/84/86)
  - [x] Balance recovery scanner
  - [x] UTXO discovery across derivation paths
  - [x] Multi-wallet detection
- [x] SLIP 39 Shamir Secret Sharing (slip39.rs)
  - [x] M-of-N secret splitting
  - [x] Share generation and recovery
  - [x] Group threshold support

### Advanced PSBT Workflows

- [x] PSBT workflow manager (psbt_workflows.rs)
  - [x] Multi-signer coordination
  - [x] PSBT versioning (v0, v2)
  - [x] PSBT templates for common patterns
  - [x] Round-trip signing workflows
  - [x] Signature collection tracking
- [x] PSBT analysis tools
  - [x] Fee verification
  - [x] Input/output validation
  - [x] Completeness checking
  - [x] Security warnings (fee overpayment, etc.)

### Chain Analysis Resistance

- [x] Transaction graph obfuscation (chain_privacy.rs)
  - [x] Decoy outputs (fake change detection)
  - [x] Amount obfuscation strategies
  - [x] Timing analysis resistance
  - [x] Transaction structure randomization
- [x] Blockchain explorers integration
  - [x] Mock blockchain explorer for testing (mock_explorer.rs)
  - [x] Query minimization strategies
  - [x] Rotating endpoint usage (RotatingExplorerClient)
  - [x] Tor-only explorer queries (TorExplorerClient)

### Bitcoin Core Advanced Integration

- [x] Advanced RPC features (rpc_advanced.rs)
  - [x] Wallet import/export (descriptors via importdescriptors/listdescriptors)
  - [x] Block template management (getblocktemplate/submitblock)
  - [x] Transaction priority management (prioritisetransaction)
  - [x] Custom network messages
- [x] ZMQ integration (zmq.rs)
  - [x] Real-time block notifications
  - [x] Transaction notifications
  - [x] Mempool notifications
  - [x] Sequence notifications
- [x] Advanced mempool analytics (mempool_analytics.rs)
  - [x] Mempool histogram analysis
  - [x] Transaction clustering detection
  - [x] Fee spike prediction
  - [x] Replacement cycle detection

### Taproot Advanced Features

- [x] Tapscript utilities (tapscript_utils.rs)
  - [x] Complex script tree construction
  - [x] Merkle proof generation
  - [x] Script path optimization
  - [x] Huffman-coded trees for efficiency
- [ ] Signature aggregation
  - [ ] Cross-input signature aggregation
  - [x] Batch verification (SchnorrSigner::batch_verify in schnorr.rs)
  - [ ] Half-aggregation schemes

### Coin Selection Enhancements

- [x] Advanced coin selection algorithms (coin_selection_advanced.rs)
  - [x] Knapsack algorithm
  - [x] Simulated annealing
  - [x] Genetic algorithm selection (via simulated annealing)
  - [x] Quality scoring and optimization
- [x] UTXO set optimization (utxo_optimizer.rs)
  - [x] Long-term UTXO health scoring
  - [x] Proactive consolidation triggers
  - [x] Fee market prediction integration

### Fee Management Advanced

- [x] Adaptive fee strategies (adaptive_fees.rs)
  - [x] Time-based fee adjustment
  - [x] User urgency preferences
  - [x] Budget-constrained optimization
  - [x] Multi-transaction fee planning
- [x] Fee bumping automation
  - [x] Automatic RBF policies
  - [x] CPFP automation rules
  - [x] Fee budget management
  - [x] Priority-based bumping

### Privacy Metrics & Analysis

- [x] Privacy scoring system (privacy_metrics.rs)
  - [x] Transaction privacy score (0-100)
  - [x] Wallet privacy health check
  - [x] Common heuristic detection
  - [x] Privacy improvement recommendations
- [x] Address clustering analysis
  - [x] Common input ownership detection
  - [x] Change output identification
  - [x] Address reuse tracking
  - [x] Entity linking detection

### Test & Development Utilities

- [x] Regtest automation (regtest_utils.rs)
  - [x] Auto-mining on transaction
  - [x] Test wallet funding
  - [x] Block generation utilities
  - [x] Time manipulation helpers
- [x] Transaction fuzzing
  - [x] Malformed transaction testing
  - [x] Edge case generation
  - [x] Stress testing utilities
- [x] Integration test framework
  - [x] End-to-end payment flows (tests/e2e_payment_flow.rs)
  - [x] Multi-party transaction coordination (tests/multi_party_coordination.rs)
  - [x] Failure scenario testing (tests/failure_scenarios.rs)
  - [x] Property-based tests (tests/property_tests.rs)

### Standards & Compatibility

- [ ] Additional BIP implementations
  - [x] BIP 340 (Schnorr signatures - core) (schnorr.rs)
  - [x] BIP 350 (Bech32m addresses) (bech32m_utils.rs)
  - [x] BIP 370 (PSBT v2) — `psbt_v2.rs` (PsbtV2, PsbtV2Builder, TxModifiable)
  - [x] BIP 371 (Taproot PSBT fields) — `taproot_psbt.rs` (TaprootPsbt, TaprootInputFields, TaprootOutputFields)
  - [x] BIP 380-386 (Output descriptors detail)
- [x] Cross-implementation compatibility
  - [x] Electrum wallet compatibility
  - [x] Bitcoin Core descriptor import/export (rpc_advanced.rs: importdescriptors/listdescriptors)
  - [x] Hardware wallet formats
  - [x] Mobile wallet standards

## Phase 8 (Stream B+C Additions — 2026-04-13)

### New Modules

- [x] `schnorr.rs` — BIP 340 Schnorr signature wrapper
  - [x] SchnorrKeyPair: generate, from_secret_bytes, public_key
  - [x] SchnorrSigner: sign, sign_with_aux_rand, verify, batch_verify
  - [x] SchnorrUtils: message_from_bytes, normalize_signature

- [x] `bech32m_utils.rs` — BIP 350 bech32m encoding/decoding utilities
  - [x] Bech32mCodec: encode, decode, is_bech32m, is_bech32, validate_taproot_address
  - [x] TaprootAddressInfo: from_address, is_taproot
  - [x] AddressConverter: normalize, to_lowercase, is_valid_bitcoin_address

- [x] `rpc_advanced.rs` — Advanced Bitcoin Core RPC operations
  - [x] AdvancedRpcConfig and AdvancedRpcClient
  - [x] import_descriptors, list_descriptors
  - [x] get_block_template, submit_block
  - [x] prioritise_transaction, get_mempool_entry

- [x] `mock_explorer.rs` — Mock blockchain explorer + rotating/Tor client
  - [x] BlockchainExplorer trait
  - [x] MockBlockchainExplorer (DashMap-backed, for tests)
  - [x] RotatingExplorerClient (round-robin multi-endpoint)
  - [x] TorExplorerClient (SOCKS5 proxy, falls back to direct)

### Integration Tests

- [x] `tests/e2e_payment_flow.rs` — End-to-end payment flow tests
- [x] `tests/multi_party_coordination.rs` — Multi-party Schnorr/descriptor tests
- [x] `tests/failure_scenarios.rs` — Error handling and edge case tests
- [x] `tests/property_tests.rs` — Property-based tests with proptest
