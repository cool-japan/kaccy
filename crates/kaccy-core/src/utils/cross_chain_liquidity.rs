//! Cross-Chain Liquidity Aggregation
//!
//! Provides comprehensive cross-chain liquidity aggregation:
//! - Multi-chain pool discovery
//! - Optimal route calculation with gas cost optimization
//! - Slippage minimization
//! - Best execution across multiple chains and pools

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

/// Re-export Chain from atomic_swaps
pub use super::atomic_swaps::Chain;

/// Liquidity pool on a specific chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityPool {
    /// Pool ID
    pub id: String,
    /// Chain this pool is on
    pub chain: Chain,
    /// DEX/protocol name
    pub protocol: String,
    /// Token A
    pub token_a: String,
    /// Token B
    pub token_b: String,
    /// Reserve A
    pub reserve_a: Decimal,
    /// Reserve B
    pub reserve_b: Decimal,
    /// Pool fee (as percentage, e.g., 0.003 for 0.3%)
    pub fee: Decimal,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

impl LiquidityPool {
    /// Calculate output amount for a swap (constant product formula)
    pub fn calculate_output(&self, input_token: &str, input_amount: Decimal) -> Option<Decimal> {
        let (reserve_in, reserve_out) = if input_token == self.token_a {
            (self.reserve_a, self.reserve_b)
        } else if input_token == self.token_b {
            (self.reserve_b, self.reserve_a)
        } else {
            return None;
        };

        if reserve_in == Decimal::ZERO || reserve_out == Decimal::ZERO {
            return None;
        }

        // x * y = k formula with fees
        // output = (reserve_out * input_amount * (1 - fee)) / (reserve_in + input_amount * (1 - fee))
        let fee_multiplier = Decimal::ONE - self.fee;
        let amount_with_fee = input_amount * fee_multiplier;
        let numerator = reserve_out * amount_with_fee;
        let denominator = reserve_in + amount_with_fee;

        if denominator == Decimal::ZERO {
            return None;
        }

        Some(numerator / denominator)
    }

    /// Calculate price impact for a swap
    pub fn calculate_price_impact(
        &self,
        input_token: &str,
        input_amount: Decimal,
    ) -> Option<Decimal> {
        let output = self.calculate_output(input_token, input_amount)?;

        let (reserve_in, reserve_out) = if input_token == self.token_a {
            (self.reserve_a, self.reserve_b)
        } else {
            (self.reserve_b, self.reserve_a)
        };

        // Spot price before swap
        let spot_price_before = reserve_out / reserve_in;

        // Effective price after swap
        let effective_price = output / input_amount;

        // Price impact = (spot_price - effective_price) / spot_price
        let impact = (spot_price_before - effective_price) / spot_price_before;

        Some(impact)
    }
}

/// Cross-chain bridge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bridge {
    /// Bridge ID
    pub id: String,
    /// Source chain
    pub from_chain: Chain,
    /// Destination chain
    pub to_chain: Chain,
    /// Supported tokens
    pub supported_tokens: Vec<String>,
    /// Bridge fee (as percentage)
    pub fee: Decimal,
    /// Estimated time (in seconds)
    pub estimated_time_seconds: u64,
    /// Gas cost (in native token of source chain)
    pub gas_cost: Decimal,
}

impl Bridge {
    /// Calculate output after bridge fees
    pub fn calculate_output(&self, amount: Decimal) -> Decimal {
        amount * (Decimal::ONE - self.fee)
    }

    /// Check if token is supported
    pub fn supports_token(&self, token: &str) -> bool {
        self.supported_tokens.iter().any(|t| t == token)
    }
}

/// Swap route segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteSegment {
    /// Swap within a pool
    Swap {
        pool: LiquidityPool,
        input_token: String,
        output_token: String,
        input_amount: Decimal,
        output_amount: Decimal,
    },
    /// Bridge between chains
    Bridge {
        bridge: Bridge,
        token: String,
        input_amount: Decimal,
        output_amount: Decimal,
    },
}

impl RouteSegment {
    /// Get output amount
    pub fn output_amount(&self) -> Decimal {
        match self {
            RouteSegment::Swap { output_amount, .. } => *output_amount,
            RouteSegment::Bridge { output_amount, .. } => *output_amount,
        }
    }

    /// Get gas cost (in USD equivalent)
    pub fn gas_cost(&self) -> Decimal {
        match self {
            RouteSegment::Swap { .. } => Decimal::from(5), // Assume $5 per swap
            RouteSegment::Bridge { bridge, .. } => bridge.gas_cost,
        }
    }
}

