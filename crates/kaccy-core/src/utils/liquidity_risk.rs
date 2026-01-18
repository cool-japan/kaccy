//! Liquidity Risk Management
//!
//! This module provides tools for managing liquidity risk, including liquidity coverage
//! ratio (LCR), funding liquidity metrics, market liquidity scores, and stress scenarios.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Liquidity asset category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LiquidityCategory {
    /// Level 1 - High-quality liquid assets (HQLA)
    Level1,
    /// Level 2A - HQLA with 15% haircut
    Level2A,
    /// Level 2B - HQLA with 50% haircut
    Level2B,
    /// Non-HQLA
    NonHQLA,
}

impl LiquidityCategory {
    /// Get the haircut (discount) for this liquidity category
    pub fn haircut(&self) -> Decimal {
        match self {
            LiquidityCategory::Level1 => dec!(0.00),  // 0% haircut
            LiquidityCategory::Level2A => dec!(0.15), // 15% haircut
            LiquidityCategory::Level2B => dec!(0.50), // 50% haircut
            LiquidityCategory::NonHQLA => dec!(1.00), // 100% haircut (not counted)
        }
    }

    /// Check if this is high-quality liquid asset
    pub fn is_hqla(&self) -> bool {
        !matches!(self, LiquidityCategory::NonHQLA)
    }
}

/// Liquid asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidAsset {
    /// Asset ID (token ID)
    pub asset_id: i64,
    /// Asset name
    pub name: String,
    /// Market value
    pub market_value: Decimal,
    /// Liquidity category
    pub category: LiquidityCategory,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl LiquidAsset {
    /// Calculate HQLA value after haircut
    pub fn hqla_value(&self) -> Decimal {
        if self.category.is_hqla() {
            self.market_value * (dec!(1) - self.category.haircut())
        } else {
            dec!(0)
        }
    }
}

/// Cash outflow scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashOutflow {
    /// Outflow category
    pub category: String,
    /// Gross outflow amount
    pub gross_amount: Decimal,
    /// Outflow rate (percentage expected to flow out)
    pub outflow_rate: Decimal,
    /// Time horizon in days
    pub time_horizon_days: u32,
}

impl CashOutflow {
    /// Calculate net outflow
    pub fn net_outflow(&self) -> Decimal {
        self.gross_amount * self.outflow_rate
    }
}

/// Cash inflow scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashInflow {
    /// Inflow category
    pub category: String,
    /// Gross inflow amount
    pub gross_amount: Decimal,
    /// Inflow rate (percentage expected to flow in)
    pub inflow_rate: Decimal,
    /// Time horizon in days
    pub time_horizon_days: u32,
}

impl CashInflow {
    /// Calculate net inflow
    pub fn net_inflow(&self) -> Decimal {
        self.gross_amount * self.inflow_rate
    }
}

/// Liquidity Coverage Ratio (LCR) calculator
pub struct LCRCalculator {
    /// High-quality liquid assets
    liquid_assets: Vec<LiquidAsset>,
    /// Cash outflows
    outflows: Vec<CashOutflow>,
    /// Cash inflows
    inflows: Vec<CashInflow>,
}

impl LCRCalculator {
    /// Create a new LCR calculator
    pub fn new() -> Self {
        Self {
            liquid_assets: Vec::new(),
            outflows: Vec::new(),
            inflows: Vec::new(),
        }
    }

    /// Add liquid asset
    pub fn add_asset(&mut self, asset: LiquidAsset) {
        self.liquid_assets.push(asset);
    }

    /// Add cash outflow
    pub fn add_outflow(&mut self, outflow: CashOutflow) {
        self.outflows.push(outflow);
    }

    /// Add cash inflow
    pub fn add_inflow(&mut self, inflow: CashInflow) {
        self.inflows.push(inflow);
    }

    /// Calculate total HQLA
    pub fn total_hqla(&self) -> Decimal {
        self.liquid_assets.iter().map(|a| a.hqla_value()).sum()
    }

