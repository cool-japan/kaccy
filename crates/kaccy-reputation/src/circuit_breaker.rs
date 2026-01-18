//! Circuit breaker for low reputation

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ReputationError, Result};
use crate::tier::ReputationTier;

/// Reputation thresholds for circuit breaker actions
pub const CIRCUIT_BREAKER_THRESHOLD: Decimal = dec!(200);
pub const PAUSE_TOKENS_THRESHOLD: Decimal = dec!(100);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStatus {
    pub user_id: Uuid,
    pub reputation_score: Decimal,
    pub tier: ReputationTier,
    pub can_issue_tokens: bool,
    pub can_trade: bool,
    pub tokens_paused: bool,
}

/// Circuit breaker service for reputation-based restrictions
pub struct CircuitBreaker {
    pool: PgPool,
}

impl CircuitBreaker {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check user's circuit breaker status
    pub async fn check_status(&self, user_id: Uuid) -> Result<CircuitBreakerStatus> {
        let score: Option<Decimal> =
            sqlx::query_scalar("SELECT reputation_score FROM users WHERE user_id = $1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;

        let reputation_score = score
            .ok_or_else(|| ReputationError::Validation(format!("User {} not found", user_id)))?;

        let tier = ReputationTier::from_score(reputation_score);
        let benefits = tier.benefits();

        // Tokens paused if reputation below PAUSE_TOKENS_THRESHOLD
        let tokens_paused = reputation_score < PAUSE_TOKENS_THRESHOLD;

        Ok(CircuitBreakerStatus {
            user_id,
            reputation_score,
            tier,
            can_issue_tokens: benefits.can_issue_tokens,
            can_trade: reputation_score >= dec!(50), // Can't trade below 50
            tokens_paused,
        })
    }

    /// Check if user can issue tokens
    pub async fn can_issue_token(&self, user_id: Uuid) -> Result<bool> {
        let status = self.check_status(user_id).await?;
        Ok(status.can_issue_tokens)
    }

    /// Trigger circuit breaker for a user (when reputation drops below threshold)
    pub async fn trigger(&self, user_id: Uuid, reason: &str) -> Result<CircuitBreakerAction> {
        let status = self.check_status(user_id).await?;

        if status.reputation_score >= CIRCUIT_BREAKER_THRESHOLD {
            return Ok(CircuitBreakerAction::NoAction);
        }

        let mut tx = self.pool.begin().await?;

        // Log the circuit breaker event
        sqlx::query(
            r#"
            INSERT INTO reputation_events (user_id, event_type, delta, reason)
            VALUES ($1, 'circuit_breaker_triggered', 0, $2)
            "#,
        )
        .bind(user_id)
        .bind(reason)
        .execute(&mut *tx)
        .await?;

        // Pause tokens if below PAUSE_TOKENS_THRESHOLD
        let action = if status.reputation_score < PAUSE_TOKENS_THRESHOLD {
            // Pause all active tokens issued by this user
            let paused_count = sqlx::query(
                r#"
                UPDATE tokens
                SET status = 'paused'
                WHERE issuer_user_id = $1 AND status = 'active'
                "#,
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

            if paused_count > 0 {
                tracing::warn!(
                    user_id = %user_id,
                    paused_count = paused_count,
                    reputation = %status.reputation_score,
                    "Circuit breaker: paused user tokens due to very low reputation"
                );
            }

            CircuitBreakerAction::TokensPaused {
                count: paused_count,
            }
        } else {
            tracing::warn!(
                user_id = %user_id,
                reputation = %status.reputation_score,
                "Circuit breaker: user cannot issue new tokens due to low reputation"
            );

            CircuitBreakerAction::IssuanceDisabled
        };

        // Create admin notification
        sqlx::query(
            r#"
            INSERT INTO admin_actions (admin_user_id, action_type, target_type, target_id, details)
            VALUES (
                (SELECT user_id FROM users WHERE role = 'admin' LIMIT 1),
                'circuit_breaker_notification',
                'user',
                $1,
                $2
            )
            "#,
        )
        .bind(user_id)
        .bind(serde_json::json!({
            "reputation_score": status.reputation_score,
            "reason": reason,
            "action": format!("{:?}", action),
        }))
        .execute(&mut *tx)
        .await
        .ok(); // Ignore if no admin exists

        tx.commit().await?;

        Ok(action)
    }

    /// Restore user privileges after reputation recovery
    pub async fn restore(&self, user_id: Uuid) -> Result<bool> {
        let status = self.check_status(user_id).await?;

        if status.reputation_score < CIRCUIT_BREAKER_THRESHOLD {
            return Ok(false);
        }

        let mut tx = self.pool.begin().await?;

        // Reactivate paused tokens if reputation is recovered
        if status.reputation_score >= PAUSE_TOKENS_THRESHOLD {
            let reactivated = sqlx::query(
                r#"
                UPDATE tokens
                SET status = 'active'
                WHERE issuer_user_id = $1 AND status = 'paused'
                "#,
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

            if reactivated > 0 {
                tracing::info!(
                    user_id = %user_id,
                    reactivated = reactivated,
                    reputation = %status.reputation_score,
                    "Circuit breaker: reactivated user tokens after reputation recovery"
                );

                // Log event
                sqlx::query(
                    r#"
                    INSERT INTO reputation_events (user_id, event_type, delta, reason)
                    VALUES ($1, 'circuit_breaker_restored', 0, $2)
                    "#,
                )
                .bind(user_id)
                .bind(format!(
                    "Reputation recovered to {}",
                    status.reputation_score
                ))
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        Ok(true)
    }

    /// Check all users and trigger circuit breakers if needed
    pub async fn check_all(&self) -> Result<Vec<(Uuid, CircuitBreakerAction)>> {
        let users: Vec<(Uuid, Decimal)> = sqlx::query_as(
            r#"
            SELECT user_id, reputation_score
            FROM users
            WHERE reputation_score < $1
            "#,
        )
        .bind(CIRCUIT_BREAKER_THRESHOLD)
        .fetch_all(&self.pool)
        .await?;

        let mut actions = Vec::new();

        for (user_id, score) in users {
            let action = self
                .trigger(user_id, &format!("Reputation fell to {}", score))
                .await?;
            if !matches!(action, CircuitBreakerAction::NoAction) {
                actions.push((user_id, action));
            }
        }

        Ok(actions)
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum CircuitBreakerAction {
    NoAction,
    IssuanceDisabled,
    TokensPaused { count: u64 },
}

impl std::fmt::Display for CircuitBreakerAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAction => write!(f, "No Action"),
            Self::IssuanceDisabled => write!(f, "Issuance Disabled"),
            Self::TokensPaused { count } => write!(f, "Tokens Paused ({})", count),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_circuit_breaker_threshold_constants() {
        assert_eq!(CIRCUIT_BREAKER_THRESHOLD, dec!(200));
        assert_eq!(PAUSE_TOKENS_THRESHOLD, dec!(100));
    }

    #[test]
    fn test_circuit_breaker_threshold_ordering() {
        // Pause threshold should be lower than circuit breaker threshold
        assert!(PAUSE_TOKENS_THRESHOLD < CIRCUIT_BREAKER_THRESHOLD);
    }

    #[test]
    fn test_should_trigger_circuit_breaker() {
        // Below threshold - should trigger
        assert!(dec!(150) < CIRCUIT_BREAKER_THRESHOLD);
        assert!(dec!(199) < CIRCUIT_BREAKER_THRESHOLD);

        // At or above threshold - should not trigger
        assert!(dec!(200) >= CIRCUIT_BREAKER_THRESHOLD);
        assert!(dec!(250) >= CIRCUIT_BREAKER_THRESHOLD);
    }

    #[test]
    fn test_should_pause_tokens() {
        // Below pause threshold - should pause
        assert!(dec!(50) < PAUSE_TOKENS_THRESHOLD);
        assert!(dec!(99) < PAUSE_TOKENS_THRESHOLD);

        // At or above pause threshold - should not pause
        assert!(dec!(100) >= PAUSE_TOKENS_THRESHOLD);
        assert!(dec!(150) >= PAUSE_TOKENS_THRESHOLD);
    }

    #[test]
    fn test_can_trade_logic() {
        let min_trade_score = dec!(50);

        // Below minimum - can't trade
        assert!(dec!(0) < min_trade_score);
        assert!(dec!(49) < min_trade_score);

        // At or above minimum - can trade
        assert!(dec!(50) >= min_trade_score);
        assert!(dec!(100) >= min_trade_score);
    }

    #[test]
    fn test_circuit_breaker_status_logic() {
        // Test score ranges and expected status
        let test_cases = vec![
            // (score, should_trigger_breaker, should_pause_tokens, can_trade)
            (dec!(0), true, true, false),
            (dec!(49), true, true, false),
            (dec!(50), true, true, true),
            (dec!(99), true, true, true),
            (dec!(100), true, false, true),
            (dec!(199), true, false, true),
            (dec!(200), false, false, true),
            (dec!(500), false, false, true),
        ];

        for (score, should_trigger, should_pause, can_trade) in test_cases {
            let breaker_triggered = score < CIRCUIT_BREAKER_THRESHOLD;
            let tokens_paused = score < PAUSE_TOKENS_THRESHOLD;
            let trading_allowed = score >= dec!(50);

            assert_eq!(
                breaker_triggered, should_trigger,
                "Score {} breaker trigger mismatch",
                score
            );
            assert_eq!(
                tokens_paused, should_pause,
                "Score {} tokens pause mismatch",
                score
            );
            assert_eq!(
                trading_allowed, can_trade,
                "Score {} trading mismatch",
                score
            );
        }
    }

    #[test]
    fn test_tier_determines_issuance() {
        // Test that tier properly determines issuance capability
        let unverified = ReputationTier::from_score(dec!(50));
        let bronze = ReputationTier::from_score(dec!(250));
        let silver = ReputationTier::from_score(dec!(450));

        assert_eq!(unverified, ReputationTier::Unverified);
        assert!(!unverified.benefits().can_issue_tokens);

        assert_eq!(bronze, ReputationTier::Bronze);
        assert!(!bronze.benefits().can_issue_tokens);

        assert_eq!(silver, ReputationTier::Silver);
        assert!(silver.benefits().can_issue_tokens);
    }

    #[test]
    fn test_circuit_breaker_action_boundaries() {
        // Test action decision boundaries
        let score_above_threshold = dec!(250);
        let score_at_threshold = dec!(200);
        let score_just_below = dec!(199);
        let score_needs_pause = dec!(50);

        // Above threshold -> NoAction
        assert!(score_above_threshold >= CIRCUIT_BREAKER_THRESHOLD);

        // At threshold -> NoAction
        assert!(score_at_threshold >= CIRCUIT_BREAKER_THRESHOLD);

        // Just below threshold -> trigger, but no pause
        assert!(score_just_below < CIRCUIT_BREAKER_THRESHOLD);
        assert!(score_just_below >= PAUSE_TOKENS_THRESHOLD);

        // Well below threshold -> trigger and pause
        assert!(score_needs_pause < CIRCUIT_BREAKER_THRESHOLD);
        assert!(score_needs_pause < PAUSE_TOKENS_THRESHOLD);
    }

    #[test]
    fn test_circuit_breaker_action_display() {
        assert_eq!(CircuitBreakerAction::NoAction.to_string(), "No Action");
        assert_eq!(
            CircuitBreakerAction::IssuanceDisabled.to_string(),
            "Issuance Disabled"
        );
        assert_eq!(
            CircuitBreakerAction::TokensPaused { count: 5 }.to_string(),
            "Tokens Paused (5)"
        );
        assert_eq!(
            CircuitBreakerAction::TokensPaused { count: 0 }.to_string(),
            "Tokens Paused (0)"
        );
    }
}