/// Complete route from input token to output token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Route segments
    pub segments: Vec<RouteSegment>,
    /// Input token
    pub input_token: String,
    /// Output token
    pub output_token: String,
    /// Input amount
    pub input_amount: Decimal,
    /// Final output amount
    pub output_amount: Decimal,
    /// Total gas cost
    pub total_gas_cost: Decimal,
    /// Price impact
    pub price_impact: Decimal,
    /// Expected execution time (seconds)
    pub execution_time_seconds: u64,
}

impl Route {
    /// Calculate effective rate (output / input after costs)
    pub fn effective_rate(&self) -> Decimal {
        if self.input_amount == Decimal::ZERO {
            return Decimal::ZERO;
        }
        (self.output_amount - self.total_gas_cost) / self.input_amount
    }

    /// Calculate net output (after gas costs)
    pub fn net_output(&self) -> Decimal {
        self.output_amount - self.total_gas_cost
    }
}

// Implement comparison for priority queue (higher effective rate = higher priority)
impl PartialEq for Route {
    fn eq(&self, other: &Self) -> bool {
        self.effective_rate() == other.effective_rate()
    }
}

impl Eq for Route {}

impl PartialOrd for Route {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Route {
    fn cmp(&self, other: &Self) -> Ordering {
        self.effective_rate().cmp(&other.effective_rate())
    }
}

/// Cross-chain liquidity aggregator
pub struct LiquidityAggregator {
    /// Available pools
    pools: Vec<LiquidityPool>,
    /// Available bridges
    bridges: Vec<Bridge>,
    /// Maximum route depth (number of hops)
    max_depth: usize,
}

impl LiquidityAggregator {
    /// Create a new liquidity aggregator
    pub fn new(max_depth: usize) -> Self {
        Self {
            pools: Vec::new(),
            bridges: Vec::new(),
            max_depth,
        }
    }

    /// Add a liquidity pool
    pub fn add_pool(&mut self, pool: LiquidityPool) {
        self.pools.push(pool);
    }

    /// Add a bridge
    pub fn add_bridge(&mut self, bridge: Bridge) {
        self.bridges.push(bridge);
    }

    /// Discover pools for a token pair on a specific chain
    pub fn discover_pools(
        &self,
        chain: Chain,
        token_a: &str,
        token_b: &str,
    ) -> Vec<&LiquidityPool> {
        self.pools
            .iter()
            .filter(|pool| {
                pool.chain == chain
                    && ((pool.token_a == token_a && pool.token_b == token_b)
                        || (pool.token_a == token_b && pool.token_b == token_a))
            })
            .collect()
    }

    /// Find all pools containing a specific token on a chain
    pub fn find_pools_with_token(&self, chain: Chain, token: &str) -> Vec<&LiquidityPool> {
        self.pools
            .iter()
            .filter(|pool| pool.chain == chain && (pool.token_a == token || pool.token_b == token))
            .collect()
    }

    /// Find bridges between two chains
    pub fn find_bridges(&self, from_chain: Chain, to_chain: Chain, token: &str) -> Vec<&Bridge> {
        self.bridges
            .iter()
            .filter(|bridge| {
                bridge.from_chain == from_chain
                    && bridge.to_chain == to_chain
                    && bridge.supports_token(token)
            })
            .collect()
    }

