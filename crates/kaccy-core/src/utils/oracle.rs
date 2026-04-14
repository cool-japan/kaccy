//! Decentralized oracle integration for price feeds and event verification
use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Oracle provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OracleProvider {
    /// Chainlink decentralized oracle network
    Chainlink,
    /// Pyth Network high-frequency oracle
    Pyth,
    /// Band Protocol oracle
    Band,
    /// API3 first-party oracle
    API3,
    /// Tellor decentralized oracle
    Tellor,
    /// UMA optimistic oracle
    UMA,
    /// Custom/internal oracle
    Custom,
}

impl OracleProvider {
    /// Get the typical update frequency in seconds
    pub fn update_frequency(&self) -> u64 {
        match self {
            OracleProvider::Chainlink => 60, // 1 minute
            OracleProvider::Pyth => 1,       // Real-time
            OracleProvider::Band => 30,      // 30 seconds
            OracleProvider::API3 => 60,      // 1 minute
            OracleProvider::Tellor => 300,   // 5 minutes
            OracleProvider::UMA => 7200,     // 2 hours
            OracleProvider::Custom => 60,    // 1 minute
        }
    }

    /// Get reliability score (0-100)
    pub fn reliability_score(&self) -> u8 {
        match self {
            OracleProvider::Chainlink => 95,
            OracleProvider::Pyth => 90,
            OracleProvider::Band => 85,
            OracleProvider::API3 => 85,
            OracleProvider::Tellor => 80,
            OracleProvider::UMA => 85,
            OracleProvider::Custom => 70,
        }
    }
}

/// Price feed from an oracle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceFeed {
    /// Unique identifier of this price feed entry
    pub id: Uuid,
    /// Trading pair symbol (e.g. "BTC/USD")
    pub symbol: String,
    /// Current price
    pub price: Decimal,
    /// Source oracle provider
    pub provider: OracleProvider,
    /// When this price was reported
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Optional ±confidence bound around the price
    pub confidence_interval: Option<Decimal>,
    /// Number of decimal places in the price
    pub decimals: u8,
}

impl PriceFeed {
    /// Create a new price feed
    pub fn new(symbol: String, price: Decimal, provider: OracleProvider, decimals: u8) -> Self {
        Self {
            id: Uuid::new_v4(),
            symbol,
            price,
            provider,
            timestamp: chrono::Utc::now(),
            confidence_interval: None,
            decimals,
        }
    }

    /// Check if price feed is stale
    pub fn is_stale(&self, max_age_seconds: u64) -> bool {
        let age = chrono::Utc::now()
            .signed_duration_since(self.timestamp)
            .num_seconds();
        age > max_age_seconds as i64
    }

    /// Get price with confidence bounds
    pub fn get_bounds(&self) -> (Decimal, Decimal) {
        if let Some(confidence) = self.confidence_interval {
            let lower = self.price - confidence;
            let upper = self.price + confidence;
            (lower, upper)
        } else {
            (self.price, self.price)
        }
    }
}

/// Oracle aggregator for consensus pricing
#[derive(Debug, Clone)]
pub struct OracleAggregator {
    /// Price feeds indexed by symbol
    feeds: HashMap<String, Vec<PriceFeed>>,
    /// Minimum number of independent sources required
    min_sources: usize,
}

impl OracleAggregator {
    /// Create a new oracle aggregator
    pub fn new(min_sources: usize) -> Self {
        Self {
            feeds: HashMap::new(),
            min_sources,
        }
    }

    /// Add a price feed
    pub fn add_feed(&mut self, feed: PriceFeed) {
        self.feeds
            .entry(feed.symbol.clone())
            .or_default()
            .push(feed);
    }

