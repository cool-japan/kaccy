//! Price oracle module for external price data
//!
//! This module provides interfaces and implementations for fetching external price data,
//! particularly BTC/USD prices from various sources.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::error::{CoreError, Result};

/// Price data from an oracle source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    /// Symbol (e.g., "BTC/USD")
    pub symbol: String,
    /// Price value
    pub price: Decimal,
    /// When the price was fetched
    pub timestamp: DateTime<Utc>,
    /// Source of the price (e.g., "coinbase", "binance")
    pub source: String,
    /// Confidence/reliability score (0.0 to 1.0)
    pub confidence: Decimal,
}

impl PriceData {
    /// Create a new price data point
    pub fn new(symbol: String, price: Decimal, source: String) -> Self {
        Self {
            symbol,
            price,
            timestamp: Utc::now(),
            source,
            confidence: dec!(1.0),
        }
    }

    /// Check if price data is stale (older than threshold)
    pub fn is_stale(&self, max_age_seconds: i64) -> bool {
        let age = Utc::now().signed_duration_since(self.timestamp);
        age.num_seconds() > max_age_seconds
    }

    /// Get age in seconds
    pub fn age_seconds(&self) -> i64 {
        Utc::now()
            .signed_duration_since(self.timestamp)
            .num_seconds()
    }
}

/// Aggregated price from multiple sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedPrice {
    /// Symbol (e.g., "BTC/USD")
    pub symbol: String,
    /// Median price from all sources
    pub median_price: Decimal,
    /// Mean (average) price from all sources
    pub mean_price: Decimal,
    /// Minimum price from all sources
    pub min_price: Decimal,
    /// Maximum price from all sources
    pub max_price: Decimal,
    /// Standard deviation of prices
    pub std_dev: Decimal,
    /// Number of sources used
    pub num_sources: usize,
    /// Individual source prices
    pub sources: Vec<PriceData>,
    /// When aggregation was performed
    pub aggregated_at: DateTime<Utc>,
}

impl AggregatedPrice {
    /// Create aggregated price from multiple sources
    pub fn from_sources(symbol: String, mut sources: Vec<PriceData>) -> Result<Self> {
        if sources.is_empty() {
            return Err(CoreError::Validation(
                "Cannot aggregate from empty sources".to_string(),
            ));
        }

        sources.sort_by(|a, b| a.price.cmp(&b.price));
        let prices: Vec<Decimal> = sources.iter().map(|s| s.price).collect();

        let median_price = if prices.len() % 2 == 0 {
            let mid = prices.len() / 2;
            (prices[mid - 1] + prices[mid]) / dec!(2)
        } else {
            prices[prices.len() / 2]
        };

        let sum: Decimal = prices.iter().sum();
        let mean_price = sum / Decimal::from(prices.len());

        let min_price = *prices.first().unwrap();
        let max_price = *prices.last().unwrap();

        // Calculate standard deviation
        let variance: Decimal = prices
            .iter()
            .map(|p| (*p - mean_price) * (*p - mean_price))
            .sum::<Decimal>()
            / Decimal::from(prices.len());
        let std_dev = variance.sqrt().unwrap_or(dec!(0));

        Ok(Self {
            symbol,
            median_price,
            mean_price,
            min_price,
            max_price,
            std_dev,
            num_sources: sources.len(),
            sources,
            aggregated_at: Utc::now(),
        })
    }

    /// Get price spread (max - min) as percentage of median
    pub fn spread_percentage(&self) -> Decimal {
        if self.median_price.is_zero() {
            return dec!(0);
        }
        ((self.max_price - self.min_price) / self.median_price) * dec!(100)
    }

    /// Check if spread is within acceptable threshold
    pub fn is_spread_acceptable(&self, max_spread_pct: Decimal) -> bool {
        self.spread_percentage() <= max_spread_pct
    }

    /// Get coefficient of variation (std_dev / mean)
    pub fn coefficient_of_variation(&self) -> Decimal {
        if self.mean_price.is_zero() {
            return dec!(0);
        }
        self.std_dev / self.mean_price
    }
}

