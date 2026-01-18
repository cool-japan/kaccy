//! Cross-Market Analytics
//!
//! This module provides enhanced arbitrage detection, price correlation analysis,
//! and market regime detection across multiple tokens and venues.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Enhanced arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunityEnhanced {
    pub opportunity_id: String,
    pub arbitrage_type: ArbitrageType,
    pub tokens_involved: Vec<String>,
    pub venues_involved: Vec<String>,
    pub expected_profit: Decimal,
    pub profit_percentage: Decimal,
    pub execution_path: Vec<TradeStep>,
    pub estimated_slippage: Decimal,
    pub confidence_score: Decimal,
    pub detected_at: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArbitrageType {
    /// Simple two-venue arbitrage
    TwoVenue,
    /// Triangular arbitrage (A->B->C->A)
    Triangular,
    /// Statistical arbitrage based on correlation breakdown
    Statistical,
    /// Cross-venue with multiple hops
    MultiHop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeStep {
    pub step_number: usize,
    pub venue: String,
    pub from_token: String,
    pub to_token: String,
    pub expected_price: Decimal,
    pub amount: Decimal,
}

/// Cross-venue arbitrage detector
pub struct CrossVenueArbitrageDetector {
    /// Price feeds from different venues
    price_feeds: HashMap<String, VenuePriceFeed>,
    /// Minimum profit threshold (in BTC)
    min_profit_threshold: Decimal,
    /// Maximum acceptable slippage
    #[allow(dead_code)]
    max_slippage: Decimal,
    /// Detected opportunities
    #[allow(dead_code)]
    opportunities: Vec<ArbitrageOpportunityEnhanced>,
    opportunity_counter: u64,
}

#[derive(Debug, Clone)]
pub struct VenuePriceFeed {
    pub venue_id: String,
    pub token_prices: HashMap<String, Decimal>,
    pub last_updated: SystemTime,
}

impl CrossVenueArbitrageDetector {
    pub fn new(min_profit_threshold: Decimal, max_slippage: Decimal) -> Self {
        Self {
            price_feeds: HashMap::new(),
            min_profit_threshold,
            max_slippage,
            opportunities: Vec::new(),
            opportunity_counter: 0,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(
            Decimal::new(1, 3), // 0.001 BTC minimum
            Decimal::new(5, 1), // 0.5% max slippage
        )
    }

    /// Updates price feed for a venue
    pub fn update_price_feed(&mut self, venue_id: String, token_prices: HashMap<String, Decimal>) {
        self.price_feeds.insert(
            venue_id.clone(),
            VenuePriceFeed {
                venue_id,
                token_prices,
                last_updated: SystemTime::now(),
            },
        );
    }

    /// Detects two-venue arbitrage opportunities
    pub fn detect_two_venue_arbitrage(
        &mut self,
        token_id: &str,
    ) -> Vec<ArbitrageOpportunityEnhanced> {
        let mut opportunities = Vec::new();
        let venues: Vec<_> = self.price_feeds.keys().cloned().collect();

        for i in 0..venues.len() {
            for j in (i + 1)..venues.len() {
                let venue1 = &venues[i];
                let venue2 = &venues[j];

                if let Some(opportunity) =
                    self.check_two_venue_opportunity(token_id, venue1, venue2)
                {
                    opportunities.push(opportunity);
                }
            }
        }

        opportunities
    }

    fn check_two_venue_opportunity(
        &mut self,
        token_id: &str,
        venue1: &str,
        venue2: &str,
    ) -> Option<ArbitrageOpportunityEnhanced> {
        let price1 = self.price_feeds.get(venue1)?.token_prices.get(token_id)?;
        let price2 = self.price_feeds.get(venue2)?.token_prices.get(token_id)?;

        let (buy_venue, sell_venue, buy_price, sell_price) = if price1 < price2 {
            (venue1, venue2, *price1, *price2)
        } else if price2 < price1 {
            (venue2, venue1, *price2, *price1)
        } else {
            return None; // No price difference
        };

        let profit_per_unit = sell_price - buy_price;
        let profit_percentage = (profit_per_unit / buy_price) * Decimal::new(100, 0);

        // Simple profit calculation (1 unit trade)
        let expected_profit = profit_per_unit;

        if expected_profit < self.min_profit_threshold {
            return None;
        }

        self.opportunity_counter += 1;
        Some(ArbitrageOpportunityEnhanced {
            opportunity_id: format!("arb_{}", self.opportunity_counter),
            arbitrage_type: ArbitrageType::TwoVenue,
            tokens_involved: vec![token_id.to_string()],
            venues_involved: vec![buy_venue.to_string(), sell_venue.to_string()],
            expected_profit,
            profit_percentage,
            execution_path: vec![
                TradeStep {
                    step_number: 1,
                    venue: buy_venue.to_string(),
                    from_token: "BTC".to_string(),
                    to_token: token_id.to_string(),
                    expected_price: buy_price,
                    amount: Decimal::ONE,
                },
                TradeStep {
                    step_number: 2,
                    venue: sell_venue.to_string(),
                    from_token: token_id.to_string(),
                    to_token: "BTC".to_string(),
                    expected_price: sell_price,
                    amount: Decimal::ONE,
                },
            ],
            estimated_slippage: Decimal::ZERO,
            confidence_score: Decimal::new(80, 2), // 0.80
            detected_at: SystemTime::now(),
        })
    }

    /// Detects triangular arbitrage opportunities (A->B->C->A)
    pub fn detect_triangular_arbitrage(
        &mut self,
        venue_id: &str,
        tokens: &[String],
    ) -> Vec<ArbitrageOpportunityEnhanced> {
        let mut opportunities = Vec::new();

        if tokens.len() < 3 {
            return opportunities;
        }

        // Clone the price feed to avoid borrow checker issues
        let feed = match self.price_feeds.get(venue_id) {
            Some(f) => f.clone(),
            None => return opportunities,
        };

        // Try all combinations of 3 tokens
        for i in 0..tokens.len() {
            for j in 0..tokens.len() {
                for k in 0..tokens.len() {
                    if i == j || j == k || i == k {
                        continue;
                    }

                    if let Some(opp) = self
                        .check_triangular_path(venue_id, &tokens[i], &tokens[j], &tokens[k], &feed)
                    {
                        opportunities.push(opp);
                    }
                }
            }
        }

        opportunities
    }

    #[allow(dead_code)]
    fn check_triangular_path(
        &mut self,
        venue_id: &str,
        token_a: &str,
        token_b: &str,
        token_c: &str,
        feed: &VenuePriceFeed,
    ) -> Option<ArbitrageOpportunityEnhanced> {
        // Get prices (assuming all priced in BTC)
        let price_a = *feed.token_prices.get(token_a)?;
        let price_b = *feed.token_prices.get(token_b)?;
        let price_c = *feed.token_prices.get(token_c)?;

        // Calculate triangular arbitrage: BTC -> A -> B -> C -> BTC
        let start_btc = Decimal::ONE;

        // Step 1: BTC -> A
        let amount_a = start_btc / price_a;

        // Step 2: A -> B (calculate via BTC)
        let btc_from_a = amount_a * price_a;
        let amount_b = btc_from_a / price_b;

        // Step 3: B -> C (calculate via BTC)
        let btc_from_b = amount_b * price_b;
        let amount_c = btc_from_b / price_c;

        // Step 4: C -> BTC
        let end_btc = amount_c * price_c;

        let profit = end_btc - start_btc;
        let profit_percentage = (profit / start_btc) * Decimal::new(100, 0);

        if profit < self.min_profit_threshold {
            return None;
        }

        self.opportunity_counter += 1;
        Some(ArbitrageOpportunityEnhanced {
            opportunity_id: format!("arb_tri_{}", self.opportunity_counter),
            arbitrage_type: ArbitrageType::Triangular,
            tokens_involved: vec![
                token_a.to_string(),
                token_b.to_string(),
                token_c.to_string(),
            ],
            venues_involved: vec![venue_id.to_string()],
            expected_profit: profit,
            profit_percentage,
            execution_path: vec![
                TradeStep {
                    step_number: 1,
                    venue: venue_id.to_string(),
                    from_token: "BTC".to_string(),
                    to_token: token_a.to_string(),
                    expected_price: price_a,
                    amount: start_btc,
                },
                TradeStep {
                    step_number: 2,
                    venue: venue_id.to_string(),
                    from_token: token_a.to_string(),
                    to_token: token_b.to_string(),
                    expected_price: price_b / price_a,
                    amount: amount_a,
                },
                TradeStep {
                    step_number: 3,
                    venue: venue_id.to_string(),
                    from_token: token_b.to_string(),
                    to_token: token_c.to_string(),
                    expected_price: price_c / price_b,
                    amount: amount_b,
                },
                TradeStep {
                    step_number: 4,
                    venue: venue_id.to_string(),
                    from_token: token_c.to_string(),
                    to_token: "BTC".to_string(),
                    expected_price: price_c,
                    amount: amount_c,
                },
            ],
            estimated_slippage: Decimal::ZERO,
            confidence_score: Decimal::new(75, 2), // 0.75
            detected_at: SystemTime::now(),
        })
    }
}

/// Price correlation analyzer
pub struct PriceCorrelationAnalyzer {
    /// Historical price data for tokens
    price_history: HashMap<String, Vec<PricePoint>>,
}

#[derive(Debug, Clone)]
pub struct PricePoint {
    pub timestamp: SystemTime,
    pub price: Decimal,
}

impl PriceCorrelationAnalyzer {
    pub fn new() -> Self {
        Self {
            price_history: HashMap::new(),
        }
    }

    /// Adds a price data point
    pub fn add_price_point(&mut self, token_id: String, price: Decimal) {
        self.price_history
            .entry(token_id)
            .or_default()
            .push(PricePoint {
                timestamp: SystemTime::now(),
                price,
            });
    }

    /// Calculates correlation coefficient between two tokens
    pub fn calculate_correlation(&self, token_a: &str, token_b: &str) -> Option<Decimal> {
        let prices_a = self.price_history.get(token_a)?;
        let prices_b = self.price_history.get(token_b)?;

        if prices_a.len() < 2 || prices_b.len() < 2 {
            return None;
        }

        // Align time series (take minimum length)
        let len = prices_a.len().min(prices_b.len());
        let values_a: Vec<f64> = prices_a[..len]
            .iter()
            .map(|p| p.price.to_string().parse().unwrap_or(0.0))
            .collect();
        let values_b: Vec<f64> = prices_b[..len]
            .iter()
            .map(|p| p.price.to_string().parse().unwrap_or(0.0))
            .collect();

        // Calculate means
        let mean_a: f64 = values_a.iter().sum::<f64>() / len as f64;
        let mean_b: f64 = values_b.iter().sum::<f64>() / len as f64;

        // Calculate correlation
        let mut numerator = 0.0;
        let mut sum_sq_a = 0.0;
        let mut sum_sq_b = 0.0;

        for i in 0..len {
            let diff_a = values_a[i] - mean_a;
            let diff_b = values_b[i] - mean_b;
            numerator += diff_a * diff_b;
            sum_sq_a += diff_a * diff_a;
            sum_sq_b += diff_b * diff_b;
        }

        let denominator = (sum_sq_a * sum_sq_b).sqrt();
        if denominator == 0.0 {
            return None;
        }

        let correlation = numerator / denominator;
        Decimal::try_from(correlation).ok()
    }

    /// Generates correlation matrix for multiple tokens
    pub fn correlation_matrix(&self, tokens: &[String]) -> CorrelationMatrix {
        let mut matrix = HashMap::new();

        for i in 0..tokens.len() {
            for j in 0..tokens.len() {
                let correlation = if i == j {
                    Some(Decimal::ONE) // Perfect correlation with self
                } else {
                    self.calculate_correlation(&tokens[i], &tokens[j])
                };

                if let Some(corr) = correlation {
                    matrix.insert((tokens[i].clone(), tokens[j].clone()), corr);
                }
            }
        }

        CorrelationMatrix {
            tokens: tokens.to_vec(),
            correlations: matrix,
        }
    }

    /// Detects correlation breakdown (potential arbitrage signal)
    pub fn detect_correlation_breakdown(
        &self,
        token_a: &str,
        token_b: &str,
        historical_correlation: Decimal,
        threshold: Decimal,
    ) -> Option<CorrelationBreakdown> {
        let current_correlation = self.calculate_correlation(token_a, token_b)?;
        let deviation = (current_correlation - historical_correlation).abs();

        if deviation > threshold {
            Some(CorrelationBreakdown {
                token_a: token_a.to_string(),
                token_b: token_b.to_string(),
                historical_correlation,
                current_correlation,
                deviation,
                detected_at: SystemTime::now(),
            })
        } else {
            None
        }
    }
}

impl Default for PriceCorrelationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Correlation matrix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    pub tokens: Vec<String>,
    pub correlations: HashMap<(String, String), Decimal>,
}

/// Correlation breakdown alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationBreakdown {
    pub token_a: String,
    pub token_b: String,
    pub historical_correlation: Decimal,
    pub current_correlation: Decimal,
    pub deviation: Decimal,
    pub detected_at: SystemTime,
}