    /// Calculate total net cash outflows over 30 days
    pub fn total_net_outflows_30d(&self) -> Decimal {
        let outflows: Decimal = self
            .outflows
            .iter()
            .filter(|o| o.time_horizon_days <= 30)
            .map(|o| o.net_outflow())
            .sum();

        let inflows: Decimal = self
            .inflows
            .iter()
            .filter(|i| i.time_horizon_days <= 30)
            .map(|i| i.net_inflow())
            .sum();

        // Net outflows = max(outflows - min(inflows, 75% of outflows), 0)
        let capped_inflows = inflows.min(outflows * dec!(0.75));
        (outflows - capped_inflows).max(dec!(0))
    }

    /// Calculate Liquidity Coverage Ratio (LCR)
    ///
    /// LCR = HQLA / Total Net Cash Outflows (30 days)
    ///
    /// Regulatory minimum is typically 100%
    pub fn calculate_lcr(&self) -> Decimal {
        let net_outflows = self.total_net_outflows_30d();
        if net_outflows == dec!(0) {
            return Decimal::MAX; // Infinite LCR if no outflows
        }
        (self.total_hqla() / net_outflows) * dec!(100)
    }

    /// Check if LCR meets regulatory minimum (100%)
    pub fn meets_regulatory_minimum(&self) -> bool {
        self.calculate_lcr() >= dec!(100)
    }
}

impl Default for LCRCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Market liquidity metrics for an asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketLiquidityMetrics {
    /// Asset ID
    pub asset_id: i64,
    /// Bid-ask spread (percentage)
    pub bid_ask_spread: Decimal,
    /// Trading volume (last 24h)
    pub trading_volume_24h: Decimal,
    /// Market depth (total order book depth within 2% of mid price)
    pub market_depth: Decimal,
    /// Price impact for $10k trade (percentage)
    pub price_impact_10k: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl MarketLiquidityMetrics {
    /// Calculate liquidity score (0-100)
    ///
    /// Combines spread, volume, depth, and price impact into a single score
    pub fn liquidity_score(&self) -> Decimal {
        // Spread component (0-25 points): lower spread = higher score
        let spread_score = if self.bid_ask_spread < dec!(0.001) {
            dec!(25)
        } else if self.bid_ask_spread < dec!(0.01) {
            dec!(20)
        } else if self.bid_ask_spread < dec!(0.05) {
            dec!(15)
        } else if self.bid_ask_spread < dec!(0.10) {
            dec!(10)
        } else {
            dec!(5)
        };

        // Volume component (0-25 points)
        let volume_score = if self.trading_volume_24h > dec!(1000000) {
            dec!(25)
        } else if self.trading_volume_24h > dec!(100000) {
            dec!(20)
        } else if self.trading_volume_24h > dec!(10000) {
            dec!(15)
        } else if self.trading_volume_24h > dec!(1000) {
            dec!(10)
        } else {
            dec!(5)
        };

        // Depth component (0-25 points)
        let depth_score = if self.market_depth > dec!(500000) {
            dec!(25)
        } else if self.market_depth > dec!(100000) {
            dec!(20)
        } else if self.market_depth > dec!(50000) {
            dec!(15)
        } else if self.market_depth > dec!(10000) {
            dec!(10)
        } else {
            dec!(5)
        };

        // Price impact component (0-25 points): lower impact = higher score
        let impact_score = if self.price_impact_10k < dec!(0.005) {
            dec!(25)
        } else if self.price_impact_10k < dec!(0.02) {
            dec!(20)
        } else if self.price_impact_10k < dec!(0.05) {
            dec!(15)
        } else if self.price_impact_10k < dec!(0.10) {
            dec!(10)
        } else {
            dec!(5)
        };

        spread_score + volume_score + depth_score + impact_score
    }

    /// Get liquidity category based on score
    pub fn liquidity_category(&self) -> &'static str {
        let score = self.liquidity_score();
        if score >= dec!(80) {
            "Highly Liquid"
        } else if score >= dec!(60) {
            "Liquid"
        } else if score >= dec!(40) {
            "Moderately Liquid"
        } else if score >= dec!(20) {
            "Illiquid"
        } else {
            "Highly Illiquid"
        }
    }
}

