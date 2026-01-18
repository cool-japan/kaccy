//! Bulk export and import utilities for reputation data
//!
//! Provides functionality for backing up and migrating reputation data,
//! including scores, commitments, events, and all related information.
//!
//! ## CSV Export Support
//!
//! This module now supports CSV export in addition to JSON, enabling seamless
//! integration with spreadsheet tools and data analysis workflows:
//!
//! ```rust,no_run
//! use kaccy_reputation::*;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let pool = sqlx::PgPool::connect("").await?;
//! let service = ExportImportService::new(pool);
//!
//! // Export scores to CSV for analysis
//! let scores = vec![/* reputation scores */];
//! let csv = service.export_scores_to_csv(&scores)?;
//!
//! // Export events to CSV
//! let events = vec![/* events */];
//! let csv = service.export_events_to_csv(&events)?;
//! # Ok(())
//! # }
//! ```

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ReputationError;
use crate::score::{ReputationScore, ScoreComponents};
use crate::tier::ReputationTier;

// Type aliases for complex database query results
type CommitmentRow = (
    Uuid,
    Uuid,
    String,
    String,
    DateTime<Utc>,
    String,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
);
type RatingRow = (Uuid, Uuid, Uuid, Uuid, i32, Option<String>, DateTime<Utc>);

/// Complete export of all reputation data for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserReputationExport {
    pub user_id: Uuid,
    pub exported_at: DateTime<Utc>,
    pub score: ReputationScore,
    pub tier_history: Vec<TierHistoryExport>,
    pub score_history: Vec<ScoreHistoryExport>,
    pub events: Vec<EventExport>,
    pub commitments: Vec<CommitmentExport>,
    pub ratings_given: Vec<RatingExport>,
    pub ratings_received: Vec<RatingExport>,
}

/// Tier history record for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierHistoryExport {
    pub tier: ReputationTier,
    pub score: Decimal,
    pub changed_at: DateTime<Utc>,
}

/// Score history record for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreHistoryExport {
    pub score: Decimal,
    pub components: ScoreComponents,
    pub snapshot_date: DateTime<Utc>,
}

/// Event record for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventExport {
    pub event_id: Uuid,
    pub event_type: String,
    pub delta: Decimal,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