/// Market regime detector
pub struct MarketRegimeDetector {
    /// Price volatility history
    volatility_history: Vec<VolatilityPoint>,
    /// Current regime
    current_regime: MarketRegime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketRegime {
    /// Low volatility, sideways movement
    LowVolatility,
    /// High volatility
    HighVolatility,
    /// Trending up
    BullTrend,
    /// Trending down
    BearTrend,
    /// Mean-reverting behavior
    MeanReverting,
}

#[derive(Debug, Clone)]
struct VolatilityPoint {
    #[allow(dead_code)]
    timestamp: SystemTime,
    volatility: Decimal,
}

impl MarketRegimeDetector {
    pub fn new() -> Self {
        Self {
            volatility_history: Vec::new(),
            current_regime: MarketRegime::LowVolatility,
        }
    }

    /// Updates market regime based on price data
    pub fn update_regime(
        &mut self,
        current_volatility: Decimal,
        price_trend: Decimal,
        mean_reversion_score: Decimal,
    ) -> MarketRegime {
        self.volatility_history.push(VolatilityPoint {
            timestamp: SystemTime::now(),
            volatility: current_volatility,
        });

        // Simple regime classification
        let regime = if current_volatility > Decimal::new(20, 0) {
            MarketRegime::HighVolatility
        } else if current_volatility < Decimal::new(5, 0) {
            if mean_reversion_score > Decimal::new(70, 2) {
                MarketRegime::MeanReverting
            } else {
                MarketRegime::LowVolatility
            }
        } else if price_trend > Decimal::new(5, 0) {
            MarketRegime::BullTrend
        } else if price_trend < Decimal::new(-5, 0) {
            MarketRegime::BearTrend
        } else if mean_reversion_score > Decimal::new(60, 2) {
            MarketRegime::MeanReverting
        } else {
            MarketRegime::LowVolatility
        };

        self.current_regime = regime;
        regime
    }