/// Funding liquidity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingLiquidityMetrics {
    /// Available funding (uncommitted credit lines, cash)
    pub available_funding: Decimal,
    /// Required funding (short-term obligations)
    pub required_funding: Decimal,
    /// Funding sources count
    pub funding_sources: usize,
    /// Largest funding source (percentage of total)
    pub largest_source_pct: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl FundingLiquidityMetrics {
    /// Calculate funding gap
    pub fn funding_gap(&self) -> Decimal {
        self.required_funding - self.available_funding
    }

    /// Check if funding is adequate
    pub fn is_adequate(&self) -> bool {
        self.available_funding >= self.required_funding
    }

    /// Calculate funding concentration risk (0-100, higher = more concentrated)
    pub fn concentration_risk(&self) -> Decimal {
        self.largest_source_pct * dec!(100)
    }

    /// Calculate funding diversification score (0-100, higher = more diversified)
    pub fn diversification_score(&self) -> Decimal {
        if self.funding_sources == 0 {
            return dec!(0);
        }

        // Base score from number of sources
        let sources_score = if self.funding_sources >= 10 {
            dec!(60)
        } else if self.funding_sources >= 5 {
            dec!(40)
        } else if self.funding_sources >= 3 {
            dec!(20)
        } else {
            dec!(10)
        };

        // Penalty for concentration
        let concentration_penalty = self.largest_source_pct * dec!(40);

        (sources_score + dec!(40) - concentration_penalty).max(dec!(0))
    }
}

/// Liquidity stress scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityStressScenario {
    /// Scenario name
    pub name: String,
    /// Description
    pub description: String,
    /// Haircut increase (additional haircut on assets)
    pub additional_haircut: Decimal,
    /// Outflow multiplier (multiply outflows by this factor)
    pub outflow_multiplier: Decimal,
    /// Inflow reduction (reduce inflows by this percentage)
    pub inflow_reduction: Decimal,
}

impl LiquidityStressScenario {
    /// Create a severe stress scenario
    pub fn severe_stress() -> Self {
        Self {
            name: "Severe Liquidity Stress".to_string(),
            description: "Market-wide liquidity crisis".to_string(),
            additional_haircut: dec!(0.30), // 30% additional haircut
            outflow_multiplier: dec!(2.0),  // 2x normal outflows
            inflow_reduction: dec!(0.50),   // 50% reduction in inflows
        }
    }

    /// Create a moderate stress scenario
    pub fn moderate_stress() -> Self {
        Self {
            name: "Moderate Liquidity Stress".to_string(),
            description: "Sector-specific liquidity stress".to_string(),
            additional_haircut: dec!(0.15), // 15% additional haircut
            outflow_multiplier: dec!(1.5),  // 1.5x normal outflows
            inflow_reduction: dec!(0.25),   // 25% reduction in inflows
        }
    }

    /// Create a mild stress scenario
    pub fn mild_stress() -> Self {
        Self {
            name: "Mild Liquidity Stress".to_string(),
            description: "Temporary liquidity pressure".to_string(),
            additional_haircut: dec!(0.05), // 5% additional haircut
            outflow_multiplier: dec!(1.2),  // 1.2x normal outflows
            inflow_reduction: dec!(0.10),   // 10% reduction in inflows
        }
    }

    /// Apply stress to LCR calculator and return stressed LCR
    pub fn apply_to_lcr(&self, calculator: &LCRCalculator) -> Decimal {
        // Apply additional haircuts to assets
        let stressed_hqla: Decimal = calculator
            .liquid_assets
            .iter()
            .map(|asset| {
                let base_haircut = asset.category.haircut();
                let total_haircut = (base_haircut + self.additional_haircut).min(dec!(1.0));
                if asset.category.is_hqla() {
                    asset.market_value * (dec!(1) - total_haircut)
                } else {
                    dec!(0)
                }
            })
            .sum();

        // Apply stress to outflows
        let stressed_outflows: Decimal = calculator
            .outflows
            .iter()
            .filter(|o| o.time_horizon_days <= 30)
            .map(|o| o.net_outflow() * self.outflow_multiplier)
            .sum();

        // Apply stress to inflows
        let stressed_inflows: Decimal = calculator
            .inflows
            .iter()
            .filter(|i| i.time_horizon_days <= 30)
            .map(|i| i.net_inflow() * (dec!(1) - self.inflow_reduction))
            .sum();

        // Calculate stressed net outflows
        let capped_inflows = stressed_inflows.min(stressed_outflows * dec!(0.75));
        let stressed_net_outflows = (stressed_outflows - capped_inflows).max(dec!(0));

        if stressed_net_outflows == dec!(0) {
            return Decimal::MAX;
        }

        (stressed_hqla / stressed_net_outflows) * dec!(100)
    }
}

