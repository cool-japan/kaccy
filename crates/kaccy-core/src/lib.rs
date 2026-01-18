//! Core business logic for Kaccy Protocol
//!
//! This crate contains the domain models, bonding curve calculations,
//! order matching, and trade execution logic.
//!
//! # Architecture
//!
//! The kaccy-core crate is organized into the following modules:
//!
//! - [`models`] - Domain models including users, tokens, orders, trades, balances, commitments, and analytics
//! - [`pricing`] - Bonding curve implementations, fee calculation, and price oracles
//! - [`trading`] - Order matching, execution, market maker functionality, and sentiment analysis
//! - [`ml`] - Machine learning integration for price prediction, anomaly detection, and strategy optimization
//! - [`events`] - Event emission system for domain events
//! - [`utils`] - Utility functions for pagination, caching, validation, risk management, and more
//! - [`error`] - Comprehensive error types with HTTP status mapping and retry logic
//!
//! # Features
//!
//! ## Bonding Curves
//!
//! The protocol supports multiple bonding curve types:
//! - Linear curves for predictable pricing
//! - Bancor curves with configurable reserve ratios
//! - Sigmoid curves for price stabilization
//! - Exponential curves for rapid growth
//! - Adaptive curves that switch between phases
//! - Square root and logarithmic curves for gentler growth
//!
//! ## Trading Features
//!
//! - Order book with price-time priority matching
//! - Market maker integration with dynamic spreads
//! - Circuit breakers for price manipulation protection
//! - Advanced order types (stop-loss, take-profit, TWAP)
//! - AMM-style liquidity pools with flash swaps
//! - Position management and risk controls
//!
//! ## Fee System
//!
//! - Platform fees with reputation-based discounts
//! - Volume-based fee tiers
//! - Maker/taker fee differentiation
//! - Fee distribution to stakers
//!
//! ## Utilities
//!
//! - Event emission and persistence
//! - Caching layer with TTL management
//! - Query optimization and N+1 detection
//! - Benchmarking and load testing
//! - Model factories and test fixtures
//! - Risk management and position sizing
//! - Price oracle integration and aggregation
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use kaccy_core::pricing::{LinearBondingCurve, BondingCurve};
//! use rust_decimal_macros::dec;
//!
//! // Create a linear bonding curve
//! let curve = LinearBondingCurve::new(dec!(0.001), dec!(1.0));
//!
//! // Calculate buy price for 100 tokens when supply is 1000
//! let price = curve.buy_price(dec!(1000), dec!(100));
//! println!("Price: {}", price);
//! ```

pub mod defi;
pub mod error;
pub mod events;
pub mod ml;
pub mod models;
pub mod pricing;
pub mod trading;
pub mod utils;

pub use error::{CoreError, Result};