    /// Find optimal route using Dijkstra's algorithm variant
    pub fn find_optimal_route(
        &self,
        input_chain: Chain,
        input_token: &str,
        output_chain: Chain,
        output_token: &str,
        input_amount: Decimal,
        max_slippage: Decimal,
    ) -> Option<Route> {
        // State: (chain, token, amount, route_so_far, gas_cost, depth)
        let mut heap = BinaryHeap::new();
        let mut visited = HashSet::new();

        // Start state
        let initial_state = RouteState {
            chain: input_chain,
            token: input_token.to_string(),
            amount: input_amount,
            segments: Vec::new(),
            total_gas_cost: Decimal::ZERO,
            depth: 0,
        };

        heap.push(initial_state);

        while let Some(state) = heap.pop() {
            // Check if we reached the destination
            if state.chain == output_chain && state.token == output_token {
                return Some(self.build_route(state, input_token, output_token, input_amount));
            }

            // Skip if too deep
            if state.depth >= self.max_depth {
                continue;
            }

            // Mark as visited
            let visit_key = (state.chain, state.token.clone(), state.depth);
            if visited.contains(&visit_key) {
                continue;
            }
            visited.insert(visit_key);

            // Explore swaps on current chain
            for pool in self.find_pools_with_token(state.chain, &state.token) {
                let output_token = if pool.token_a == state.token {
                    &pool.token_b
                } else {
                    &pool.token_a
                };

                if let Some(output_amount) = pool.calculate_output(&state.token, state.amount) {
                    // Check slippage
                    if let Some(impact) = pool.calculate_price_impact(&state.token, state.amount) {
                        if impact > max_slippage {
                            continue;
                        }
                    }

                    let mut new_segments = state.segments.clone();
                    new_segments.push(RouteSegment::Swap {
                        pool: pool.clone(),
                        input_token: state.token.clone(),
                        output_token: output_token.clone(),
                        input_amount: state.amount,
                        output_amount,
                    });

                    let new_state = RouteState {
                        chain: state.chain,
                        token: output_token.clone(),
                        amount: output_amount,
                        segments: new_segments,
                        total_gas_cost: state.total_gas_cost + Decimal::from(5),
                        depth: state.depth + 1,
                    };

                    heap.push(new_state);
                }
            }

            // Explore bridges to other chains
            for target_chain in &[
                Chain::Bitcoin,
                Chain::Ethereum,
                Chain::BinanceSmartChain,
                Chain::Polygon,
            ] {
                if *target_chain == state.chain {
                    continue;
                }

                for bridge in self.find_bridges(state.chain, *target_chain, &state.token) {
                    let output_amount = bridge.calculate_output(state.amount);

                    let mut new_segments = state.segments.clone();
                    new_segments.push(RouteSegment::Bridge {
                        bridge: bridge.clone(),
                        token: state.token.clone(),
                        input_amount: state.amount,
                        output_amount,
                    });

                    let new_state = RouteState {
                        chain: *target_chain,
                        token: state.token.clone(),
                        amount: output_amount,
                        segments: new_segments,
                        total_gas_cost: state.total_gas_cost + bridge.gas_cost,
                        depth: state.depth + 1,
                    };

                    heap.push(new_state);
                }
            }
        }

        None
    }

    /// Build final route from state
    fn build_route(
        &self,
        state: RouteState,
        input_token: &str,
        output_token: &str,
        input_amount: Decimal,
    ) -> Route {
        let mut total_gas_cost = Decimal::ZERO;
        let mut execution_time = 0u64;
        let mut total_impact = Decimal::ZERO;

        for segment in &state.segments {
            total_gas_cost += segment.gas_cost();
            match segment {
                RouteSegment::Swap {
                    pool,
                    input_token,
                    input_amount,
                    ..
                } => {
                    execution_time += 15; // Assume 15 seconds per swap
                    if let Some(impact) = pool.calculate_price_impact(input_token, *input_amount) {
                        total_impact += impact;
                    }
                }
                RouteSegment::Bridge { bridge, .. } => {
                    execution_time += bridge.estimated_time_seconds;
                    total_impact += bridge.fee; // Bridge fee contributes to price impact
                }
            }
        }

        Route {
            segments: state.segments,
            input_token: input_token.to_string(),
            output_token: output_token.to_string(),
            input_amount,
            output_amount: state.amount,
            total_gas_cost,
            price_impact: total_impact,
            execution_time_seconds: execution_time,
        }
    }