    /// Get median price from multiple sources
    pub fn get_median_price(&self, symbol: &str) -> Result<Decimal, CoreError> {
        let feeds = self
            .feeds
            .get(symbol)
            .ok_or_else(|| CoreError::NotFound(format!("No price feeds for symbol: {}", symbol)))?;

        if feeds.len() < self.min_sources {
            return Err(CoreError::Validation(format!(
                "Insufficient price sources: {} < {}",
                feeds.len(),
                self.min_sources
            )));
        }

        // Remove stale feeds
        let fresh_feeds: Vec<_> = feeds
            .iter()
            .filter(|f| !f.is_stale(300)) // 5 minute staleness threshold
            .collect();

        if fresh_feeds.len() < self.min_sources {
            return Err(CoreError::Validation(
                "Insufficient fresh price sources".to_string(),
            ));
        }

        // Get median price
        let mut prices: Vec<Decimal> = fresh_feeds.iter().map(|f| f.price).collect();
        prices.sort();

        let median = if prices.len() % 2 == 0 {
            let mid = prices.len() / 2;
            (prices[mid - 1] + prices[mid]) / Decimal::from(2)
        } else {
            prices[prices.len() / 2]
        };

        Ok(median)
    }

    /// Get weighted average price based on provider reliability
    pub fn get_weighted_price(&self, symbol: &str) -> Result<Decimal, CoreError> {
        let feeds = self
            .feeds
            .get(symbol)
            .ok_or_else(|| CoreError::NotFound(format!("No price feeds for symbol: {}", symbol)))?;

        if feeds.len() < self.min_sources {
            return Err(CoreError::Validation(
                "Insufficient price sources".to_string(),
            ));
        }

        let fresh_feeds: Vec<_> = feeds.iter().filter(|f| !f.is_stale(300)).collect();

        if fresh_feeds.is_empty() {
            return Err(CoreError::Validation("No fresh price feeds".to_string()));
        }

        let mut total_weighted_price = Decimal::ZERO;
        let mut total_weight = Decimal::ZERO;

        for feed in fresh_feeds {
            let weight = Decimal::from(feed.provider.reliability_score());
            total_weighted_price += feed.price * weight;
            total_weight += weight;
        }

        if total_weight == Decimal::ZERO {
            return Err(CoreError::Validation("Zero total weight".to_string()));
        }

        Ok(total_weighted_price / total_weight)
    }

    /// Detect price anomalies (outliers)
    pub fn detect_outliers(&self, symbol: &str) -> Vec<(OracleProvider, Decimal)> {
        let feeds = match self.feeds.get(symbol) {
            Some(f) => f,
            None => return vec![],
        };

        if feeds.len() < 3 {
            return vec![];
        }

        let prices: Vec<Decimal> = feeds.iter().map(|f| f.price).collect();
        let mean = prices.iter().sum::<Decimal>() / Decimal::from(prices.len());

        // Calculate standard deviation
        let variance = prices
            .iter()
            .map(|p| {
                let diff = *p - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / Decimal::from(prices.len());

        // Simple outlier detection: prices > 2 standard deviations from mean
        let threshold = Decimal::from(2);
        let mut outliers = Vec::new();

        for feed in feeds {
            let diff = (feed.price - mean).abs();
            // Simplified check (would need proper sqrt for std dev)
            if diff * diff > variance * threshold {
                outliers.push((feed.provider, feed.price));
            }
        }

        outliers
    }
}

/// Oracle dispute resolution system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleDispute {
    /// Unique identifier of this dispute
    pub id: Uuid,
    /// Trading pair symbol under dispute
    pub symbol: String,
    /// Price being challenged
    pub disputed_price: Decimal,
    /// Oracle that reported the disputed price
    pub disputed_provider: OracleProvider,
    /// User who raised the dispute
    pub challenger_id: Uuid,
    /// Amount staked by the challenger
    pub stake_amount: Decimal,
    /// Current state of the dispute
    pub status: DisputeStatus,
    /// When the dispute was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the dispute was resolved or rejected
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Outcome of the dispute
    pub resolution: Option<DisputeResolution>,
}

/// Status of an oracle price dispute
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeStatus {
    /// Dispute submitted and awaiting review
    Pending,
    /// Dispute is being actively reviewed
    UnderReview,
    /// Dispute was resolved in the challenger's favour
    Resolved,
    /// Dispute was rejected as invalid
    Rejected,
}

/// Outcome record for a resolved oracle dispute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeResolution {
    /// The correct price determined by the resolution process
    pub correct_price: Decimal,
    /// Oracle provider that was penalised, if any
    pub slashed_provider: Option<OracleProvider>,
    /// Reward paid to the successful challenger
    pub reward_amount: Decimal,
}

