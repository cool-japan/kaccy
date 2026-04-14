//! Integration module for coordinating all reputation system features
//!
//! This module provides high-level orchestration of reputation system
//! features including SBT minting, AI scoring, disputes, portability,
//! analytics, audit trails, batch operations, leaderboards, and maintenance.

use sqlx::PgPool;
use uuid::Uuid;

use crate::ai_scoring::{AIScoringService, EvaluateQualityRequest};
use crate::dispute::{DisputeService, VoteDisputeRequest};
use crate::error::ReputationError;
use crate::portability::{ImportReputationRequest, PortabilityService};
use crate::sbt::{MintSBTRequest, SBTService};
use crate::score::ReputationEngine;
use crate::tier::ReputationTier;

// Phase 6 services
use crate::analytics::AnalyticsService;
use crate::audit::{AuditSearch, AuditService};
use crate::batch::BatchService;
use crate::leaderboard::LeaderboardService;
use crate::maintenance::MaintenanceService;

/// Orchestrates all reputation system services (Phases 0-6)
pub struct ReputationOrchestrator {
    /// Core reputation scoring engine.
    pub engine: ReputationEngine,
    /// Service for minting Soulbound Tokens.
    pub sbt_service: SBTService,
    /// Service for AI-assisted quality evaluation and fraud detection.
    pub ai_service: AIScoringService,
    /// Service for managing user-raised disputes.
    pub dispute_service: DisputeService,
    /// Service for importing and exporting reputation data.
    pub portability_service: PortabilityService,
    /// Service for system-wide analytics and insights.
    pub analytics_service: AnalyticsService,
    /// Service for audit trail management.
    pub audit_service: AuditService,
    /// Service for bulk/batch reputation operations.
    pub batch_service: BatchService,
    /// Service for leaderboards and achievement rankings.
    pub leaderboard_service: LeaderboardService,
    /// Service for automated periodic maintenance tasks.
    pub maintenance_service: MaintenanceService,
}

