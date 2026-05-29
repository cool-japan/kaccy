//! Advanced portfolio tracking and analytics

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Portfolio holding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holding {
    /// Token ID
    pub token_id: Uuid,
    /// Token symbol
    pub symbol: String,
    /// Amount held
    pub amount: Decimal,
    /// Average entry price
    pub avg_entry_price: Decimal,
    /// Current price
    pub current_price: Decimal,
    /// Total cost basis
    pub cost_basis: Decimal,
    /// Current value
    pub current_value: Decimal,
    /// Unrealized P&L
    pub unrealized_pnl: Decimal,
    /// Percentage of portfolio
    pub portfolio_percentage: Decimal,
}

impl Holding {
    /// Calculate metrics
    pub fn calculate_metrics(&mut self) {
        self.cost_basis = self.amount * self.avg_entry_price;
        self.current_value = self.amount * self.current_price;
        self.unrealized_pnl = self.current_value - self.cost_basis;
    }

    /// Get return percentage
    pub fn return_percentage(&self) -> Decimal {
        if self.cost_basis.is_zero() {
            return Decimal::ZERO;
        }
        (self.unrealized_pnl / self.cost_basis) * dec!(100)
    }
}

/// Portfolio analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioAnalytics {
    /// User ID
    pub user_id: Uuid,
    /// Holdings
    pub holdings: Vec<Holding>,
    /// Total portfolio value
    pub total_value: Decimal,
    /// Total cost basis
    pub total_cost_basis: Decimal,
    /// Total unrealized P&L
    pub total_unrealized_pnl: Decimal,
    /// Total return percentage
    pub total_return_pct: Decimal,
    /// Number of positions
    pub position_count: usize,
    /// Diversification score (0-1, higher = more diversified)
    pub diversification_score: Decimal,
    /// Updated at
    pub updated_at: DateTime<Utc>,
}

