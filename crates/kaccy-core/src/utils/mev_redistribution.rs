//! MEV Redistribution Mechanisms
//!
//! Provides comprehensive MEV (Maximal Extractable Value) redistribution including:
//! - MEV capture and redistribution to users
//! - Searcher competition optimization
//! - Fair ordering guarantees
//! - Priority gas auctions (PGA) mitigation

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

use crate::error::{CoreError, Result};

/// MEV opportunity type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MEVOpportunityType {
    /// Arbitrage between different pools/venues
    Arbitrage,
    /// Sandwich attack (front-run + back-run)
    Sandwich,
    /// Liquidation opportunity
    Liquidation,
    /// Backrunning a trade
    Backrun,
    /// Frontrunning a trade
    Frontrun,
    /// JIT (Just-In-Time) liquidity
    JITLiquidity,
}

/// MEV opportunity detected in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MEVOpportunity {
    /// Opportunity ID
    pub id: String,
    /// Type of MEV
    pub opportunity_type: MEVOpportunityType,
    /// Estimated profit (in base currency)
    pub estimated_profit: Decimal,
    /// Block number where opportunity exists
    pub block_number: u64,
    /// Transaction indices involved
    pub transaction_indices: Vec<u64>,
    /// Token pairs involved
    pub token_pairs: Vec<(String, String)>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// MEV searcher (entity that searches for and captures MEV)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MEVSearcher {
    /// Searcher ID
    pub id: String,
    /// Searcher address
    pub address: String,
    /// Total MEV captured
    pub total_mev_captured: Decimal,
    /// Number of successful captures
    pub successful_captures: u64,
    /// Number of failed attempts
    pub failed_attempts: u64,
    /// Success rate
    pub success_rate: f64,
    /// Reputation score (0.0 to 1.0)
    pub reputation: f64,
    /// Stake amount (for reputation)
    pub stake: Decimal,
    /// Registration timestamp
    pub registered_at: DateTime<Utc>,
}

impl MEVSearcher {
    /// Create a new MEV searcher
    pub fn new(id: String, address: String, stake: Decimal) -> Self {
        Self {
            id,
            address,
            total_mev_captured: Decimal::ZERO,
            successful_captures: 0,
            failed_attempts: 0,
            success_rate: 0.0,
            reputation: 0.5, // Start with neutral reputation
            stake,
            registered_at: Utc::now(),
        }
    }

    /// Record a successful MEV capture
    pub fn record_success(&mut self, mev_captured: Decimal) {
        self.total_mev_captured += mev_captured;
        self.successful_captures += 1;
        self.update_success_rate();
        self.update_reputation();
    }

    /// Record a failed MEV attempt
    pub fn record_failure(&mut self) {
        self.failed_attempts += 1;
        self.update_success_rate();
        self.update_reputation();
    }

    /// Update success rate
    fn update_success_rate(&mut self) {
        let total_attempts = self.successful_captures + self.failed_attempts;
        if total_attempts > 0 {
            self.success_rate = self.successful_captures as f64 / total_attempts as f64;
        }
    }

    /// Update reputation based on success rate and stake
    fn update_reputation(&mut self) {
        // Reputation is a combination of success rate (70%) and stake amount (30%)
        let stake_ratio = self.stake / dec!(1000);
        let stake_component = if self.stake >= dec!(1000) {
            1.0
        } else {
            stake_ratio.to_string().parse::<f64>().unwrap_or(0.0)
        };

        self.reputation = (self.success_rate * 0.7) + (stake_component * 0.3);
    }
}