impl OracleDispute {
    /// Create a new oracle dispute
    pub fn new(
        symbol: String,
        disputed_price: Decimal,
        disputed_provider: OracleProvider,
        challenger_id: Uuid,
        stake_amount: Decimal,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            symbol,
            disputed_price,
            disputed_provider,
            challenger_id,
            stake_amount,
            status: DisputeStatus::Pending,
            created_at: chrono::Utc::now(),
            resolved_at: None,
            resolution: None,
        }
    }

    /// Resolve the dispute
    pub fn resolve(&mut self, resolution: DisputeResolution) {
        self.status = DisputeStatus::Resolved;
        self.resolved_at = Some(chrono::Utc::now());
        self.resolution = Some(resolution);
    }

    /// Reject the dispute
    pub fn reject(&mut self) {
        self.status = DisputeStatus::Rejected;
        self.resolved_at = Some(chrono::Utc::now());
    }
}

/// Event oracle for external event verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventOracle {
    /// Unique identifier of this event oracle
    pub id: Uuid,
    /// Category or name of the event being verified
    pub event_type: String,
    /// Data payload associated with the event
    pub event_data: String,
    /// Attesting data sources
    pub sources: Vec<EventSource>,
    /// Whether a majority consensus was reached
    pub consensus_reached: bool,
    /// When consensus was achieved
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// A single attesting source for an event oracle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSource {
    /// Oracle provider supplying this attestation
    pub provider: OracleProvider,
    /// Raw data string from this source
    pub data: String,
    /// Whether this source has been individually verified
    pub verified: bool,
    /// When this attestation was submitted
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl EventOracle {
    /// Create a new event oracle
    pub fn new(event_type: String, event_data: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type,
            event_data,
            sources: Vec::new(),
            consensus_reached: false,
            verified_at: None,
        }
    }

    /// Add an event source
    pub fn add_source(&mut self, provider: OracleProvider, data: String) {
        self.sources.push(EventSource {
            provider,
            data,
            verified: false,
            timestamp: chrono::Utc::now(),
        });
    }

    /// Check for consensus (simple majority)
    pub fn check_consensus(&mut self, min_sources: usize) -> bool {
        if self.sources.len() < min_sources {
            return false;
        }

        // Count matching data
        let mut data_counts: HashMap<String, usize> = HashMap::new();
        for source in &self.sources {
            *data_counts.entry(source.data.clone()).or_insert(0) += 1;
        }

        // Check if any data has majority
        let total_sources = self.sources.len();
        for (data, count) in data_counts {
            if count > total_sources / 2 {
                self.consensus_reached = true;
                self.verified_at = Some(chrono::Utc::now());
                self.event_data = data;
                return true;
            }
        }

        false
    }
}

/// Fallback mechanism for oracle failures
#[derive(Debug, Clone)]
pub struct OracleFallback {
    /// Preferred oracle providers to query first
    #[allow(dead_code)]
    primary_providers: Vec<OracleProvider>,
    /// Backup oracle providers used when primary sources fail
    #[allow(dead_code)]
    fallback_providers: Vec<OracleProvider>,
    /// Most recent successfully fetched price
    last_successful_price: Option<Decimal>,
    /// Number of consecutive failures since the last success
    failure_count: usize,
}