impl PortfolioAnalytics {
    /// Create new portfolio analytics
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            holdings: Vec::new(),
            total_value: Decimal::ZERO,
            total_cost_basis: Decimal::ZERO,
            total_unrealized_pnl: Decimal::ZERO,
            total_return_pct: Decimal::ZERO,
            position_count: 0,
            diversification_score: Decimal::ZERO,
            updated_at: Utc::now(),
        }
    }

    /// Add a holding
    pub fn add_holding(&mut self, mut holding: Holding) {
        holding.calculate_metrics();
        self.holdings.push(holding);
        self.recalculate();
    }

    /// Recalculate portfolio metrics
    pub fn recalculate(&mut self) {
        self.total_value = self.holdings.iter().map(|h| h.current_value).sum();
        self.total_cost_basis = self.holdings.iter().map(|h| h.cost_basis).sum();
        self.total_unrealized_pnl = self.total_value - self.total_cost_basis;

        self.total_return_pct = if self.total_cost_basis.is_zero() {
            Decimal::ZERO
        } else {
            (self.total_unrealized_pnl / self.total_cost_basis) * dec!(100)
        };

        self.position_count = self.holdings.len();

        // Calculate portfolio percentages
        for holding in &mut self.holdings {
            holding.portfolio_percentage = if self.total_value.is_zero() {
                Decimal::ZERO
            } else {
                (holding.current_value / self.total_value) * dec!(100)
            };
        }

        // Calculate diversification score (Herfindahl index)
        self.diversification_score = self.calculate_diversification();

        self.updated_at = Utc::now();
    }

    /// Calculate diversification score using Herfindahl index
    fn calculate_diversification(&self) -> Decimal {
        if self.holdings.is_empty() || self.total_value.is_zero() {
            return Decimal::ZERO;
        }

        let herfindahl: Decimal = self
            .holdings
            .iter()
            .map(|h| {
                let weight = h.current_value / self.total_value;
                weight * weight
            })
            .sum();

        // Convert to diversification score (1 - H)
        dec!(1) - herfindahl
    }

    /// Get largest positions
    pub fn top_positions(&self, n: usize) -> Vec<&Holding> {
        let mut holdings = self.holdings.iter().collect::<Vec<_>>();
        holdings.sort_by(|a, b| {
            b.current_value
                .partial_cmp(&a.current_value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        holdings.into_iter().take(n).collect()
    }

    /// Get best performers
    pub fn best_performers(&self, n: usize) -> Vec<&Holding> {
        let mut holdings = self.holdings.iter().collect::<Vec<_>>();
        holdings.sort_by(|a, b| {
            b.return_percentage()
                .partial_cmp(&a.return_percentage())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        holdings.into_iter().take(n).collect()
    }

    /// Get worst performers
    pub fn worst_performers(&self, n: usize) -> Vec<&Holding> {
        let mut holdings = self.holdings.iter().collect::<Vec<_>>();
        holdings.sort_by(|a, b| {
            a.return_percentage()
                .partial_cmp(&b.return_percentage())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        holdings.into_iter().take(n).collect()
    }
}

/// Correlation matrix for portfolio analysis
pub struct CorrelationMatrix {
    /// Token pairs
    correlations: HashMap<(Uuid, Uuid), Decimal>,
}

impl CorrelationMatrix {
    /// Create new correlation matrix
    pub fn new() -> Self {
        Self {
            correlations: HashMap::new(),
        }
    }

    /// Set correlation between two tokens
    pub fn set_correlation(&mut self, token1: Uuid, token2: Uuid, correlation: Decimal) {
        self.correlations.insert((token1, token2), correlation);
        self.correlations.insert((token2, token1), correlation);
    }

    /// Get correlation
    pub fn get_correlation(&self, token1: Uuid, token2: Uuid) -> Decimal {
        self.correlations
            .get(&(token1, token2))
            .copied()
            .unwrap_or(Decimal::ZERO)
    }

    /// Calculate portfolio risk (simplified)
    pub fn portfolio_risk(&self, holdings: &[Holding]) -> Decimal {
        if holdings.is_empty() {
            return Decimal::ZERO;
        }

        let mut total_risk = Decimal::ZERO;

        for (i, h1) in holdings.iter().enumerate() {
            for h2 in holdings.iter().skip(i + 1) {
                let corr = self.get_correlation(h1.token_id, h2.token_id);
                let weight1 = h1.portfolio_percentage / dec!(100);
                let weight2 = h2.portfolio_percentage / dec!(100);
                total_risk += weight1 * weight2 * corr;
            }
        }

        total_risk
    }
}

impl Default for CorrelationMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_holding_metrics() {
        let mut holding = Holding {
            token_id: Uuid::new_v4(),
            symbol: "TEST".to_string(),
            amount: dec!(100),
            avg_entry_price: dec!(10),
            current_price: dec!(15),
            cost_basis: Decimal::ZERO,
            current_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            portfolio_percentage: Decimal::ZERO,
        };

        holding.calculate_metrics();

        assert_eq!(holding.cost_basis, dec!(1000)); // 100 * 10
        assert_eq!(holding.current_value, dec!(1500)); // 100 * 15
        assert_eq!(holding.unrealized_pnl, dec!(500)); // 1500 - 1000
        assert_eq!(holding.return_percentage(), dec!(50)); // (500 / 1000) * 100
    }

    #[test]
    fn test_portfolio_analytics() {
        let user_id = Uuid::new_v4();
        let mut portfolio = PortfolioAnalytics::new(user_id);

        let holding1 = Holding {
            token_id: Uuid::new_v4(),
            symbol: "TOKEN1".to_string(),
            amount: dec!(100),
            avg_entry_price: dec!(10),
            current_price: dec!(15),
            cost_basis: Decimal::ZERO,
            current_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            portfolio_percentage: Decimal::ZERO,
        };

        let holding2 = Holding {
            token_id: Uuid::new_v4(),
            symbol: "TOKEN2".to_string(),
            amount: dec!(50),
            avg_entry_price: dec!(20),
            current_price: dec!(18),
            cost_basis: Decimal::ZERO,
            current_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            portfolio_percentage: Decimal::ZERO,
        };

        portfolio.add_holding(holding1);
        portfolio.add_holding(holding2);

        assert_eq!(portfolio.position_count, 2);
        assert_eq!(portfolio.total_value, dec!(2400)); // 1500 + 900
        assert_eq!(portfolio.total_cost_basis, dec!(2000)); // 1000 + 1000
        assert_eq!(portfolio.total_unrealized_pnl, dec!(400)); // 2400 - 2000
    }

    #[test]
    fn test_portfolio_percentages() {
        let user_id = Uuid::new_v4();
        let mut portfolio = PortfolioAnalytics::new(user_id);

        portfolio.add_holding(Holding {
            token_id: Uuid::new_v4(),
            symbol: "TOKEN1".to_string(),
            amount: dec!(100),
            avg_entry_price: dec!(10),
            current_price: dec!(15),
            cost_basis: Decimal::ZERO,
            current_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            portfolio_percentage: Decimal::ZERO,
        });

        portfolio.add_holding(Holding {
            token_id: Uuid::new_v4(),
            symbol: "TOKEN2".to_string(),
            amount: dec!(50),
            avg_entry_price: dec!(10),
            current_price: dec!(10),
            cost_basis: Decimal::ZERO,
            current_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            portfolio_percentage: Decimal::ZERO,
        });

        // TOKEN1: 1500, TOKEN2: 500, Total: 2000
        assert_eq!(portfolio.holdings[0].portfolio_percentage, dec!(75)); // 1500/2000 * 100
        assert_eq!(portfolio.holdings[1].portfolio_percentage, dec!(25)); // 500/2000 * 100
    }

    #[test]
    fn test_top_positions() {
        let user_id = Uuid::new_v4();
        let mut portfolio = PortfolioAnalytics::new(user_id);

        for i in 1..=5 {
            portfolio.add_holding(Holding {
                token_id: Uuid::new_v4(),
                symbol: format!("TOKEN{}", i),
                amount: dec!(10),
                avg_entry_price: dec!(10),
                current_price: Decimal::from(i) * dec!(10),
                cost_basis: Decimal::ZERO,
                current_value: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
                portfolio_percentage: Decimal::ZERO,
            });
        }

        let top = portfolio.top_positions(2);
        assert_eq!(top.len(), 2);
        // Should be sorted by value descending
        assert!(top[0].current_value >= top[1].current_value);
    }

    #[test]
    fn test_best_performers() {
        let user_id = Uuid::new_v4();
        let mut portfolio = PortfolioAnalytics::new(user_id);

        portfolio.add_holding(Holding {
            token_id: Uuid::new_v4(),
            symbol: "WINNER".to_string(),
            amount: dec!(10),
            avg_entry_price: dec!(10),
            current_price: dec!(20), // 100% gain
            cost_basis: Decimal::ZERO,
            current_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            portfolio_percentage: Decimal::ZERO,
        });

        portfolio.add_holding(Holding {
            token_id: Uuid::new_v4(),
            symbol: "LOSER".to_string(),
            amount: dec!(10),
            avg_entry_price: dec!(10),
            current_price: dec!(5), // 50% loss
            cost_basis: Decimal::ZERO,
            current_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            portfolio_percentage: Decimal::ZERO,
        });

        let best = portfolio.best_performers(1);
        assert_eq!(best[0].symbol, "WINNER");

        let worst = portfolio.worst_performers(1);
        assert_eq!(worst[0].symbol, "LOSER");
    }

    #[test]
    fn test_correlation_matrix() {
        let mut matrix = CorrelationMatrix::new();

        let token1 = Uuid::new_v4();
        let token2 = Uuid::new_v4();

        matrix.set_correlation(token1, token2, dec!(0.75));

        assert_eq!(matrix.get_correlation(token1, token2), dec!(0.75));
        assert_eq!(matrix.get_correlation(token2, token1), dec!(0.75)); // Symmetric
    }

    #[test]
    fn test_diversification_score() {
        let user_id = Uuid::new_v4();
        let mut portfolio = PortfolioAnalytics::new(user_id);

        // Concentrated portfolio (one large holding)
        portfolio.add_holding(Holding {
            token_id: Uuid::new_v4(),
            symbol: "BIG".to_string(),
            amount: dec!(90),
            avg_entry_price: dec!(10),
            current_price: dec!(10),
            cost_basis: Decimal::ZERO,
            current_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            portfolio_percentage: Decimal::ZERO,
        });

        portfolio.add_holding(Holding {
            token_id: Uuid::new_v4(),
            symbol: "SMALL".to_string(),
            amount: dec!(10),
            avg_entry_price: dec!(10),
            current_price: dec!(10),
            cost_basis: Decimal::ZERO,
            current_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            portfolio_percentage: Decimal::ZERO,
        });

        // Diversification should be low (closer to 0)
        assert!(portfolio.diversification_score < dec!(0.5));
    }
}