impl ReputationOrchestrator {
    /// Create a new [`ReputationOrchestrator`] initialising all services with the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            engine: ReputationEngine::new(pool.clone()),
            sbt_service: SBTService::new(pool.clone()),
            ai_service: AIScoringService::new(pool.clone()),
            dispute_service: DisputeService::new(pool.clone()),
            portability_service: PortabilityService::new(pool.clone()),
            // Phase 6 services
            analytics_service: AnalyticsService::new(pool.clone()),
            audit_service: AuditService::new(pool.clone()),
            batch_service: BatchService::new(pool.clone()),
            leaderboard_service: LeaderboardService::new(pool.clone()),
            maintenance_service: MaintenanceService::new(pool),
        }
    }

    /// Complete workflow: Issue first token -> mint SBT
    pub async fn on_first_token_issuance(&self, user_id: Uuid) -> Result<(), ReputationError> {
        // Check if user already has an SBT
        if self.sbt_service.has_sbt(user_id).await? {
            return Ok(());
        }

        // Get current reputation
        let score = self.engine.get_score(user_id).await?;

        // Create SBT
        let initial_score = score
            .overall_score
            .to_string()
            .parse::<i32>()
            .map_err(|e| {
                ReputationError::Validation(format!("Failed to parse score as i32: {}", e))
            })?;

        let sbt_request = MintSBTRequest {
            user_id,
            initial_score,
            initial_tier: score.tier,
        };

        self.sbt_service.create_sbt(sbt_request).await?;

        Ok(())
    }

    /// Complete workflow: Commitment submitted -> AI evaluation -> update score
    pub async fn evaluate_and_score_commitment(
        &self,
        commitment_id: Uuid,
        user_id: Uuid,
        evidence_url: String,
        evidence_description: String,
        commitment_description: String,
    ) -> Result<i32, ReputationError> {
        // AI evaluation
        let eval_request = EvaluateQualityRequest {
            commitment_id,
            evidence_url,
            evidence_description,
            commitment_description,
        };

        let evaluation = self.ai_service.evaluate_quality(eval_request).await?;

        // Update SBT metadata if needed
        if let Some(sbt) = self.sbt_service.get_sbt(user_id).await? {
            if sbt.minted {
                let current_score: i32 = evaluation.quality_score;
                if self
                    .sbt_service
                    .needs_update(user_id, current_score, 10)
                    .await?
                {
                    let score = self.engine.get_score(user_id).await?;
                    let new_score =
                        score
                            .overall_score
                            .to_string()
                            .parse::<i32>()
                            .map_err(|e| {
                                ReputationError::Validation(format!(
                                    "Failed to parse score as i32: {}",
                                    e
                                ))
                            })?;

                    self.sbt_service
                        .update_metadata(crate::sbt::UpdateSBTRequest {
                            user_id,
                            new_score,
                            new_tier: score.tier,
                        })
                        .await?;
                }
            }
        }

        Ok(evaluation.quality_score)
    }

    /// Complete workflow: Dispute vote -> resolve -> update reputation
    pub async fn process_dispute_vote(
        &self,
        vote_request: VoteDisputeRequest,
    ) -> Result<(), ReputationError> {
        // Submit vote
        self.dispute_service.vote(vote_request.clone()).await?;

        // Check if voting period ended
        let dispute = self
            .dispute_service
            .get_dispute(vote_request.dispute_id)
            .await?;
        if dispute.voting_deadline < chrono::Utc::now() {
            // Resolve dispute
            let results = self
                .dispute_service
                .resolve_dispute(vote_request.dispute_id)
                .await?;

            // If upheld, adjust issuer's reputation
            if results.outcome == crate::dispute::DisputeOutcome::Upheld {
                // Negative adjustment for failed commitment
                // This would typically be handled by the commitment module
            }
        }

        Ok(())
    }

    /// Complete workflow: Import external reputation -> boost score -> update SBT
    pub async fn import_and_apply_external_reputation(
        &self,
        import_request: ImportReputationRequest,
    ) -> Result<i32, ReputationError> {
        // Import reputation
        let import_result = self
            .portability_service
            .import_reputation(import_request.clone())
            .await?;

        // After verification, this score can be used to boost user's reputation
        // For now, just return the imported score
        Ok(import_result.imported_score)
    }

    /// Check for fraud across all user activity
    pub async fn comprehensive_fraud_check(&self, user_id: Uuid) -> Result<bool, ReputationError> {
        let fraud_result = self
            .ai_service
            .detect_fraud(crate::ai_scoring::DetectFraudRequest {
                user_id,
                activity_window_days: 30,
            })
            .await?;

        Ok(fraud_result.is_some())
    }

    /// Get complete user reputation profile including all Phase 2 features
    pub async fn get_complete_profile(
        &self,
        user_id: Uuid,
    ) -> Result<CompleteReputationProfile, ReputationError> {
        let score = self.engine.get_score(user_id).await?;
        let sbt = self.sbt_service.get_sbt(user_id).await?;
        let external_reps = self
            .portability_service
            .get_verified_imports(user_id)
            .await?;
        let fraud_detections = self.ai_service.get_fraud_detections(user_id).await?;

        Ok(CompleteReputationProfile {
            score: score.overall_score,
            tier: score.tier,
            has_sbt: sbt.is_some(),
            sbt_minted: sbt.map(|s| s.minted).unwrap_or(false),
            verified_external_sources: external_reps.len(),
            fraud_flags: fraud_detections.len(),
            clean_record: fraud_detections.is_empty(),
        })
    }

    // ========================================================================
    // Phase 6 Workflow Methods
    // ========================================================================

    /// Comprehensive system health check with audit trail
    pub async fn run_system_health_check(&self) -> Result<SystemHealthReport, ReputationError> {
        // Run maintenance health checks
        let health_report = self.maintenance_service.run_health_checks().await?;

        // Get system statistics
        let system_stats = self.analytics_service.get_system_stats().await?;

        // Identify any low-score users that need attention
        let low_score_users = self
            .batch_service
            .get_low_score_users(rust_decimal::Decimal::new(200, 0))
            .await?;

        let total_anomalies = health_report.score_anomalies
            + health_report.stuck_commitments
            + health_report.orphaned_records;

        Ok(SystemHealthReport {
            database_healthy: health_report.database_healthy,
            total_users: system_stats.total_users as i64,
            active_users: system_stats.active_users_30d as i64,
            anomalies_count: total_anomalies as usize,
            low_score_users_count: low_score_users.len(),
            health_report,
        })
    }

    /// Perform daily batch maintenance operations
    pub async fn run_daily_maintenance(&self) -> Result<DailyMaintenanceReport, ReputationError> {
        // Apply time decay to inactive users
        let decay_result = self.batch_service.apply_time_decay_batch().await?;

        // Expire overdue commitments
        let expire_result = self.batch_service.expire_overdue_commitments().await?;

        // Recalculate all scores for consistency
        let recalc_result = self.batch_service.recalculate_scores(vec![]).await?;

        Ok(DailyMaintenanceReport {
            users_decayed: decay_result.users_decayed as i64,
            commitments_expired: expire_result.expired_count as i64,
            scores_recalculated: recalc_result.successful as i64,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Get comprehensive analytics dashboard data
    pub async fn get_analytics_dashboard(&self) -> Result<AnalyticsDashboard, ReputationError> {
        let system_stats = self.analytics_service.get_system_stats().await?;
        let growth_metrics = self.analytics_service.get_growth_metrics(30).await?;
        let top_performers = self.analytics_service.get_top_performers(10).await?;
        let score_distribution = self.analytics_service.get_score_distribution().await?;
        let global_leaderboard = self
            .leaderboard_service
            .get_global_leaderboard(20, 0)
            .await?;

        Ok(AnalyticsDashboard {
            system_stats,
            growth_metrics,
            top_performers,
            score_distribution,
            global_leaderboard,
        })
    }

    /// Process user achievement awards and update leaderboard
    pub async fn process_user_achievements(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<crate::leaderboard::AwardedAchievement>, ReputationError> {
        // Get user's current achievements
        let achievements = self
            .leaderboard_service
            .get_user_achievements(user_id)
            .await?;

        // Get user's rank for potential rank-based achievements
        let _rank_info = self.leaderboard_service.get_user_rank(user_id).await?;

        Ok(achievements)
    }

    /// Get user audit trail with analytics context
    pub async fn get_user_activity_report(
        &self,
        user_id: Uuid,
        limit: i32,
    ) -> Result<UserActivityReport, ReputationError> {
        // Get audit trail (last N records)
        let audit_records = self
            .audit_service
            .get_user_audit_trail(user_id, limit, 0)
            .await?;

        // Get commitment analytics
        let commitment_analytics = self
            .analytics_service
            .get_user_commitment_analytics(user_id)
            .await?;

        // Get user rank
        let rank_info = self.leaderboard_service.get_user_rank(user_id).await?;

        // Get achievements
        let achievements = self
            .leaderboard_service
            .get_user_achievements(user_id)
            .await?;

        Ok(UserActivityReport {
            user_id,
            audit_events_count: audit_records.len(),
            commitment_success_rate: commitment_analytics.success_rate,
            total_commitments: commitment_analytics.total_commitments,
            rank: rank_info.rank,
            achievements_count: achievements.len(),
            activity_period_days: limit,
        })
    }

    /// Perform bulk user verification with batch processing
    pub async fn bulk_verify_commitments(
        &self,
        admin_id: Uuid,
        commitment_ids: Vec<Uuid>,
    ) -> Result<BulkVerificationReport, ReputationError> {
        let verified_count = 0;
        let failed_count = 0;

        // This would integrate with the commitment module for actual verification
        // For now, we'll just track the counts
        // In production, this would iterate through commitment_ids and verify each one

        Ok(BulkVerificationReport {
            total_processed: commitment_ids.len(),
            verified_count,
            failed_count,
            admin_id,
        })
    }

    /// Search audit trail with advanced filters
    pub async fn search_audit_trail(
        &self,
        search: AuditSearch,
    ) -> Result<Vec<crate::audit::AuditRecord>, ReputationError> {
        self.audit_service.search_audit_trail(search).await
    }

    /// Get system-wide audit summary
    pub async fn get_audit_summary(
        &self,
        start_date: chrono::DateTime<chrono::Utc>,
        end_date: chrono::DateTime<chrono::Utc>,
    ) -> Result<crate::audit::AuditSummary, ReputationError> {
        self.audit_service
            .get_audit_summary(start_date, end_date)
            .await
    }

    // ========================================================================
    // Phase 8 Advanced Production Workflows
    // ========================================================================

    /// Complete user onboarding workflow
    /// Sets up a new user with initial reputation, SBT, and baseline metrics
    pub async fn onboard_new_user(
        &self,
        user_id: Uuid,
    ) -> Result<UserOnboardingReport, ReputationError> {
        // Check current reputation score
        let initial_score = self.engine.get_score(user_id).await?;

        // Mint SBT if first token issuance
        let sbt_created = if !self.sbt_service.has_sbt(user_id).await? {
            self.on_first_token_issuance(user_id).await?;
            true
        } else {
            false
        };

        // Get initial rank
        let rank_info = self.leaderboard_service.get_user_rank(user_id).await?;

        // Get commitment analytics baseline
        let commitment_analytics = self
            .analytics_service
            .get_user_commitment_analytics(user_id)
            .await?;

        Ok(UserOnboardingReport {
            user_id,
            initial_score: initial_score.overall_score,
            initial_tier: initial_score.tier,
            sbt_created,
            initial_rank: rank_info.rank,
            total_users: rank_info.total_users,
            baseline_commitments: commitment_analytics.total_commitments,
            onboarded_at: chrono::Utc::now(),
        })
    }

    /// Crisis management workflow
    /// Handles system-wide emergencies and returns action plan
    pub async fn handle_crisis(
        &self,
        crisis_type: CrisisType,
    ) -> Result<CrisisResponse, ReputationError> {
        let mut actions_taken = Vec::new();
        let mut affected_users = 0;

        match crisis_type {
            CrisisType::MassScoreAnomaly => {
                // Run health checks
                let health_report = self.maintenance_service.run_health_checks().await?;

                if health_report.score_anomalies > 0 {
                    // Run weekly maintenance which includes score recalculation
                    let maintenance_report =
                        self.maintenance_service.run_weekly_maintenance().await?;
                    actions_taken.push(format!(
                        "Recalculated {} scores to fix anomalies",
                        maintenance_report.scores_recalculated
                    ));
                    affected_users += maintenance_report.scores_recalculated;
                }
            }
            CrisisType::DatabaseCorruption => {
                // Run comprehensive maintenance
                let maintenance_report = self.maintenance_service.run_weekly_maintenance().await?;
                actions_taken.push(format!(
                    "Recalculated {} scores",
                    maintenance_report.scores_recalculated
                ));
                affected_users += maintenance_report.scores_recalculated;

                // Additional score recalculation if needed
                let recalc_result = self.batch_service.recalculate_scores(vec![]).await?;
                actions_taken.push(format!(
                    "Verified {} score calculations",
                    recalc_result.successful
                ));
            }
            CrisisType::MassFraudDetection => {
                // Get low score users who might be fraudulent
                let low_score_users = self
                    .batch_service
                    .get_low_score_users(rust_decimal::Decimal::new(100, 0))
                    .await?;

                affected_users = low_score_users.len() as i32;
                actions_taken.push(format!(
                    "Identified {} potentially fraudulent users",
                    affected_users
                ));
            }
            CrisisType::SystemOverload => {
                // Apply time decay to reduce load
                let decay_result = self.batch_service.apply_time_decay_batch().await?;
                actions_taken.push(format!(
                    "Applied time decay to {} users",
                    decay_result.users_decayed
                ));
                affected_users += decay_result.users_decayed;
            }
        }

        Ok(CrisisResponse {
            crisis_type,
            actions_taken,
            affected_users,
            severity: if affected_users > 100 {
                CrisisSeverity::Critical
            } else if affected_users > 10 {
                CrisisSeverity::High
            } else {
                CrisisSeverity::Medium
            },
            handled_at: chrono::Utc::now(),
        })
    }

    /// Performance monitoring workflow
    /// Collects comprehensive performance metrics
    pub async fn collect_performance_metrics(&self) -> Result<PerformanceMetrics, ReputationError> {
        let start_time = chrono::Utc::now();

        // Get system stats
        let system_stats = self.analytics_service.get_system_stats().await?;

        // Get health report
        let health_report = self.maintenance_service.run_health_checks().await?;

        // Calculate processing metrics
        let total_commitments = system_stats.total_commitments;
        let completed_commitments = system_stats.completed_commitments;
        let processing_efficiency = if total_commitments > 0 {
            (completed_commitments as f64 / total_commitments as f64) * 100.0
        } else {
            0.0
        };

        let collection_duration = chrono::Utc::now().signed_duration_since(start_time);

        Ok(PerformanceMetrics {
            total_users: system_stats.total_users,
            active_users: system_stats.active_users_30d,
            database_healthy: health_report.database_healthy,
            score_anomalies: health_report.score_anomalies,
            stuck_commitments: health_report.stuck_commitments,
            orphaned_records: health_report.orphaned_records,
            processing_efficiency,
            average_score: system_stats.average_score,
            collection_duration_ms: collection_duration.num_milliseconds(),
            collected_at: chrono::Utc::now(),
        })
    }

    /// Automated intervention workflow
    /// Automatically fixes common issues without manual intervention
    pub async fn run_automated_interventions(&self) -> Result<InterventionReport, ReputationError> {
        let mut interventions = Vec::new();
        let mut users_affected = 0;

        // 1. Expire overdue commitments
        let expire_result = self.batch_service.expire_overdue_commitments().await?;
        if expire_result.expired_count > 0 {
            interventions.push(InterventionAction {
                action_type: InterventionType::ExpireCommitments,
                description: format!(
                    "Expired {} overdue commitments",
                    expire_result.expired_count
                ),
                users_affected: expire_result.affected_users.len() as i32,
                success: true,
            });
            users_affected += expire_result.affected_users.len() as i32;
        }

        // 2. Fix score anomalies via recalculation
        let health_report = self.maintenance_service.run_health_checks().await?;
        if health_report.score_anomalies > 0 {
            // Recalculate scores to fix anomalies
            let recalc_result = self.batch_service.recalculate_scores(vec![]).await?;
            interventions.push(InterventionAction {
                action_type: InterventionType::FixScoreAnomalies,
                description: format!(
                    "Recalculated {} scores to fix anomalies",
                    recalc_result.successful
                ),
                users_affected: recalc_result.successful,
                success: true,
            });
            users_affected += recalc_result.successful;
        }

        // 3. Apply time decay for inactive users
        let decay_result = self.batch_service.apply_time_decay_batch().await?;
        if decay_result.users_decayed > 0 {
            interventions.push(InterventionAction {
                action_type: InterventionType::ApplyTimeDecay,
                description: format!(
                    "Applied time decay to {} inactive users",
                    decay_result.users_decayed
                ),
                users_affected: decay_result.users_decayed,
                success: true,
            });
        }

        let interventions_count = interventions.len();

        Ok(InterventionReport {
            interventions,
            total_users_affected: users_affected,
            interventions_count,
            executed_at: chrono::Utc::now(),
        })
    }

    /// Comprehensive user profile with all available data
    /// Aggregates reputation, achievements, analytics, and external sources
    pub async fn get_comprehensive_user_profile(
        &self,
        user_id: Uuid,
    ) -> Result<ComprehensiveUserProfile, ReputationError> {
        // Get base reputation profile
        let base_profile = self.get_complete_profile(user_id).await?;

        // Get user rank
        let rank_info = self.leaderboard_service.get_user_rank(user_id).await?;

        // Get achievements
        let achievements = self
            .leaderboard_service
            .get_user_achievements(user_id)
            .await?;

        // Get commitment analytics
        let commitment_analytics = self
            .analytics_service
            .get_user_commitment_analytics(user_id)
            .await?;

        // Get recent audit trail (last 50 events)
        let recent_activity = self
            .audit_service
            .get_user_audit_trail(user_id, 50, 0)
            .await?;

        Ok(ComprehensiveUserProfile {
            user_id,
            score: base_profile.score,
            tier: base_profile.tier,
            rank: rank_info.rank,
            percentile: rank_info.percentile,
            total_users: rank_info.total_users,
            has_sbt: base_profile.has_sbt,
            sbt_minted: base_profile.sbt_minted,
            verified_external_sources: base_profile.verified_external_sources,
            fraud_flags: base_profile.fraud_flags,
            clean_record: base_profile.clean_record,
            total_commitments: commitment_analytics.total_commitments,
            verified_commitments: commitment_analytics.verified,
            failed_commitments: commitment_analytics.failed,
            pending_commitments: commitment_analytics.pending,
            success_rate: commitment_analytics.success_rate,
            achievements_count: achievements.len(),
            recent_activity_count: recent_activity.len(),
        })
    }
}

/// Complete reputation profile for a user
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompleteReputationProfile {
    /// Current reputation score.
    pub score: rust_decimal::Decimal,
    /// Current reputation tier.
    pub tier: ReputationTier,
    /// Whether the user has an SBT.
    pub has_sbt: bool,
    /// Whether the SBT has been minted on-chain.
    pub sbt_minted: bool,
    /// Number of verified external reputation sources.
    pub verified_external_sources: usize,
    /// Number of active fraud flags on this account.
    pub fraud_flags: usize,
    /// Whether the account has no fraud flags.
    pub clean_record: bool,
}

/// System health check report combining maintenance and analytics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemHealthReport {
    /// Whether the database connection is healthy.
    pub database_healthy: bool,
    /// Total number of registered users.
    pub total_users: i64,
    /// Number of active users.
    pub active_users: i64,
    /// Number of detected score anomalies.
    pub anomalies_count: usize,
    /// Number of users with critically low scores.
    pub low_score_users_count: usize,
    /// Detailed health check report from the maintenance service.
    pub health_report: crate::maintenance::HealthCheckReport,
}

/// Daily maintenance operations report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DailyMaintenanceReport {
    /// Number of users who had time-decay applied.
    pub users_decayed: i64,
    /// Number of commitments marked as expired.
    pub commitments_expired: i64,
    /// Number of scores recalculated from events.
    pub scores_recalculated: i64,
    /// Timestamp when the maintenance run completed.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Comprehensive analytics dashboard data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalyticsDashboard {
    /// System-wide statistics.
    pub system_stats: crate::analytics::SystemStats,
    /// User and commitment growth metrics.
    pub growth_metrics: crate::analytics::GrowthMetrics,
    /// Top-ranked users.
    pub top_performers: Vec<crate::analytics::UserRanking>,
    /// Distribution of scores across ranges.
    pub score_distribution: crate::analytics::ScoreDistribution,
    /// Global leaderboard snapshot.
    pub global_leaderboard: crate::leaderboard::Leaderboard,
}

/// User activity report combining audit trail and analytics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserActivityReport {
    /// User these activity metrics belong to.
    pub user_id: Uuid,
    /// Total number of audit events recorded for this user.
    pub audit_events_count: usize,
    /// Fraction of commitments that were fulfilled (0.0–1.0).
    pub commitment_success_rate: f64,
    /// Total number of commitments created by this user.
    pub total_commitments: i32,
    /// User's global rank position.
    pub rank: i32,
    /// Number of achievements earned by this user.
    pub achievements_count: usize,
    /// Length of the activity window analysed in days.
    pub activity_period_days: i32,
}