/// Price oracle trait for fetching external prices
#[async_trait::async_trait]
pub trait PriceOracle: Send + Sync {
    /// Get the name of this oracle source
    fn source_name(&self) -> &str;

    /// Fetch current price for a symbol
    async fn fetch_price(&self, symbol: &str) -> Result<PriceData>;

    /// Check if this oracle supports a symbol
    fn supports_symbol(&self, symbol: &str) -> bool;
}

/// Mock price oracle for testing
pub struct MockPriceOracle {
    name: String,
    prices: HashMap<String, Decimal>,
}

impl MockPriceOracle {
    /// Create a new mock oracle
    pub fn new(name: String) -> Self {
        Self {
            name,
            prices: HashMap::new(),
        }
    }

    /// Set a mock price for a symbol
    pub fn set_price(&mut self, symbol: String, price: Decimal) {
        self.prices.insert(symbol, price);
    }
}

#[async_trait::async_trait]
impl PriceOracle for MockPriceOracle {
    fn source_name(&self) -> &str {
        &self.name
    }

    async fn fetch_price(&self, symbol: &str) -> Result<PriceData> {
        self.prices
            .get(symbol)
            .map(|&price| PriceData::new(symbol.to_string(), price, self.name.clone()))
            .ok_or_else(|| CoreError::NotFound(format!("Price not found for {}", symbol)))
    }

    fn supports_symbol(&self, symbol: &str) -> bool {
        self.prices.contains_key(symbol)
    }
}

/// Price oracle aggregator that combines multiple oracles
pub struct OracleAggregator {
    oracles: Vec<Box<dyn PriceOracle>>,
    max_age_seconds: i64,
    max_spread_pct: Decimal,
}

impl OracleAggregator {
    /// Create a new aggregator
    pub fn new() -> Self {
        Self {
            oracles: Vec::new(),
            max_age_seconds: 300,      // 5 minutes
            max_spread_pct: dec!(5.0), // 5% max spread
        }
    }

    /// Add an oracle source
    pub fn add_oracle(&mut self, oracle: Box<dyn PriceOracle>) {
        self.oracles.push(oracle);
    }

    /// Set maximum age for price data
    pub fn with_max_age(mut self, seconds: i64) -> Self {
        self.max_age_seconds = seconds;
        self
    }

    /// Set maximum acceptable spread
    pub fn with_max_spread(mut self, spread_pct: Decimal) -> Self {
        self.max_spread_pct = spread_pct;
        self
    }

    /// Fetch aggregated price from all oracles
    pub async fn fetch_aggregated_price(&self, symbol: &str) -> Result<AggregatedPrice> {
        if self.oracles.is_empty() {
            return Err(CoreError::Configuration(
                "No oracles configured".to_string(),
            ));
        }

        let mut prices = Vec::new();
        let mut errors = Vec::new();

        for oracle in &self.oracles {
            if !oracle.supports_symbol(symbol) {
                continue;
            }

            match oracle.fetch_price(symbol).await {
                Ok(price_data) => {
                    if !price_data.is_stale(self.max_age_seconds) {
                        prices.push(price_data);
                    }
                }
                Err(e) => {
                    errors.push((oracle.source_name(), e));
                }
            }
        }

        if prices.is_empty() {
            return Err(CoreError::NotFound(format!(
                "No valid price data available for {} (errors: {})",
                symbol,
                errors.len()
            )));
        }

        let aggregated = AggregatedPrice::from_sources(symbol.to_string(), prices)?;

        // Check if spread is acceptable
        if !aggregated.is_spread_acceptable(self.max_spread_pct) {
            tracing::warn!(
                symbol = symbol,
                spread_pct = %aggregated.spread_percentage(),
                max_spread_pct = %self.max_spread_pct,
                "Price spread exceeds threshold"
            );
        }

        Ok(aggregated)
    }

    /// Get number of configured oracles
    pub fn oracle_count(&self) -> usize {
        self.oracles.len()
    }
}

