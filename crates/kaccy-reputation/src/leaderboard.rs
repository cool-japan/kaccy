//! Leaderboard module for competitive rankings and achievements
//!
//! Provides gamification features including leaderboards, achievements,
//! and competitive rankings to encourage user engagement.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::ReputationError;
use crate::tier::ReputationTier;

/// Service for leaderboards and achievements
pub struct LeaderboardService {
    pool: PgPool,
}

impl LeaderboardService {
    /// Create a new [`LeaderboardService`] backed by the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get global leaderboard (top users by score)
    pub async fn get_global_leaderboard(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<Leaderboard, ReputationError> {
        let query = r#"
            SELECT user_id, reputation_score
            FROM users
            WHERE reputation_score IS NOT NULL
            ORDER BY reputation_score DESC
            LIMIT $1 OFFSET $2
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut entries = Vec::new();
        for (idx, row) in rows.iter().enumerate() {
            let user_id: Uuid = row.try_get("user_id").map_err(ReputationError::Database)?;
            let score: Decimal = row
                .try_get("reputation_score")
                .map_err(ReputationError::Database)?;

            entries.push(LeaderboardEntry {
                rank: (offset + idx as i32 + 1),
                user_id,
                score,
                tier: ReputationTier::from_score(score),
            });
        }

        Ok(Leaderboard {
            entries,
            leaderboard_type: LeaderboardType::Global,
            generated_at: Utc::now(),
        })
    }

    /// Get tier-specific leaderboard
    pub async fn get_tier_leaderboard(
        &self,
        tier: ReputationTier,
        limit: i32,
    ) -> Result<Leaderboard, ReputationError> {
        let (min_score, max_score) = tier.score_range();

        let query = r#"
            SELECT user_id, reputation_score
            FROM users
            WHERE reputation_score >= $1
            AND reputation_score <= $2
            ORDER BY reputation_score DESC
            LIMIT $3
        "#;

        let rows = sqlx::query(query)
            .bind(min_score)
            .bind(max_score)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut entries = Vec::new();
        for (idx, row) in rows.iter().enumerate() {
            let user_id: Uuid = row.try_get("user_id").map_err(ReputationError::Database)?;
            let score: Decimal = row
                .try_get("reputation_score")
                .map_err(ReputationError::Database)?;

            entries.push(LeaderboardEntry {
                rank: (idx + 1) as i32,
                user_id,
                score,
                tier,
            });
        }

        Ok(Leaderboard {
            entries,
            leaderboard_type: LeaderboardType::Tier(tier),
            generated_at: Utc::now(),
        })
    }

    /// Get user's rank in global leaderboard
    pub async fn get_user_rank(&self, user_id: Uuid) -> Result<UserRankInfo, ReputationError> {
        // Get user's score
        let score_query = "SELECT reputation_score FROM users WHERE user_id = $1";
        let row = sqlx::query(score_query)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(ReputationError::Database)?
            .ok_or_else(|| ReputationError::UserNotFound(user_id.to_string()))?;

        let score: Decimal = row
            .try_get("reputation_score")
            .map_err(ReputationError::Database)?;

        // Count users with higher scores
        let rank_query = r#"
            SELECT COUNT(*) + 1 as rank
            FROM users
            WHERE reputation_score > $1
        "#;

        let rank_row = sqlx::query(rank_query)
            .bind(score)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let rank: i64 = rank_row
            .try_get("rank")
            .map_err(ReputationError::Database)?;

        // Get total users
        let total_query = "SELECT COUNT(*) as total FROM users WHERE reputation_score IS NOT NULL";
        let total_row = sqlx::query(total_query)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let total_users: i64 = total_row
            .try_get("total")
            .map_err(ReputationError::Database)?;

        let percentile = if total_users > 0 {
            ((total_users - rank + 1) as f64 / total_users as f64) * 100.0
        } else {
            0.0
        };

        Ok(UserRankInfo {
            user_id,
            rank: rank as i32,
            score,
            tier: ReputationTier::from_score(score),
            percentile,
            total_users: total_users as i32,
        })
    }

    /// Award achievement to user
    pub async fn award_achievement(
        &self,
        user_id: Uuid,
        achievement: Achievement,
    ) -> Result<(), ReputationError> {
        let query = r#"
            INSERT INTO user_achievements (user_id, achievement_type, achievement_name, awarded_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id, achievement_type) DO NOTHING
        "#;

        sqlx::query(query)
            .bind(user_id)
            .bind(achievement.achievement_type.to_string())
            .bind(&achievement.name)
            .bind(Utc::now())
            .execute(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        Ok(())
    }

    /// Get user's achievements
    pub async fn get_user_achievements(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<AwardedAchievement>, ReputationError> {
        let query = r#"
            SELECT achievement_type, achievement_name, awarded_at
            FROM user_achievements
            WHERE user_id = $1
            ORDER BY awarded_at DESC
        "#;

        let rows = sqlx::query(query)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let mut achievements = Vec::new();
        for row in rows {
            let type_str: String = row
                .try_get("achievement_type")
                .map_err(ReputationError::Database)?;

            achievements.push(AwardedAchievement {
                achievement_type: type_str
                    .parse::<AchievementType>()
                    .map_err(|e| ReputationError::Validation(e.to_string()))?,
                name: row
                    .try_get("achievement_name")
                    .map_err(ReputationError::Database)?,
                awarded_at: row
                    .try_get("awarded_at")
                    .map_err(ReputationError::Database)?,
            });
        }

        Ok(achievements)
    }

    /// Check and award automatic achievements
    pub async fn check_automatic_achievements(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<Achievement>, ReputationError> {
        let mut awarded = Vec::new();

        // Check score milestones
        let score_query = "SELECT reputation_score FROM users WHERE user_id = $1";
        let row = sqlx::query(score_query)
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        if let Some(row) = row {
            let score: Decimal = row
                .try_get("reputation_score")
                .map_err(ReputationError::Database)?;

            let milestones = vec![
                (
                    Decimal::new(500, 0),
                    Achievement {
                        achievement_type: AchievementType::Score,
                        name: "Rising Star".to_string(),
                        description: "Reached 500 reputation score".to_string(),
                        icon: "⭐".to_string(),
                    },
                ),
                (
                    Decimal::new(750, 0),
                    Achievement {
                        achievement_type: AchievementType::Score,
                        name: "Elite Contributor".to_string(),
                        description: "Reached 750 reputation score".to_string(),
                        icon: "🌟".to_string(),
                    },
                ),
                (
                    Decimal::new(900, 0),
                    Achievement {
                        achievement_type: AchievementType::Score,
                        name: "Legend".to_string(),
                        description: "Reached 900 reputation score".to_string(),
                        icon: "💎".to_string(),
                    },
                ),
            ];

            for (threshold, achievement) in milestones {
                if score >= threshold
                    && self
                        .award_achievement(user_id, achievement.clone())
                        .await
                        .is_ok()
                {
                    awarded.push(achievement);
                }
            }
        }

        // Check commitment count
        let commitment_query = r#"
            SELECT COUNT(*) as count
            FROM output_commitments
            WHERE user_id = $1 AND status = 'Verified'
        "#;

        let row = sqlx::query(commitment_query)
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(ReputationError::Database)?;

        let commitment_count: i64 = row.try_get("count").map_err(ReputationError::Database)?;

        let commitment_milestones = vec![
            (
                10,
                Achievement {
                    achievement_type: AchievementType::Commitment,
                    name: "Committed".to_string(),
                    description: "Completed 10 commitments".to_string(),
                    icon: "📝".to_string(),
                },
            ),
            (
                50,
                Achievement {
                    achievement_type: AchievementType::Commitment,
                    name: "Dedicated".to_string(),
                    description: "Completed 50 commitments".to_string(),
                    icon: "📚".to_string(),
                },
            ),
            (
                100,
                Achievement {
                    achievement_type: AchievementType::Commitment,
                    name: "Unstoppable".to_string(),
                    description: "Completed 100 commitments".to_string(),
                    icon: "🚀".to_string(),
                },
            ),
        ];

        for (threshold, achievement) in commitment_milestones {
            if commitment_count >= threshold
                && self
                    .award_achievement(user_id, achievement.clone())
                    .await
                    .is_ok()
            {
                awarded.push(achievement);
            }
        }

        Ok(awarded)
    }
}

/// Leaderboard with entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leaderboard {
    /// Ordered list of leaderboard entries (rank 1 first).
    pub entries: Vec<LeaderboardEntry>,
    /// Whether this is the global leaderboard or tier-scoped.
    pub leaderboard_type: LeaderboardType,
    /// Timestamp when this leaderboard was generated.
    pub generated_at: DateTime<Utc>,
}

/// Single leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    /// Position in the leaderboard (1-based).
    pub rank: i32,
    /// User identifier.
    pub user_id: Uuid,
    /// User's reputation score.
    pub score: Decimal,
    /// User's reputation tier.
    pub tier: ReputationTier,
}

/// Type of leaderboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeaderboardType {
    /// Leaderboard spanning all users regardless of tier.
    Global,
    /// Leaderboard restricted to users within a specific tier.
    Tier(ReputationTier),
}

impl std::fmt::Display for LeaderboardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Global => write!(f, "Global"),
            Self::Tier(tier) => write!(f, "{} Tier", tier),
        }
    }
}

