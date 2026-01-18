//! # Kaccy Protocol
//!
//! Complete platform for fractional Bitcoin transactions with reputation-backed personal tokens.
//!
//! ## Overview
//!
//! Kaccy is a next-generation value distribution protocol where professionals can tokenize
//! their expertise using Bitcoin and Lightning Network. Token holders gain access to creators'
//! services and future outputs, backed by a robust reputation system.
//!
//! ## Key Features
//!
//! - **Personal Tokens (FOT)**: Issue "Future Output Tokens" representing access to your services
//! - **Bonding Curve Pricing**: Automatic price discovery with always-available liquidity
//! - **Bitcoin Native**: All transactions in BTC, with Lightning Network for instant micropayments
//! - **Reputation System**: SBT-based reputation tracking with commitment verification
//! - **AI Integration**: Quality evaluation and fraud detection powered by AI
//!
//! ## Crates
//!
//! This meta-crate re-exports all Kaccy sub-crates for convenient access:
//!
//! - [`kaccy_core`] - Core business logic (tokens, orders, matching)
//! - [`kaccy_db`] - Database layer (PostgreSQL, Redis, caching)
//! - [`kaccy_bitcoin`] - Bitcoin Core & Lightning integration
//! - [`kaccy_reputation`] - Reputation scoring & SBT
//! - [`kaccy_ai`] - AI evaluation services
//! - [`kaccy_api`] - REST/WebSocket API (Axum)
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use kaccy::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Setup database
//!     let pool = kaccy_db::create_pool(&std::env::var("DATABASE_URL")?).await?;
//!
//!     // Initialize services
//!     let user_repo = kaccy_db::UserRepository::new(pool.clone());
//!     let token_repo = kaccy_db::TokenRepository::new(pool.clone());
//!
//!     // Connect to Bitcoin
//!     let bitcoin_client = kaccy_bitcoin::BitcoinClient::new(
//!         "http://localhost:8332",
//!         "rpcuser",
//!         "rpcpassword",
//!     )?;
//!
//!     // Create reputation engine
//!     let reputation = kaccy_reputation::ReputationEngine::new(pool.clone());
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Usage by Module
//!
//! ### Core Business Logic
//!
//! ```rust,no_run
//! use kaccy::kaccy_core::pricing::{BondingCurve, LinearBondingCurve};
//! use rust_decimal_macros::dec;
//!
//! let curve = LinearBondingCurve::new(dec!(0.0001), dec!(0.00001));
//! let cost = curve.buy_price(dec!(0), dec!(10));
//! ```
//!
//! ### Database Operations
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use kaccy::kaccy_db::{create_pool, UserRepository};
//!
//! let pool = create_pool("postgresql://...").await?;
//! let user_repo = UserRepository::new(pool);
//! let user = user_repo.find_by_email("user@example.com").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Bitcoin Integration
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use kaccy::kaccy_bitcoin::{BitcoinClient, HdWallet};
//!
//! let client = BitcoinClient::new("http://localhost:8332", "user", "pass")?;
//! let wallet = HdWallet::new(&std::env::var("WALLET_XPUB")?)?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Reputation System
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use kaccy::kaccy_reputation::{ReputationEngine, ReputationEventType};
//! use rust_decimal_macros::dec;
//! # use sqlx::PgPool;
//! # let pool: PgPool = todo!();
//! # let user_id = uuid::Uuid::new_v4();
//!
//! let engine = ReputationEngine::new(pool);
//! let score = engine.get_score(user_id).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### AI Services
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use kaccy::kaccy_ai::{AiEvaluator, EvaluatorConfig};
//! use kaccy::kaccy_ai::llm::{LlmClient, OpenAiClient};
//!
//! let openai = OpenAiClient::with_default_model("api-key");
//! let llm_client = LlmClient::new(Box::new(openai));
//! let evaluator = AiEvaluator::with_config(llm_client, EvaluatorConfig::default());
//! # Ok(())
//! # }
//! ```
//!
//! ## Documentation
//!
//! See individual crate documentation for detailed API references:
//!
//! - [kaccy-core docs](https://docs.rs/kaccy-core)
//! - [kaccy-db docs](https://docs.rs/kaccy-db)
//! - [kaccy-bitcoin docs](https://docs.rs/kaccy-bitcoin)
//! - [kaccy-reputation docs](https://docs.rs/kaccy-reputation)
//! - [kaccy-ai docs](https://docs.rs/kaccy-ai)
//! - [kaccy-api docs](https://docs.rs/kaccy-api)
//!
//! ## License
//!
//! Proprietary - All rights reserved

#![warn(missing_docs)]
#![deny(unsafe_code)]

// Re-export all sub-crates
pub use kaccy_ai;
pub use kaccy_api;
pub use kaccy_bitcoin;
pub use kaccy_core;
pub use kaccy_db;
pub use kaccy_reputation;

pub mod prelude {
    //! Prelude module for convenient imports.
    //!
    //! Commonly used types and traits from all Kaccy crates.
    //!
    //! Import everything with:
    //! ```
    //! use kaccy::prelude::*;
    //! ```

    // Core types
    pub use kaccy_core::models::{Balance, Order, Token, Trade, User};
    pub use kaccy_core::pricing::{BondingCurve, LinearBondingCurve};

    // Database
    pub use kaccy_db::{
        BalanceRepository, OrderRepository, TokenRepository, TradeRepository, UserRepository,
        create_pool, create_pool_with_retry,
    };

    // Bitcoin
    pub use kaccy_bitcoin::{BitcoinClient, HdWallet, PaymentMonitor};

    // Reputation
    pub use kaccy_reputation::{
        ReputationEngine, ReputationEventType, ReputationTier, TierBenefits,
    };

    // AI
    pub use kaccy_ai::{AiEvaluator, EvaluatorConfig};

    // Common external types
    pub use chrono::{DateTime, Utc};
    pub use rust_decimal::Decimal;
    pub use uuid::Uuid;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_meta_crate_compiles() {
        // This test ensures all re-exports compile
        use crate::prelude::*;

        // Type assertions to ensure imports work
        let _: Option<Uuid> = None;
        let _: Option<Decimal> = None;
        let _: Option<DateTime<Utc>> = None;
    }
}