impl OracleFallback {
    /// Create a new fallback mechanism
    pub fn new(
        primary_providers: Vec<OracleProvider>,
        fallback_providers: Vec<OracleProvider>,
    ) -> Self {
        Self {
            primary_providers,
            fallback_providers,
            last_successful_price: None,
            failure_count: 0,
        }
    }

    /// Get price with fallback
    pub fn get_price_with_fallback(
        &mut self,
        symbol: &str,
        aggregator: &OracleAggregator,
    ) -> Result<Decimal, CoreError> {
        // Try primary providers first
        match aggregator.get_median_price(symbol) {
            Ok(price) => {
                self.last_successful_price = Some(price);
                self.failure_count = 0;
                Ok(price)
            }
            Err(_) => {
                self.failure_count += 1;

                // If we have recent successful price, use it with warning
                if let Some(last_price) = self.last_successful_price {
                    if self.failure_count < 3 {
                        return Ok(last_price);
                    }
                }

                // Try fallback providers
                aggregator.get_weighted_price(symbol)
            }
        }
    }

    /// Check if system is in fallback mode
    pub fn is_in_fallback_mode(&self) -> bool {
        self.failure_count > 0
    }
}

/// Automated settlement trigger based on oracle events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomatedSettlement {
    /// Unique identifier of this settlement rule
    pub id: Uuid,
    /// Human-readable condition that must be satisfied to trigger settlement
    pub event_condition: String,
    /// Action to execute once the condition is met
    pub settlement_action: SettlementAction,
    /// Oracle providers that must attest to the event
    pub oracle_sources: Vec<OracleProvider>,
    /// Minimum number of sources required for consensus
    pub min_consensus: usize,
    /// Whether the settlement has been triggered
    pub triggered: bool,
    /// Whether the settlement action has been executed
    pub executed: bool,
}

/// Action to take when an automated settlement triggers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SettlementAction {
    /// Release escrowed funds to a recipient
    ReleaseEscrow {
        /// Amount to release
        amount: Decimal,
        /// Recipient user ID
        recipient: Uuid,
    },
    /// Execute a trade on-chain
    ExecuteTrade {
        /// Token to trade
        token_id: Uuid,
        /// Trade amount
        amount: Decimal,
    },
    /// Distribute rewards from a pool
    DistributeRewards {
        /// Pool to distribute from
        pool_id: Uuid,
    },
    /// Pause a smart contract
    PauseContract {
        /// Contract to pause
        contract_id: Uuid,
    },
}