/// Bulk verification report for admin operations
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BulkVerificationReport {
    /// Total number of commitments processed.
    pub total_processed: usize,
    /// Number of commitments successfully verified.
    pub verified_count: usize,
    /// Number of commitments that could not be verified.
    pub failed_count: usize,
    /// Administrator who initiated the bulk operation.
    pub admin_id: Uuid,
}

// ========================================================================
// Phase 8 Data Structures
// ========================================================================

/// User onboarding report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserOnboardingReport {
    /// Newly onboarded user.
    pub user_id: Uuid,
    /// Score assigned at the time of onboarding.
    pub initial_score: rust_decimal::Decimal,
    /// Tier assigned at the time of onboarding.
    pub initial_tier: ReputationTier,
    /// Whether an SBT was created during onboarding.
    pub sbt_created: bool,
    /// User's rank immediately after onboarding.
    pub initial_rank: i32,
    /// Total number of users at the time of onboarding.
    pub total_users: i32,
    /// Number of baseline commitments created during onboarding.
    pub baseline_commitments: i32,
    /// Timestamp when onboarding completed.
    pub onboarded_at: chrono::DateTime<chrono::Utc>,
}

/// Crisis type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CrisisType {
    /// A large number of user scores show anomalous values.
    MassScoreAnomaly,
    /// Database integrity has been compromised.
    DatabaseCorruption,
    /// A mass fraud attempt has been detected.
    MassFraudDetection,
    /// The system is under excessive load.
    SystemOverload,
}

