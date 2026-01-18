// Cross-chain bridge integration for asset transfers
use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Supported blockchain networks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Chain {
    Bitcoin,
    Ethereum,
    Polygon,
    Arbitrum,
    Optimism,
    Base,
    Stacks,
}

impl Chain {
    /// Get the chain ID for the network
    pub fn chain_id(&self) -> u64 {
        match self {
            Chain::Bitcoin => 0,
            Chain::Ethereum => 1,
            Chain::Polygon => 137,
            Chain::Arbitrum => 42161,
            Chain::Optimism => 10,
            Chain::Base => 8453,
            Chain::Stacks => 1,
        }
    }

    /// Get the native currency symbol
    pub fn native_currency(&self) -> &str {
        match self {
            Chain::Bitcoin => "BTC",
            Chain::Ethereum => "ETH",
            Chain::Polygon => "MATIC",
            Chain::Arbitrum => "ETH",
            Chain::Optimism => "ETH",
            Chain::Base => "ETH",
            Chain::Stacks => "STX",
        }
    }
}

/// Bridge provider abstraction
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BridgeProvider {
    Native,     // Our native bridge
    LayerZero,  // LayerZero omnichain protocol
    Wormhole,   // Wormhole bridge
    Stargate,   // Stargate Finance
    Axelar,     // Axelar Network
    Multichain, // Multichain (formerly Anyswap)
}

impl BridgeProvider {
    /// Get supported chains for this provider
    pub fn supported_chains(&self) -> Vec<Chain> {
        match self {
            BridgeProvider::Native => vec![Chain::Bitcoin, Chain::Stacks],
            BridgeProvider::LayerZero => vec![
                Chain::Ethereum,
                Chain::Polygon,
                Chain::Arbitrum,
                Chain::Optimism,
                Chain::Base,
            ],
            BridgeProvider::Wormhole => vec![
                Chain::Ethereum,
                Chain::Polygon,
                Chain::Arbitrum,
                Chain::Optimism,
            ],
            BridgeProvider::Stargate => vec![
                Chain::Ethereum,
                Chain::Polygon,
                Chain::Arbitrum,
                Chain::Optimism,
            ],
            BridgeProvider::Axelar => vec![Chain::Ethereum, Chain::Polygon, Chain::Arbitrum],
            BridgeProvider::Multichain => vec![Chain::Ethereum, Chain::Polygon, Chain::Arbitrum],
        }
    }

    /// Get estimated bridge fee as percentage
    pub fn fee_percentage(&self) -> Decimal {
        match self {
            BridgeProvider::Native => Decimal::new(10, 3), // 0.01% (0.001)
            BridgeProvider::LayerZero => Decimal::new(5, 3), // 0.005%
            BridgeProvider::Wormhole => Decimal::new(15, 3), // 0.015%
            BridgeProvider::Stargate => Decimal::new(6, 3), // 0.006%
            BridgeProvider::Axelar => Decimal::new(20, 3), // 0.020%
            BridgeProvider::Multichain => Decimal::new(10, 3), // 0.010%
        }
    }
}

/// Status of a bridge transfer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeTransferStatus {
    Pending,
    Confirming,
    InProgress,
    Completed,
    Failed,
    Refunded,
}

/// Bridge transfer record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTransfer {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_id: Uuid,
    pub source_chain: Chain,
    pub destination_chain: Chain,
    pub provider: BridgeProvider,
    pub amount: Decimal,
    pub fee: Decimal,
    pub status: BridgeTransferStatus,
    pub source_tx_hash: Option<String>,
    pub destination_tx_hash: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl BridgeTransfer {
    /// Create a new bridge transfer
    pub fn new(
        user_id: Uuid,
        token_id: Uuid,
        source_chain: Chain,
        destination_chain: Chain,
        provider: BridgeProvider,
        amount: Decimal,
    ) -> Result<Self, CoreError> {
        if amount <= Decimal::ZERO {
            return Err(CoreError::InvalidAmount);
        }

        // Validate chains are supported by provider
        let supported = provider.supported_chains();
        if !supported.contains(&source_chain) || !supported.contains(&destination_chain) {
            return Err(CoreError::InvalidBridgeRoute);
        }

        if source_chain == destination_chain {
            return Err(CoreError::InvalidBridgeRoute);
        }

        let fee = amount * provider.fee_percentage();
        let now = chrono::Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            user_id,
            token_id,
            source_chain,
            destination_chain,
            provider,
            amount,
            fee,
            status: BridgeTransferStatus::Pending,
            source_tx_hash: None,
            destination_tx_hash: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        })
    }

    /// Get the net amount after fees
    pub fn net_amount(&self) -> Decimal {
        self.amount - self.fee
    }

    /// Update transfer status
    pub fn update_status(&mut self, status: BridgeTransferStatus) {
        self.status = status.clone();
        self.updated_at = chrono::Utc::now();

        if status == BridgeTransferStatus::Completed {
            self.completed_at = Some(chrono::Utc::now());
        }
    }
}