    /// Gets current market regime
    pub fn current_regime(&self) -> MarketRegime {
        self.current_regime
    }

    /// Gets regime statistics
    pub fn get_regime_stats(&self) -> RegimeStatistics {
        let avg_volatility = if !self.volatility_history.is_empty() {
            let sum: Decimal = self.volatility_history.iter().map(|v| v.volatility).sum();
            sum / Decimal::from(self.volatility_history.len())
        } else {
            Decimal::ZERO
        };

        RegimeStatistics {
            current_regime: self.current_regime,
            average_volatility: avg_volatility,
            data_points: self.volatility_history.len(),
        }
    }
}

impl Default for MarketRegimeDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeStatistics {
    pub current_regime: MarketRegime,
    pub average_volatility: Decimal,
    pub data_points: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_venue_arbitrage_detection() {
        let mut detector = CrossVenueArbitrageDetector::with_defaults();

        let mut venue1_prices = HashMap::new();
        venue1_prices.insert("TOKEN1".to_string(), Decimal::new(100, 0));

        let mut venue2_prices = HashMap::new();
        venue2_prices.insert("TOKEN1".to_string(), Decimal::new(105, 0));

        detector.update_price_feed("venue1".to_string(), venue1_prices);
        detector.update_price_feed("venue2".to_string(), venue2_prices);

        let opportunities = detector.detect_two_venue_arbitrage("TOKEN1");
        assert!(!opportunities.is_empty());

        let opp = &opportunities[0];
        assert_eq!(opp.arbitrage_type, ArbitrageType::TwoVenue);
        assert!(opp.expected_profit > Decimal::ZERO);
    }

