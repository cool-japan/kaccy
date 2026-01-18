//! On-chain analytics system
//!
//! This module provides on-chain analytics capabilities including:
//! - Wallet profiling
//! - Token holder analysis
//! - Transaction graph analysis
//! - Smart money tracking

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Wallet profile with trading behavior analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletProfile {
    /// Wallet address
    pub address: String,

    /// Total transaction count
    pub tx_count: u64,

    /// Total volume traded (in USD)
    pub total_volume: Decimal,

    /// Number of unique tokens traded
    pub unique_tokens: usize,

    /// Win rate (profitable trades / total trades)
    pub win_rate: f64,

    /// Average profit per trade
    pub avg_profit: Decimal,

    /// Wallet age (days)
    pub age_days: u64,

    /// Activity score (0-100)
    pub activity_score: u32,

    /// Risk score (0-100, higher = riskier)
    pub risk_score: u32,

    /// Wallet category
    pub category: WalletCategory,

    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,
}

/// Wallet category classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletCategory {
    /// High-frequency trader
    HFT,

    /// Long-term holder
    Holder,

    /// Whale (large holder)
    Whale,

    /// Smart money (consistently profitable)
    SmartMoney,

    /// Bot (automated trading)
    Bot,

    /// Casual user
    Casual,

    /// Suspicious (potential manipulation)
    Suspicious,
}

impl WalletProfile {
    /// Create a new wallet profile
    pub fn new(address: String) -> Self {
        Self {
            address,
            tx_count: 0,
            total_volume: Decimal::ZERO,
            unique_tokens: 0,
            win_rate: 0.0,
            avg_profit: Decimal::ZERO,
            age_days: 0,
            activity_score: 0,
            risk_score: 50,
            category: WalletCategory::Casual,
            last_activity: Utc::now(),
            created_at: Utc::now(),
        }
    }

    /// Update profile with new transaction
    pub fn update_with_tx(&mut self, volume: Decimal, profit: Decimal, _token: String) {
        self.tx_count += 1;
        self.total_volume += volume;
        self.last_activity = Utc::now();

        // Update average profit
        let total_profit = self.avg_profit * Decimal::from(self.tx_count - 1) + profit;
        self.avg_profit = total_profit / Decimal::from(self.tx_count);

        // Recalculate scores
        self.recalculate_scores();
    }

    /// Recalculate activity and risk scores
    fn recalculate_scores(&mut self) {
        // Activity score based on transaction frequency and volume
        let volume_score = (self.total_volume / dec!(1000000))
            .min(dec!(50))
            .to_u32()
            .unwrap_or(0);
        let tx_score = (self.tx_count / 10).min(50) as u32;
        self.activity_score = volume_score + tx_score;

        // Risk score based on various factors
        let mut risk = 50u32;

        // High win rate might indicate insider trading
        if self.win_rate > 0.8 {
            risk += 20;
        }

        // Very high activity might indicate bot
        if self.tx_count > 1000 {
            risk += 15;
        }

        self.risk_score = risk.min(100);

        // Update category based on behavior
        self.category = self.classify_wallet();
    }

    /// Classify wallet based on trading behavior
    fn classify_wallet(&self) -> WalletCategory {
        // Whale: > $10M volume
        if self.total_volume > dec!(10000000) {
            return WalletCategory::Whale;
        }

        // Smart money: high win rate and profitable
        if self.win_rate > 0.75 && self.avg_profit > dec!(1000) {
            return WalletCategory::SmartMoney;
        }

        // HFT: high transaction count with low average volume
        if self.tx_count > 500 && self.total_volume / Decimal::from(self.tx_count) < dec!(1000) {
            return WalletCategory::HFT;
        }

        // Bot: very high transaction count with consistent patterns
        if self.tx_count > 1000 {
            return WalletCategory::Bot;
        }

        // Suspicious: high risk score
        if self.risk_score > 80 {
            return WalletCategory::Suspicious;
        }

        // Holder: low transaction count, long age
        if self.tx_count < 50 && self.age_days > 365 {
            return WalletCategory::Holder;
        }

        WalletCategory::Casual
    }

    /// Check if wallet is smart money
    pub fn is_smart_money(&self) -> bool {
        matches!(self.category, WalletCategory::SmartMoney)
    }

    /// Check if wallet is suspicious
    pub fn is_suspicious(&self) -> bool {
        matches!(self.category, WalletCategory::Suspicious) || self.risk_score > 80
    }
}

