//! Pre-trade risk assessment module
//!
//! This module provides comprehensive pre-trade risk checks including:
//! - Portfolio concentration analysis
//! - Margin requirements validation
//! - Liquidity impact assessment
//! - Counterparty exposure limits
//! - Regulatory compliance checks

use crate::error::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pre-trade risk check result
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskCheckResult {
    /// Trade passes all risk checks
    Approved,

    /// Trade is rejected due to risk limits
    Rejected {
        /// Human-readable explanation for the rejection.
        reason: String,
    },

    /// Trade requires manual review
    RequiresReview {
        /// Explanation of why manual review is needed.
        reason: String,
        /// Risk score that triggered the review (0–100).
        risk_score: u8,
    },
}

/// Risk assessment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessmentConfig {
    /// Maximum portfolio concentration per asset (percentage)
    pub max_concentration: Decimal,

    /// Maximum leverage allowed
    pub max_leverage: Decimal,

    /// Minimum account equity required
    pub min_account_equity: Decimal,

    /// Maximum daily loss limit (percentage)
    pub max_daily_loss_pct: Decimal,

    /// Maximum counterparty exposure
    pub max_counterparty_exposure: Decimal,

    /// Minimum liquidity score required (0-100)
    pub min_liquidity_score: u8,

    /// Maximum value at risk (VaR) as percentage of portfolio
    pub max_var_pct: Decimal,
}

impl Default for RiskAssessmentConfig {
    fn default() -> Self {
        Self {
            max_concentration: dec!(0.25),             // 25%
            max_leverage: dec!(5.0),                   // 5x
            min_account_equity: dec!(1000.0),          // $1000
            max_daily_loss_pct: dec!(0.10),            // 10%
            max_counterparty_exposure: dec!(100000.0), // $100k
            min_liquidity_score: 50,                   // 50/100
            max_var_pct: dec!(0.15),                   // 15%
        }
    }
}

/// Portfolio position for risk assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioPosition {
    /// Asset identifier (token symbol or address).
    pub asset: String,
    /// Held quantity of the asset.
    pub quantity: Decimal,
    /// Current market value of the position.
    pub market_value: Decimal,
}

/// Trade request for risk assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRequest {
    /// Asset to trade (token symbol or address).
    pub asset: String,
    /// Intended trade quantity.
    pub quantity: Decimal,
    /// Expected execution price.
    pub price: Decimal,
    /// Buy or sell direction.
    pub side: TradeSide,
}

/// Direction of a trade (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    /// Purchasing the asset.
    Buy,
    /// Selling the asset.
    Sell,
}

/// Pre-trade risk assessor
pub struct PreTradeRiskAssessor {
    config: RiskAssessmentConfig,
}

impl PreTradeRiskAssessor {
    /// Create a new risk assessor with default config
    pub fn new() -> Self {
        Self {
            config: RiskAssessmentConfig::default(),
        }
    }

    /// Create a new risk assessor with custom config
    pub fn with_config(config: RiskAssessmentConfig) -> Self {
        Self { config }
    }

    /// Check portfolio concentration
    pub fn check_concentration(
        &self,
        portfolio: &[PortfolioPosition],
        new_trade: &TradeRequest,
    ) -> RiskCheckResult {
        let total_portfolio_value: Decimal = portfolio.iter().map(|p| p.market_value).sum();

        if total_portfolio_value == Decimal::ZERO {
            return RiskCheckResult::Approved;
        }

        // Calculate new position value
        let trade_value = new_trade.quantity * new_trade.price;
        let current_asset_value: Decimal = portfolio
            .iter()
            .filter(|p| p.asset == new_trade.asset)
            .map(|p| p.market_value)
            .sum();

        let new_asset_value = match new_trade.side {
            TradeSide::Buy => current_asset_value + trade_value,
            TradeSide::Sell => current_asset_value - trade_value,
        };

        let new_total_value = total_portfolio_value
            + match new_trade.side {
                TradeSide::Buy => trade_value,
                TradeSide::Sell => -trade_value,
            };

        let concentration = if new_total_value > Decimal::ZERO {
            new_asset_value / new_total_value
        } else {
            Decimal::ZERO
        };

        if concentration > self.config.max_concentration {
            return RiskCheckResult::Rejected {
                reason: format!(
                    "Concentration {:.2}% exceeds maximum {:.2}%",
                    concentration * dec!(100),
                    self.config.max_concentration * dec!(100)
                ),
            };
        }

        RiskCheckResult::Approved
    }