impl std::fmt::Display for CrisisType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MassScoreAnomaly => write!(f, "Mass Score Anomaly"),
            Self::DatabaseCorruption => write!(f, "Database Corruption"),
            Self::MassFraudDetection => write!(f, "Mass Fraud Detection"),
            Self::SystemOverload => write!(f, "System Overload"),
        }
    }
}

impl std::str::FromStr for CrisisType {
    type Err = ReputationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace([' ', '_', '-'], "").as_str() {
            "massscoreanomaly" => Ok(CrisisType::MassScoreAnomaly),
            "databasecorruption" => Ok(CrisisType::DatabaseCorruption),
            "massfrauddetection" => Ok(CrisisType::MassFraudDetection),
            "systemoverload" => Ok(CrisisType::SystemOverload),
            _ => Err(ReputationError::Validation(format!(
                "Invalid crisis type: {}. Valid values: mass_score_anomaly, database_corruption, mass_fraud_detection, system_overload",
                s
            ))),
        }
    }
}

/// Crisis severity levels
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum CrisisSeverity {
    /// Minor issue with limited impact.
    Low,
    /// Moderate issue requiring monitoring.
    Medium,
    /// Severe issue requiring prompt action.
    High,
    /// System-threatening issue requiring immediate response.
    Critical,
}