/// Token holder analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenHolderAnalysis {
    /// Token symbol
    pub token_symbol: String,

    /// Total number of holders
    pub total_holders: usize,

    /// Top 10 holders concentration (%)
    pub top10_concentration: f64,

    /// Top 100 holders concentration (%)
    pub top100_concentration: f64,

    /// Whale count (holders with > 1% of supply)
    pub whale_count: usize,

    /// Average holding time (days)
    pub avg_holding_time: f64,

    /// Holder growth rate (30-day)
    pub holder_growth_rate: f64,

    /// Distribution score (0-100, higher = more distributed)
    pub distribution_score: u32,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl TokenHolderAnalysis {
    /// Create new token holder analysis
    pub fn new(token_symbol: String) -> Self {
        Self {
            token_symbol,
            total_holders: 0,
            top10_concentration: 0.0,
            top100_concentration: 0.0,
            whale_count: 0,
            avg_holding_time: 0.0,
            holder_growth_rate: 0.0,
            distribution_score: 0,
            updated_at: Utc::now(),
        }
    }

    /// Analyze token holders
    pub fn analyze(&mut self, holders: &[(String, Decimal)], total_supply: Decimal) {
        self.total_holders = holders.len();

        // Calculate concentration
        let mut sorted_holders = holders.to_vec();
        sorted_holders.sort_by(|a, b| b.1.cmp(&a.1));

        // Top 10 concentration
        let top10_sum: Decimal = sorted_holders
            .iter()
            .take(10)
            .map(|(_, amount)| amount)
            .sum();
        self.top10_concentration = if total_supply > Decimal::ZERO {
            (top10_sum / total_supply * dec!(100))
                .to_f64()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        // Top 100 concentration
        let top100_sum: Decimal = sorted_holders
            .iter()
            .take(100)
            .map(|(_, amount)| amount)
            .sum();
        self.top100_concentration = if total_supply > Decimal::ZERO {
            (top100_sum / total_supply * dec!(100))
                .to_f64()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        // Count whales (holders with > 1% of supply)
        let whale_threshold = total_supply * dec!(0.01);
        self.whale_count = sorted_holders
            .iter()
            .filter(|(_, amount)| *amount > whale_threshold)
            .count();

        // Calculate distribution score (higher is better)
        self.distribution_score = if self.top10_concentration < 10.0 {
            90
        } else if self.top10_concentration < 20.0 {
            75
        } else if self.top10_concentration < 40.0 {
            50
        } else if self.top10_concentration < 60.0 {
            25
        } else {
            10
        };

        self.updated_at = Utc::now();
    }

    /// Check if distribution is healthy
    pub fn is_healthy_distribution(&self) -> bool {
        self.distribution_score >= 50 && self.top10_concentration < 50.0
    }
}

/// Transaction graph node (wallet)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub address: String,
    pub connections: HashSet<String>,
    pub total_interactions: u64,
    pub total_value_transferred: Decimal,
}

/// Transaction graph for analyzing wallet interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionGraph {
    /// Graph nodes (wallets)
    pub nodes: HashMap<String, GraphNode>,

    /// Edges (wallet interactions)
    pub edges: Vec<(String, String, Decimal)>,
}

impl TransactionGraph {
    /// Create new transaction graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    /// Add transaction to graph
    pub fn add_transaction(&mut self, from: String, to: String, value: Decimal) {
        // Add or update from node
        let from_node = self.nodes.entry(from.clone()).or_insert_with(|| GraphNode {
            address: from.clone(),
            connections: HashSet::new(),
            total_interactions: 0,
            total_value_transferred: Decimal::ZERO,
        });
        from_node.connections.insert(to.clone());
        from_node.total_interactions += 1;
        from_node.total_value_transferred += value;

        // Add or update to node
        let to_node = self.nodes.entry(to.clone()).or_insert_with(|| GraphNode {
            address: to.clone(),
            connections: HashSet::new(),
            total_interactions: 0,
            total_value_transferred: Decimal::ZERO,
        });
        to_node.connections.insert(from.clone());
        to_node.total_interactions += 1;
        to_node.total_value_transferred += value;

        // Add edge
        self.edges.push((from, to, value));
    }

    /// Find highly connected wallets (potential hubs)
    pub fn find_hubs(&self, min_connections: usize) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.connections.len() >= min_connections)
            .map(|(addr, _)| addr.clone())
            .collect()
    }

    /// Detect potential money laundering rings (circular transactions)
    pub fn detect_rings(&self, max_depth: usize) -> Vec<Vec<String>> {
        let mut rings = Vec::new();

        for start_addr in self.nodes.keys() {
            if let Some(ring) = self.find_ring(start_addr, start_addr, &mut Vec::new(), max_depth) {
                if ring.len() >= 3 && !rings.contains(&ring) {
                    rings.push(ring);
                }
            }
        }

        rings
    }

    /// Helper function to find rings using DFS
    fn find_ring(
        &self,
        current: &str,
        target: &str,
        path: &mut Vec<String>,
        depth: usize,
    ) -> Option<Vec<String>> {
        if depth == 0 {
            return None;
        }

        path.push(current.to_string());

        if path.len() > 1 && current == target {
            return Some(path.clone());
        }

        if let Some(node) = self.nodes.get(current) {
            for next in &node.connections {
                if path.len() == 1 || next != &path[path.len() - 2] {
                    if let Some(ring) = self.find_ring(next, target, path, depth - 1) {
                        return Some(ring);
                    }
                }
            }
        }

        path.pop();
        None
    }
}

impl Default for TransactionGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Smart money tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartMoneyTracker {
    /// Tracked wallets
    pub tracked_wallets: HashMap<String, WalletProfile>,