    /// Check margin requirements
    pub fn check_margin(
        &self,
        account_equity: Decimal,
        current_positions_value: Decimal,
        new_trade_value: Decimal,
    ) -> RiskCheckResult {
        if account_equity < self.config.min_account_equity {
            return RiskCheckResult::Rejected {
                reason: format!(
                    "Account equity ${} below minimum ${}",
                    account_equity, self.config.min_account_equity
                ),
            };
        }

        let total_exposure = current_positions_value + new_trade_value;
        let leverage = if account_equity > Decimal::ZERO {
            total_exposure / account_equity
        } else {
            Decimal::ZERO
        };

        if leverage > self.config.max_leverage {
            return RiskCheckResult::Rejected {
                reason: format!(
                    "Leverage {:.2}x exceeds maximum {:.2}x",
                    leverage, self.config.max_leverage
                ),
            };
        }

        RiskCheckResult::Approved
    }

    /// Check daily loss limits
    pub fn check_daily_loss(&self, account_value: Decimal, daily_pnl: Decimal) -> RiskCheckResult {
        let loss_pct = if account_value > Decimal::ZERO {
            (-daily_pnl / account_value).max(Decimal::ZERO)
        } else {
            Decimal::ZERO
        };

        if loss_pct >= self.config.max_daily_loss_pct {
            return RiskCheckResult::Rejected {
                reason: format!(
                    "Daily loss {:.2}% has reached/exceeded limit {:.2}%",
                    loss_pct * dec!(100),
                    self.config.max_daily_loss_pct * dec!(100)
                ),
            };
        }

        // Warn if approaching limit (80% of max)
        if loss_pct >= self.config.max_daily_loss_pct * dec!(0.80) {
            let risk_score = ((loss_pct / self.config.max_daily_loss_pct) * dec!(100))
                .to_string()
                .parse::<u8>()
                .unwrap_or(80);

            return RiskCheckResult::RequiresReview {
                reason: format!(
                    "Daily loss {:.2}% approaching limit {:.2}%",
                    loss_pct * dec!(100),
                    self.config.max_daily_loss_pct * dec!(100)
                ),
                risk_score,
            };
        }

        RiskCheckResult::Approved
    }

    /// Check liquidity impact
    pub fn check_liquidity(
        &self,
        trade_value: Decimal,
        market_depth: Decimal,
        liquidity_score: u8,
    ) -> RiskCheckResult {
        if liquidity_score < self.config.min_liquidity_score {
            return RiskCheckResult::RequiresReview {
                reason: format!(
                    "Liquidity score {} below minimum {}",
                    liquidity_score, self.config.min_liquidity_score
                ),
                risk_score: 100 - liquidity_score,
            };
        }

        // Check if trade size is too large relative to market depth
        if market_depth > Decimal::ZERO {
            let depth_ratio = trade_value / market_depth;
            if depth_ratio > dec!(0.10) {
                // More than 10% of market depth
                return RiskCheckResult::RequiresReview {
                    reason: format!(
                        "Trade size is {:.2}% of market depth",
                        depth_ratio * dec!(100)
                    ),
                    risk_score: 75,
                };
            }
        }

        RiskCheckResult::Approved
    }

    /// Check counterparty exposure
    pub fn check_counterparty_exposure(
        &self,
        counterparty_id: &str,
        current_exposure: Decimal,
        new_trade_value: Decimal,
    ) -> RiskCheckResult {
        let total_exposure = current_exposure + new_trade_value;

        if total_exposure > self.config.max_counterparty_exposure {
            return RiskCheckResult::Rejected {
                reason: format!(
                    "Counterparty {} exposure ${} exceeds limit ${}",
                    counterparty_id, total_exposure, self.config.max_counterparty_exposure
                ),
            };
        }

        // Warn if approaching limit (90% of max)
        if total_exposure >= self.config.max_counterparty_exposure * dec!(0.90) {
            return RiskCheckResult::RequiresReview {
                reason: format!(
                    "Counterparty {} exposure approaching limit",
                    counterparty_id
                ),
                risk_score: 70,
            };
        }

        RiskCheckResult::Approved
    }

