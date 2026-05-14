//! Social trading and copy trading system
//!
//! This module provides functionality for users to follow and copy trades from successful traders.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Trader profile for social trading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraderProfile {
    /// Trader ID
    pub trader_id: Uuid,
    /// Username
    pub username: String,
    /// Total followers
    pub follower_count: u32,
    /// Win rate (0.0-1.0)
    pub win_rate: Decimal,
    /// Total profit/loss
    pub total_pnl: Decimal,
    /// Average monthly return
    pub avg_monthly_return: Decimal,
    /// Maximum drawdown
    pub max_drawdown: Decimal,
    /// Total trades executed
    pub total_trades: u32,
    /// Commission rate for followers (e.g., 0.10 = 10%)
    pub commission_rate: Decimal,
    /// Whether accepting new followers
    pub accepting_followers: bool,
    /// Maximum exposure per follower
    pub max_follower_exposure: Decimal,
}

/// Copy trading relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyRelationship {
    /// Relationship ID
    pub id: Uuid,
    /// Follower user ID
    pub follower_id: Uuid,
    /// Leader trader ID
    pub leader_id: Uuid,
    /// Copy percentage (0.0-1.0, how much of follower's capital to use)
    pub copy_percentage: Decimal,
    /// Maximum exposure limit
    pub max_exposure: Decimal,
    /// Current exposure
    pub current_exposure: Decimal,
    /// Started copying at
    pub started_at: DateTime<Utc>,
    /// Whether currently active
    pub active: bool,
    /// Total profit/loss from copying
    pub total_pnl: Decimal,
}

impl CopyRelationship {
    /// Create a new copy relationship
    pub fn new(
        follower_id: Uuid,
        leader_id: Uuid,
        copy_percentage: Decimal,
        max_exposure: Decimal,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            follower_id,
            leader_id,
            copy_percentage,
            max_exposure,
            current_exposure: Decimal::ZERO,
            started_at: Utc::now(),
            active: true,
            total_pnl: Decimal::ZERO,
        }
    }

    /// Check if can copy a trade
    pub fn can_copy(&self, trade_amount: Decimal) -> bool {
        self.active && (self.current_exposure + trade_amount) <= self.max_exposure
    }

    /// Calculate follower's trade size
    pub fn calculate_copy_amount(
        &self,
        leader_trade_amount: Decimal,
        follower_balance: Decimal,
    ) -> Decimal {
        let max_by_percentage = follower_balance * self.copy_percentage;
        let max_by_exposure = self.max_exposure - self.current_exposure;
        let scaled_amount = leader_trade_amount * self.copy_percentage;

        scaled_amount.min(max_by_percentage).min(max_by_exposure)
    }
}

/// Copy trade execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTrade {
    /// Copy trade ID
    pub id: Uuid,
    /// Original leader trade ID
    pub leader_trade_id: Uuid,
    /// Follower trade ID
    pub follower_trade_id: Uuid,
    /// Copy relationship
    pub relationship_id: Uuid,
    /// Leader user ID
    pub leader_id: Uuid,
    /// Follower user ID
    pub follower_id: Uuid,
    /// Token traded
    pub token_id: Uuid,
    /// Leader trade amount
    pub leader_amount: Decimal,
    /// Follower trade amount
    pub follower_amount: Decimal,
    /// Execution price
    pub price: Decimal,
    /// Commission paid to leader
    pub commission: Decimal,
    /// Timestamp
    pub executed_at: DateTime<Utc>,
}

/// Social trading manager
pub struct SocialTradingManager {
    /// Trader profiles
    profiles: HashMap<Uuid, TraderProfile>,
    /// Copy relationships
    relationships: HashMap<Uuid, CopyRelationship>,
    /// Follower to leader mapping
    follower_leaders: HashMap<Uuid, Vec<Uuid>>,
    /// Leader to followers mapping
    leader_followers: HashMap<Uuid, Vec<Uuid>>,
}