impl Default for OracleAggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for OracleAggregator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OracleAggregator")
            .field("oracle_count", &self.oracles.len())
            .field("max_age_seconds", &self.max_age_seconds)
            .field("max_spread_pct", &self.max_spread_pct)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_data_creation() {
        let price = PriceData::new("BTC/USD".to_string(), dec!(50000), "test".to_string());
        assert_eq!(price.symbol, "BTC/USD");
        assert_eq!(price.price, dec!(50000));
        assert_eq!(price.source, "test");
        assert_eq!(price.confidence, dec!(1.0));
    }

    #[test]
    fn test_price_data_staleness() {
        let price = PriceData::new("BTC/USD".to_string(), dec!(50000), "test".to_string());
        assert!(!price.is_stale(3600)); // Not stale after 1 hour
        assert!(price.age_seconds() >= 0);
    }

    #[test]
    fn test_aggregated_price_from_sources() {
        let sources = vec![
            PriceData::new("BTC/USD".to_string(), dec!(50000), "source1".to_string()),
            PriceData::new("BTC/USD".to_string(), dec!(50100), "source2".to_string()),
            PriceData::new("BTC/USD".to_string(), dec!(49900), "source3".to_string()),
        ];

        let aggregated = AggregatedPrice::from_sources("BTC/USD".to_string(), sources).unwrap();
        assert_eq!(aggregated.median_price, dec!(50000));
        assert_eq!(aggregated.min_price, dec!(49900));
        assert_eq!(aggregated.max_price, dec!(50100));
        assert_eq!(aggregated.num_sources, 3);
    }

    #[test]
    fn test_aggregated_price_spread() {
        let sources = vec![
            PriceData::new("BTC/USD".to_string(), dec!(50000), "source1".to_string()),
            PriceData::new("BTC/USD".to_string(), dec!(52000), "source2".to_string()),
        ];

        let aggregated = AggregatedPrice::from_sources("BTC/USD".to_string(), sources).unwrap();
        let spread = aggregated.spread_percentage();
        assert!(spread > dec!(3.8) && spread < dec!(4.0)); // ~3.9%
    }

    #[test]
    fn test_aggregated_price_empty_sources() {
        let result = AggregatedPrice::from_sources("BTC/USD".to_string(), vec![]);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_oracle() {
        let mut oracle = MockPriceOracle::new("test".to_string());
        oracle.set_price("BTC/USD".to_string(), dec!(50000));

        let price = oracle.fetch_price("BTC/USD").await.unwrap();
        assert_eq!(price.price, dec!(50000));
        assert_eq!(price.source, "test");

        assert!(oracle.supports_symbol("BTC/USD"));
        assert!(!oracle.supports_symbol("ETH/USD"));
    }

    #[tokio::test]
    async fn test_oracle_aggregator() {
        let mut oracle1 = MockPriceOracle::new("oracle1".to_string());
        oracle1.set_price("BTC/USD".to_string(), dec!(50000));

        let mut oracle2 = MockPriceOracle::new("oracle2".to_string());
        oracle2.set_price("BTC/USD".to_string(), dec!(50100));

        let mut aggregator = OracleAggregator::new();
        aggregator.add_oracle(Box::new(oracle1));
        aggregator.add_oracle(Box::new(oracle2));

        let aggregated = aggregator.fetch_aggregated_price("BTC/USD").await.unwrap();
        assert_eq!(aggregated.num_sources, 2);
        assert!(aggregated.median_price > dec!(49000));
        assert!(aggregated.median_price < dec!(51000));
    }

    #[tokio::test]
    async fn test_oracle_aggregator_no_oracles() {
        let aggregator = OracleAggregator::new();
        let result = aggregator.fetch_aggregated_price("BTC/USD").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_oracle_aggregator_configuration() {
        let aggregator = OracleAggregator::new()
            .with_max_age(600)
            .with_max_spread(dec!(10.0));

        assert_eq!(aggregator.max_age_seconds, 600);
        assert_eq!(aggregator.max_spread_pct, dec!(10.0));
    }
}
