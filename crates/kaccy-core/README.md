# kaccy-core

Core business logic for Kaccy Protocol.

## Overview

This crate contains the fundamental domain models, bonding curve calculations, order matching, and trade execution logic for the Kaccy personal token platform.

## Modules

### `models`

Domain models representing the core entities:

- **`User`** - User account with DID, KYC status, and reputation score
- **`Token`** - Personal FOT (Future Output Token) with bonding curve parameters
- **`Order`** - Buy/sell orders with BTC payment information
- **`Trade`** - Executed trade records
- **`Balance`** - User token balances

### `pricing`

Bonding curve implementations for automatic price discovery:

- **`BondingCurve`** - Trait defining the bonding curve interface
- **`LinearBondingCurve`** - Linear price model: `P(n) = initial_price + (n * increment)`
- (Planned) `BancorCurve`, `SigmoidCurve`, `AdaptiveCurve`

### `trading`

Trade execution logic:

- **`TradeExecutor`** - Executes trades with ACID guarantees
- Order matching engine (Phase 2: Limit orders)

### `error`

Error types for the core module.

## Usage

```rust
use kaccy_core::pricing::{BondingCurve, LinearBondingCurve};
use rust_decimal_macros::dec;

// Create a linear bonding curve
let curve = LinearBondingCurve::new(
    dec!(0.0001),  // Initial price: 0.0001 BTC
    dec!(0.00001), // Increment: 0.00001 BTC per token
);

// Calculate buy price for 10 tokens at supply 0
let cost = curve.buy_price(dec!(0), dec!(10));
println!("Cost to buy 10 tokens: {} BTC", cost);

// Get current spot price at supply 100
let price = curve.spot_price(dec!(100));
println!("Spot price at supply 100: {} BTC", price);
```

## Architecture

```
kaccy-core/
├── src/
│   ├── lib.rs           # Crate root
│   ├── error.rs         # Error types
│   ├── models/
│   │   ├── mod.rs
│   │   ├── user.rs      # User model
│   │   ├── token.rs     # Token model
│   │   ├── order.rs     # Order model
│   │   ├── trade.rs     # Trade model
│   │   └── balance.rs   # Balance model
│   ├── pricing/
│   │   ├── mod.rs
│   │   └── bonding_curve.rs  # Bonding curve implementations
│   └── trading/
│       ├── mod.rs
│       └── executor.rs  # Trade execution
└── Cargo.toml
```

## Dependencies

- `rust_decimal` - High-precision decimal arithmetic
- `chrono` - Date/time handling
- `uuid` - UUID generation
- `serde` - Serialization/deserialization
- `sqlx` - Database types

## Testing

```bash
cargo test -p kaccy-core
```