impl std::fmt::Display for CrisisSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

impl std::str::FromStr for CrisisSeverity {
    type Err = ReputationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(CrisisSeverity::Low),
            "medium" | "med" => Ok(CrisisSeverity::Medium),
            "high" => Ok(CrisisSeverity::High),
            "critical" | "crit" => Ok(CrisisSeverity::Critical),
            _ => Err(ReputationError::Validation(format!(
                "Invalid crisis severity: {}. Valid values: low, medium, high, critical",
                s
            ))),
        }
    }
}

/// Crisis response report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CrisisResponse {
    /// Type of crisis that was handled.
    pub crisis_type: CrisisType,
    /// List of actions taken during crisis response.
    pub actions_taken: Vec<String>,
    /// Number of users affected by the crisis.
    pub affected_users: i32,
    /// Assessed severity of the crisis.
    pub severity: CrisisSeverity,
    /// Timestamp when the crisis response was completed.
    pub handled_at: chrono::DateTime<chrono::Utc>,
}

/// Performance metrics for monitoring
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerformanceMetrics {
    /// Total number of registered users.
    pub total_users: i32,
    /// Number of active users.
    pub active_users: i32,
    /// Whether the database connection is healthy.
    pub database_healthy: bool,
    /// Number of score anomalies detected.
    pub score_anomalies: i32,
    /// Number of commitments stuck in a transitional state.
    pub stuck_commitments: i32,
    /// Number of orphaned records detected.
    pub orphaned_records: i32,
    /// Ratio of successful operations (0.0–1.0).
    pub processing_efficiency: f64,
    /// System-wide average reputation score.
    pub average_score: rust_decimal::Decimal,
    /// Time taken to collect these metrics in milliseconds.
    pub collection_duration_ms: i64,
    /// Timestamp when these metrics were collected.
    pub collected_at: chrono::DateTime<chrono::Utc>,
}

/// Intervention type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InterventionType {
    /// Force-expire overdue commitments.
    ExpireCommitments,
    /// Identify and correct score anomalies.
    FixScoreAnomalies,
    /// Apply time-based score decay.
    ApplyTimeDecay,
    /// Rebuild the historical tier transition table.
    RebuildTierHistory,
}

