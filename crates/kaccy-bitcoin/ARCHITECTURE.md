# kaccy-bitcoin Architecture

This document provides an architectural overview of the `kaccy-bitcoin` crate, including sequence diagrams for key flows, component interaction diagrams, and state machine diagrams.

## Table of Contents

1. [System Overview](#system-overview)
2. [Core Components](#core-components)
3. [Sequence Diagrams](#sequence-diagrams)
4. [Component Interaction Diagrams](#component-interaction-diagrams)
5. [State Machine Diagrams](#state-machine-diagrams)
6. [Data Flow](#data-flow)
7. [Integration Points](#integration-points)

---

## System Overview

The `kaccy-bitcoin` crate provides comprehensive Bitcoin integration for the Kaccy Protocol, including:

- **Bitcoin Core RPC Client**: Direct communication with Bitcoin Core nodes
- **HD Wallet Management**: BIP32/BIP44/BIP84 hierarchical deterministic wallets
- **Payment Monitoring**: Real-time transaction tracking and confirmation monitoring
- **Lightning Network**: LND integration for instant payments
- **Advanced Features**: Multi-signature, PSBT, Taproot, CoinJoin, Atomic Swaps, DLCs
- **Security**: Transaction limits, activity monitoring, hardware wallet integration
- **Performance**: Connection pooling, caching, batch optimization

### Architecture Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                     Application Layer                            │
│  (Kaccy Protocol API - uses kaccy-bitcoin as dependency)        │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                   kaccy-bitcoin Crate                            │
│ ┌─────────────┬─────────────┬──────────────┬──────────────┐    │
│ │  Payment    │  Wallet     │  Transaction │  Security    │    │
│ │  Monitoring │  Management │  Management  │  & Limits    │    │
│ └─────────────┴─────────────┴──────────────┴──────────────┘    │
│ ┌─────────────┬─────────────┬──────────────┬──────────────┐    │
│ │  Lightning  │  Advanced   │  Observability│  Integration│    │
│ │  Network    │  Features   │  & Metrics   │  (L2/Privacy)│    │
│ └─────────────┴─────────────┴──────────────┴──────────────┘    │
│                    Core Infrastructure                           │
│ ┌─────────────────────────────────────────────────────────┐    │
│ │  Bitcoin RPC Client + Connection Pool + Caching         │    │
│ └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│              External Services                                   │
│  ┌──────────────┬──────────────┬──────────────┬─────────────┐  │
│  │ Bitcoin Core │     LND      │  Stacks/RSK  │   Hardware  │  │
│  │     RPC      │    (REST)    │   (L2 APIs)  │   Wallets   │  │
│  └──────────────┴──────────────┴──────────────┴─────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Core Components

### 1. Bitcoin Client (`client.rs`)
- **Purpose**: Low-level Bitcoin Core RPC communication
- **Key Features**:
  - Automatic reconnection with exponential backoff
  - Network info, block queries, transaction retrieval
  - Mempool monitoring, fee estimation
- **Dependencies**: `reqwest` for HTTP RPC calls

### 2. HD Wallet (`hd_wallet.rs`)
- **Purpose**: BIP84 (native SegWit) address derivation
- **Key Features**:
  - XPUB-based address generation
  - External (receiving) and internal (change) chains
  - Address caching for performance
- **Dependencies**: `bitcoin::bip32` for derivation

### 3. Payment Monitor (`monitor.rs`)
- **Purpose**: Poll Bitcoin Core for incoming transactions
- **Key Features**:
  - Configurable polling interval (default: 30 seconds)
  - Match transactions to orders via address
  - Amount validation (exact, underpaid, overpaid)
- **Dependencies**: `BitcoinClient`, `TransactionMatcher`, `ConfirmationTracker`

### 4. Transaction Matcher (`matcher.rs`)
- **Purpose**: Match incoming transactions to expected payments
- **Key Features**:
  - Address-based matching
  - Amount comparison with tolerance
  - Match status tracking

### 5. Confirmation Tracker (`confirmation.rs`)
- **Purpose**: Track confirmation levels and emit events
- **Key Features**:
  - Configurable confirmation thresholds (1, 3, 6 confirmations)
  - Event emission for state changes
  - Statistics tracking

### 6. PSBT Manager (`psbt.rs`)
- **Purpose**: Create and manage Partially Signed Bitcoin Transactions
- **Key Features**:
  - Withdrawal transaction creation
  - Hardware wallet signing support
  - Batch withdrawals
  - Fee estimation integration

### 7. Multi-Signature (`multisig.rs`)
- **Purpose**: M-of-N multi-signature wallet management
- **Key Features**:
  - P2WSH native SegWit multisig
  - PSBT-based signing coordination
  - Tiered custody management

### 8. Lightning Network (`lightning.rs`)
- **Purpose**: LND integration for instant payments
- **Key Features**:
  - Invoice generation and monitoring
  - Channel management
  - Payment tracking

### 9. Transaction Manager (`transaction_manager.rs`)
- **Purpose**: High-level transaction orchestration
- **Key Features**:
  - Integrates limits, UTXO selection, activity monitoring
  - Transaction validation and analytics
  - Consolidation recommendations

### 10. Activity Monitor (`activity_monitor.rs`)
- **Purpose**: Detect suspicious transaction patterns
- **Key Features**:
  - Risk scoring (0-100)
  - Velocity limits
  - Alert broadcasting for anomalies

---

## Sequence Diagrams

### 1. Payment Processing Flow

```
User → API → PaymentMonitor → BitcoinClient → Bitcoin Core
                ↓                    ↓
         TransactionMatcher    ConfirmationTracker
                ↓                    ↓
         AdminNotification      EventEmitter
                ↓                    ↓
           Database              WebSocket

Detailed Flow:
1. User sends BTC to generated address
2. PaymentMonitor polls Bitcoin Core every 30s via list_since_block
3. TransactionMatcher identifies transaction by address
4. TransactionMatcher validates amount (exact/underpaid/overpaid)
5. ConfirmationTracker monitors confirmations (0 → 1 → 3 → 6+)
6. Events emitted at each confirmation level
7. Admin notified of discrepancies
8. Order status updated in database
```

#### Sequence Diagram (ASCII):

```
┌──────┐  ┌─────┐  ┌────────────┐  ┌──────────┐  ┌───────────┐
│ User │  │ API │  │   Monitor  │  │  Client  │  │ Bitcoin   │
│      │  │     │  │            │  │          │  │   Core    │
└──┬───┘  └──┬──┘  └─────┬──────┘  └────┬─────┘  └─────┬─────┘
   │         │            │              │              │
   │ Pay BTC │            │              │              │
   ├─────────>            │              │              │
   │         │            │              │              │
   │         │  Poll (30s)│              │              │
   │         │<───────────┤              │              │
   │         │            │ list_since_block()          │
   │         │            ├──────────────>              │
   │         │            │              ├──────────────>
   │         │            │              │ transactions │
   │         │            │              <──────────────┤
   │         │            │ transactions │              │
   │         │            <──────────────┤              │
   │         │            │              │              │
   │         │  Match TX  │              │              │
   │         │<───────────┤              │              │
   │         │            │              │              │
   │         │ Confirm 1  │              │              │
   │         │<───────────┤              │              │
   │         │            │              │              │
   │         │ Confirm 6  │              │              │
   │         │<───────────┤              │              │
   │         │            │              │              │
   │ Updated │            │              │              │
   <─────────┤            │              │              │
   │         │            │              │              │
```

### 2. HD Wallet Address Generation

```
API → HdWallet → DerivationPath → AddressCache → Bitcoin Address

Detailed Flow:
1. API requests new address for order
2. HdWallet derives path: m/84'/0'/0'/0/{index}
3. Check AddressCache for existing address
4. If not cached, derive from XPUB
5. Generate P2WPKH (Bech32) address
6. Store in cache with order ID
7. Return address to API
```

#### Sequence Diagram (ASCII):

```
┌─────┐  ┌──────────┐  ┌──────────┐  ┌───────┐
│ API │  │ HdWallet │  │   Cache  │  │  BTC  │
└──┬──┘  └────┬─────┘  └────┬─────┘  └───┬───┘
   │          │              │            │
   │ get_address(order_id)   │            │
   ├──────────>              │            │
   │          │ check_cache  │            │
   │          ├──────────────>            │
   │          │     None     │            │
   │          <──────────────┤            │
   │          │              │            │
   │          │ derive(m/84'/0'/0'/0/N)  │
   │          ├──────────────────────────>│
   │          │         address          │
   │          <──────────────────────────┤
   │          │              │            │
   │          │ store(addr)  │            │
   │          ├──────────────>            │
   │          │      OK      │            │
   │          <──────────────┤            │
   │  address │              │            │
   <──────────┤              │            │
   │          │              │            │
```

### 3. Withdrawal Transaction Flow (PSBT)

```
User → API → TransactionManager → PsbtManager → HardwareWallet
                ↓                      ↓
         ActivityMonitor          UtxoManager
                ↓                      ↓
          LimitEnforcer           FeeEstimation
                ↓                      ↓
            Database              BitcoinClient

Detailed Flow:
1. User requests withdrawal
2. TransactionManager validates limits
3. ActivityMonitor checks for suspicious patterns
4. UtxoManager selects optimal inputs
5. FeeEstimation calculates appropriate fee
6. PsbtManager creates unsigned PSBT
7. PSBT sent to hardware wallet for signing
8. Signed PSBT returned and finalized
9. Transaction broadcast to Bitcoin network
10. Update database with txid
```

#### Sequence Diagram (ASCII):

```
┌──────┐ ┌─────┐ ┌─────────┐ ┌──────┐ ┌─────┐ ┌──────┐
│ User │ │ API │ │ TxMgr   │ │ PSBT │ │ HWI │ │  BTC │
└──┬───┘ └──┬──┘ └────┬────┘ └───┬──┘ └──┬──┘ └───┬──┘
   │        │         │          │       │        │
   │ Withdraw         │          │       │        │
   ├────────>         │          │       │        │
   │        │ validate│          │       │        │
   │        ├─────────>          │       │        │
   │        │   OK    │          │       │        │
   │        <─────────┤          │       │        │
   │        │         │ create_psbt      │        │
   │        │         ├──────────>       │        │
   │        │         │  unsigned        │        │
   │        │         <──────────┤       │        │
   │        │         │          │ sign  │        │
   │        │         ├──────────────────>        │
   │        │         │          │ signed│        │
   │        │         <──────────────────┤        │
   │        │         │ finalize │       │        │
   │        │         ├──────────>       │        │
   │        │         │  tx      │       │        │
   │        │         <──────────┤       │        │
   │        │         │          │broadcast       │
   │        │         ├────────────────────────────>
   │        │         │          │       │  txid  │
   │        │         <────────────────────────────┤
   │  txid  │         │          │       │        │
   <────────┤         │          │       │        │
   │        │         │          │       │        │
```

### 4. Lightning Invoice Payment Flow

```
User → API → LightningPaymentManager → LndClient → LND
                      ↓
              InvoiceMonitor
                      ↓
                 EventEmitter
                      ↓
                  Database

Detailed Flow:
1. User requests Lightning invoice
2. LightningPaymentManager creates invoice via LND
3. Invoice returned to user with payment request
4. User pays invoice
5. InvoiceMonitor polls LND for status updates
6. Payment detected (settled/expired)
7. Event emitted with payment status
8. Order updated in database
```

### 5. Atomic Swap Flow

```
Initiator → SwapManager → HTLCManager → BitcoinClient
                ↓               ↓
         Participant      PreimageStore
                ↓               ↓
           SwapTracker    TimelockMonitor

Detailed Flow:
1. Initiator creates swap with payment hash
2. HTLCManager creates HTLC script with timelock
3. Initiator funds HTLC address
4. Participant responds and creates reciprocal HTLC
5. Participant funds their HTLC
6. Initiator claims participant's HTLC (reveals preimage)
7. Participant learns preimage from blockchain
8. Participant claims initiator's HTLC
9. Swap completed successfully
10. If timeout: refund paths activated
```

### 6. Multi-Signature Transaction Flow

```
Platform → MultisigManager → SignatureCoordinator → Signers (Platform/User/Cold)
                ↓                    ↓
          PsbtBuilder          BitcoinClient
                ↓                    ↓
           Database            Bitcoin Core

Detailed Flow:
1. Platform initiates multisig transaction (e.g., 2-of-3)
2. MultisigManager creates PSBT
3. SignatureCoordinator collects signatures from required parties
4. Platform signs with hot key
5. User signs with their key (or hardware wallet)
6. Once threshold met (2 signatures), finalize transaction
7. Broadcast to Bitcoin network
8. Update database with transaction details
```

---

## Component Interaction Diagrams

### 1. Payment Monitoring System

```
┌─────────────────────────────────────────────────────────────┐
│                   Payment Monitoring System                  │
│                                                              │
│  ┌──────────────┐         ┌──────────────┐                 │
│  │   Monitor    │◄────────┤   Config     │                 │
│  │   (Poller)   │         │ (30s interval)│                 │
│  └───────┬──────┘         └──────────────┘                 │
│          │                                                   │
│          ├──────────┐                                       │
│          │          │                                       │
│          ▼          ▼                                       │
│  ┌──────────┐  ┌──────────┐      ┌─────────────┐          │
│  │ Bitcoin  │  │Transaction│◄─────┤   Address   │          │
│  │  Client  │  │  Matcher  │      │    Book     │          │
│  └──────────┘  └─────┬────┘      └─────────────┘          │
│                      │                                      │
│                      ▼                                      │
│            ┌──────────────────┐                            │
│            │ Confirmation     │                            │
│            │   Tracker        │                            │
│            └────────┬─────────┘                            │
│                     │                                      │
│          ┌──────────┼──────────┐                          │
│          ▼          ▼           ▼                          │
│   ┌──────────┐ ┌────────┐ ┌─────────┐                    │
│   │  Event   │ │ Notify │ │  Stats  │                    │
│   │ Emitter  │ │ Service│ │ Tracker │                    │
│   └──────────┘ └────────┘ └─────────┘                    │
└─────────────────────────────────────────────────────────────┘
```

### 2. Transaction Management System

```
┌─────────────────────────────────────────────────────────────┐
│               Transaction Management System                  │
│                                                              │
│              ┌─────────────────┐                            │
│              │  Transaction    │                            │
│              │    Manager      │                            │
│              └────────┬────────┘                            │
│                       │                                      │
│      ┌────────────────┼────────────────┐                   │
│      │                │                │                   │
│      ▼                ▼                ▼                   │
│ ┌─────────┐    ┌──────────┐    ┌──────────┐              │
│ │  Limit  │    │ Activity │    │   UTXO   │              │
│ │Enforcer │    │ Monitor  │    │ Manager  │              │
│ └────┬────┘    └─────┬────┘    └─────┬────┘              │
│      │               │               │                    │
│      │               ▼               │                    │
│      │         ┌──────────┐          │                    │
│      │         │   Risk   │          │                    │
│      │         │  Scorer  │          │                    │
│      │         └──────────┘          │                    │
│      │                               │                    │
│      └───────────┬───────────────────┘                    │
│                  │                                         │
│                  ▼                                         │
│          ┌──────────────┐                                 │
│          │    PSBT      │                                 │
│          │   Manager    │                                 │
│          └──────┬───────┘                                 │
│                 │                                          │
│                 ▼                                          │
│         ┌──────────────┐                                  │
│         │   Bitcoin    │                                  │
│         │    Client    │                                  │
│         └──────────────┘                                  │
└─────────────────────────────────────────────────────────────┘
```

### 3. HD Wallet System

```
┌─────────────────────────────────────────────────────────────┐
│                    HD Wallet System                          │
│                                                              │
│              ┌─────────────┐                                │
│              │  HdWallet   │                                │
│              └──────┬──────┘                                │
│                     │                                        │
│          ┌──────────┼──────────┐                           │
│          │          │          │                           │
│          ▼          ▼          ▼                           │
│   ┌──────────┐ ┌────────┐ ┌──────────┐                   │
│   │   XPUB   │ │Derivat-│ │ Address  │                   │
│   │ Validator│ │  ion   │ │  Cache   │                   │
│   └──────────┘ │ Engine │ └──────────┘                   │
│                └────┬───┘                                  │
│                     │                                      │
│          ┌──────────┼──────────┐                          │
│          │          │          │                          │
│          ▼          ▼          ▼                          │
│   ┌──────────┐ ┌────────┐ ┌──────────┐                  │
│   │ External │ │Internal│ │   Gap    │                  │
│   │  Chain   │ │ Chain  │ │  Limit   │                  │
│   │(Receive) │ │(Change)│ │ Tracker  │                  │
│   └──────────┘ └────────┘ └──────────┘                  │
│                     │                                     │
│                     ▼                                     │
│            ┌────────────────┐                            │
│            │  Address Book  │                            │
│            │ (Label mapping)│                            │
│            └────────────────┘                            │
└─────────────────────────────────────────────────────────────┘
```

### 4. Security & Monitoring System

```
┌─────────────────────────────────────────────────────────────┐
│              Security & Monitoring System                    │
│                                                              │
│          ┌──────────────────────┐                          │
│          │  Activity Monitor    │                          │
│          └──────────┬───────────┘                          │
│                     │                                       │
│          ┌──────────┼──────────┐                           │
│          │          │          │                           │
│          ▼          ▼          ▼                           │
│   ┌──────────┐ ┌────────┐ ┌──────────┐                   │
│   │ Velocity │ │  Large │ │  Rapid   │                   │
│   │  Checker │ │   TX   │ │Succession│                   │
│   └──────────┘ │Detector│ └──────────┘                   │
│                └────────┘                                  │
│                     │                                      │
│                     ▼                                      │
│            ┌────────────────┐                             │
│            │  Risk Scorer   │                             │
│            │    (0-100)     │                             │
│            └────────┬───────┘                             │
│                     │                                      │
│          ┌──────────┼──────────┐                          │
│          │          │          │                          │
│          ▼          ▼          ▼                          │
│   ┌──────────┐ ┌────────┐ ┌──────────┐                  │
│   │  Alert   │ │  Audit │ │  Metrics │                  │
│   │Broadcast │ │ Logger │ │Collector │                  │
│   └──────────┘ └────────┘ └──────────┘                  │
│                     │                                     │
│                     ▼                                     │
│              ┌────────────┐                              │
│              │ Tx Limits  │                              │
│              │  Enforcer  │                              │
│              └────────────┘                              │
└─────────────────────────────────────────────────────────────┘
```

### 5. Infrastructure Layer

```
┌─────────────────────────────────────────────────────────────┐
│                 Infrastructure Layer                         │
│                                                              │
│              ┌─────────────────┐                            │
│              │ Bitcoin Client  │                            │
│              └────────┬────────┘                            │
│                       │                                      │
│          ┌────────────┼────────────┐                        │
│          │            │            │                        │
│          ▼            ▼            ▼                        │
│   ┌──────────┐ ┌──────────┐ ┌──────────┐                  │
│   │Connection│ │  Cache   │ │ Metrics  │                  │
│   │   Pool   │ │ Manager  │ │ Tracker  │                  │
│   └─────┬────┘ └─────┬────┘ └──────────┘                  │
│         │            │                                      │
│         │      ┌─────┼─────┐                               │
│         │      │     │     │                               │
│         │      ▼     ▼     ▼                               │
│         │   ┌────┐┌────┐┌────┐                            │
│         │   │ TX ││UTXO││Block│                           │
│         │   │Cache││Cache││Cache│                         │
│         │   └────┘└────┘└────┘                            │
│         │                                                  │
│         ▼                                                  │
│  ┌──────────────┐                                         │
│  │Health Monitor│                                         │
│  │& Auto-Retry  │                                         │
│  └──────┬───────┘                                         │
│         │                                                  │
│         ▼                                                  │
│  ┌──────────────┐                                         │
│  │ Structured   │                                         │
│  │   Logging    │                                         │
│  └──────────────┘                                         │
└─────────────────────────────────────────────────────────────┘
```

---

## State Machine Diagrams

### 1. Payment Status State Machine

```
                    ┌──────────────┐
                    │   PENDING    │ (Initial state: address generated)
                    └──────┬───────┘
                           │
              Transaction  │  received in mempool
                 detected  │
                           ▼
                    ┌──────────────┐
                    │ UNCONFIRMED  │ (0 confirmations)
                    └──────┬───────┘
                           │
                  1 block  │  mined
                confirmed  │
                           ▼
                    ┌──────────────┐
         ┌──────────┤  CONFIRMED   │ (1-2 confirmations)
         │          └──────┬───────┘
         │                 │
         │        3 blocks │  mined
         │      confirmed  │
         │                 ▼
         │          ┌──────────────┐
         │          │FULL_CONFIRMED│ (3-5 confirmations)
         │          └──────┬───────┘
         │                 │
         │        6 blocks │  mined
         │      confirmed  │
         │                 ▼
         │          ┌──────────────┐
         │          │   FINALIZED  │ (6+ confirmations)
         │          └──────────────┘
         │
         │ (Any state can transition to FAILED)
         │
         └─────────►┌──────────────┐
                    │    FAILED    │ (Error/timeout/invalid)
                    └──────────────┘

State Transitions:
- PENDING → UNCONFIRMED: Transaction detected in mempool
- UNCONFIRMED → CONFIRMED: First block confirmation
- CONFIRMED → FULL_CONFIRMED: Third block confirmation
- FULL_CONFIRMED → FINALIZED: Sixth block confirmation
- Any → FAILED: Error, timeout, or invalid transaction
```

### 2. Lightning Invoice State Machine

```
                    ┌──────────────┐
                    │     OPEN     │ (Invoice created, awaiting payment)
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
     Payment  │   Timeout  │   Payment  │
     received │  exceeded  │  in flight │
              │            │            │
              ▼            ▼            ▼
       ┌──────────┐ ┌──────────┐ ┌──────────┐
       │ SETTLED  │ │ EXPIRED  │ │  PENDING │
       └──────────┘ └──────────┘ └────┬─────┘
                                       │
                           ┌───────────┼───────────┐
                           │                       │
                  Payment  │              Payment  │
                 completed │               failed  │
                           │                       │
                           ▼                       ▼
                    ┌──────────┐           ┌──────────┐
                    │ SETTLED  │           │ EXPIRED  │
                    └──────────┘           └──────────┘

State Transitions:
- OPEN → PENDING: HTLC received, payment in flight
- OPEN → EXPIRED: Timeout exceeded without payment
- PENDING → SETTLED: HTLC successfully settled
- PENDING → EXPIRED: HTLC failed or timeout
```

### 3. Atomic Swap State Machine

```
                    ┌──────────────┐
                    │  INITIATED   │ (Initiator creates swap)
                    └──────┬───────┘
                           │
            Participant    │  accepts swap
                 responds  │
                           ▼
                    ┌──────────────┐
         ┌──────────┤    LOCKED    │ (Both parties funded HTLCs)
         │          └──────┬───────┘
         │                 │
         │      Initiator  │  claims with preimage
         │         claims  │
         │                 ▼
         │          ┌──────────────┐
         │          │  COMPLETED   │ (Successful swap)
         │          └──────────────┘
         │
         │ (Alternative paths)
         │
         ├─────────►┌──────────────┐
         │          │   REFUNDED   │ (Timeout, refund activated)
         │          └──────────────┘
         │
         └─────────►┌──────────────┐
                    │  CANCELLED   │ (Cancelled before lock)
                    └──────────────┘

State Transitions:
- INITIATED → LOCKED: Both HTLCs funded
- INITIATED → CANCELLED: Swap cancelled before funding
- LOCKED → COMPLETED: Initiator claims, revealing preimage
- LOCKED → REFUNDED: Timeout exceeded, refund paths activated
```

### 4. PSBT (Transaction Signing) State Machine

```
                    ┌──────────────┐
                    │   CREATED    │ (Unsigned PSBT created)
                    └──────┬───────┘
                           │
            First signer   │  adds signature
               signs PSBT  │
                           ▼
                    ┌──────────────┐
                    │PARTIALLY_SIGN│ (1+ signatures, needs more)
                    └──────┬───────┘
                           │
          Threshold met    │  (e.g., 2-of-3)
          (all required    │
           signatures)     │
                           ▼
                    ┌──────────────┐
         ┌──────────┤   SIGNED     │ (All required signatures present)
         │          └──────┬───────┘
         │                 │
         │      Finalize   │  and extract transaction
         │        PSBT     │
         │                 ▼
         │          ┌──────────────┐
         │          │  FINALIZED   │ (Ready to broadcast)
         │          └──────┬───────┘
         │                 │
         │     Broadcast   │  to network
         │  to Bitcoin Core│
         │                 ▼
         │          ┌──────────────┐
         │          │  BROADCAST   │ (Transaction in mempool/blockchain)
         │          └──────────────┘
         │
         │ (Error path)
         │
         └─────────►┌──────────────┐
                    │   REJECTED   │ (Invalid/rejected)
                    └──────────────┘

State Transitions:
- CREATED → PARTIALLY_SIGN: First signature added
- PARTIALLY_SIGN → PARTIALLY_SIGN: Additional signatures (not yet threshold)
- PARTIALLY_SIGN → SIGNED: Threshold met
- SIGNED → FINALIZED: PSBT finalized and transaction extracted
- FINALIZED → BROADCAST: Transaction broadcast to network
- Any → REJECTED: Validation failure or rejection
```

### 5. UTXO Consolidation State Machine

```
                    ┌──────────────┐
                    │     IDLE     │ (No consolidation needed)
                    └──────┬───────┘
                           │
        High UTXO count    │  OR low fee environment
           detected        │  triggers consolidation
                           ▼
                    ┌──────────────┐
                    │  ANALYZING   │ (Checking UTXOs and fees)
                    └──────┬───────┘
                           │
           Plan created    │  with optimal inputs
                           ▼
                    ┌──────────────┐
         ┌──────────┤   PLANNED    │ (Consolidation plan ready)
         │          └──────┬───────┘
         │                 │
         │    Transaction  │  built and signed
         │        created  │
         │                 ▼
         │          ┌──────────────┐
         │          │ CONSOLIDATING│ (Transaction broadcast)
         │          └──────┬───────┘
         │                 │
         │    Transaction  │  confirmed
         │      confirmed  │
         │                 ▼
         │          ┌──────────────┐
         │          │  COMPLETED   │ (UTXOs consolidated)
         │          └──────────────┘
         │                 │
         │                 │  Return to monitoring
         │                 ▼
         │          ┌──────────────┐
         └─────────►│     IDLE     │
                    └──────────────┘

State Transitions:
- IDLE → ANALYZING: Consolidation trigger detected
- ANALYZING → PLANNED: Plan created successfully
- PLANNED → CONSOLIDATING: Transaction broadcast
- CONSOLIDATING → COMPLETED: Transaction confirmed
- COMPLETED → IDLE: Return to monitoring
```

---

## Data Flow

### 1. Incoming Payment Data Flow

```
Bitcoin Network
      │
      ▼
Bitcoin Core RPC
      │
      ▼
BitcoinClient (kaccy-bitcoin)
      │
      ▼
PaymentMonitor (polling every 30s)
      │
      ├──► TransactionMatcher (match by address)
      │         │
      │         ▼
      │    MatchStatus (Exact/Underpaid/Overpaid)
      │
      ├──► ConfirmationTracker (track 0→1→3→6)
      │         │
      │         ▼
      │    ConfirmationEvents
      │
      ├──► AdminNotificationService (if discrepancy)
      │
      └──► EventEmitter
                │
                ├──► WebSocket (real-time updates)
                │
                └──► Database (order status update)
```

### 2. Outgoing Payment Data Flow

```
User Request (Withdrawal)
      │
      ▼
API (kaccy-api)
      │
      ▼
TransactionManager (kaccy-bitcoin)
      │
      ├──► LimitEnforcer (check daily/monthly limits)
      │
      ├──► ActivityMonitor (check risk score)
      │
      └──► UtxoManager (select optimal inputs)
            │
            ▼
      PsbtManager (create unsigned PSBT)
            │
            ▼
      HardwareWallet / Signer (sign PSBT)
            │
            ▼
      PsbtManager (finalize PSBT)
            │
            ▼
      BitcoinClient (broadcast transaction)
            │
            ▼
      Bitcoin Core RPC
            │
            ▼
      Bitcoin Network
```

### 3. Address Generation Data Flow

```
Order Created (kaccy-api)
      │
      ▼
HdWallet.get_new_address(order_id)
      │
      ├──► AddressCache.check(order_id)
      │         │
      │         ▼
      │    Cache Miss
      │
      ├──► DerivationPath.derive(m/84'/0'/0'/0/{index})
      │         │
      │         ▼
      │    BIP32 Derivation
      │         │
      │         ▼
      │    P2WPKH Address (Bech32)
      │
      ├──► AddressCache.store(order_id, address)
      │
      └──► AddressBook.map(order_id, address, label)
            │
            ▼
      Return address to API
```

---

## Integration Points

### 1. External Service Integrations

#### Bitcoin Core (RPC)
- **Protocol**: JSON-RPC over HTTP
- **Authentication**: Basic auth (username/password)
- **Key Methods**:
  - `getnetworkinfo`: Network status
  - `listsinceblock`: List transactions since block
  - `sendrawtransaction`: Broadcast transaction
  - `estimatesmartfee`: Fee estimation
  - `getmempoolinfo`: Mempool status

#### LND (Lightning Network Daemon)
- **Protocol**: REST API
- **Authentication**: Macaroon-based
- **Key Endpoints**:
  - `POST /v1/invoices`: Create invoice
  - `GET /v1/invoice/{r_hash}`: Lookup invoice
  - `GET /v1/channels`: List channels
  - `POST /v1/channels`: Open channel

#### Stacks (L2)
- **Protocol**: HTTP REST API
- **Key Operations**:
  - Deploy Clarity smart contracts
  - Monitor bridge transactions
  - Track token transfers

#### RSK (L2)
- **Protocol**: Ethereum-compatible JSON-RPC
- **Key Operations**:
  - Peg-in (BTC → rBTC)
  - Peg-out (rBTC → BTC)
  - Smart contract deployment

### 2. Internal Integrations (Kaccy Ecosystem)

#### kaccy-db
- **Purpose**: Persistent storage for orders, transactions, users
- **Key Functions**:
  - `update_btc_txid`: Link transaction to order
  - `get_order_by_address`: Retrieve order by payment address
  - `update_order_status`: Update payment status

#### kaccy-api
- **Purpose**: REST API serving user requests
- **Key Functions**:
  - Order creation (triggers address generation)
  - Withdrawal requests (triggers PSBT creation)
  - WebSocket notifications (payment updates)

#### kaccy-core
- **Purpose**: Core business logic
- **Key Functions**:
  - Event bus integration
  - Pricing calculations
  - Trading logic

### 3. Hardware Wallet Integration (HWI)

#### Supported Devices
- **Ledger**: Via HID/USB
- **Trezor**: Via HID/USB
- **Generic**: Via Bitcoin Core HWI tool

#### Key Operations
- PSBT signing
- Address verification
- Multi-signature coordination

### 4. Observability Integrations

#### Metrics (Prometheus)
- Transaction volume counters
- Fee estimation gauges
- RPC call latency histograms
- Confirmation time tracking

#### Logging (Structured)
- Correlation IDs for request tracing
- Performance spans
- RPC request/response logging

#### Health Checks
- Bitcoin Core connectivity
- LND connectivity
- Connection pool health
- Cache hit rates

---

## Architectural Decisions

### 1. Polling vs. WebSocket for Bitcoin Core
- **Decision**: Polling (30-second interval)
- **Rationale**:
  - Bitcoin Core RPC doesn't provide native WebSocket/streaming
  - ZMQ available but adds complexity
  - 30s polling provides acceptable latency for on-chain payments
  - Simpler error handling and reconnection logic

### 2. PSBT for Transaction Signing
- **Decision**: Use PSBT (BIP 174) for all transaction signing
- **Rationale**:
  - Hardware wallet compatibility
  - Multi-signature support
  - External signing workflows (air-gapped)
  - Industry standard

### 3. BIP84 (Native SegWit) for Address Generation
- **Decision**: Use BIP84 (P2WPKH) as default address type
- **Rationale**:
  - Lower transaction fees (SegWit)
  - Widely supported across wallets
  - Enhanced security (malleability fix)
  - Future-proof for Taproot migration

### 4. Connection Pooling for Bitcoin RPC
- **Decision**: Implement connection pool with health monitoring
- **Rationale**:
  - High concurrency support
  - Reduced connection overhead
  - Automatic failover
  - Better resource utilization

### 5. Tiered Caching Strategy
- **Decision**: Multi-level cache (transactions, UTXOs, blocks)
- **Rationale**:
  - Reduce RPC calls to Bitcoin Core
  - Improve response times
  - TTL-based invalidation prevents stale data
  - Configurable per use case

---

## Security Considerations

### 1. Key Management
- **XPUB-only** for address generation (no private keys in server)
- Hardware wallet integration for signing
- Multi-signature for high-value transactions
- Key rotation policies

### 2. Transaction Limits
- Per-user daily/monthly limits
- Platform-wide limits
- Single transaction maximum
- Velocity limits (amount per hour)

### 3. Activity Monitoring
- Risk scoring (0-100) for transactions
- Large transaction alerts
- Rapid succession detection
- Admin notifications for anomalies

### 4. Audit Logging
- Immutable audit trail
- All transaction operations logged
- Compliance reporting
- Query capabilities for investigations

### 5. Network Security
- TLS for all RPC connections
- Macaroon authentication for LND
- IP whitelisting support
- Rate limiting

---

## Performance Optimizations

### 1. Caching
- Transaction cache (1-hour TTL)
- UTXO cache (invalidated on spend)
- Block header cache (immutable)
- Address cache (persistent)

### 2. Batch Processing
- Batch withdrawals (multiple outputs in single transaction)
- UTXO consolidation during low-fee periods
- Batch RBF operations

### 3. Connection Management
- Connection pooling (reuse connections)
- Health monitoring (automatic reconnection)
- Concurrent request handling

### 4. UTXO Management
- Optimal coin selection algorithms
- Dust prevention
- Consolidation automation
- Branch-and-bound selection

---

## Future Architecture Considerations

### 1. Horizontal Scaling
- Stateless design for multiple instances
- Shared cache (Redis) for distributed deployments
- Load balancing across Bitcoin Core nodes

### 2. Event-Driven Architecture
- Migrate to event bus for all state changes
- Async processing for non-critical paths
- Event sourcing for audit trail

### 3. Microservices Separation
- Separate payment monitoring service
- Dedicated Lightning service
- Independent transaction signing service

### 4. Enhanced Privacy
- CoinJoin integration by default
- PayJoin for all payments
- Tor support for RPC connections

---

## Conclusion

The `kaccy-bitcoin` crate provides a comprehensive, production-ready Bitcoin integration with:
- **Robust payment monitoring** with confirmation tracking
- **Flexible wallet management** via BIP84 HD wallets
- **Advanced transaction handling** with PSBT, multi-sig, Taproot
- **Security-first design** with limits, monitoring, and audit logging
- **Performance optimizations** via caching, pooling, and batch processing
- **Lightning Network integration** for instant payments
- **L2 support** for Stacks and RSK
- **Privacy features** like CoinJoin, PayJoin, and atomic swaps

This architecture is designed to scale from MVP to production while maintaining security, reliability, and performance.