/// Liquidity risk manager
pub struct LiquidityRiskManager {
    /// Market liquidity metrics by asset
    market_liquidity: HashMap<i64, MarketLiquidityMetrics>,
    /// Funding liquidity metrics (historical)
    funding_metrics_history: Vec<FundingLiquidityMetrics>,
}

impl LiquidityRiskManager {
    /// Create a new liquidity risk manager
    pub fn new() -> Self {
        Self {
            market_liquidity: HashMap::new(),
            funding_metrics_history: Vec::new(),
        }
    }

    /// Update market liquidity metrics for an asset
    pub fn update_market_liquidity(&mut self, metrics: MarketLiquidityMetrics) {
        self.market_liquidity.insert(metrics.asset_id, metrics);
    }

    /// Add funding liquidity metrics
    pub fn add_funding_metrics(&mut self, metrics: FundingLiquidityMetrics) {
        self.funding_metrics_history.push(metrics);
    }

    /// Get market liquidity score for an asset
    pub fn get_market_liquidity_score(&self, asset_id: i64) -> Option<Decimal> {
        self.market_liquidity
            .get(&asset_id)
            .map(|m| m.liquidity_score())
    }

    /// Get assets with low liquidity (score < 40)
    pub fn get_low_liquidity_assets(&self) -> Vec<i64> {
        self.market_liquidity
            .iter()
            .filter(|(_, m)| m.liquidity_score() < dec!(40))
            .map(|(&id, _)| id)
            .collect()
    }

    /// Calculate average liquidity score across all assets
    pub fn average_liquidity_score(&self) -> Decimal {
        if self.market_liquidity.is_empty() {
            return dec!(0);
        }

        let total: Decimal = self
            .market_liquidity
            .values()
            .map(|m| m.liquidity_score())
            .sum();

        total / Decimal::from(self.market_liquidity.len())
    }

    /// Get latest funding metrics
    pub fn latest_funding_metrics(&self) -> Option<&FundingLiquidityMetrics> {
        self.funding_metrics_history.last()
    }

    /// Check if funding gap is increasing (trend analysis)
    pub fn is_funding_gap_increasing(&self) -> bool {
        if self.funding_metrics_history.len() < 2 {
            return false;
        }

        let recent = &self.funding_metrics_history[self.funding_metrics_history.len() - 1];
        let previous = &self.funding_metrics_history[self.funding_metrics_history.len() - 2];

        recent.funding_gap() > previous.funding_gap()
    }
}

