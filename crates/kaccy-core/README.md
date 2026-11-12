# kaccy-core

[![Status: Alpha](https://img.shields.io/badge/status-alpha-orange.svg)](https://github.com/cool-japan/kaccy)

Core business logic for the Kaccy Protocol — a personal Future Output Token (FOT) platform.
This crate houses all domain models, pricing engines, trading infrastructure, DeFi primitives,
ML/analytics pipelines, and shared utilities that power the Kaccy exchange.

**Scale (v0.2.0):** 210 Rust source files · ~90,548 lines · 4,663 public items · 1,640 tests passing.

---

## Features

### Models
Full domain-model layer covering every entity in the protocol:
User, Token, Order, Trade, Balance, Governance, KYC, Leaderboard, Payment, Position,
Reputation, Vesting, Airdrop, and Notification — each with Serde serialization and
SQLx-compatible types.

### Pricing
Seven production-ready bonding curve implementations backed by high-precision
`rust_decimal` arithmetic, plus a dynamic fee engine, fee optimizer, price oracle,
staking rewards, liquidity-mining incentives, and buyback automation.

### Trading
A complete order-routing stack: limit/market order book, market maker, AMM, arbitrage
engine, flash-loan executor, MEV detection and mitigation, HFT metrics, smart order
routing, backtesting harness, batch auctions, conditional orders, options pricing,
VPIN calculation, and commit-reveal order submission.

### DeFi
Margin trading, a lending protocol, perpetual futures, concentrated-liquidity AMM,
liquid staking, yield farming, synthetic assets, P2P lending, intent-based trading,
JIT liquidity provisioning, distributed consensus, cross-margin accounting, and
auto-compounding vaults.

### ML / Analytics
Time-series forecasting (ARIMA, GARCH, Prophet), ensemble methods, sentiment analysis,
GPU-accelerated inference, online learning, reinforcement learning, feature engineering,
and model-persistence utilities — all implemented in pure Rust without Python bindings.

### Utils
A broad utility layer including risk management, portfolio optimization, on-chain analytics,
multi-level caching, compliance checks, encryption, KYC/AML screening, cross-chain bridge
adapters, database optimization (sharding, replicas), fuzz testing, load testing, memory
profiling (`TrackingAllocator`, `MemoryProfiler`, `MemoryGuard`), structured metrics,
distributed tracing, webhook dispatch, and threshold encryption.

---

## Architecture

| Subsystem | Files | Description |
|-----------|------:|-------------|
| `models/` | 24 | Domain entities: User, Token, Order, Trade, Balance, Governance, KYC, Leaderboard, Payment, Position, Reputation, Vesting, Airdrop, Notification |
| `pricing/` | 13 | 7 bonding curves, dynamic fees, fee optimization, price oracle, staking, liquidity mining, buyback automation |
| `trading/` | 27 | Order book, market maker, AMM, arbitrage, flash loans, MEV detection/mitigation, HFT metrics, smart routing, backtesting, batch auctions, conditional orders, options pricing, VPIN, commit-reveal |
| `utils/` | 104 | Risk management, portfolio optimization, analytics, caching, compliance, encryption, KYC/AML, cross-chain bridges, DB optimization/sharding/replicas, fuzz/load/memory testing, metrics, tracing, webhooks, threshold encryption |
| `ml/` | 20 | ARIMA, GARCH, Prophet, ensemble methods, sentiment analysis, GPU acceleration, online learning, RL, feature engineering, model persistence |
| `events/` | 7 | Event bus, payloads, persistence, event sourcing, subscriber registry, typed event catalog |
| `defi/` | 15 | Margin trading, lending, perpetuals, concentrated-liquidity AMM, liquid staking, yield farming, synthetics, P2P lending, intent trading, JIT liquidity, distributed consensus, cross-margin, auto-compounding |
| `error.rs` | 1 | Unified error taxonomy with `thiserror`-derived variants |

---

## Bonding Curves

All seven curves implement the `BondingCurve` trait and expose `spot_price`,
`buy_price` (integral over supply range), and `sell_price` methods.
SquareRoot and Logarithmic curves use closed-form O(1) antiderivatives.

| Curve | Type | Characteristic |
|-------|------|----------------|
| `LinearBondingCurve` | Linear | `P(s) = base + slope · s` — simple, predictable growth |
| `BancorCurve` | Power-law | Continuous reserve ratio; connector weight controls steepness |
| `SigmoidCurve` | S-shaped | Slow start, rapid mid-growth, plateau at saturation |
| `ExponentialCurve` | Exponential | Aggressive price appreciation rewarding early adopters |
| `SquareRootCurve` | Sub-linear | Dampened growth; closed-form integral for O(1) cost |
| `LogarithmicCurve` | Sub-linear | Decelerating growth; closed-form integral for O(1) cost |
| `AdaptiveCurve` | Dynamic | Parameters self-adjust based on trading volume and volatility |

---

## Installation

```toml
[dependencies]
kaccy-core = "0.2.0"
```

---

## Quick Start

### Pricing a bonding curve

```rust,no_run
use kaccy_core::pricing::{BondingCurve, LinearBondingCurve};
use rust_decimal_macros::dec;

let curve = LinearBondingCurve::new(
    dec!(0.0001),   // base price (BTC)
    dec!(0.00001),  // slope: price increment per token
);

// Spot price at supply 100
let spot = curve.spot_price(dec!(100));

// Cost to purchase 10 tokens starting at supply 100 (integral)
let cost = curve.buy_price(dec!(100), dec!(10));
println!("spot={spot}  cost={cost}");
```

### Submitting an order

```rust,no_run
use kaccy_core::models::{Order, OrderSide, OrderType};
use kaccy_core::trading::OrderBook;
use rust_decimal_macros::dec;
use uuid::Uuid;

let mut book = OrderBook::new();

let order = Order::new(
    Uuid::new_v4(),          // user_id
    Uuid::new_v4(),          // token_id
    OrderSide::Buy,
    OrderType::Limit,
    dec!(10),                // quantity
    Some(dec!(0.0015)),      // limit price (BTC)
);

let fills = book.submit(order)?;
```

---

## Testing

| Suite | Count | Command |
|-------|------:|---------|
| Unit tests | 1,640 | `cargo nextest run -p kaccy-core` |
| Property tests (proptest) | 32 invariants across all 7 curves | included in unit run |
| Criterion benchmarks | full bonding-curve suite | `cargo bench -p kaccy-core` |

Property tests verify mathematical invariants for every bonding curve:
monotonicity, non-negativity, round-trip buy/sell consistency, and supply
conservation across arbitrary `(supply, quantity)` pairs generated by proptest.

---

## Performance

- **SquareRoot and Logarithmic curves** use closed-form antiderivatives for O(1)
  integral computation instead of numerical summation.
- **SIMD pricing paths** are enabled when `target-feature` includes AVX2 or NEON,
  reducing batch-price calculation latency for HFT and batch-auction workloads.
- The `TrackingAllocator` / `MemoryProfiler` pair in `utils::memory_profile` lets
  callers measure per-operation allocation cost in tests and benchmarks.

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `sqlx` | Async database access and type mapping |
| `tokio` | Async runtime |
| `serde` / `serde_json` | Serialization and deserialization |
| `uuid` | RFC 4122 UUID generation |
| `chrono` | Date/time arithmetic |
| `rust_decimal` | High-precision decimal arithmetic |
| `statrs` | Statistical distributions and functions |
| `regex` | Pattern matching in compliance and parsing modules |

---

## License

Copyright COOLJAPAN OU (Team KitaSan). All rights reserved.