impl SocialTradingManager {
    /// Create a new social trading manager
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            relationships: HashMap::new(),
            follower_leaders: HashMap::new(),
            leader_followers: HashMap::new(),
        }
    }

    /// Add a trader profile
    pub fn add_profile(&mut self, profile: TraderProfile) {
        self.profiles.insert(profile.trader_id, profile);
    }

    /// Start following a trader
    pub fn follow(
        &mut self,
        follower_id: Uuid,
        leader_id: Uuid,
        copy_percentage: Decimal,
        max_exposure: Decimal,
    ) -> Result<Uuid, String> {
        // Check if leader exists and accepts followers
        let leader = self.profiles.get(&leader_id).ok_or("Leader not found")?;

        if !leader.accepting_followers {
            return Err("Leader not accepting followers".to_string());
        }

        // Create relationship
        let relationship =
            CopyRelationship::new(follower_id, leader_id, copy_percentage, max_exposure);
        let relationship_id = relationship.id;

        self.relationships.insert(relationship_id, relationship);

        // Update mappings
        self.follower_leaders
            .entry(follower_id)
            .or_default()
            .push(leader_id);

        self.leader_followers
            .entry(leader_id)
            .or_default()
            .push(follower_id);

        Ok(relationship_id)
    }

    /// Stop following a trader
    pub fn unfollow(&mut self, relationship_id: Uuid) -> Result<(), String> {
        if let Some(relationship) = self.relationships.get_mut(&relationship_id) {
            relationship.active = false;
            Ok(())
        } else {
            Err("Relationship not found".to_string())
        }
    }

    /// Get copy trades for a leader's trade
    pub fn get_copy_trades(
        &self,
        leader_id: Uuid,
        leader_trade_amount: Decimal,
        follower_balances: &HashMap<Uuid, Decimal>,
    ) -> Vec<(Uuid, Decimal)> {
        let followers = match self.leader_followers.get(&leader_id) {
            Some(f) => f,
            None => return Vec::new(),
        };

        let mut copy_trades = Vec::new();

        for follower_id in followers {
            if let Some(relationship) = self.find_relationship(*follower_id, leader_id) {
                if let Some(&balance) = follower_balances.get(follower_id) {
                    let copy_amount =
                        relationship.calculate_copy_amount(leader_trade_amount, balance);
                    if copy_amount > Decimal::ZERO && relationship.can_copy(copy_amount) {
                        copy_trades.push((*follower_id, copy_amount));
                    }
                }
            }
        }

        copy_trades
    }

    /// Find a relationship between follower and leader
    fn find_relationship(&self, follower_id: Uuid, leader_id: Uuid) -> Option<&CopyRelationship> {
        self.relationships
            .values()
            .find(|r| r.follower_id == follower_id && r.leader_id == leader_id && r.active)
    }

    /// Calculate commission for a copy trade
    pub fn calculate_commission(&self, leader_id: Uuid, follower_profit: Decimal) -> Decimal {
        if let Some(profile) = self.profiles.get(&leader_id) {
            if follower_profit > Decimal::ZERO {
                return follower_profit * profile.commission_rate;
            }
        }
        Decimal::ZERO
    }

    /// Get top traders by performance
    pub fn get_top_traders(&self, limit: usize) -> Vec<&TraderProfile> {
        let mut profiles: Vec<&TraderProfile> = self.profiles.values().collect();
        profiles.sort_by(|a, b| {
            b.avg_monthly_return
                .partial_cmp(&a.avg_monthly_return)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        profiles.into_iter().take(limit).collect()
    }

    /// Get followers count for a leader
    pub fn get_follower_count(&self, leader_id: Uuid) -> usize {
        self.leader_followers
            .get(&leader_id)
            .map(|f| f.len())
            .unwrap_or(0)
    }
}

impl Default for SocialTradingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_copy_relationship_creation() {
        let follower_id = Uuid::new_v4();
        let leader_id = Uuid::new_v4();

        let relationship = CopyRelationship::new(follower_id, leader_id, dec!(0.5), dec!(10000));

        assert_eq!(relationship.follower_id, follower_id);
        assert_eq!(relationship.leader_id, leader_id);
        assert_eq!(relationship.copy_percentage, dec!(0.5));
        assert!(relationship.active);
    }

    #[test]
    fn test_can_copy() {
        let relationship =
            CopyRelationship::new(Uuid::new_v4(), Uuid::new_v4(), dec!(0.5), dec!(1000));

        assert!(relationship.can_copy(dec!(500)));
        assert!(relationship.can_copy(dec!(1000)));
        assert!(!relationship.can_copy(dec!(1001)));
    }

    #[test]
    fn test_calculate_copy_amount() {
        let relationship =
            CopyRelationship::new(Uuid::new_v4(), Uuid::new_v4(), dec!(0.5), dec!(1000));

        // Leader trades 1000, follower has 2000 balance
        let copy_amount = relationship.calculate_copy_amount(dec!(1000), dec!(2000));

        // Should be min of: 1000*0.5=500, 2000*0.5=1000, 1000-0=1000
        assert_eq!(copy_amount, dec!(500));
    }

    #[test]
    fn test_follow_trader() {
        let mut manager = SocialTradingManager::new();

        let leader_id = Uuid::new_v4();
        let leader_profile = TraderProfile {
            trader_id: leader_id,
            username: "Leader".to_string(),
            follower_count: 0,
            win_rate: dec!(0.75),
            total_pnl: dec!(10000),
            avg_monthly_return: dec!(0.15),
            max_drawdown: dec!(-500),
            total_trades: 100,
            commission_rate: dec!(0.10),
            accepting_followers: true,
            max_follower_exposure: dec!(50000),
        };

        manager.add_profile(leader_profile);

        let follower_id = Uuid::new_v4();
        let result = manager.follow(follower_id, leader_id, dec!(0.5), dec!(10000));

        assert!(result.is_ok());
        assert_eq!(manager.get_follower_count(leader_id), 1);
    }

    #[test]
    fn test_unfollow_trader() {
        let mut manager = SocialTradingManager::new();

        let leader_id = Uuid::new_v4();
        let leader_profile = TraderProfile {
            trader_id: leader_id,
            username: "Leader".to_string(),
            follower_count: 0,
            win_rate: dec!(0.75),
            total_pnl: dec!(10000),
            avg_monthly_return: dec!(0.15),
            max_drawdown: dec!(-500),
            total_trades: 100,
            commission_rate: dec!(0.10),
            accepting_followers: true,
            max_follower_exposure: dec!(50000),
        };

        manager.add_profile(leader_profile);

        let follower_id = Uuid::new_v4();
        let relationship_id = manager
            .follow(follower_id, leader_id, dec!(0.5), dec!(10000))
            .unwrap();

        let result = manager.unfollow(relationship_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_copy_trades() {
        let mut manager = SocialTradingManager::new();

        let leader_id = Uuid::new_v4();
        let leader_profile = TraderProfile {
            trader_id: leader_id,
            username: "Leader".to_string(),
            follower_count: 0,
            win_rate: dec!(0.75),
            total_pnl: dec!(10000),
            avg_monthly_return: dec!(0.15),
            max_drawdown: dec!(-500),
            total_trades: 100,
            commission_rate: dec!(0.10),
            accepting_followers: true,
            max_follower_exposure: dec!(50000),
        };

        manager.add_profile(leader_profile);

        let follower_id = Uuid::new_v4();
        manager
            .follow(follower_id, leader_id, dec!(0.5), dec!(10000))
            .unwrap();

        let mut balances = HashMap::new();
        balances.insert(follower_id, dec!(5000));

        let copy_trades = manager.get_copy_trades(leader_id, dec!(1000), &balances);

        assert_eq!(copy_trades.len(), 1);
        assert_eq!(copy_trades[0].0, follower_id);
    }

    #[test]
    fn test_calculate_commission() {
        let mut manager = SocialTradingManager::new();

        let leader_id = Uuid::new_v4();
        let leader_profile = TraderProfile {
            trader_id: leader_id,
            username: "Leader".to_string(),
            follower_count: 0,
            win_rate: dec!(0.75),
            total_pnl: dec!(10000),
            avg_monthly_return: dec!(0.15),
            max_drawdown: dec!(-500),
            total_trades: 100,
            commission_rate: dec!(0.10),
            accepting_followers: true,
            max_follower_exposure: dec!(50000),
        };

        manager.add_profile(leader_profile);

        let commission = manager.calculate_commission(leader_id, dec!(1000));
        assert_eq!(commission, dec!(100)); // 10% of 1000

        // No commission on losses
        let commission_loss = manager.calculate_commission(leader_id, dec!(-500));
        assert_eq!(commission_loss, Decimal::ZERO);
    }

    #[test]
    fn test_get_top_traders() {
        let mut manager = SocialTradingManager::new();

        for i in 1..=5 {
            let leader_id = Uuid::new_v4();
            let profile = TraderProfile {
                trader_id: leader_id,
                username: format!("Trader{}", i),
                follower_count: 0,
                win_rate: dec!(0.70),
                total_pnl: dec!(5000),
                avg_monthly_return: Decimal::from(i) / dec!(10), // 0.1, 0.2, 0.3, 0.4, 0.5
                max_drawdown: dec!(-100),
                total_trades: 50,
                commission_rate: dec!(0.10),
                accepting_followers: true,
                max_follower_exposure: dec!(10000),
            };
            manager.add_profile(profile);
        }

        let top_traders = manager.get_top_traders(3);
        assert_eq!(top_traders.len(), 3);
        // Should be sorted by avg_monthly_return descending
        assert!(top_traders[0].avg_monthly_return >= top_traders[1].avg_monthly_return);
        assert!(top_traders[1].avg_monthly_return >= top_traders[2].avg_monthly_return);
    }
}