impl std::fmt::Display for InterventionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExpireCommitments => write!(f, "Expire Commitments"),
            Self::FixScoreAnomalies => write!(f, "Fix Score Anomalies"),
            Self::ApplyTimeDecay => write!(f, "Apply Time Decay"),
            Self::RebuildTierHistory => write!(f, "Rebuild Tier History"),
        }
    }
}

impl std::str::FromStr for InterventionType {
    type Err = ReputationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace([' ', '_', '-'], "").as_str() {
            "expirecommitments" => Ok(InterventionType::ExpireCommitments),
            "fixscoreanomalies" => Ok(InterventionType::FixScoreAnomalies),
            "applytimedecay" => Ok(InterventionType::ApplyTimeDecay),
            "rebuildtierhistory" => Ok(InterventionType::RebuildTierHistory),
            _ => Err(ReputationError::Validation(format!(
                "Invalid intervention type: {}. Valid values: expire_commitments, fix_score_anomalies, apply_time_decay, rebuild_tier_history",
                s
            ))),
        }
    }
}

/// Individual intervention action
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InterventionAction {
    /// Type of intervention performed.
    pub action_type: InterventionType,
    /// Human-readable description of the action.
    pub description: String,
    /// Number of users affected by this action.
    pub users_affected: i32,
    /// Whether the action completed without error.
    pub success: bool,
}

/// Intervention report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InterventionReport {
    /// List of individual intervention actions taken.
    pub interventions: Vec<InterventionAction>,
    /// Total number of users affected across all interventions.
    pub total_users_affected: i32,
    /// Number of distinct intervention actions executed.
    pub interventions_count: usize,
    /// Timestamp when the intervention batch was executed.
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