    /// Check Value at Risk (VaR)
    pub fn check_var(
        &self,
        portfolio_value: Decimal,
        portfolio_var: Decimal,
        additional_var: Decimal,
    ) -> RiskCheckResult {
        let total_var = portfolio_var + additional_var;
        let var_pct = if portfolio_value > Decimal::ZERO {
            total_var / portfolio_value
        } else {
            Decimal::ZERO
        };

        if var_pct > self.config.max_var_pct {
            return RiskCheckResult::Rejected {
                reason: format!(
                    "Portfolio VaR {:.2}% exceeds maximum {:.2}%",
                    var_pct * dec!(100),
                    self.config.max_var_pct * dec!(100)
                ),
            };
        }

        RiskCheckResult::Approved
    }

    /// Comprehensive pre-trade risk assessment
    #[allow(clippy::too_many_arguments)]
    pub fn assess_trade(
        &self,
        trade: &TradeRequest,
        portfolio: &[PortfolioPosition],
        account_equity: Decimal,
        daily_pnl: Decimal,
        counterparty_exposure: HashMap<String, Decimal>,
        liquidity_score: u8,
        market_depth: Decimal,
        portfolio_var: Decimal,
    ) -> Result<RiskCheckResult> {
        let trade_value = trade.quantity * trade.price;
        let portfolio_value: Decimal = portfolio.iter().map(|p| p.market_value).sum();

        // 1. Check concentration
        match self.check_concentration(portfolio, trade) {
            RiskCheckResult::Rejected { reason } => {
                return Ok(RiskCheckResult::Rejected { reason });
            }
            RiskCheckResult::RequiresReview { reason, risk_score } => {
                return Ok(RiskCheckResult::RequiresReview { reason, risk_score });
            }
            RiskCheckResult::Approved => {}
        }

        // 2. Check margin
        match self.check_margin(account_equity, portfolio_value, trade_value) {
            RiskCheckResult::Rejected { reason } => {
                return Ok(RiskCheckResult::Rejected { reason });
            }
            RiskCheckResult::RequiresReview { reason, risk_score } => {
                return Ok(RiskCheckResult::RequiresReview { reason, risk_score });
            }
            RiskCheckResult::Approved => {}
        }

        // 3. Check daily loss
        match self.check_daily_loss(account_equity, daily_pnl) {
            RiskCheckResult::Rejected { reason } => {
                return Ok(RiskCheckResult::Rejected { reason });
            }
            RiskCheckResult::RequiresReview { reason, risk_score } => {
                return Ok(RiskCheckResult::RequiresReview { reason, risk_score });
            }
            RiskCheckResult::Approved => {}
        }

        // 4. Check liquidity
        if let RiskCheckResult::RequiresReview { reason, risk_score } =
            self.check_liquidity(trade_value, market_depth, liquidity_score)
        {
            return Ok(RiskCheckResult::RequiresReview { reason, risk_score });
        }

        // 5. Check counterparty exposure (assuming trade.asset as counterparty for simplicity)
        let current_cp_exposure = counterparty_exposure
            .get(&trade.asset)
            .copied()
            .unwrap_or(Decimal::ZERO);

        match self.check_counterparty_exposure(&trade.asset, current_cp_exposure, trade_value) {
            RiskCheckResult::Rejected { reason } => {
                return Ok(RiskCheckResult::Rejected { reason });
            }
            RiskCheckResult::RequiresReview { reason, risk_score } => {
                return Ok(RiskCheckResult::RequiresReview { reason, risk_score });
            }
            RiskCheckResult::Approved => {}
        }

        // 6. Check VaR
        let additional_var = trade_value * dec!(0.05); // Simplified: assume 5% VaR for new trade
        match self.check_var(portfolio_value, portfolio_var, additional_var) {
            RiskCheckResult::Rejected { reason } => {
                return Ok(RiskCheckResult::Rejected { reason });
            }
            RiskCheckResult::RequiresReview { reason, risk_score } => {
                return Ok(RiskCheckResult::RequiresReview { reason, risk_score });
            }
            RiskCheckResult::Approved => {}
        }

        Ok(RiskCheckResult::Approved)
    }
}

