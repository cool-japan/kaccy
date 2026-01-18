//! Bonding curve pricing module
//!
//! This module provides various bonding curve implementations for token pricing,
//! fee calculation, and liquidity mining rewards.
//!
//! # Bonding Curves
//!
//! The [`bonding_curve`] module contains multiple curve implementations:
//!
//! - [`LinearBondingCurve`] - Simple linear price growth
//! - [`BancorBondingCurve`] - Constant reserve ratio with power approximation
//! - [`SigmoidBondingCurve`] - S-curve for price stabilization
//! - [`ExponentialBondingCurve`] - Exponential growth
//! - [`AdaptiveBondingCurve`] - Phase-based curve switching
//! - [`SquareRootBondingCurve`] - Square root curve for gentler growth
//! - [`LogarithmicBondingCurve`] - Logarithmic curve with diminishing growth
//!
//! All curves implement the [`BondingCurve`] trait which provides:
//! - `buy_price()` - Calculate total cost to buy tokens
//! - `sell_price()` - Calculate total proceeds from selling tokens
//! - `current_price()` - Get current spot price
//! - `market_cap()` - Calculate total market capitalization
//! - `price_impact()` - Estimate price impact of a trade
//!
//! # Fee System
//!
//! The [`fees`] module provides:
//! - Platform fee calculation with reputation discounts
//! - Volume-based fee tiers
//! - Maker/taker fee differentiation
//! - Fee distribution to stakers
//!
//! # Analytics
//!
//! The [`analytics`] module offers:
//! - Market data aggregation (OHLCV, VWAP, TWAP)
//! - Trading statistics and volume tracking
//! - Price change and volatility metrics
//!
//! # Price Oracles
//!
//! The [`oracle`] module provides:
//! - External price data integration (BTC/USD, etc.)
//! - Multi-source price aggregation
//! - Price staleness detection
//! - Spread and deviation analysis
//!
//! # Example
//!
//! ```rust,no_run
//! use kaccy_core::pricing::{LinearBondingCurve, BondingCurve};
//! use rust_decimal_macros::dec;
//!
//! // Create a linear curve with slope 0.001 and initial price 1.0
//! let curve = LinearBondingCurve::new(dec!(0.001), dec!(1.0));
//!
//! // Calculate price to buy 100 tokens when supply is 1000
//! let buy_cost = curve.buy_price(dec!(1000), dec!(100));
//! println!("Cost to buy: {}", buy_cost);
//!
//! // Calculate proceeds from selling 50 tokens
//! let sell_proceeds = curve.sell_price(dec!(1100), dec!(50));
//! println!("Proceeds from sell: {}", sell_proceeds);
//! ```

pub mod analytics;
pub mod bonding_curve;
pub mod buyback_automation;
pub mod curve_optimization;
pub mod dynamic_fees;
pub mod dynamic_supply;
pub mod fee_optimization;
pub mod fees;
pub mod liquidity_mining;
pub mod oracle;
pub mod simd_pricing;
pub mod staking;

pub use analytics::*;
pub use bonding_curve::*;
pub use buyback_automation::*;
pub use curve_optimization::*;
pub use dynamic_fees::*;
pub use dynamic_supply::*;
pub use fee_optimization::{
    AdjustmentDirection, EligibilityCriterion, FeeConfiguration, FeeOptimizer, FeePerformance,
    GasOptimizer, GasPrice, OptimizationStrategy, PendingTransaction, RebateManager, RebateProgram,
    RebateType, TransactionPriority, UserRebate,
};
pub use fees::*;
pub use liquidity_mining::*;
pub use oracle::*;
pub use simd_pricing::{F64SimdCalculator, ParallelPriceCalculator, SimdPriceCalculator};
pub use staking::*;