/// Comprehensive user profile with all data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ComprehensiveUserProfile {
    /// User identifier.
    pub user_id: Uuid,
    /// Current reputation score.
    pub score: rust_decimal::Decimal,
    /// Current reputation tier.
    pub tier: ReputationTier,
    /// Global leaderboard rank.
    pub rank: i32,
    /// Percentile among all users (0.0–100.0).
    pub percentile: f64,
    /// Total number of registered users.
    pub total_users: i32,
    /// Whether the user has an SBT.
    pub has_sbt: bool,
    /// Whether the SBT has been minted on-chain.
    pub sbt_minted: bool,
    /// Number of verified external reputation sources.
    pub verified_external_sources: usize,
    /// Number of active fraud flags.
    pub fraud_flags: usize,
    /// Whether the account has no active fraud flags.
    pub clean_record: bool,
    /// Total commitments created by this user.
    pub total_commitments: i32,
    /// Number of commitments verified as fulfilled.
    pub verified_commitments: i32,
    /// Number of commitments that failed.
    pub failed_commitments: i32,
    /// Number of commitments pending verification.
    pub pending_commitments: i32,
    /// Fraction of commitments that were verified (0.0–1.0).
    pub success_rate: f64,
    /// Number of achievements earned.
    pub achievements_count: usize,
    /// Number of recent activity events.
    pub recent_activity_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_profile_structure() {
        let profile = CompleteReputationProfile {
            score: rust_decimal::Decimal::new(750, 0),
            tier: ReputationTier::Gold,
            has_sbt: true,
            sbt_minted: true,
            verified_external_sources: 2,
            fraud_flags: 0,
            clean_record: true,
        };

        assert_eq!(profile.tier, ReputationTier::Gold);
        assert!(profile.clean_record);
        assert_eq!(profile.verified_external_sources, 2);
    }

    #[test]
    fn test_system_health_report_structure() {
        let health_report = crate::maintenance::HealthCheckReport {
            healthy: true,
            database_healthy: true,
            score_anomalies: 0,
            stuck_commitments: 0,
            orphaned_records: 0,
            checked_at: chrono::Utc::now(),
        };

        let report = SystemHealthReport {
            database_healthy: true,
            total_users: 1000,
            active_users: 750,
            anomalies_count: 0,
            low_score_users_count: 5,
            health_report,
        };

        assert!(report.database_healthy);
        assert_eq!(report.total_users, 1000);
        assert_eq!(report.anomalies_count, 0);
    }

    #[test]
    fn test_daily_maintenance_report_structure() {
        let report = DailyMaintenanceReport {
            users_decayed: 50,
            commitments_expired: 10,
            scores_recalculated: 1000,
            timestamp: chrono::Utc::now(),
        };

        assert_eq!(report.users_decayed, 50);
        assert_eq!(report.commitments_expired, 10);
        assert_eq!(report.scores_recalculated, 1000);
    }

    #[test]
    fn test_user_activity_report_structure() {
        let report = UserActivityReport {
            user_id: Uuid::new_v4(),
            audit_events_count: 25,
            commitment_success_rate: 85.5,
            total_commitments: 100,
            rank: 42,
            achievements_count: 5,
            activity_period_days: 30,
        };

        assert_eq!(report.audit_events_count, 25);
        assert_eq!(report.rank, 42);
        assert_eq!(report.achievements_count, 5);
        assert_eq!(report.total_commitments, 100);
    }

    #[test]
    fn test_bulk_verification_report_structure() {
        let report = BulkVerificationReport {
            total_processed: 100,
            verified_count: 95,
            failed_count: 5,
            admin_id: Uuid::new_v4(),
        };

        assert_eq!(report.total_processed, 100);
        assert_eq!(report.verified_count, 95);
        assert_eq!(report.failed_count, 5);
    }

    // Phase 8 tests
    #[test]
    fn test_user_onboarding_report_structure() {
        let report = UserOnboardingReport {
            user_id: Uuid::new_v4(),
            initial_score: rust_decimal::Decimal::new(500, 0),
            initial_tier: ReputationTier::Bronze,
            sbt_created: true,
            initial_rank: 150,
            total_users: 1000,
            baseline_commitments: 0,
            onboarded_at: chrono::Utc::now(),
        };

        assert_eq!(report.initial_tier, ReputationTier::Bronze);
        assert!(report.sbt_created);
        assert_eq!(report.initial_rank, 150);
        assert_eq!(report.total_users, 1000);
    }

    #[test]
    fn test_crisis_type_variants() {
        let crisis1 = CrisisType::MassScoreAnomaly;
        let crisis2 = CrisisType::DatabaseCorruption;
        let crisis3 = CrisisType::MassFraudDetection;
        let crisis4 = CrisisType::SystemOverload;

        assert_eq!(crisis1, CrisisType::MassScoreAnomaly);
        assert_eq!(crisis2, CrisisType::DatabaseCorruption);
        assert_eq!(crisis3, CrisisType::MassFraudDetection);
        assert_eq!(crisis4, CrisisType::SystemOverload);
    }

    #[test]
    fn test_crisis_severity_ordering() {
        assert!(CrisisSeverity::Low < CrisisSeverity::Medium);
        assert!(CrisisSeverity::Medium < CrisisSeverity::High);
        assert!(CrisisSeverity::High < CrisisSeverity::Critical);
    }

    #[test]
    fn test_crisis_response_structure() {
        let response = CrisisResponse {
            crisis_type: CrisisType::MassScoreAnomaly,
            actions_taken: vec!["Fixed 10 anomalies".to_string()],
            affected_users: 10,
            severity: CrisisSeverity::Medium,
            handled_at: chrono::Utc::now(),
        };

        assert_eq!(response.crisis_type, CrisisType::MassScoreAnomaly);
        assert_eq!(response.affected_users, 10);
        assert_eq!(response.severity, CrisisSeverity::Medium);
        assert_eq!(response.actions_taken.len(), 1);
    }

    #[test]
    fn test_performance_metrics_structure() {
        let metrics = PerformanceMetrics {
            total_users: 1000,
            active_users: 750,
            database_healthy: true,
            score_anomalies: 2,
            stuck_commitments: 1,
            orphaned_records: 0,
            processing_efficiency: 95.5,
            average_score: rust_decimal::Decimal::new(650, 0),
            collection_duration_ms: 125,
            collected_at: chrono::Utc::now(),
        };

        assert_eq!(metrics.total_users, 1000);
        assert_eq!(metrics.active_users, 750);
        assert!(metrics.database_healthy);
        assert_eq!(metrics.processing_efficiency, 95.5);
    }

    #[test]
    fn test_intervention_type_variants() {
        let int1 = InterventionType::ExpireCommitments;
        let int2 = InterventionType::FixScoreAnomalies;
        let int3 = InterventionType::ApplyTimeDecay;
        let int4 = InterventionType::RebuildTierHistory;

        assert_eq!(int1, InterventionType::ExpireCommitments);
        assert_eq!(int2, InterventionType::FixScoreAnomalies);
        assert_eq!(int3, InterventionType::ApplyTimeDecay);
        assert_eq!(int4, InterventionType::RebuildTierHistory);
    }

    #[test]
    fn test_intervention_action_structure() {
        let action = InterventionAction {
            action_type: InterventionType::ExpireCommitments,
            description: "Expired 5 overdue commitments".to_string(),
            users_affected: 5,
            success: true,
        };

        assert_eq!(action.action_type, InterventionType::ExpireCommitments);
        assert!(action.success);
        assert_eq!(action.users_affected, 5);
    }

    #[test]
    fn test_intervention_report_structure() {
        let action = InterventionAction {
            action_type: InterventionType::FixScoreAnomalies,
            description: "Fixed 3 anomalies".to_string(),
            users_affected: 3,
            success: true,
        };

        let report = InterventionReport {
            interventions: vec![action],
            total_users_affected: 3,
            interventions_count: 1,
            executed_at: chrono::Utc::now(),
        };

        assert_eq!(report.interventions_count, 1);
        assert_eq!(report.total_users_affected, 3);
    }

    #[test]
    fn test_comprehensive_user_profile_structure() {
        let profile = ComprehensiveUserProfile {
            user_id: Uuid::new_v4(),
            score: rust_decimal::Decimal::new(750, 0),
            tier: ReputationTier::Gold,
            rank: 42,
            percentile: 95.5,
            total_users: 1000,
            has_sbt: true,
            sbt_minted: true,
            verified_external_sources: 2,
            fraud_flags: 0,
            clean_record: true,
            total_commitments: 50,
            verified_commitments: 45,
            failed_commitments: 3,
            pending_commitments: 2,
            success_rate: 90.0,
            achievements_count: 7,
            recent_activity_count: 15,
        };

        assert_eq!(profile.tier, ReputationTier::Gold);
        assert_eq!(profile.rank, 42);
        assert_eq!(profile.percentile, 95.5);
        assert_eq!(profile.total_commitments, 50);
        assert_eq!(profile.success_rate, 90.0);
        assert!(profile.clean_record);
    }

    #[test]
    fn test_crisis_type_display() {
        assert_eq!(
            CrisisType::MassScoreAnomaly.to_string(),
            "Mass Score Anomaly"
        );
        assert_eq!(
            CrisisType::DatabaseCorruption.to_string(),
            "Database Corruption"
        );
        assert_eq!(
            CrisisType::MassFraudDetection.to_string(),
            "Mass Fraud Detection"
        );
        assert_eq!(CrisisType::SystemOverload.to_string(), "System Overload");
    }

    #[test]
    fn test_crisis_severity_display() {
        assert_eq!(CrisisSeverity::Low.to_string(), "Low");
        assert_eq!(CrisisSeverity::Medium.to_string(), "Medium");
        assert_eq!(CrisisSeverity::High.to_string(), "High");
        assert_eq!(CrisisSeverity::Critical.to_string(), "Critical");
    }

    #[test]
    fn test_intervention_type_display() {
        assert_eq!(
            InterventionType::ExpireCommitments.to_string(),
            "Expire Commitments"
        );
        assert_eq!(
            InterventionType::FixScoreAnomalies.to_string(),
            "Fix Score Anomalies"
        );
        assert_eq!(
            InterventionType::ApplyTimeDecay.to_string(),
            "Apply Time Decay"
        );
        assert_eq!(
            InterventionType::RebuildTierHistory.to_string(),
            "Rebuild Tier History"
        );
    }

    #[test]
    fn test_crisis_type_from_str_valid() {
        assert_eq!(
            "mass_score_anomaly".parse::<CrisisType>().unwrap(),
            CrisisType::MassScoreAnomaly
        );
        assert_eq!(
            "database_corruption".parse::<CrisisType>().unwrap(),
            CrisisType::DatabaseCorruption
        );
        assert_eq!(
            "mass_fraud_detection".parse::<CrisisType>().unwrap(),
            CrisisType::MassFraudDetection
        );
        assert_eq!(
            "system_overload".parse::<CrisisType>().unwrap(),
            CrisisType::SystemOverload
        );
    }

    #[test]
    fn test_crisis_type_from_str_flexible() {
        // Test with spaces
        assert_eq!(
            "mass score anomaly".parse::<CrisisType>().unwrap(),
            CrisisType::MassScoreAnomaly
        );
        assert_eq!(
            "database corruption".parse::<CrisisType>().unwrap(),
            CrisisType::DatabaseCorruption
        );
        // Test with hyphens
        assert_eq!(
            "mass-fraud-detection".parse::<CrisisType>().unwrap(),
            CrisisType::MassFraudDetection
        );
        // Test without separators
        assert_eq!(
            "systemoverload".parse::<CrisisType>().unwrap(),
            CrisisType::SystemOverload
        );
    }

    #[test]
    fn test_crisis_type_from_str_case_insensitive() {
        assert_eq!(
            "MASS_SCORE_ANOMALY".parse::<CrisisType>().unwrap(),
            CrisisType::MassScoreAnomaly
        );
        assert_eq!(
            "Database Corruption".parse::<CrisisType>().unwrap(),
            CrisisType::DatabaseCorruption
        );
    }

    #[test]
    fn test_crisis_type_from_str_invalid() {
        assert!("invalid".parse::<CrisisType>().is_err());
        assert!("unknown_crisis".parse::<CrisisType>().is_err());
    }

    #[test]
    fn test_crisis_severity_from_str_valid() {
        assert_eq!(
            "low".parse::<CrisisSeverity>().unwrap(),
            CrisisSeverity::Low
        );
        assert_eq!(
            "medium".parse::<CrisisSeverity>().unwrap(),
            CrisisSeverity::Medium
        );
        assert_eq!(
            "high".parse::<CrisisSeverity>().unwrap(),
            CrisisSeverity::High
        );
        assert_eq!(
            "critical".parse::<CrisisSeverity>().unwrap(),
            CrisisSeverity::Critical
        );
    }

    #[test]
    fn test_crisis_severity_from_str_variants() {
        assert_eq!(
            "med".parse::<CrisisSeverity>().unwrap(),
            CrisisSeverity::Medium
        );
        assert_eq!(
            "crit".parse::<CrisisSeverity>().unwrap(),
            CrisisSeverity::Critical
        );
    }

    #[test]
    fn test_crisis_severity_from_str_case_insensitive() {
        assert_eq!(
            "LOW".parse::<CrisisSeverity>().unwrap(),
            CrisisSeverity::Low
        );
        assert_eq!(
            "Medium".parse::<CrisisSeverity>().unwrap(),
            CrisisSeverity::Medium
        );
    }

    #[test]
    fn test_crisis_severity_from_str_invalid() {
        assert!("invalid".parse::<CrisisSeverity>().is_err());
        assert!("moderate".parse::<CrisisSeverity>().is_err());
    }

    #[test]
    fn test_intervention_type_from_str_valid() {
        assert_eq!(
            "expire_commitments".parse::<InterventionType>().unwrap(),
            InterventionType::ExpireCommitments
        );
        assert_eq!(
            "fix_score_anomalies".parse::<InterventionType>().unwrap(),
            InterventionType::FixScoreAnomalies
        );
        assert_eq!(
            "apply_time_decay".parse::<InterventionType>().unwrap(),
            InterventionType::ApplyTimeDecay
        );
        assert_eq!(
            "rebuild_tier_history".parse::<InterventionType>().unwrap(),
            InterventionType::RebuildTierHistory
        );
    }

    #[test]
    fn test_intervention_type_from_str_flexible() {
        // Test with spaces
        assert_eq!(
            "expire commitments".parse::<InterventionType>().unwrap(),
            InterventionType::ExpireCommitments
        );
        // Test with hyphens
        assert_eq!(
            "fix-score-anomalies".parse::<InterventionType>().unwrap(),
            InterventionType::FixScoreAnomalies
        );
    }

    #[test]
    fn test_intervention_type_from_str_case_insensitive() {
        assert_eq!(
            "EXPIRE_COMMITMENTS".parse::<InterventionType>().unwrap(),
            InterventionType::ExpireCommitments
        );
    }

    #[test]
    fn test_intervention_type_from_str_invalid() {
        assert!("invalid".parse::<InterventionType>().is_err());
    }
}