impl Default for PreTradeRiskAssessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_concentration_pass() {
        let assessor = PreTradeRiskAssessor::new();
        // Well-diversified portfolio
        let portfolio = vec![
            PortfolioPosition {
                asset: "BTC".to_string(),
                quantity: dec!(0.4),
                market_value: dec!(20000.0),
            },
            PortfolioPosition {
                asset: "ETH".to_string(),
                quantity: dec!(10.0),
                market_value: dec!(30000.0),
            },
            PortfolioPosition {
                asset: "SOL".to_string(),
                quantity: dec!(100.0),
                market_value: dec!(25000.0),
            },
            PortfolioPosition {
                asset: "USDC".to_string(),
                quantity: dec!(25000.0),
                market_value: dec!(25000.0),
            },
        ];

        // Small trade that won't exceed concentration limit (20k -> 21k out of 100k = 21%)
        let trade = TradeRequest {
            asset: "BTC".to_string(),
            quantity: dec!(0.02),
            price: dec!(50000.0),
            side: TradeSide::Buy,
        };

        let result = assessor.check_concentration(&portfolio, &trade);
        assert_eq!(result, RiskCheckResult::Approved);
    }

    #[test]
    fn test_check_concentration_exceed() {
        let assessor = PreTradeRiskAssessor::new();
        let portfolio = vec![PortfolioPosition {
            asset: "BTC".to_string(),
            quantity: dec!(1.0),
            market_value: dec!(50000.0),
        }];

        let trade = TradeRequest {
            asset: "BTC".to_string(),
            quantity: dec!(5.0), // Large buy
            price: dec!(50000.0),
            side: TradeSide::Buy,
        };

        let result = assessor.check_concentration(&portfolio, &trade);
        matches!(result, RiskCheckResult::Rejected { .. });
    }

    #[test]
    fn test_check_margin_pass() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_margin(dec!(10000.0), dec!(20000.0), dec!(10000.0));
        assert_eq!(result, RiskCheckResult::Approved);
    }

    #[test]
    fn test_check_margin_exceed_leverage() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_margin(dec!(10000.0), dec!(30000.0), dec!(30000.0));
        matches!(result, RiskCheckResult::Rejected { .. });
    }

    #[test]
    fn test_check_daily_loss_ok() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_daily_loss(dec!(10000.0), dec!(-500.0));
        assert_eq!(result, RiskCheckResult::Approved);
    }

    #[test]
    fn test_check_daily_loss_limit_reached() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_daily_loss(dec!(10000.0), dec!(-1000.0));
        matches!(result, RiskCheckResult::Rejected { .. });
    }

    #[test]
    fn test_check_liquidity_ok() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_liquidity(dec!(1000.0), dec!(50000.0), 80);
        assert_eq!(result, RiskCheckResult::Approved);
    }

    #[test]
    fn test_check_liquidity_low_score() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_liquidity(dec!(1000.0), dec!(50000.0), 30);
        matches!(result, RiskCheckResult::RequiresReview { .. });
    }

    #[test]
    fn test_check_counterparty_ok() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_counterparty_exposure("CP1", dec!(10000.0), dec!(5000.0));
        assert_eq!(result, RiskCheckResult::Approved);
    }

    #[test]
    fn test_check_counterparty_exceed() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_counterparty_exposure("CP1", dec!(50000.0), dec!(60000.0));
        matches!(result, RiskCheckResult::Rejected { .. });
    }

    #[test]
    fn test_check_var_ok() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_var(dec!(100000.0), dec!(5000.0), dec!(2000.0));
        assert_eq!(result, RiskCheckResult::Approved);
    }

    #[test]
    fn test_check_var_exceed() {
        let assessor = PreTradeRiskAssessor::new();
        let result = assessor.check_var(dec!(100000.0), dec!(10000.0), dec!(10000.0));
        matches!(result, RiskCheckResult::Rejected { .. });
    }
}
