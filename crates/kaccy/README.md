# kaccy

Meta-crate for the Kaccy Protocol - Complete platform for fractional Bitcoin transactions.

## Overview

This is a convenience meta-crate that re-exports all Kaccy sub-crates, allowing you to use the entire Kaccy ecosystem through a single dependency.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
kaccy = "0.2.0"
```

## Usage

### Using the Prelude

The easiest way to get started:

```rust
use kaccy::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // All common types are available
    let pool = create_pool(&std::env::var("DATABASE_URL")?).await?;
    let user_repo = UserRepository::new(pool.clone());

    Ok(())
}
```

### Using Individual Crates

Access specific crate functionality:

```rust
use kaccy::kaccy_core::pricing::LinearBondingCurve;
use kaccy::kaccy_bitcoin::BitcoinClient;
use kaccy::kaccy_reputation::ReputationEngine;
use kaccy::kaccy_ai::AiEvaluator;
```

## What's Included

This meta-crate includes all Kaccy components:

- **kaccy-core** - Core business logic (tokens, orders, matching, bonding curves)
- **kaccy-db** - Database layer (PostgreSQL, Redis, caching, repositories)
- **kaccy-bitcoin** - Bitcoin integration (Core RPC, HD wallets, Lightning, UTXO management)
- **kaccy-reputation** - Reputation system (scoring, tiers, SBT, commitments)
- **kaccy-ai** - AI services (code evaluation, fraud detection, verification)
- **kaccy-api** - REST API (Axum web server, authentication, endpoints)

## Individual Crate Installation

If you only need specific components, you can install them individually:

```toml
[dependencies]
kaccy-core = "0.2.0"
kaccy-db = "0.2.0"
kaccy-bitcoin = "0.2.0"
kaccy-reputation = "0.2.0"
kaccy-ai = "0.2.0"
kaccy-api = "0.2.0"
```

## Documentation

- [Full Documentation](https://docs.rs/kaccy)
- [GitHub Repository](https://github.com/cool-japan/kaccy)
- [API Documentation](https://docs.rs/kaccy-api)

Individual crate documentation:
- [kaccy-core](https://docs.rs/kaccy-core)
- [kaccy-db](https://docs.rs/kaccy-db)
- [kaccy-bitcoin](https://docs.rs/kaccy-bitcoin)
- [kaccy-reputation](https://docs.rs/kaccy-reputation)
- [kaccy-ai](https://docs.rs/kaccy-ai)
- [kaccy-api](https://docs.rs/kaccy-api)

## Quick Start Example

```rust
use kaccy::prelude::*;
use rust_decimal_macros::dec;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup database
    let pool = create_pool(&std::env::var("DATABASE_URL")?).await?;

    // Create repositories
    let user_repo = UserRepository::new(pool.clone());
    let token_repo = TokenRepository::new(pool.clone());

    // Initialize Bitcoin client
    let bitcoin_client = kaccy::kaccy_bitcoin::BitcoinClient::new(
        "http://localhost:8332",
        "rpcuser",
        "rpcpassword",
    )?;

    // Create HD wallet for address generation
    let wallet = kaccy::kaccy_bitcoin::HdWallet::new(
        &std::env::var("WALLET_XPUB")?
    )?;

    // Initialize reputation engine
    let reputation = kaccy::kaccy_reputation::ReputationEngine::new(pool.clone());

    // Use bonding curve for pricing
    let curve = LinearBondingCurve::new(
        dec!(0.0001),  // Initial price
        dec!(0.00001), // Increment
    );

    let cost = curve.buy_price(dec!(0), dec!(10));
    println!("Cost for 10 tokens: {} BTC", cost);

    Ok(())
}
```

## Features

All features from sub-crates are available:

- Token creation and trading
- Bonding curve pricing
- Bitcoin payment monitoring
- Lightning Network support
- Reputation scoring and tiers
- AI-powered evaluation
- REST API with WebSocket
- Multi-signature wallets
- Database operations with caching

## Requirements

- Rust 1.85+
- PostgreSQL 15+
- Bitcoin Core (for Bitcoin integration)
- Redis (optional, for caching)

## Environment Variables

```bash
# Database
DATABASE_URL=postgresql://user:password@localhost/kaccy

# Bitcoin
BITCOIN_RPC_URL=http://localhost:8332
BITCOIN_RPC_USER=rpcuser
BITCOIN_RPC_PASSWORD=rpcpassword
WALLET_XPUB=xpub...

# API
JWT_SECRET=your-secret-key
PORT=8080

# AI Services (optional)
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GEMINI_API_KEY=...
```

## License

Proprietary - All rights reserved

## Support

- GitHub: https://github.com/cool-japan/kaccy
- Email: contact@kaccy.net
- Website: https://kaccy.net