/// Multi-chain token tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiChainToken {
    pub token_id: Uuid,
    pub symbol: String,
    pub chain_addresses: HashMap<Chain, String>,
    pub total_supply_by_chain: HashMap<Chain, Decimal>,
}

impl MultiChainToken {
    /// Create a new multi-chain token
    pub fn new(token_id: Uuid, symbol: String) -> Self {
        Self {
            token_id,
            symbol,
            chain_addresses: HashMap::new(),
            total_supply_by_chain: HashMap::new(),
        }
    }

    /// Add a chain deployment
    pub fn add_chain(&mut self, chain: Chain, address: String, initial_supply: Decimal) {
        self.chain_addresses.insert(chain, address);
        self.total_supply_by_chain.insert(chain, initial_supply);
    }

    /// Get total supply across all chains
    pub fn total_supply(&self) -> Decimal {
        self.total_supply_by_chain.values().sum()
    }

    /// Update supply on a specific chain
    pub fn update_chain_supply(&mut self, chain: Chain, new_supply: Decimal) {
        self.total_supply_by_chain.insert(chain, new_supply);
    }

    /// Get chains where token is deployed
    pub fn deployed_chains(&self) -> Vec<Chain> {
        self.chain_addresses.keys().copied().collect()
    }
}

/// Cross-chain arbitrage opportunity detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainArbitrage {
    pub token_id: Uuid,
    pub buy_chain: Chain,
    pub sell_chain: Chain,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub profit_percentage: Decimal,
    pub bridge_provider: BridgeProvider,
    pub estimated_profit: Decimal,
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

impl CrossChainArbitrage {
    /// Detect arbitrage opportunity
    pub fn detect(
        token_id: Uuid,
        chain_prices: HashMap<Chain, Decimal>,
        bridge_provider: BridgeProvider,
        amount: Decimal,
    ) -> Option<Self> {
        let mut opportunities = Vec::new();

        // Find all pairs of chains with price differences
        let chains: Vec<_> = chain_prices.keys().copied().collect();
        for i in 0..chains.len() {
            for j in (i + 1)..chains.len() {
                let chain_a = chains[i];
                let chain_b = chains[j];
                let price_a = chain_prices[&chain_a];
                let price_b = chain_prices[&chain_b];

                // Check if both chains are supported by the provider
                let supported = bridge_provider.supported_chains();
                if !supported.contains(&chain_a) || !supported.contains(&chain_b) {
                    continue;
                }

                // Calculate profit considering bridge fees
                let bridge_fee_pct = bridge_provider.fee_percentage();
                let bridge_fee = amount * bridge_fee_pct;

                if price_a < price_b {
                    // Buy on chain A, sell on chain B
                    let buy_cost = amount * price_a;
                    let sell_revenue = (amount - bridge_fee) * price_b;
                    let profit = sell_revenue - buy_cost;
                    let profit_pct = (profit / buy_cost) * Decimal::from(100);

                    if profit > Decimal::ZERO {
                        opportunities.push(CrossChainArbitrage {
                            token_id,
                            buy_chain: chain_a,
                            sell_chain: chain_b,
                            buy_price: price_a,
                            sell_price: price_b,
                            profit_percentage: profit_pct,
                            bridge_provider: bridge_provider.clone(),
                            estimated_profit: profit,
                            detected_at: chrono::Utc::now(),
                        });
                    }
                } else if price_b < price_a {
                    // Buy on chain B, sell on chain A
                    let buy_cost = amount * price_b;
                    let sell_revenue = (amount - bridge_fee) * price_a;
                    let profit = sell_revenue - buy_cost;
                    let profit_pct = (profit / buy_cost) * Decimal::from(100);

                    if profit > Decimal::ZERO {
                        opportunities.push(CrossChainArbitrage {
                            token_id,
                            buy_chain: chain_b,
                            sell_chain: chain_a,
                            buy_price: price_b,
                            sell_price: price_a,
                            profit_percentage: profit_pct,
                            bridge_provider: bridge_provider.clone(),
                            estimated_profit: profit,
                            detected_at: chrono::Utc::now(),
                        });
                    }
                }
            }
        }

        // Return the most profitable opportunity
        opportunities
            .into_iter()
            .max_by(|a, b| a.estimated_profit.cmp(&b.estimated_profit))
    }