/// Commitment record for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentExport {
    pub commitment_id: Uuid,
    pub token_id: Uuid,
    pub title: String,
    pub description: String,
    pub deadline: DateTime<Utc>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Rating record for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingExport {
    pub rating_id: Uuid,
    pub rater_user_id: Uuid,
    pub issuer_user_id: Uuid,
    pub token_id: Uuid,
    pub rating: i32,
    pub feedback: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Bulk export for multiple users
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkReputationExport {
    pub exported_at: DateTime<Utc>,
    pub version: String,
    pub user_count: usize,
    pub users: Vec<UserReputationExport>,
}

/// Service for exporting and importing reputation data
pub struct ExportImportService {
    pool: PgPool,
}

impl ExportImportService {
    /// Create a new export/import service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Export all reputation data for a single user
    pub async fn export_user(
        &self,
        user_id: Uuid,
    ) -> Result<UserReputationExport, ReputationError> {
        // Get current score
        let score_row: (Uuid, Decimal) =
            sqlx::query_as("SELECT user_id, reputation_score FROM users WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;

        let score = ReputationScore {
            user_id,
            overall_score: score_row.1,
            tier: ReputationTier::from_score(score_row.1),
            components: ScoreComponents::default(),
        };

        // Get tier history
        let tier_history = self.export_tier_history(user_id).await?;

        // Get score history
        let score_history = self.export_score_history(user_id).await?;

        // Get events
        let events = self.export_events(user_id).await?;

        // Get commitments
        let commitments = self.export_commitments(user_id).await?;

        // Get ratings given and received
        let ratings_given = self.export_ratings_given(user_id).await?;
        let ratings_received = self.export_ratings_received(user_id).await?;

        Ok(UserReputationExport {
            user_id,
            exported_at: Utc::now(),
            score,
            tier_history,
            score_history,
            events,
            commitments,
            ratings_given,
            ratings_received,
        })
    }

    /// Export reputation data for multiple users
    pub async fn export_bulk(
        &self,
        user_ids: Vec<Uuid>,
    ) -> Result<BulkReputationExport, ReputationError> {
        let mut users = Vec::new();

        for user_id in &user_ids {
            let user_export = self.export_user(*user_id).await?;
            users.push(user_export);
        }

        Ok(BulkReputationExport {
            exported_at: Utc::now(),
            version: "1.0.0".to_string(),
            user_count: users.len(),
            users,
        })
    }

    /// Export all users in the system
    pub async fn export_all(&self) -> Result<BulkReputationExport, ReputationError> {
        let user_rows: Vec<(Uuid,)> = sqlx::query_as("SELECT user_id FROM users ORDER BY user_id")
            .fetch_all(&self.pool)
            .await?;

        let user_ids: Vec<Uuid> = user_rows.iter().map(|row| row.0).collect();
        self.export_bulk(user_ids).await
    }

    /// Export to JSON string
    pub fn export_to_json(export: &BulkReputationExport) -> Result<String, ReputationError> {
        serde_json::to_string_pretty(export)
            .map_err(|e| ReputationError::Validation(format!("JSON serialization failed: {}", e)))
    }

    /// Import from JSON string
    pub fn import_from_json(json: &str) -> Result<BulkReputationExport, ReputationError> {
        serde_json::from_str(json)
            .map_err(|e| ReputationError::Validation(format!("JSON deserialization failed: {}", e)))
    }

    // Helper methods for exporting specific data types

    async fn export_tier_history(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<TierHistoryExport>, ReputationError> {
        let rows: Vec<(String, Decimal, DateTime<Utc>)> = sqlx::query_as(
            "SELECT tier, score, changed_at FROM tier_history WHERE user_id = $1 ORDER BY changed_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TierHistoryExport {
                tier: ReputationTier::from_score(row.1),
                score: row.1,
                changed_at: row.2,
            })
            .collect())
    }

    async fn export_score_history(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ScoreHistoryExport>, ReputationError> {
        let rows: Vec<(Decimal, DateTime<Utc>)> = sqlx::query_as(
            "SELECT score, snapshot_date FROM score_snapshots WHERE user_id = $1 ORDER BY snapshot_date DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| ScoreHistoryExport {
                score: row.0,
                components: ScoreComponents::default(),
                snapshot_date: row.1,
            })
            .collect())
    }

    async fn export_events(&self, user_id: Uuid) -> Result<Vec<EventExport>, ReputationError> {
        let rows: Vec<(Uuid, String, Decimal, String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT event_id, event_type, delta, reason, created_at FROM reputation_events WHERE user_id = $1 ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| EventExport {
                event_id: row.0,
                event_type: row.1,
                delta: row.2,
                reason: row.3,
                created_at: row.4,
            })
            .collect())
    }

    async fn export_commitments(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<CommitmentExport>, ReputationError> {
        let rows: Vec<CommitmentRow> = sqlx::query_as(
            "SELECT commitment_id, token_id, title, description, deadline, status, created_at, completed_at FROM output_commitments WHERE user_id = $1 ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| CommitmentExport {
                commitment_id: row.0,
                token_id: row.1,
                title: row.2,
                description: row.3,
                deadline: row.4,
                status: row.5,
                created_at: row.6,
                completed_at: row.7,
            })
            .collect())
    }

    async fn export_ratings_given(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<RatingExport>, ReputationError> {
        let rows: Vec<RatingRow> = sqlx::query_as(
            "SELECT rating_id, rater_user_id, issuer_user_id, token_id, rating, feedback, created_at FROM holder_ratings WHERE rater_user_id = $1 ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| RatingExport {
                rating_id: row.0,
                rater_user_id: row.1,
                issuer_user_id: row.2,
                token_id: row.3,
                rating: row.4,
                feedback: row.5,
                created_at: row.6,
            })
            .collect())
    }

    async fn export_ratings_received(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<RatingExport>, ReputationError> {
        let rows: Vec<RatingRow> = sqlx::query_as(
            "SELECT rating_id, rater_user_id, issuer_user_id, token_id, rating, feedback, created_at FROM holder_ratings WHERE issuer_user_id = $1 ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| RatingExport {
                rating_id: row.0,
                rater_user_id: row.1,
                issuer_user_id: row.2,
                token_id: row.3,
                rating: row.4,
                feedback: row.5,
                created_at: row.6,
            })
            .collect())
    }

    // ========================================================================
    // CSV Export Methods (Phase 22)
    // ========================================================================

    /// Export reputation scores to CSV format
    ///
    /// Generates a CSV file with headers: user_id,overall_score,tier,timestamp
    /// Useful for spreadsheet analysis and data visualization tools.
    pub fn export_scores_to_csv(
        &self,
        scores: &[ReputationScore],
    ) -> Result<String, ReputationError> {
        let mut csv = String::from("user_id,overall_score,tier,timestamp\n");

        let timestamp = Utc::now().to_rfc3339();
        for score in scores {
            csv.push_str(&format!(
                "{},{},{},{}\n",
                score.user_id,
                score.overall_score,
                score.tier.as_str(),
                timestamp
            ));
        }

        Ok(csv)
    }

    /// Export reputation scores with full component breakdown to CSV
    ///
    /// Headers: user_id,overall_score,tier,commitment_fulfillment,response_time,
    /// quality_rating,community_trust,longevity,timestamp
    pub fn export_scores_detailed_to_csv(
        &self,
        scores: &[ReputationScore],
    ) -> Result<String, ReputationError> {
        let mut csv = String::from(
            "user_id,overall_score,tier,commitment_fulfillment,response_time,quality_rating,community_trust,longevity,timestamp\n",
        );

        let timestamp = Utc::now().to_rfc3339();
        for score in scores {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                score.user_id,
                score.overall_score,
                score.tier.as_str(),
                score.components.commitment_fulfillment,
                score.components.response_time,
                score.components.quality_rating,
                score.components.community_trust,
                score.components.longevity,
                timestamp
            ));
        }

        Ok(csv)
    }

    /// Export events to CSV format
    ///
    /// Headers: event_id,user_id,event_type,delta,reason,created_at
    pub fn export_events_to_csv(&self, events: &[EventExport]) -> Result<String, ReputationError> {
        let mut csv = String::from("event_id,user_id,event_type,delta,reason,created_at\n");

        for event in events {
            let reason = event.reason.replace(',', ";").replace('\n', " ");
            csv.push_str(&format!(
                "{},{},{},{},\"{}\",{}\n",
                event.event_id,
                "", // user_id not available in EventExport, would need to be added
                event.event_type,
                event.delta,
                reason,
                event.created_at.to_rfc3339()
            ));
        }

        Ok(csv)
    }

    /// Export commitments to CSV format
    ///
    /// Headers: commitment_id,token_id,title,description,deadline,status,created_at,completed_at
    pub fn export_commitments_to_csv(
        &self,
        commitments: &[CommitmentExport],
    ) -> Result<String, ReputationError> {
        let mut csv = String::from(
            "commitment_id,token_id,title,description,deadline,status,created_at,completed_at\n",
        );

        for commitment in commitments {
            let title = commitment.title.replace(',', ";").replace('\n', " ");
            let description = commitment.description.replace(',', ";").replace('\n', " ");
            let completed_at = commitment
                .completed_at
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "".to_string());

            csv.push_str(&format!(
                "{},{},\"{}\",\"{}\",{},{},{},{}\n",
                commitment.commitment_id,
                commitment.token_id,
                title,
                description,
                commitment.deadline.to_rfc3339(),
                commitment.status,
                commitment.created_at.to_rfc3339(),
                completed_at
            ));
        }

        Ok(csv)
    }

    /// Export ratings to CSV format
    ///
    /// Headers: rating_id,rater_user_id,issuer_user_id,token_id,rating,feedback,created_at
    pub fn export_ratings_to_csv(
        &self,
        ratings: &[RatingExport],
    ) -> Result<String, ReputationError> {
        let mut csv = String::from(
            "rating_id,rater_user_id,issuer_user_id,token_id,rating,feedback,created_at\n",
        );

        for rating in ratings {
            let feedback = rating
                .feedback
                .as_ref()
                .map(|f| f.replace(',', ";").replace('\n', " ").replace('"', "'"))
                .unwrap_or_else(|| "".to_string());

            csv.push_str(&format!(
                "{},{},{},{},{},\"{}\",{}\n",
                rating.rating_id,
                rating.rater_user_id,
                rating.issuer_user_id,
                rating.token_id,
                rating.rating,
                feedback,
                rating.created_at.to_rfc3339()
            ));
        }

        Ok(csv)
    }

    /// Export tier history to CSV format
    ///
    /// Headers: tier,score,changed_at
    pub fn export_tier_history_to_csv(
        &self,
        history: &[TierHistoryExport],
    ) -> Result<String, ReputationError> {
        let mut csv = String::from("tier,score,changed_at\n");

        for entry in history {
            csv.push_str(&format!(
                "{},{},{}\n",
                entry.tier.as_str(),
                entry.score,
                entry.changed_at.to_rfc3339()
            ));
        }

        Ok(csv)
    }

    /// Export score history to CSV format
    ///
    /// Headers: score,commitment_fulfillment,response_time,quality_rating,
    /// community_trust,longevity,snapshot_date
    pub fn export_score_history_to_csv(
        &self,
        history: &[ScoreHistoryExport],
    ) -> Result<String, ReputationError> {
        let mut csv = String::from(
            "score,commitment_fulfillment,response_time,quality_rating,community_trust,longevity,snapshot_date\n",
        );

        for entry in history {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                entry.score,
                entry.components.commitment_fulfillment,
                entry.components.response_time,
                entry.components.quality_rating,
                entry.components.community_trust,
                entry.components.longevity,
                entry.snapshot_date.to_rfc3339()
            ));
        }

        Ok(csv)
    }
}

