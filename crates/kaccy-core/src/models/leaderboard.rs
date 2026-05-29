//! User leaderboards and ranking system
//!
//! This module provides functionality for tracking and displaying user rankings
//! across various metrics including trading volume, profit/loss, and reputation.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Time period for leaderboard rankings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LeaderboardPeriod {
    /// Daily rankings (last 24 hours)
    Daily,
    /// Weekly rankings (last 7 days)
    Weekly,
    /// Monthly rankings (last 30 days)
    Monthly,
    /// All-time rankings
    AllTime,
}

impl LeaderboardPeriod {
    /// Get the cutoff timestamp for this period
    pub fn cutoff_time(&self) -> DateTime<Utc> {
        let now = Utc::now();
        match self {
            Self::Daily => now - chrono::Duration::days(1),
            Self::Weekly => now - chrono::Duration::weeks(1),
            Self::Monthly => now - chrono::Duration::days(30),
            Self::AllTime => {
                DateTime::<Utc>::from_timestamp(0, 0).expect("Unix epoch timestamp 0 is valid")
            }
        }
    }
}

/// Category for leaderboard rankings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LeaderboardCategory {
    /// Trading volume rankings
    TradingVolume,
    /// Profit/Loss rankings
    ProfitLoss,
    /// Reputation score rankings
    Reputation,
    /// Number of trades rankings
    TradeCount,
    /// Win rate rankings (profitable trades / total trades)
    WinRate,
    /// Average trade size rankings
    AverageTradeSize,
    /// Liquidity provided rankings
    LiquidityProvided,
    /// Referral earnings rankings
    ReferralEarnings,
}

impl LeaderboardCategory {
    /// Get a human-readable name for the category
    pub fn name(&self) -> &'static str {
        match self {
            Self::TradingVolume => "Trading Volume",
            Self::ProfitLoss => "Profit/Loss",
            Self::Reputation => "Reputation",
            Self::TradeCount => "Trade Count",
            Self::WinRate => "Win Rate",
            Self::AverageTradeSize => "Average Trade Size",
            Self::LiquidityProvided => "Liquidity Provided",
            Self::ReferralEarnings => "Referral Earnings",
        }
    }
}

/// Achievement badge for user accomplishments
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Badge {
    /// Badge ID
    pub id: String,
    /// Badge name
    pub name: String,
    /// Badge description
    pub description: String,
    /// Badge tier (Bronze, Silver, Gold, etc.)
    pub tier: BadgeTier,
    /// When the badge was earned
    pub earned_at: DateTime<Utc>,
}

/// Badge tier levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BadgeTier {
    /// Bronze tier
    Bronze,
    /// Silver tier
    Silver,
    /// Gold tier
    Gold,
    /// Platinum tier
    Platinum,
    /// Diamond tier
    Diamond,
    /// Legendary tier
    Legendary,
}

impl BadgeTier {
    /// Get the tier name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Bronze => "Bronze",
            Self::Silver => "Silver",
            Self::Gold => "Gold",
            Self::Platinum => "Platinum",
            Self::Diamond => "Diamond",
            Self::Legendary => "Legendary",
        }
    }
}