/// MEV auction bid from a searcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MEVAuctionBid {
    /// Bid ID
    pub id: String,
    /// Searcher making the bid
    pub searcher_id: String,
    /// Opportunity being bid on
    pub opportunity_id: String,
    /// Bid amount (how much searcher is willing to share)
    pub bid_amount: Decimal,
    /// Share percentage (percentage of MEV profit to redistribute)
    pub share_percentage: Decimal,
    /// Gas price offered
    pub gas_price: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl MEVAuctionBid {
    /// Create a new MEV auction bid
    pub fn new(
        id: String,
        searcher_id: String,
        opportunity_id: String,
        bid_amount: Decimal,
        share_percentage: Decimal,
        gas_price: Decimal,
    ) -> Result<Self> {
        if share_percentage < Decimal::ZERO || share_percentage > dec!(100) {
            return Err(CoreError::Validation(
                "Share percentage must be between 0 and 100".to_string(),
            ));
        }

        Ok(Self {
            id,
            searcher_id,
            opportunity_id,
            bid_amount,
            share_percentage,
            gas_price,
            timestamp: Utc::now(),
        })
    }

    /// Calculate the value score for ranking bids
    /// Higher is better (combines bid amount and share percentage)
    pub fn value_score(&self) -> Decimal {
        self.bid_amount * (self.share_percentage / dec!(100))
    }
}

/// MEV redistribution recipient
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MEVRecipient {
    /// Recipient address
    pub address: String,
    /// Weight in redistribution (based on activity, fees paid, etc.)
    pub weight: Decimal,
    /// Total MEV received
    pub total_received: Decimal,
}

/// MEV redistribution event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MEVRedistribution {
    /// Redistribution ID
    pub id: String,
    /// Opportunity that generated the MEV
    pub opportunity_id: String,
    /// Total MEV captured
    pub total_mev: Decimal,
    /// Amount redistributed to users
    pub redistributed_amount: Decimal,
    /// Amount kept by protocol
    pub protocol_fee: Decimal,
    /// Recipients and their amounts
    pub recipients: Vec<(String, Decimal)>,
    /// Searcher who captured the MEV
    pub searcher_id: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Transaction ordering mode for fair ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderingMode {
    /// First-come-first-serve (timestamp order)
    FCFS,
    /// Fair sequencing (encrypted mempool + reveal)
    FairSequencing,
    /// Batch auction (all txs in a block priced equally)
    BatchAuction,
    /// Priority by fee (traditional)
    PriorityFee,
}

/// Fair ordering configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FairOrderingConfig {
    /// Ordering mode
    pub mode: OrderingMode,
    /// Minimum time between transactions from same user (ms)
    pub min_time_between_txs: u64,
    /// Maximum position change allowed in reordering
    pub max_position_change: u32,
    /// Enable anti-sandwich protection
    pub anti_sandwich: bool,
    /// Enable frontrunning detection
    pub frontrun_detection: bool,
}

impl Default for FairOrderingConfig {
    fn default() -> Self {
        Self {
            mode: OrderingMode::FairSequencing,
            min_time_between_txs: 100, // 100ms
            max_position_change: 3,
            anti_sandwich: true,
            frontrun_detection: true,
        }
    }
}

/// MEV redistribution manager
#[derive(Debug)]
pub struct MEVRedistributionManager {
    /// Registered searchers
    pub searchers: HashMap<String, MEVSearcher>,
    /// Detected opportunities
    pub opportunities: HashMap<String, MEVOpportunity>,
    /// Active auction bids
    pub bids: HashMap<String, Vec<MEVAuctionBid>>,
    /// Recipients for redistribution
    pub recipients: HashMap<String, MEVRecipient>,
    /// Redistribution history
    pub redistributions: Vec<MEVRedistribution>,
    /// Fair ordering configuration
    pub fair_ordering_config: FairOrderingConfig,
    /// Protocol MEV fee percentage (default 10%)
    pub protocol_fee_percentage: Decimal,
    /// Minimum share percentage required from searchers (default 50%)
    pub min_share_percentage: Decimal,
}