    #[test]
    fn test_price_correlation() {
        let mut analyzer = PriceCorrelationAnalyzer::new();

        // Add correlated price data
        for i in 0..10 {
            let price = Decimal::from(100 + i * 10);
            analyzer.add_price_point("TOKEN_A".to_string(), price);
            analyzer.add_price_point("TOKEN_B".to_string(), price + Decimal::from(5));
        }

        let correlation = analyzer.calculate_correlation("TOKEN_A", "TOKEN_B");
        assert!(correlation.is_some());

        // Should be highly correlated
        let corr = correlation.unwrap();
        assert!(corr > Decimal::new(90, 2)); // > 0.90
    }

    #[test]
    fn test_correlation_matrix() {
        let mut analyzer = PriceCorrelationAnalyzer::new();

        let tokens = vec!["TOKEN_A".to_string(), "TOKEN_B".to_string()];

        for i in 0..10 {
            analyzer.add_price_point("TOKEN_A".to_string(), Decimal::from(100 + i));
            analyzer.add_price_point("TOKEN_B".to_string(), Decimal::from(100 + i));
        }

        let matrix = analyzer.correlation_matrix(&tokens);
        assert_eq!(matrix.tokens.len(), 2);
    }

    #[test]
    fn test_market_regime_detection() {
        let mut detector = MarketRegimeDetector::new();

        // High volatility scenario
        let regime = detector.update_regime(
            Decimal::new(25, 0), // 25% volatility
            Decimal::new(2, 0),  // 2% trend
            Decimal::new(30, 2), // 0.30 mean reversion score
        );

        assert_eq!(regime, MarketRegime::HighVolatility);

        // Low volatility scenario
        let regime2 = detector.update_regime(
            Decimal::new(3, 0),  // 3% volatility
            Decimal::new(1, 0),  // 1% trend
            Decimal::new(80, 2), // 0.80 mean reversion score
        );

        assert_eq!(regime2, MarketRegime::MeanReverting);
    }

    #[test]
    fn test_regime_statistics() {
        let mut detector = MarketRegimeDetector::new();

        detector.update_regime(Decimal::new(10, 0), Decimal::new(5, 0), Decimal::new(40, 2));

        let stats = detector.get_regime_stats();
        assert_eq!(stats.data_points, 1);
        assert!(stats.average_volatility > Decimal::ZERO);
    }
}