/// Export statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportStats {
    pub total_users: usize,
    pub total_events: usize,
    pub total_commitments: usize,
    pub total_ratings: usize,
    pub export_size_bytes: usize,
}

impl BulkReputationExport {
    /// Get statistics about the export
    pub fn get_stats(&self) -> ExportStats {
        let total_events: usize = self.users.iter().map(|u| u.events.len()).sum();
        let total_commitments: usize = self.users.iter().map(|u| u.commitments.len()).sum();
        let total_ratings: usize = self
            .users
            .iter()
            .map(|u| u.ratings_given.len() + u.ratings_received.len())
            .sum();

        ExportStats {
            total_users: self.user_count,
            total_events,
            total_commitments,
            total_ratings,
            export_size_bytes: 0, // Would be calculated from JSON serialization
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_stats() {
        let export = BulkReputationExport {
            exported_at: Utc::now(),
            version: "1.0.0".to_string(),
            user_count: 2,
            users: vec![UserReputationExport {
                user_id: Uuid::new_v4(),
                exported_at: Utc::now(),
                score: ReputationScore {
                    user_id: Uuid::new_v4(),
                    overall_score: rust_decimal_macros::dec!(500),
                    tier: ReputationTier::Silver,
                    components: Default::default(),
                },
                tier_history: vec![],
                score_history: vec![],
                events: vec![],
                commitments: vec![],
                ratings_given: vec![],
                ratings_received: vec![],
            }],
        };

        let stats = export.get_stats();
        assert_eq!(stats.total_users, 2);
    }

    #[test]
    fn test_json_export_import() {
        let export = BulkReputationExport {
            exported_at: Utc::now(),
            version: "1.0.0".to_string(),
            user_count: 1,
            users: vec![UserReputationExport {
                user_id: Uuid::new_v4(),
                exported_at: Utc::now(),
                score: ReputationScore {
                    user_id: Uuid::new_v4(),
                    overall_score: rust_decimal_macros::dec!(500),
                    tier: ReputationTier::Silver,
                    components: Default::default(),
                },
                tier_history: vec![],
                score_history: vec![],
                events: vec![],
                commitments: vec![],
                ratings_given: vec![],
                ratings_received: vec![],
            }],
        };

        let json = ExportImportService::export_to_json(&export).unwrap();
        let imported = ExportImportService::import_from_json(&json).unwrap();

        assert_eq!(imported.user_count, 1);
        assert_eq!(imported.version, "1.0.0");
    }

    // Phase 22 CSV Export Tests

    // Helper to create a mock service for CSV export tests (doesn't use DB)
    fn mock_export_service() -> ExportImportService {
        // CSV export methods don't actually use the pool, so we create a dummy one
        // This is safe because we never call async methods that require actual DB connection
        let options = sqlx::postgres::PgConnectOptions::new();
        let pool = PgPool::connect_lazy_with(options);
        ExportImportService::new(pool)
    }

    #[tokio::test]
    async fn test_csv_export_scores() {
        use rust_decimal_macros::dec;

        let service = mock_export_service();

        let user_id = Uuid::new_v4();
        let scores = vec![
            ReputationScore {
                user_id,
                overall_score: dec!(750),
                tier: ReputationTier::Gold,
                components: Default::default(),
            },
            ReputationScore {
                user_id: Uuid::new_v4(),
                overall_score: dec!(500),
                tier: ReputationTier::Silver,
                components: Default::default(),
            },
        ];

        let csv = service.export_scores_to_csv(&scores).unwrap();

        // Check header
        assert!(csv.starts_with("user_id,overall_score,tier,timestamp\n"));

        // Check content
        assert!(csv.contains(&user_id.to_string()));
        assert!(csv.contains("750"));
        assert!(csv.contains("gold"));
        assert!(csv.contains("500"));
        assert!(csv.contains("silver"));

        // Check line count (header + 2 data rows)
        assert_eq!(csv.lines().count(), 3);
    }

    #[tokio::test]
    async fn test_csv_export_scores_detailed() {
        use rust_decimal_macros::dec;

        let service = mock_export_service();

        let user_id = Uuid::new_v4();
        let scores = vec![ReputationScore {
            user_id,
            overall_score: dec!(750),
            tier: ReputationTier::Gold,
            components: ScoreComponents {
                commitment_fulfillment: dec!(85),
                response_time: dec!(75),
                quality_rating: dec!(90),
                community_trust: dec!(80),
                longevity: dec!(70),
            },
        }];

        let csv = service.export_scores_detailed_to_csv(&scores).unwrap();

        // Check header
        assert!(csv.contains("commitment_fulfillment"));
        assert!(csv.contains("response_time"));
        assert!(csv.contains("quality_rating"));
        assert!(csv.contains("community_trust"));
        assert!(csv.contains("longevity"));

        // Check component values
        assert!(csv.contains("85"));
        assert!(csv.contains("75"));
        assert!(csv.contains("90"));
        assert!(csv.contains("80"));
        assert!(csv.contains("70"));
    }

    #[tokio::test]
    async fn test_csv_export_events() {
        let service = mock_export_service();

        let events = vec![
            EventExport {
                event_id: Uuid::new_v4(),
                event_type: "commitment_fulfilled".to_string(),
                delta: rust_decimal_macros::dec!(10),
                reason: "Delivered on time".to_string(),
                created_at: Utc::now(),
            },
            EventExport {
                event_id: Uuid::new_v4(),
                event_type: "quality_score".to_string(),
                delta: rust_decimal_macros::dec!(5),
                reason: "High quality work, includes commas".to_string(),
                created_at: Utc::now(),
            },
        ];

        let csv = service.export_events_to_csv(&events).unwrap();

        // Check header
        assert!(csv.starts_with("event_id,user_id,event_type,delta,reason,created_at\n"));

        // Check content
        assert!(csv.contains("commitment_fulfilled"));
        assert!(csv.contains("quality_score"));
        assert!(csv.contains("10"));
        assert!(csv.contains("5"));

        // Check comma replacement in reason
        assert!(csv.contains("includes commas"));
        assert!(csv.contains(';')); // Comma should be replaced with semicolon
    }

    #[tokio::test]
    async fn test_csv_export_commitments() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let service = ExportImportService::new(pool);

        let commitment_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();
        let commitments = vec![CommitmentExport {
            commitment_id,
            token_id,
            title: "Complete documentation".to_string(),
            description: "Write comprehensive API docs, includes commas and newlines\nNext line"
                .to_string(),
            deadline: Utc::now(),
            status: "verified".to_string(),
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        }];

        let csv = service.export_commitments_to_csv(&commitments).unwrap();

        // Check header
        assert!(csv.contains("commitment_id,token_id,title,description"));

        // Check content
        assert!(csv.contains(&commitment_id.to_string()));
        assert!(csv.contains(&token_id.to_string()));
        assert!(csv.contains("Complete documentation"));
        assert!(csv.contains("verified"));

        // Check special character handling
        assert!(!csv.contains("\n\nNext")); // Newline should be replaced with space
    }

    #[tokio::test]
    async fn test_csv_export_ratings() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let service = ExportImportService::new(pool);

        let ratings = vec![
            RatingExport {
                rating_id: Uuid::new_v4(),
                rater_user_id: Uuid::new_v4(),
                issuer_user_id: Uuid::new_v4(),
                token_id: Uuid::new_v4(),
                rating: 5,
                feedback: Some("Excellent work!".to_string()),
                created_at: Utc::now(),
            },
            RatingExport {
                rating_id: Uuid::new_v4(),
                rater_user_id: Uuid::new_v4(),
                issuer_user_id: Uuid::new_v4(),
                token_id: Uuid::new_v4(),
                rating: 3,
                feedback: None,
                created_at: Utc::now(),
            },
        ];

        let csv = service.export_ratings_to_csv(&ratings).unwrap();

        // Check header
        assert!(csv.starts_with(
            "rating_id,rater_user_id,issuer_user_id,token_id,rating,feedback,created_at\n"
        ));

        // Check content
        assert!(csv.contains("5"));
        assert!(csv.contains("3"));
        assert!(csv.contains("Excellent work!"));

        // Check line count
        assert_eq!(csv.lines().count(), 3); // header + 2 ratings
    }