impl std::str::FromStr for LeaderboardType {
    type Err = crate::error::ReputationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_lowercase();
        if lower == "global" {
            return Ok(LeaderboardType::Global);
        }

        // Try to parse as "tier:bronze" or just "bronze"
        let tier_str = if let Some(stripped) = lower.strip_prefix("tier:") {
            stripped
        } else if let Some(stripped) = lower.strip_suffix(" tier") {
            stripped
        } else {
            lower.as_str()
        };

        let tier: ReputationTier = tier_str.trim().parse().map_err(|e| {
            crate::error::ReputationError::Validation(format!(
                "Failed to parse tier in leaderboard type: {}",
                e
            ))
        })?;
        Ok(LeaderboardType::Tier(tier))
    }
}

/// User rank information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRankInfo {
    /// User identifier.
    pub user_id: Uuid,
    /// User's position in the global leaderboard (1-based).
    pub rank: i32,
    /// User's current reputation score.
    pub score: Decimal,
    /// User's reputation tier.
    pub tier: ReputationTier,
    /// Percentage of users this user outranks (0.0–100.0).
    pub percentile: f64,
    /// Total number of users in the leaderboard.
    pub total_users: i32,
}

/// Achievement definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    /// Classification of this achievement.
    pub achievement_type: AchievementType,
    /// Display name for the achievement.
    pub name: String,
    /// Human-readable description of how to earn this achievement.
    pub description: String,
    /// Icon identifier or URL for visual display.
    pub icon: String,
}