    /// Check if opportunity is still profitable after time decay
    pub fn is_still_profitable(&self, min_profit_percentage: Decimal) -> bool {
        self.profit_percentage >= min_profit_percentage
    }
}

/// Multi-chain portfolio view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiChainPortfolio {
    pub user_id: Uuid,
    pub balances_by_chain: HashMap<Chain, HashMap<Uuid, Decimal>>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl MultiChainPortfolio {
    /// Create a new multi-chain portfolio
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            balances_by_chain: HashMap::new(),
            last_updated: chrono::Utc::now(),
        }
    }

    /// Add balance on a specific chain
    pub fn add_balance(&mut self, chain: Chain, token_id: Uuid, amount: Decimal) {
        self.balances_by_chain
            .entry(chain)
            .or_default()
            .insert(token_id, amount);
        self.last_updated = chrono::Utc::now();
    }

    /// Get balance for a token across all chains
    pub fn total_balance(&self, token_id: Uuid) -> Decimal {
        self.balances_by_chain
            .values()
            .filter_map(|balances| balances.get(&token_id))
            .sum()
    }

    /// Get balance for a token on a specific chain
    pub fn balance_on_chain(&self, chain: Chain, token_id: Uuid) -> Decimal {
        self.balances_by_chain
            .get(&chain)
            .and_then(|balances| balances.get(&token_id))
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Get all chains where user has balances
    pub fn active_chains(&self) -> Vec<Chain> {
        self.balances_by_chain
            .iter()
            .filter(|(_, balances)| !balances.is_empty())
            .map(|(chain, _)| *chain)
            .collect()
    }

    /// Get total number of unique tokens across all chains
    pub fn unique_tokens(&self) -> usize {
        let mut tokens = std::collections::HashSet::<&Uuid>::new();
        for balances in self.balances_by_chain.values() {
            tokens.extend(balances.keys());
        }
        tokens.len()
    }

    /// Calculate total portfolio value (requires price data)
    pub fn total_value(&self, prices: &HashMap<(Chain, Uuid), Decimal>) -> Decimal {
        let mut total = Decimal::ZERO;
        for (chain, balances) in &self.balances_by_chain {
            for (token_id, amount) in balances {
                if let Some(price) = prices.get(&(*chain, *token_id)) {
                    total += amount * price;
                }
            }
        }
        total
    }
}

/// Cross-chain risk analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainRiskMetrics {
    pub portfolio_id: Uuid,
    pub chain_concentration: HashMap<Chain, Decimal>, // % of portfolio on each chain
    pub bridge_risk_score: Decimal,                   // 0-100, higher = riskier
    pub chain_diversification_score: Decimal,         // 0-100, higher = better
    pub recommended_rebalance: Vec<RebalanceAction>,
}

/// Recommended rebalance action for cross-chain portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceAction {
    pub token_id: Uuid,
    pub from_chain: Chain,
    pub to_chain: Chain,
    pub amount: Decimal,
    pub reason: String,
}