impl AutomatedSettlement {
    /// Create a new automated settlement
    pub fn new(
        event_condition: String,
        settlement_action: SettlementAction,
        oracle_sources: Vec<OracleProvider>,
        min_consensus: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_condition,
            settlement_action,
            oracle_sources,
            min_consensus,
            triggered: false,
            executed: false,
        }
    }

    /// Check if settlement should trigger
    pub fn should_trigger(&mut self, event_oracle: &EventOracle) -> bool {
        if self.triggered {
            return false;
        }

        if event_oracle.consensus_reached && event_oracle.sources.len() >= self.min_consensus {
            self.triggered = true;
            true
        } else {
            false
        }
    }

    /// Mark as executed
    pub fn mark_executed(&mut self) {
        self.executed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oracle_provider_properties() {
        assert_eq!(OracleProvider::Chainlink.update_frequency(), 60);
        assert_eq!(OracleProvider::Pyth.update_frequency(), 1);
        assert!(OracleProvider::Chainlink.reliability_score() > 90);
    }

    #[test]
    fn test_price_feed_creation() {
        let feed = PriceFeed::new(
            "BTC/USD".to_string(),
            Decimal::from(50000),
            OracleProvider::Chainlink,
            8,
        );

        assert_eq!(feed.symbol, "BTC/USD");
        assert_eq!(feed.price, Decimal::from(50000));
        assert!(!feed.is_stale(300));
    }

    #[test]
    fn test_price_feed_bounds() {
        let mut feed = PriceFeed::new(
            "BTC/USD".to_string(),
            Decimal::from(50000),
            OracleProvider::Chainlink,
            8,
        );

        feed.confidence_interval = Some(Decimal::from(100));
        let (lower, upper) = feed.get_bounds();

        assert_eq!(lower, Decimal::from(49900));
        assert_eq!(upper, Decimal::from(50100));
    }

    #[test]
    fn test_oracle_aggregator() {
        let mut aggregator = OracleAggregator::new(2);

        let feed1 = PriceFeed::new(
            "BTC/USD".to_string(),
            Decimal::from(50000),
            OracleProvider::Chainlink,
            8,
        );
        let feed2 = PriceFeed::new(
            "BTC/USD".to_string(),
            Decimal::from(50100),
            OracleProvider::Pyth,
            8,
        );

        aggregator.add_feed(feed1);
        aggregator.add_feed(feed2);

        let median = aggregator.get_median_price("BTC/USD").unwrap();
        assert!(median >= Decimal::from(50000) && median <= Decimal::from(50100));
    }

    #[test]
    fn test_oracle_weighted_price() {
        let mut aggregator = OracleAggregator::new(2);

        let feed1 = PriceFeed::new(
            "BTC/USD".to_string(),
            Decimal::from(50000),
            OracleProvider::Chainlink,
            8,
        );
        let feed2 = PriceFeed::new(
            "BTC/USD".to_string(),
            Decimal::from(50100),
            OracleProvider::Pyth,
            8,
        );

        aggregator.add_feed(feed1);
        aggregator.add_feed(feed2);

        let weighted = aggregator.get_weighted_price("BTC/USD").unwrap();
        assert!(weighted > Decimal::ZERO);
    }

    #[test]
    fn test_oracle_dispute() {
        let mut dispute = OracleDispute::new(
            "BTC/USD".to_string(),
            Decimal::from(50000),
            OracleProvider::Custom,
            Uuid::new_v4(),
            Decimal::from(100),
        );

        assert_eq!(dispute.status, DisputeStatus::Pending);

        let resolution = DisputeResolution {
            correct_price: Decimal::from(50100),
            slashed_provider: Some(OracleProvider::Custom),
            reward_amount: Decimal::from(100),
        };

        dispute.resolve(resolution);
        assert_eq!(dispute.status, DisputeStatus::Resolved);
    }

    #[test]
    fn test_event_oracle() {
        let mut event = EventOracle::new("price_target".to_string(), "50000".to_string());

        event.add_source(OracleProvider::Chainlink, "50000".to_string());
        event.add_source(OracleProvider::Pyth, "50000".to_string());
        event.add_source(OracleProvider::Band, "49900".to_string());

        assert!(event.check_consensus(2));
        assert!(event.consensus_reached);
    }

    #[test]
    fn test_oracle_fallback() {
        let mut fallback =
            OracleFallback::new(vec![OracleProvider::Chainlink], vec![OracleProvider::Pyth]);

        assert!(!fallback.is_in_fallback_mode());

        // Simulate successful price fetch
        fallback.last_successful_price = Some(Decimal::from(50000));

        assert!(!fallback.is_in_fallback_mode());
    }

    #[test]
    fn test_automated_settlement() {
        let mut settlement = AutomatedSettlement::new(
            "BTC > 50000".to_string(),
            SettlementAction::ReleaseEscrow {
                amount: Decimal::from(1000),
                recipient: Uuid::new_v4(),
            },
            vec![OracleProvider::Chainlink, OracleProvider::Pyth],
            2,
        );

        let mut event = EventOracle::new("price_condition".to_string(), "true".to_string());
        event.add_source(OracleProvider::Chainlink, "true".to_string());
        event.add_source(OracleProvider::Pyth, "true".to_string());
        event.check_consensus(2);

        assert!(settlement.should_trigger(&event));
        assert!(settlement.triggered);
    }
}