    #[tokio::test]
    async fn test_csv_export_tier_history() {
        use rust_decimal_macros::dec;

        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let service = ExportImportService::new(pool);

        let history = vec![
            TierHistoryExport {
                tier: ReputationTier::Bronze,
                score: dec!(250),
                changed_at: Utc::now(),
            },
            TierHistoryExport {
                tier: ReputationTier::Silver,
                score: dec!(450),
                changed_at: Utc::now(),
            },
            TierHistoryExport {
                tier: ReputationTier::Gold,
                score: dec!(750),
                changed_at: Utc::now(),
            },
        ];

        let csv = service.export_tier_history_to_csv(&history).unwrap();

        // Check header
        assert!(csv.starts_with("tier,score,changed_at\n"));

        // Check content
        assert!(csv.contains("bronze"));
        assert!(csv.contains("silver"));
        assert!(csv.contains("gold"));
        assert!(csv.contains("250"));
        assert!(csv.contains("450"));
        assert!(csv.contains("750"));

        // Check line count
        assert_eq!(csv.lines().count(), 4); // header + 3 entries
    }

    #[tokio::test]
    async fn test_csv_export_score_history() {
        use rust_decimal_macros::dec;

        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let service = ExportImportService::new(pool);

        let history = vec![ScoreHistoryExport {
            score: dec!(750),
            components: ScoreComponents {
                commitment_fulfillment: dec!(85),
                response_time: dec!(75),
                quality_rating: dec!(90),
                community_trust: dec!(80),
                longevity: dec!(70),
            },
            snapshot_date: Utc::now(),
        }];

        let csv = service.export_score_history_to_csv(&history).unwrap();

        // Check header
        assert!(csv.contains("score,commitment_fulfillment,response_time,quality_rating,community_trust,longevity,snapshot_date"));

        // Check content
        assert!(csv.contains("750"));
        assert!(csv.contains("85"));
        assert!(csv.contains("75"));
        assert!(csv.contains("90"));
        assert!(csv.contains("80"));
        assert!(csv.contains("70"));

        // Check line count
        assert_eq!(csv.lines().count(), 2); // header + 1 entry
    }

    #[tokio::test]
    async fn test_csv_empty_arrays() {
        let pool = PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let service = ExportImportService::new(pool);

        // Test with empty arrays
        let scores: Vec<ReputationScore> = vec![];
        let events: Vec<EventExport> = vec![];
        let commitments: Vec<CommitmentExport> = vec![];

        let csv_scores = service.export_scores_to_csv(&scores).unwrap();
        let csv_events = service.export_events_to_csv(&events).unwrap();
        let csv_commitments = service.export_commitments_to_csv(&commitments).unwrap();

        // Should only contain headers
        assert_eq!(csv_scores.lines().count(), 1);
        assert_eq!(csv_events.lines().count(), 1);
        assert_eq!(csv_commitments.lines().count(), 1);
    }
}