impl Default for LiquidityRiskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidity_category_haircut() {
        assert_eq!(LiquidityCategory::Level1.haircut(), dec!(0.00));
        assert_eq!(LiquidityCategory::Level2A.haircut(), dec!(0.15));
        assert_eq!(LiquidityCategory::Level2B.haircut(), dec!(0.50));
        assert_eq!(LiquidityCategory::NonHQLA.haircut(), dec!(1.00));
    }

    #[test]
    fn test_liquid_asset_hqla_value() {
        let asset = LiquidAsset {
            asset_id: 1,
            name: "Treasury Bond".to_string(),
            market_value: dec!(10000),
            category: LiquidityCategory::Level2A,
            timestamp: Utc::now(),
        };

        // 10000 * (1 - 0.15) = 8500
        assert_eq!(asset.hqla_value(), dec!(8500));
    }

    #[test]
    fn test_lcr_calculation() {
        let mut calculator = LCRCalculator::new();

        // Add HQLA worth $100,000
        calculator.add_asset(LiquidAsset {
            asset_id: 1,
            name: "Cash".to_string(),
            market_value: dec!(100000),
            category: LiquidityCategory::Level1,
            timestamp: Utc::now(),
        });

        // Add outflow of $80,000 (100% outflow rate)
        calculator.add_outflow(CashOutflow {
            category: "Retail Deposits".to_string(),
            gross_amount: dec!(80000),
            outflow_rate: dec!(1.0),
            time_horizon_days: 30,
        });

        // Add inflow of $20,000 (100% inflow rate)
        calculator.add_inflow(CashInflow {
            category: "Receivables".to_string(),
            gross_amount: dec!(20000),
            inflow_rate: dec!(1.0),
            time_horizon_days: 30,
        });

        // Net outflows = 80000 - min(20000, 80000 * 0.75) = 80000 - 20000 = 60000
        // LCR = 100000 / 60000 * 100 = 166.67%
        let lcr = calculator.calculate_lcr();
        assert!(lcr > dec!(166) && lcr < dec!(167));
        assert!(calculator.meets_regulatory_minimum());
    }

    #[test]
    fn test_market_liquidity_score() {
        let metrics = MarketLiquidityMetrics {
            asset_id: 1,
            bid_ask_spread: dec!(0.0005),      // Very tight spread
            trading_volume_24h: dec!(2000000), // High volume
            market_depth: dec!(600000),        // Deep market
            price_impact_10k: dec!(0.003),     // Low price impact
            timestamp: Utc::now(),
        };

        let score = metrics.liquidity_score();
        assert_eq!(score, dec!(100)); // Perfect score
        assert_eq!(metrics.liquidity_category(), "Highly Liquid");
    }

    #[test]
    fn test_funding_liquidity_metrics() {
        let metrics = FundingLiquidityMetrics {
            available_funding: dec!(1000000),
            required_funding: dec!(800000),
            funding_sources: 5,
            largest_source_pct: dec!(0.30),
            timestamp: Utc::now(),
        };

        assert_eq!(metrics.funding_gap(), dec!(-200000)); // Surplus
        assert!(metrics.is_adequate());
        assert_eq!(metrics.concentration_risk(), dec!(30));
        assert!(metrics.diversification_score() > dec!(0));
    }

    #[test]
    fn test_liquidity_stress_scenario() {
        let mut calculator = LCRCalculator::new();

        calculator.add_asset(LiquidAsset {
            asset_id: 1,
            name: "HQLA".to_string(),
            market_value: dec!(100000),
            category: LiquidityCategory::Level1,
            timestamp: Utc::now(),
        });

        calculator.add_outflow(CashOutflow {
            category: "Deposits".to_string(),
            gross_amount: dec!(50000),
            outflow_rate: dec!(1.0),
            time_horizon_days: 30,
        });

        let normal_lcr = calculator.calculate_lcr();
        let scenario = LiquidityStressScenario::severe_stress();
        let stressed_lcr = scenario.apply_to_lcr(&calculator);

        assert!(stressed_lcr < normal_lcr);
    }

    #[test]
    fn test_liquidity_risk_manager() {
        let mut manager = LiquidityRiskManager::new();

        manager.update_market_liquidity(MarketLiquidityMetrics {
            asset_id: 1,
            bid_ask_spread: dec!(0.001),
            trading_volume_24h: dec!(100000),
            market_depth: dec!(50000),
            price_impact_10k: dec!(0.02),
            timestamp: Utc::now(),
        });

        manager.update_market_liquidity(MarketLiquidityMetrics {
            asset_id: 2,
            bid_ask_spread: dec!(0.10),
            trading_volume_24h: dec!(100),
            market_depth: dec!(1000),
            price_impact_10k: dec!(0.20),
            timestamp: Utc::now(),
        });

        let avg_score = manager.average_liquidity_score();
        assert!(avg_score > dec!(0));

        let low_liquidity_assets = manager.get_low_liquidity_assets();
        assert!(!low_liquidity_assets.is_empty());
    }

    #[test]
    fn test_cash_outflow_net_outflow() {
        let outflow = CashOutflow {
            category: "Test".to_string(),
            gross_amount: dec!(1000),
            outflow_rate: dec!(0.5),
            time_horizon_days: 30,
        };

        assert_eq!(outflow.net_outflow(), dec!(500));
    }

    #[test]
    fn test_cash_inflow_net_inflow() {
        let inflow = CashInflow {
            category: "Test".to_string(),
            gross_amount: dec!(1000),
            inflow_rate: dec!(0.8),
            time_horizon_days: 30,
        };

        assert_eq!(inflow.net_inflow(), dec!(800));
    }
}