    /// Recent trades by smart money
    pub recent_trades: Vec<SmartMoneyTrade>,

    /// Token accumulation by smart money
    pub accumulation: HashMap<String, Decimal>,
}

/// Smart money trade record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartMoneyTrade {
    pub wallet: String,
    pub token: String,
    pub side: TradeSide,
    pub amount: Decimal,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Trade side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

impl SmartMoneyTracker {
    /// Create new smart money tracker
    pub fn new() -> Self {
        Self {
            tracked_wallets: HashMap::new(),
            recent_trades: Vec::new(),
            accumulation: HashMap::new(),
        }
    }

    /// Add wallet to tracking
    pub fn track_wallet(&mut self, profile: WalletProfile) {
        if profile.is_smart_money() {
            self.tracked_wallets
                .insert(profile.address.clone(), profile);
        }
    }

    /// Record smart money trade
    pub fn record_trade(
        &mut self,
        wallet: String,
        token: String,
        side: TradeSide,
        amount: Decimal,
        price: Decimal,
    ) {
        // Only track if wallet is in tracked list
        if self.tracked_wallets.contains_key(&wallet) {
            let trade = SmartMoneyTrade {
                wallet: wallet.clone(),
                token: token.clone(),
                side,
                amount,
                price,
                timestamp: Utc::now(),
            };

            self.recent_trades.push(trade);

            // Update accumulation
            let value = amount * price;
            let entry = self.accumulation.entry(token).or_insert(Decimal::ZERO);

            match side {
                TradeSide::Buy => *entry += value,
                TradeSide::Sell => *entry -= value,
            }

            // Keep only last 1000 trades
            if self.recent_trades.len() > 1000 {
                self.recent_trades.remove(0);
            }
        }
    }

    /// Get tokens being accumulated by smart money
    pub fn get_accumulating_tokens(&self) -> Vec<(String, Decimal)> {
        let mut tokens: Vec<_> = self
            .accumulation
            .iter()
            .filter(|(_, value)| **value > Decimal::ZERO)
            .map(|(token, value)| (token.clone(), *value))
            .collect();

        tokens.sort_by(|a, b| b.1.cmp(&a.1));
        tokens
    }

    /// Get tokens being distributed by smart money
    pub fn get_distributing_tokens(&self) -> Vec<(String, Decimal)> {
        let mut tokens: Vec<_> = self
            .accumulation
            .iter()
            .filter(|(_, value)| **value < Decimal::ZERO)
            .map(|(token, value)| (token.clone(), value.abs()))
            .collect();

        tokens.sort_by(|a, b| b.1.cmp(&a.1));
        tokens
    }

    /// Get smart money sentiment for a token (positive = bullish)
    pub fn get_sentiment(&self, token: &str) -> Decimal {
        self.accumulation
            .get(token)
            .copied()
            .unwrap_or(Decimal::ZERO)
    }
}

impl Default for SmartMoneyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_wallet_profile() {
        let mut profile = WalletProfile::new("wallet1".to_string());

        profile.update_with_tx(dec!(1000), dec!(100), "BTC".to_string());
        profile.update_with_tx(dec!(2000), dec!(200), "ETH".to_string());

        assert_eq!(profile.tx_count, 2);
        assert_eq!(profile.total_volume, dec!(3000));
        assert_eq!(profile.avg_profit, dec!(150));
    }

    #[test]
    fn test_token_holder_analysis() {
        let mut analysis = TokenHolderAnalysis::new("BTC".to_string());

        let holders = vec![
            ("whale1".to_string(), dec!(5000)),
            ("whale2".to_string(), dec!(3000)),
            ("holder1".to_string(), dec!(100)),
            ("holder2".to_string(), dec!(50)),
        ];

        analysis.analyze(&holders, dec!(10000));

        assert_eq!(analysis.total_holders, 4);
        assert_eq!(analysis.whale_count, 2);
        assert!(analysis.top10_concentration > 0.0);
    }

    #[test]
    fn test_transaction_graph() {
        let mut graph = TransactionGraph::new();

        graph.add_transaction("A".to_string(), "B".to_string(), dec!(100));
        graph.add_transaction("B".to_string(), "C".to_string(), dec!(200));
        graph.add_transaction("A".to_string(), "C".to_string(), dec!(150));

        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.edges.len(), 3);

        let hubs = graph.find_hubs(2);
        assert!(!hubs.is_empty());
    }

    #[test]
    fn test_smart_money_tracker() {
        let mut tracker = SmartMoneyTracker::new();

        let mut profile = WalletProfile::new("smart1".to_string());
        profile.category = WalletCategory::SmartMoney;
        tracker.track_wallet(profile);

        tracker.record_trade(
            "smart1".to_string(),
            "BTC".to_string(),
            TradeSide::Buy,
            dec!(1),
            dec!(50000),
        );

        let accumulating = tracker.get_accumulating_tokens();
        assert_eq!(accumulating.len(), 1);
        assert_eq!(accumulating[0].0, "BTC");
    }
}