impl CrossChainRiskMetrics {
    /// Calculate risk metrics for a multi-chain portfolio
    pub fn calculate(
        portfolio: &MultiChainPortfolio,
        prices: &HashMap<(Chain, Uuid), Decimal>,
    ) -> Self {
        let total_value = portfolio.total_value(prices);
        let mut chain_concentration = HashMap::new();

        // Calculate chain concentration
        for (chain, balances) in &portfolio.balances_by_chain {
            let mut chain_value = Decimal::ZERO;
            for (token_id, amount) in balances {
                if let Some(price) = prices.get(&(*chain, *token_id)) {
                    chain_value += amount * price;
                }
            }
            if total_value > Decimal::ZERO {
                let concentration = (chain_value / total_value) * Decimal::from(100);
                chain_concentration.insert(*chain, concentration);
            }
        }

        // Calculate bridge risk score (higher concentration = higher risk)
        let max_concentration = chain_concentration
            .values()
            .max()
            .copied()
            .unwrap_or(Decimal::ZERO);
        let bridge_risk_score = max_concentration; // Simple: max concentration = risk

        // Calculate diversification score
        let num_chains = portfolio.active_chains().len();
        let ideal_concentration = Decimal::from(100) / Decimal::from(num_chains.max(1));
        let mut variance = Decimal::ZERO;
        for concentration in chain_concentration.values() {
            let diff = concentration - ideal_concentration;
            variance += diff * diff;
        }
        // Use standard deviation (sqrt of variance) and scale to 0-100
        // Lower deviation = higher score
        let std_dev = if variance > Decimal::ZERO {
            // Approximate sqrt for demonstration (use proper sqrt in production)
            let v = variance / Decimal::from(num_chains.max(1));
            v / Decimal::from(10) // Normalize to roughly 0-10 range
        } else {
            Decimal::ZERO
        };
        let diversification_score =
            (Decimal::from(100) - std_dev.min(Decimal::from(100))).max(Decimal::ZERO);

        // Generate rebalance recommendations if needed
        let mut recommended_rebalance = Vec::new();
        if max_concentration > Decimal::from(60) {
            // If any chain has > 60% concentration, recommend rebalancing
            // This is a simplified version - real implementation would be more sophisticated
            for (chain, concentration) in &chain_concentration {
                if concentration > &Decimal::from(60) {
                    recommended_rebalance.push(RebalanceAction {
                        token_id: Uuid::new_v4(), // Would need actual token data
                        from_chain: *chain,
                        to_chain: Chain::Bitcoin, // Would calculate optimal target
                        amount: Decimal::from(1000), // Would calculate optimal amount
                        reason: format!("High concentration on {:?} ({}%)", chain, concentration),
                    });
                }
            }
        }

        Self {
            portfolio_id: portfolio.user_id,
            chain_concentration,
            bridge_risk_score,
            chain_diversification_score: diversification_score,
            recommended_rebalance,
        }
    }
}

/// Chain-specific fee optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainFeeOptimizer {
    pub chain: Chain,
    pub current_gas_price: Decimal,
    pub recommended_gas_price: Decimal,
    pub estimated_cost: Decimal,
    pub optimal_execution_time: chrono::DateTime<chrono::Utc>,
}

impl ChainFeeOptimizer {
    /// Optimize transaction fees for a specific chain
    pub fn optimize(chain: Chain, urgency: TransactionUrgency) -> Self {
        let (current_gas, recommended_gas, delay_minutes) = match urgency {
            TransactionUrgency::Immediate => (Decimal::from(50), Decimal::from(50), 0),
            TransactionUrgency::Normal => (Decimal::from(50), Decimal::from(30), 5),
            TransactionUrgency::Low => (Decimal::from(50), Decimal::from(20), 15),
        };

        let estimated_cost = recommended_gas * Decimal::new(21000, 0); // Base gas limit
        let optimal_time = chrono::Utc::now() + chrono::Duration::minutes(delay_minutes);

        Self {
            chain,
            current_gas_price: current_gas,
            recommended_gas_price: recommended_gas,
            estimated_cost,
            optimal_execution_time: optimal_time,
        }
    }

    /// Calculate potential savings
    pub fn potential_savings(&self) -> Decimal {
        (self.current_gas_price - self.recommended_gas_price) * Decimal::new(21000, 0)
    }
}