/// Achievement type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AchievementType {
    /// Achievement related to reaching a score milestone.
    Score,
    /// Achievement related to commitment activity.
    Commitment,
    /// Achievement related to reaching a tier.
    Tier,
    /// Achievement for maintaining a streak of fulfilled commitments.
    Streak,
    /// Special or one-off achievement.
    Special,
}

impl std::fmt::Display for AchievementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AchievementType::Score => write!(f, "Score"),
            AchievementType::Commitment => write!(f, "Commitment"),
            AchievementType::Tier => write!(f, "Tier"),
            AchievementType::Streak => write!(f, "Streak"),
            AchievementType::Special => write!(f, "Special"),
        }
    }
}

impl std::str::FromStr for AchievementType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "score" => Ok(AchievementType::Score),
            "commitment" => Ok(AchievementType::Commitment),
            "tier" => Ok(AchievementType::Tier),
            "streak" => Ok(AchievementType::Streak),
            "special" => Ok(AchievementType::Special),
            _ => Err(format!("Unknown achievement type: {}", s)),
        }
    }
}

/// Awarded achievement with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwardedAchievement {
    /// Classification of the awarded achievement.
    pub achievement_type: AchievementType,
    /// Display name of the achievement.
    pub name: String,
    /// Timestamp when the achievement was awarded.
    pub awarded_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leaderboard_entry_structure() {
        let entry = LeaderboardEntry {
            rank: 1,
            user_id: Uuid::new_v4(),
            score: Decimal::new(950, 0),
            tier: ReputationTier::Diamond,
        };

        assert_eq!(entry.rank, 1);
        assert_eq!(entry.tier, ReputationTier::Diamond);
    }

    #[test]
    fn test_user_rank_info_percentile() {
        let info = UserRankInfo {
            user_id: Uuid::new_v4(),
            rank: 10,
            score: Decimal::new(800, 0),
            tier: ReputationTier::Platinum,
            percentile: 90.0,
            total_users: 100,
        };

        assert_eq!(info.percentile, 90.0);
        assert_eq!(info.rank, 10);
    }

    #[test]
    fn test_achievement_type_display() {
        assert_eq!(AchievementType::Score.to_string(), "Score");
        assert_eq!(AchievementType::Commitment.to_string(), "Commitment");
        assert_eq!(AchievementType::Tier.to_string(), "Tier");
    }

    #[test]
    fn test_achievement_type_from_str() {
        assert_eq!(
            "score".parse::<AchievementType>().unwrap(),
            AchievementType::Score
        );
        assert_eq!(
            "commitment".parse::<AchievementType>().unwrap(),
            AchievementType::Commitment
        );
        assert_eq!(
            "TIER".parse::<AchievementType>().unwrap(),
            AchievementType::Tier
        );
    }

    #[test]
    fn test_achievement_structure() {
        let achievement = Achievement {
            achievement_type: AchievementType::Score,
            name: "Rising Star".to_string(),
            description: "Reached 500 reputation".to_string(),
            icon: "⭐".to_string(),
        };

        assert_eq!(achievement.name, "Rising Star");
        assert_eq!(achievement.icon, "⭐");
    }

    #[test]
    fn test_leaderboard_type() {
        let global = LeaderboardType::Global;
        let tier = LeaderboardType::Tier(ReputationTier::Gold);

        match global {
            LeaderboardType::Global => {
                // Correctly identified as Global variant
            }
            _ => panic!("Expected Global"),
        }

        match tier {
            LeaderboardType::Tier(t) => assert_eq!(t, ReputationTier::Gold),
            _ => panic!("Expected Tier"),
        }
    }

    #[test]
    fn test_leaderboard_type_display() {
        assert_eq!(LeaderboardType::Global.to_string(), "Global");
        assert_eq!(
            LeaderboardType::Tier(ReputationTier::Bronze).to_string(),
            "Bronze Tier"
        );
        assert_eq!(
            LeaderboardType::Tier(ReputationTier::Gold).to_string(),
            "Gold Tier"
        );
        assert_eq!(
            LeaderboardType::Tier(ReputationTier::Diamond).to_string(),
            "Diamond Tier"
        );
    }

    #[test]
    fn test_achievement_type_hash() {
        use std::collections::{HashMap, HashSet};

        // Test that AchievementType can be used as HashMap key
        let mut map: HashMap<AchievementType, &str> = HashMap::new();
        map.insert(AchievementType::Score, "score_achievement");
        map.insert(AchievementType::Commitment, "commitment_achievement");
        map.insert(AchievementType::Tier, "tier_achievement");

        assert_eq!(map.get(&AchievementType::Score), Some(&"score_achievement"));
        assert_eq!(
            map.get(&AchievementType::Commitment),
            Some(&"commitment_achievement")
        );
        assert_eq!(map.get(&AchievementType::Tier), Some(&"tier_achievement"));

        // Test that AchievementType can be used in HashSet
        let mut set: HashSet<AchievementType> = HashSet::new();
        set.insert(AchievementType::Score);
        set.insert(AchievementType::Commitment);
        set.insert(AchievementType::Score); // Duplicate

        assert_eq!(set.len(), 2); // Only 2 unique values
        assert!(set.contains(&AchievementType::Score));
        assert!(set.contains(&AchievementType::Commitment));
        assert!(!set.contains(&AchievementType::Tier));
    }

    #[test]
    fn test_achievement_type_hash_consistency() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Same value should produce same hash
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();

        AchievementType::Score.hash(&mut hasher1);
        AchievementType::Score.hash(&mut hasher2);

        assert_eq!(hasher1.finish(), hasher2.finish());

        // Different values should (likely) produce different hashes
        let mut hasher3 = DefaultHasher::new();
        AchievementType::Commitment.hash(&mut hasher3);

        assert_ne!(hasher1.finish(), hasher3.finish());
    }
}