/// User entry in a leaderboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    /// Rank position (1-indexed)
    pub rank: u32,
    /// User ID
    pub user_id: Uuid,
    /// Username
    pub username: String,
    /// Metric value
    pub value: Decimal,
    /// Change in rank since last period (-1 = down, 0 = same, 1 = up)
    pub rank_change: i32,
    /// User's badges
    pub badges: Vec<Badge>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Leaderboard with user rankings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leaderboard {
    /// Leaderboard category
    pub category: LeaderboardCategory,
    /// Time period
    pub period: LeaderboardPeriod,
    /// Ranked entries
    pub entries: Vec<LeaderboardEntry>,
    /// Total number of participants
    pub total_participants: u32,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl Leaderboard {
    /// Create a new leaderboard
    pub fn new(category: LeaderboardCategory, period: LeaderboardPeriod) -> Self {
        Self {
            category,
            period,
            entries: Vec::new(),
            total_participants: 0,
            updated_at: Utc::now(),
        }
    }

    /// Get the top N entries
    pub fn top(&self, n: usize) -> &[LeaderboardEntry] {
        &self.entries[..self.entries.len().min(n)]
    }

    /// Find a user's rank
    pub fn find_user_rank(&self, user_id: Uuid) -> Option<u32> {
        self.entries
            .iter()
            .find(|e| e.user_id == user_id)
            .map(|e| e.rank)
    }

    /// Get percentile for a user (0.0 to 1.0)
    pub fn user_percentile(&self, user_id: Uuid) -> Option<f64> {
        self.find_user_rank(user_id).map(|rank| {
            if self.total_participants == 0 {
                0.0
            } else {
                1.0 - (rank as f64 / self.total_participants as f64)
            }
        })
    }
}

/// Leaderboard manager for updating and querying rankings
pub struct LeaderboardManager {
    /// Cached leaderboards by category and period
    leaderboards: HashMap<(LeaderboardCategory, LeaderboardPeriod), Leaderboard>,
    /// Previous rankings for tracking changes
    previous_rankings: HashMap<(LeaderboardCategory, LeaderboardPeriod, Uuid), u32>,
}

impl LeaderboardManager {
    /// Create a new leaderboard manager
    pub fn new() -> Self {
        Self {
            leaderboards: HashMap::new(),
            previous_rankings: HashMap::new(),
        }
    }

    /// Update a leaderboard with new data
    pub fn update_leaderboard(
        &mut self,
        category: LeaderboardCategory,
        period: LeaderboardPeriod,
        mut user_data: Vec<(Uuid, String, Decimal, Vec<Badge>)>,
    ) {
        // Store previous rankings
        if let Some(prev_leaderboard) = self.leaderboards.get(&(category, period)) {
            for entry in &prev_leaderboard.entries {
                self.previous_rankings
                    .insert((category, period, entry.user_id), entry.rank);
            }
        }

        // Sort by value descending
        user_data.sort_by_key(|b| std::cmp::Reverse(b.2));

        // Create ranked entries
        let entries: Vec<LeaderboardEntry> = user_data
            .into_iter()
            .enumerate()
            .map(|(idx, (user_id, username, value, badges))| {
                let rank = (idx + 1) as u32;
                let prev_rank = self
                    .previous_rankings
                    .get(&(category, period, user_id))
                    .copied();

                let rank_change = if let Some(prev) = prev_rank {
                    (prev as i32) - (rank as i32) // Positive = improved rank
                } else {
                    0
                };

                LeaderboardEntry {
                    rank,
                    user_id,
                    username,
                    value,
                    rank_change,
                    badges,
                    updated_at: Utc::now(),
                }
            })
            .collect();

        let total_participants = entries.len() as u32;

        let leaderboard = Leaderboard {
            category,
            period,
            entries,
            total_participants,
            updated_at: Utc::now(),
        };

        self.leaderboards.insert((category, period), leaderboard);
    }

    /// Get a leaderboard by category and period
    pub fn get_leaderboard(
        &self,
        category: LeaderboardCategory,
        period: LeaderboardPeriod,
    ) -> Option<&Leaderboard> {
        self.leaderboards.get(&(category, period))
    }

    /// Get all leaderboards for a period
    pub fn get_period_leaderboards(&self, period: LeaderboardPeriod) -> Vec<&Leaderboard> {
        self.leaderboards
            .iter()
            .filter(|((_, p), _)| *p == period)
            .map(|(_, lb)| lb)
            .collect()
    }

    /// Get user's ranking across all categories
    pub fn get_user_rankings(
        &self,
        user_id: Uuid,
        period: LeaderboardPeriod,
    ) -> HashMap<LeaderboardCategory, u32> {
        self.leaderboards
            .iter()
            .filter(|((_, p), _)| *p == period)
            .filter_map(|((cat, _), lb)| lb.find_user_rank(user_id).map(|rank| (*cat, rank)))
            .collect()
    }

    /// Check if a user should earn a badge
    pub fn check_badge_eligibility(
        &self,
        user_id: Uuid,
        category: LeaderboardCategory,
    ) -> Option<Badge> {
        // Check all-time leaderboard for badge eligibility
        if let Some(leaderboard) = self.get_leaderboard(category, LeaderboardPeriod::AllTime) {
            if let Some(rank) = leaderboard.find_user_rank(user_id) {
                return match rank {
                    1 => Some(Badge {
                        id: format!("{:?}_legendary", category),
                        name: format!("{} Champion", category.name()),
                        description: format!("Ranked #1 in {} all-time", category.name()),
                        tier: BadgeTier::Legendary,
                        earned_at: Utc::now(),
                    }),
                    2..=10 => Some(Badge {
                        id: format!("{:?}_diamond", category),
                        name: format!("{} Master", category.name()),
                        description: format!("Ranked top 10 in {} all-time", category.name()),
                        tier: BadgeTier::Diamond,
                        earned_at: Utc::now(),
                    }),
                    11..=50 => Some(Badge {
                        id: format!("{:?}_platinum", category),
                        name: format!("{} Expert", category.name()),
                        description: format!("Ranked top 50 in {} all-time", category.name()),
                        tier: BadgeTier::Platinum,
                        earned_at: Utc::now(),
                    }),
                    51..=100 => Some(Badge {
                        id: format!("{:?}_gold", category),
                        name: format!("{} Professional", category.name()),
                        description: format!("Ranked top 100 in {} all-time", category.name()),
                        tier: BadgeTier::Gold,
                        earned_at: Utc::now(),
                    }),
                    101..=500 => Some(Badge {
                        id: format!("{:?}_silver", category),
                        name: format!("{} Enthusiast", category.name()),
                        description: format!("Ranked top 500 in {} all-time", category.name()),
                        tier: BadgeTier::Silver,
                        earned_at: Utc::now(),
                    }),
                    _ => None,
                };
            }
        }
        None
    }
}

impl Default for LeaderboardManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_leaderboard_creation() {
        let leaderboard =
            Leaderboard::new(LeaderboardCategory::TradingVolume, LeaderboardPeriod::Daily);

        assert_eq!(leaderboard.category, LeaderboardCategory::TradingVolume);
        assert_eq!(leaderboard.period, LeaderboardPeriod::Daily);
        assert_eq!(leaderboard.entries.len(), 0);
        assert_eq!(leaderboard.total_participants, 0);
    }

    #[test]
    fn test_leaderboard_manager_update() {
        let mut manager = LeaderboardManager::new();

        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();
        let user3 = Uuid::new_v4();

        let data = vec![
            (user1, "alice".to_string(), dec!(1000), vec![]),
            (user2, "bob".to_string(), dec!(2000), vec![]),
            (user3, "charlie".to_string(), dec!(1500), vec![]),
        ];

        manager.update_leaderboard(
            LeaderboardCategory::TradingVolume,
            LeaderboardPeriod::Daily,
            data,
        );

        let leaderboard = manager
            .get_leaderboard(LeaderboardCategory::TradingVolume, LeaderboardPeriod::Daily)
            .unwrap();

        assert_eq!(leaderboard.entries.len(), 3);
        assert_eq!(leaderboard.entries[0].rank, 1);
        assert_eq!(leaderboard.entries[0].user_id, user2); // bob has highest volume
        assert_eq!(leaderboard.entries[1].rank, 2);
        assert_eq!(leaderboard.entries[1].user_id, user3); // charlie is second
        assert_eq!(leaderboard.entries[2].rank, 3);
        assert_eq!(leaderboard.entries[2].user_id, user1); // alice is third
    }

    #[test]
    fn test_rank_changes() {
        let mut manager = LeaderboardManager::new();

        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        // First update
        manager.update_leaderboard(
            LeaderboardCategory::TradingVolume,
            LeaderboardPeriod::Daily,
            vec![
                (user1, "alice".to_string(), dec!(1000), vec![]),
                (user2, "bob".to_string(), dec!(500), vec![]),
            ],
        );

        // Second update - positions changed
        manager.update_leaderboard(
            LeaderboardCategory::TradingVolume,
            LeaderboardPeriod::Daily,
            vec![
                (user1, "alice".to_string(), dec!(500), vec![]),
                (user2, "bob".to_string(), dec!(1000), vec![]),
            ],
        );

        let leaderboard = manager
            .get_leaderboard(LeaderboardCategory::TradingVolume, LeaderboardPeriod::Daily)
            .unwrap();

        // bob moved from rank 2 to rank 1 (improved by 1)
        assert_eq!(leaderboard.entries[0].user_id, user2);
        assert_eq!(leaderboard.entries[0].rank_change, 1);

        // alice moved from rank 1 to rank 2 (declined by 1)
        assert_eq!(leaderboard.entries[1].user_id, user1);
        assert_eq!(leaderboard.entries[1].rank_change, -1);
    }

    #[test]
    fn test_find_user_rank() {
        let mut manager = LeaderboardManager::new();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        manager.update_leaderboard(
            LeaderboardCategory::Reputation,
            LeaderboardPeriod::AllTime,
            vec![
                (user1, "alice".to_string(), dec!(1000), vec![]),
                (user2, "bob".to_string(), dec!(500), vec![]),
            ],
        );

        let leaderboard = manager
            .get_leaderboard(LeaderboardCategory::Reputation, LeaderboardPeriod::AllTime)
            .unwrap();

        assert_eq!(leaderboard.find_user_rank(user1), Some(1));
        assert_eq!(leaderboard.find_user_rank(user2), Some(2));
    }

    #[test]
    fn test_user_percentile() {
        let mut manager = LeaderboardManager::new();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();
        let user3 = Uuid::new_v4();
        let user4 = Uuid::new_v4();

        manager.update_leaderboard(
            LeaderboardCategory::TradingVolume,
            LeaderboardPeriod::Monthly,
            vec![
                (user1, "alice".to_string(), dec!(1000), vec![]),
                (user2, "bob".to_string(), dec!(800), vec![]),
                (user3, "charlie".to_string(), dec!(600), vec![]),
                (user4, "dave".to_string(), dec!(400), vec![]),
            ],
        );

        let leaderboard = manager
            .get_leaderboard(
                LeaderboardCategory::TradingVolume,
                LeaderboardPeriod::Monthly,
            )
            .unwrap();

        // user1 is rank 1 out of 4 = 75th percentile (1 - 1/4)
        assert_eq!(leaderboard.user_percentile(user1), Some(0.75));

        // user4 is rank 4 out of 4 = 0th percentile (1 - 4/4)
        assert_eq!(leaderboard.user_percentile(user4), Some(0.0));
    }

    #[test]
    fn test_badge_eligibility() {
        let mut manager = LeaderboardManager::new();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        manager.update_leaderboard(
            LeaderboardCategory::ProfitLoss,
            LeaderboardPeriod::AllTime,
            vec![
                (user1, "alice".to_string(), dec!(10000), vec![]),
                (user2, "bob".to_string(), dec!(5000), vec![]),
            ],
        );

        let badge1 = manager.check_badge_eligibility(user1, LeaderboardCategory::ProfitLoss);
        assert!(badge1.is_some());
        let badge1 = badge1.unwrap();
        assert_eq!(badge1.tier, BadgeTier::Legendary);
        assert!(badge1.name.contains("Champion"));

        let badge2 = manager.check_badge_eligibility(user2, LeaderboardCategory::ProfitLoss);
        assert!(badge2.is_some());
        let badge2 = badge2.unwrap();
        assert_eq!(badge2.tier, BadgeTier::Diamond);
        assert!(badge2.name.contains("Master"));
    }

    #[test]
    fn test_top_n_entries() {
        let mut leaderboard =
            Leaderboard::new(LeaderboardCategory::TradeCount, LeaderboardPeriod::Weekly);

        for i in 0..10 {
            leaderboard.entries.push(LeaderboardEntry {
                rank: i + 1,
                user_id: Uuid::new_v4(),
                username: format!("user{}", i),
                value: dec!(100) - Decimal::from(i),
                rank_change: 0,
                badges: vec![],
                updated_at: Utc::now(),
            });
        }

        let top_3 = leaderboard.top(3);
        assert_eq!(top_3.len(), 3);
        assert_eq!(top_3[0].rank, 1);
        assert_eq!(top_3[1].rank, 2);
        assert_eq!(top_3[2].rank, 3);
    }

    #[test]
    fn test_category_names() {
        assert_eq!(LeaderboardCategory::TradingVolume.name(), "Trading Volume");
        assert_eq!(LeaderboardCategory::ProfitLoss.name(), "Profit/Loss");
        assert_eq!(LeaderboardCategory::Reputation.name(), "Reputation");
        assert_eq!(LeaderboardCategory::TradeCount.name(), "Trade Count");
    }

    #[test]
    fn test_badge_tiers() {
        assert!(BadgeTier::Legendary > BadgeTier::Diamond);
        assert!(BadgeTier::Diamond > BadgeTier::Platinum);
        assert!(BadgeTier::Platinum > BadgeTier::Gold);
        assert!(BadgeTier::Gold > BadgeTier::Silver);
        assert!(BadgeTier::Silver > BadgeTier::Bronze);
    }

    #[test]
    fn test_period_cutoff_times() {
        let now = Utc::now();
        let daily_cutoff = LeaderboardPeriod::Daily.cutoff_time();
        let weekly_cutoff = LeaderboardPeriod::Weekly.cutoff_time();
        let monthly_cutoff = LeaderboardPeriod::Monthly.cutoff_time();

        assert!(daily_cutoff < now);
        assert!(weekly_cutoff < daily_cutoff);
        assert!(monthly_cutoff < weekly_cutoff);
    }
}
