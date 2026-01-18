//! DeFi primitives and protocols
//!
//! This module provides decentralized finance primitives including:
//! - Lending and borrowing with collateralization
//! - Synthetic assets
//! - Yield farming mechanisms
//! - Margin trading
//! - Collateral management
//! - Distributed consensus mechanisms
//! - Peer-to-peer trading features
//! - Liquid staking
//! - Just-in-time liquidity provisioning
//! - Perpetual futures/contracts with funding rates
//! - Concentrated liquidity (Uniswap V3 style) pools
//! - Cross-margining for capital efficiency
//! - Intent-based trading with competitive solvers

pub mod auto_compounding;
pub mod collateral;
pub mod concentrated_liquidity;
pub mod cross_margin;
pub mod distributed_consensus;
pub mod hooks;
pub mod intent_trading;
pub mod jit_liquidity;
pub mod lending;
pub mod liquid_staking;
pub mod margin;
pub mod p2p;
pub mod perpetual;
pub mod synthetic;
pub mod yield_farming;

pub use auto_compounding::*;
pub use collateral::*;
pub use concentrated_liquidity::*;
pub use cross_margin::*;
pub use distributed_consensus::*;
pub use hooks::*;
pub use intent_trading::*;
pub use jit_liquidity::*;
pub use lending::*;
pub use liquid_staking::*;
pub use margin::*;
pub use p2p::*;
// Selective exports from perpetual to avoid conflicts
pub use perpetual::{FundingRate, InsuranceFund, MarginMode, PerpetualManager, PerpetualPosition};
pub use synthetic::*;
pub use yield_farming::*;