impl MEVRedistributionManager {
    /// Create a new MEV redistribution manager
    pub fn new(protocol_fee_percentage: Decimal, min_share_percentage: Decimal) -> Result<Self> {
        if protocol_fee_percentage < Decimal::ZERO || protocol_fee_percentage > dec!(100) {
            return Err(CoreError::Validation(
                "Protocol fee percentage must be between 0 and 100".to_string(),
            ));
        }
        if min_share_percentage < Decimal::ZERO || min_share_percentage > dec!(100) {
            return Err(CoreError::Validation(
                "Min share percentage must be between 0 and 100".to_string(),
            ));
        }

        Ok(Self {
            searchers: HashMap::new(),
            opportunities: HashMap::new(),
            bids: HashMap::new(),
            recipients: HashMap::new(),
            redistributions: Vec::new(),
            fair_ordering_config: FairOrderingConfig::default(),
            protocol_fee_percentage,
            min_share_percentage,
        })
    }

    /// Register a new MEV searcher
    pub fn register_searcher(&mut self, searcher: MEVSearcher) -> Result<()> {
        if self.searchers.contains_key(&searcher.id) {
            return Err(CoreError::Validation(format!(
                "Searcher {} already registered",
                searcher.id
            )));
        }
        self.searchers.insert(searcher.id.clone(), searcher);
        Ok(())
    }

    /// Detect and register an MEV opportunity
    pub fn detect_opportunity(&mut self, opportunity: MEVOpportunity) -> Result<()> {
        if self.opportunities.contains_key(&opportunity.id) {
            return Err(CoreError::Validation(format!(
                "Opportunity {} already exists",
                opportunity.id
            )));
        }
        let opportunity_id = opportunity.id.clone();
        self.opportunities
            .insert(opportunity_id.clone(), opportunity);
        self.bids.insert(opportunity_id, Vec::new());
        Ok(())
    }

    /// Submit a bid for an MEV opportunity
    pub fn submit_bid(&mut self, bid: MEVAuctionBid) -> Result<()> {
        // Verify searcher exists
        if !self.searchers.contains_key(&bid.searcher_id) {
            return Err(CoreError::Validation(format!(
                "Searcher {} not registered",
                bid.searcher_id
            )));
        }

        // Verify opportunity exists
        if !self.opportunities.contains_key(&bid.opportunity_id) {
            return Err(CoreError::Validation(format!(
                "Opportunity {} not found",
                bid.opportunity_id
            )));
        }

        // Verify minimum share percentage
        if bid.share_percentage < self.min_share_percentage {
            return Err(CoreError::Validation(format!(
                "Share percentage {} is below minimum {}",
                bid.share_percentage, self.min_share_percentage
            )));
        }

        // Add bid to auction
        self.bids.get_mut(&bid.opportunity_id).unwrap().push(bid);

        Ok(())
    }