/// Transaction urgency level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionUrgency {
    Immediate,
    Normal,
    Low,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_info() {
        assert_eq!(Chain::Ethereum.chain_id(), 1);
        assert_eq!(Chain::Ethereum.native_currency(), "ETH");
        assert_eq!(Chain::Bitcoin.chain_id(), 0);
        assert_eq!(Chain::Bitcoin.native_currency(), "BTC");
    }

    #[test]
    fn test_bridge_provider_supported_chains() {
        let native = BridgeProvider::Native;
        assert!(native.supported_chains().contains(&Chain::Bitcoin));

        let layerzero = BridgeProvider::LayerZero;
        assert!(layerzero.supported_chains().contains(&Chain::Ethereum));
    }

    #[test]
    fn test_bridge_transfer_creation() {
        let transfer = BridgeTransfer::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Chain::Ethereum,
            Chain::Polygon,
            BridgeProvider::LayerZero,
            Decimal::from(100),
        )
        .unwrap();

        assert_eq!(transfer.source_chain, Chain::Ethereum);
        assert_eq!(transfer.destination_chain, Chain::Polygon);
        assert_eq!(transfer.status, BridgeTransferStatus::Pending);
    }

    #[test]
    fn test_bridge_transfer_invalid_amount() {
        let result = BridgeTransfer::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Chain::Ethereum,
            Chain::Polygon,
            BridgeProvider::LayerZero,
            Decimal::ZERO,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_bridge_transfer_same_chain() {
        let result = BridgeTransfer::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Chain::Ethereum,
            Chain::Ethereum,
            BridgeProvider::LayerZero,
            Decimal::from(100),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_multi_chain_token() {
        let mut token = MultiChainToken::new(Uuid::new_v4(), "TEST".to_string());
        token.add_chain(Chain::Ethereum, "0x123".to_string(), Decimal::from(1000));
        token.add_chain(Chain::Polygon, "0x456".to_string(), Decimal::from(2000));

        assert_eq!(token.total_supply(), Decimal::from(3000));
        assert_eq!(token.deployed_chains().len(), 2);
    }

    #[test]
    fn test_cross_chain_arbitrage_detection() {
        let mut prices = HashMap::new();
        prices.insert(Chain::Ethereum, Decimal::from(100));
        prices.insert(Chain::Polygon, Decimal::from(105));

        let arb = CrossChainArbitrage::detect(
            Uuid::new_v4(),
            prices,
            BridgeProvider::LayerZero,
            Decimal::from(10),
        );

        assert!(arb.is_some());
        let arb = arb.unwrap();
        assert_eq!(arb.buy_chain, Chain::Ethereum);
        assert_eq!(arb.sell_chain, Chain::Polygon);
    }

    #[test]
    fn test_multi_chain_portfolio() {
        let mut portfolio = MultiChainPortfolio::new(Uuid::new_v4());
        let token_id = Uuid::new_v4();

        portfolio.add_balance(Chain::Ethereum, token_id, Decimal::from(100));
        portfolio.add_balance(Chain::Polygon, token_id, Decimal::from(200));

        assert_eq!(portfolio.total_balance(token_id), Decimal::from(300));
        assert_eq!(
            portfolio.balance_on_chain(Chain::Ethereum, token_id),
            Decimal::from(100)
        );
        assert_eq!(portfolio.active_chains().len(), 2);
    }

    #[test]
    fn test_cross_chain_risk_metrics() {
        let mut portfolio = MultiChainPortfolio::new(Uuid::new_v4());
        let token_id = Uuid::new_v4();

        portfolio.add_balance(Chain::Ethereum, token_id, Decimal::from(100));
        portfolio.add_balance(Chain::Polygon, token_id, Decimal::from(50));

        let mut prices = HashMap::new();
        prices.insert((Chain::Ethereum, token_id), Decimal::from(10));
        prices.insert((Chain::Polygon, token_id), Decimal::from(10));

        let metrics = CrossChainRiskMetrics::calculate(&portfolio, &prices);
        assert!(metrics.bridge_risk_score > Decimal::ZERO);
        assert!(metrics.chain_diversification_score > Decimal::ZERO);
    }

    #[test]
    fn test_fee_optimizer() {
        let optimizer = ChainFeeOptimizer::optimize(Chain::Ethereum, TransactionUrgency::Normal);
        assert!(optimizer.recommended_gas_price <= optimizer.current_gas_price);
        assert!(optimizer.potential_savings() >= Decimal::ZERO);
    }
}