    /// Get liquidity depth for a token pair
    pub fn get_liquidity_depth(&self, chain: Chain, token_a: &str, token_b: &str) -> Decimal {
        self.discover_pools(chain, token_a, token_b)
            .iter()
            .map(|pool| {
                if pool.token_a == token_a {
                    pool.reserve_a
                } else {
                    pool.reserve_b
                }
            })
            .sum()
    }
}

/// Internal routing state for Dijkstra's algorithm
#[derive(Clone)]
struct RouteState {
    chain: Chain,
    token: String,
    amount: Decimal,
    segments: Vec<RouteSegment>,
    total_gas_cost: Decimal,
    depth: usize,
}

// Implement comparison for priority queue (higher amount = higher priority)
impl PartialEq for RouteState {
    fn eq(&self, other: &Self) -> bool {
        self.amount == other.amount
    }
}

impl Eq for RouteState {}

impl PartialOrd for RouteState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RouteState {
    fn cmp(&self, other: &Self) -> Ordering {
        self.amount.cmp(&other.amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pool(
        id: &str,
        chain: Chain,
        token_a: &str,
        token_b: &str,
        reserve_a: u64,
        reserve_b: u64,
    ) -> LiquidityPool {
        LiquidityPool {
            id: id.to_string(),
            chain,
            protocol: "UniswapV2".to_string(),
            token_a: token_a.to_string(),
            token_b: token_b.to_string(),
            reserve_a: Decimal::from(reserve_a),
            reserve_b: Decimal::from(reserve_b),
            fee: Decimal::new(3, 3), // 0.3%
            last_updated: Utc::now(),
        }
    }

    #[test]
    fn test_pool_output_calculation() {
        let pool = create_test_pool("pool1", Chain::Ethereum, "ETH", "USDT", 1000, 2000000);

        // Swap 10 ETH for USDT
        let output = pool.calculate_output("ETH", Decimal::from(10)).unwrap();

        // With 0.3% fee: output = (2000000 * 10 * 0.997) / (1000 + 10 * 0.997)
        // output ≈ 19742 USDT
        assert!(output > Decimal::from(19700) && output < Decimal::from(19800));
    }

    #[test]
    fn test_price_impact_calculation() {
        let pool = create_test_pool("pool1", Chain::Ethereum, "ETH", "USDT", 1000, 2000000);

        let impact = pool
            .calculate_price_impact("ETH", Decimal::from(10))
            .unwrap();

        // Should have some price impact for 1% of pool
        assert!(impact > Decimal::ZERO);
        assert!(impact < Decimal::new(1, 1)); // Less than 10%
    }

    #[test]
    fn test_pool_discovery() {
        let mut aggregator = LiquidityAggregator::new(3);
        aggregator.add_pool(create_test_pool(
            "pool1",
            Chain::Ethereum,
            "ETH",
            "USDT",
            1000,
            2000000,
        ));
        aggregator.add_pool(create_test_pool(
            "pool2",
            Chain::Ethereum,
            "ETH",
            "DAI",
            500,
            1000000,
        ));
        aggregator.add_pool(create_test_pool(
            "pool3",
            Chain::Polygon,
            "ETH",
            "USDT",
            300,
            600000,
        ));

        let pools = aggregator.discover_pools(Chain::Ethereum, "ETH", "USDT");
        assert_eq!(pools.len(), 1);
        assert_eq!(pools[0].id, "pool1");
    }

    #[test]
    fn test_find_pools_with_token() {
        let mut aggregator = LiquidityAggregator::new(3);
        aggregator.add_pool(create_test_pool(
            "pool1",
            Chain::Ethereum,
            "ETH",
            "USDT",
            1000,
            2000000,
        ));
        aggregator.add_pool(create_test_pool(
            "pool2",
            Chain::Ethereum,
            "ETH",
            "DAI",
            500,
            1000000,
        ));
        aggregator.add_pool(create_test_pool(
            "pool3",
            Chain::Ethereum,
            "BTC",
            "USDT",
            10,
            200000,
        ));

        let pools = aggregator.find_pools_with_token(Chain::Ethereum, "ETH");
        assert_eq!(pools.len(), 2);
    }

    #[test]
    fn test_bridge_output_calculation() {
        let bridge = Bridge {
            id: "bridge1".to_string(),
            from_chain: Chain::Ethereum,
            to_chain: Chain::Polygon,
            supported_tokens: vec!["USDT".to_string()],
            fee: Decimal::new(5, 3), // 0.5%
            estimated_time_seconds: 300,
            gas_cost: Decimal::from(10),
        };

        let output = bridge.calculate_output(Decimal::from(1000));
        assert_eq!(output, Decimal::from(995)); // 1000 * (1 - 0.005) = 995
    }

    #[test]
    fn test_route_effective_rate() {
        let route = Route {
            segments: vec![],
            input_token: "ETH".to_string(),
            output_token: "USDT".to_string(),
            input_amount: Decimal::from(10),
            output_amount: Decimal::from(20000),
            total_gas_cost: Decimal::from(50),
            price_impact: Decimal::new(5, 2), // 0.05 = 5%
            execution_time_seconds: 60,
        };

        let effective_rate = route.effective_rate();
        // (20000 - 50) / 10 = 1995
        assert_eq!(effective_rate, Decimal::from(1995));
    }

    #[test]
    fn test_liquidity_depth() {
        let mut aggregator = LiquidityAggregator::new(3);
        aggregator.add_pool(create_test_pool(
            "pool1",
            Chain::Ethereum,
            "ETH",
            "USDT",
            1000,
            2000000,
        ));
        aggregator.add_pool(create_test_pool(
            "pool2",
            Chain::Ethereum,
            "ETH",
            "USDT",
            500,
            1000000,
        ));

        let depth = aggregator.get_liquidity_depth(Chain::Ethereum, "ETH", "USDT");
        assert_eq!(depth, Decimal::from(1500)); // 1000 + 500
    }
}