    /// Select winning bid for an opportunity
    pub fn select_winner(&mut self, opportunity_id: &str) -> Result<MEVAuctionBid> {
        let bids = self.bids.get(opportunity_id).ok_or_else(|| {
            CoreError::Validation(format!("No bids for opportunity {}", opportunity_id))
        })?;

        if bids.is_empty() {
            return Err(CoreError::Validation(format!(
                "No bids submitted for opportunity {}",
                opportunity_id
            )));
        }

        // Select bid with highest value score
        let winning_bid = bids
            .iter()
            .max_by(|a, b| {
                a.value_score()
                    .partial_cmp(&b.value_score())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap()
            .clone();

        Ok(winning_bid)
    }

    /// Add a recipient for MEV redistribution
    pub fn add_recipient(&mut self, address: String, weight: Decimal) {
        self.recipients
            .entry(address.clone())
            .and_modify(|r| r.weight = weight)
            .or_insert(MEVRecipient {
                address,
                weight,
                total_received: Decimal::ZERO,
            });
    }

    /// Execute MEV capture and redistribution
    pub fn execute_redistribution(
        &mut self,
        opportunity_id: &str,
        actual_mev_captured: Decimal,
    ) -> Result<MEVRedistribution> {
        // Get the opportunity (verify it exists)
        let _opportunity = self.opportunities.get(opportunity_id).ok_or_else(|| {
            CoreError::Validation(format!("Opportunity {} not found", opportunity_id))
        })?;

        // Select winning bid
        let winning_bid = self.select_winner(opportunity_id)?;

        // Calculate redistribution
        let protocol_fee = actual_mev_captured * (self.protocol_fee_percentage / dec!(100));
        let searcher_share = actual_mev_captured - protocol_fee;
        let redistributed_amount = searcher_share * (winning_bid.share_percentage / dec!(100));

        // Distribute to recipients based on weights
        let total_weight: Decimal = self.recipients.values().map(|r| r.weight).sum();
        let mut recipient_amounts = Vec::new();

        if total_weight > Decimal::ZERO {
            for recipient in self.recipients.values_mut() {
                let share = (recipient.weight / total_weight) * redistributed_amount;
                recipient.total_received += share;
                recipient_amounts.push((recipient.address.clone(), share));
            }
        }

        // Update searcher stats
        if let Some(searcher) = self.searchers.get_mut(&winning_bid.searcher_id) {
            searcher.record_success(actual_mev_captured);
        }

        // Create redistribution record
        let redistribution = MEVRedistribution {
            id: format!("redistrib-{}", uuid::Uuid::new_v4()),
            opportunity_id: opportunity_id.to_string(),
            total_mev: actual_mev_captured,
            redistributed_amount,
            protocol_fee,
            recipients: recipient_amounts,
            searcher_id: winning_bid.searcher_id.clone(),
            timestamp: Utc::now(),
        };

        self.redistributions.push(redistribution.clone());

        // Clear bids for this opportunity
        self.bids.remove(opportunity_id);

        Ok(redistribution)
    }

    /// Get total MEV redistributed
    pub fn total_redistributed(&self) -> Decimal {
        self.redistributions
            .iter()
            .map(|r| r.redistributed_amount)
            .sum()
    }

    /// Get total protocol fees collected
    pub fn total_protocol_fees(&self) -> Decimal {
        self.redistributions.iter().map(|r| r.protocol_fee).sum()
    }

    /// Get searcher statistics
    pub fn get_searcher_stats(&self, searcher_id: &str) -> Option<&MEVSearcher> {
        self.searchers.get(searcher_id)
    }

    /// Get recipient statistics
    pub fn get_recipient_stats(&self, address: &str) -> Option<&MEVRecipient> {
        self.recipients.get(address)
    }
}

impl Default for MEVRedistributionManager {
    fn default() -> Self {
        Self::new(dec!(10), dec!(50)).unwrap()
    }
}

/// Priority Gas Auction (PGA) mitigation system
#[derive(Debug)]
pub struct PGAMitigator {
    /// Recent gas prices seen
    pub recent_gas_prices: VecDeque<Decimal>,
    /// Maximum gas price increase allowed per block (e.g., 50% = 1.5x)
    pub max_gas_price_multiplier: Decimal,
    /// Flagged transactions (potential PGA participants)
    pub flagged_txs: HashMap<String, u32>,
    /// Window size for tracking gas prices
    pub window_size: usize,
}

impl PGAMitigator {
    /// Create a new PGA mitigator
    pub fn new(max_gas_price_multiplier: Decimal, window_size: usize) -> Self {
        Self {
            recent_gas_prices: VecDeque::with_capacity(window_size),
            max_gas_price_multiplier,
            flagged_txs: HashMap::new(),
            window_size,
        }
    }

    /// Check if a transaction is engaging in PGA
    pub fn check_transaction(&mut self, tx_hash: &str, gas_price: Decimal) -> Result<bool> {
        // Calculate median gas price from recent transactions
        let median_gas_price = if self.recent_gas_prices.is_empty() {
            gas_price
        } else {
            let mut sorted: Vec<Decimal> = self.recent_gas_prices.iter().copied().collect();
            sorted.sort();
            sorted[sorted.len() / 2]
        };

        // Check if gas price is excessively high
        let is_excessive = gas_price > median_gas_price * self.max_gas_price_multiplier;

        if is_excessive {
            // Flag the transaction
            *self.flagged_txs.entry(tx_hash.to_string()).or_insert(0) += 1;
        }

        // Add to recent gas prices
        if self.recent_gas_prices.len() >= self.window_size {
            self.recent_gas_prices.pop_front();
        }
        self.recent_gas_prices.push_back(gas_price);

        Ok(is_excessive)
    }

    /// Get the number of times a transaction has been flagged
    pub fn get_flag_count(&self, tx_hash: &str) -> u32 {
        *self.flagged_txs.get(tx_hash).unwrap_or(&0)
    }

    /// Clear flags older than a certain threshold
    pub fn clear_old_flags(&mut self) {
        self.flagged_txs.retain(|_, count| *count < 10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mev_searcher() {
        let mut searcher =
            MEVSearcher::new("searcher-1".to_string(), "0x123".to_string(), dec!(1000));

        assert_eq!(searcher.successful_captures, 0);
        assert_eq!(searcher.total_mev_captured, Decimal::ZERO);

        searcher.record_success(dec!(100));
        assert_eq!(searcher.successful_captures, 1);
        assert_eq!(searcher.total_mev_captured, dec!(100));

        searcher.record_failure();
        assert_eq!(searcher.failed_attempts, 1);
        assert_eq!(searcher.success_rate, 0.5);
    }

    #[test]
    fn test_mev_auction_bid() {
        let bid = MEVAuctionBid::new(
            "bid-1".to_string(),
            "searcher-1".to_string(),
            "opp-1".to_string(),
            dec!(100),
            dec!(60),
            dec!(50),
        )
        .unwrap();

        assert_eq!(bid.value_score(), dec!(60));

        // Invalid share percentage
        let invalid = MEVAuctionBid::new(
            "bid-2".to_string(),
            "searcher-1".to_string(),
            "opp-1".to_string(),
            dec!(100),
            dec!(150), // > 100%
            dec!(50),
        );
        assert!(invalid.is_err());
    }

    #[test]
    fn test_mev_redistribution_manager() {
        let mut manager = MEVRedistributionManager::default();

        // Register searcher
        let searcher = MEVSearcher::new("searcher-1".to_string(), "0x123".to_string(), dec!(1000));
        manager.register_searcher(searcher).unwrap();

        // Detect opportunity
        let opportunity = MEVOpportunity {
            id: "opp-1".to_string(),
            opportunity_type: MEVOpportunityType::Arbitrage,
            estimated_profit: dec!(100),
            block_number: 1000,
            transaction_indices: vec![1, 2],
            token_pairs: vec![("USDC".to_string(), "ETH".to_string())],
            timestamp: Utc::now(),
        };
        manager.detect_opportunity(opportunity).unwrap();

        // Submit bid
        let bid = MEVAuctionBid::new(
            "bid-1".to_string(),
            "searcher-1".to_string(),
            "opp-1".to_string(),
            dec!(100),
            dec!(60),
            dec!(50),
        )
        .unwrap();
        manager.submit_bid(bid).unwrap();

        // Add recipients
        manager.add_recipient("user1".to_string(), dec!(1));
        manager.add_recipient("user2".to_string(), dec!(2));

        // Execute redistribution
        let redistribution = manager.execute_redistribution("opp-1", dec!(100)).unwrap();

        assert_eq!(redistribution.total_mev, dec!(100));
        assert!(redistribution.redistributed_amount > Decimal::ZERO);
        assert!(redistribution.protocol_fee > Decimal::ZERO);
    }

    #[test]
    fn test_pga_mitigator() {
        let mut mitigator = PGAMitigator::new(dec!(1.5), 10);

        // Normal gas prices
        assert!(!mitigator.check_transaction("tx1", dec!(50)).unwrap());
        assert!(!mitigator.check_transaction("tx2", dec!(55)).unwrap());

        // Excessive gas price (3x median)
        assert!(mitigator.check_transaction("tx3", dec!(150)).unwrap());
        assert_eq!(mitigator.get_flag_count("tx3"), 1);
    }
}
